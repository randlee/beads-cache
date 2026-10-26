//! The post-sync hook: its request, its isolated run and its backoff
//! (REQ-BDC-0723, REQ-BDC-0727, ADR-SYNC-0008).
//!
//! After the built-in sync, the hook named in the built-in row runs as
//! `<absolute interpreter> -I -m bd_cache_rs.run post_sync <module:function>`
//! through sc-cron's exported supervisor (REQ-CRON-0018): from a private temp
//! cwd, with the environment cleared except `PATH`, `HOME`, `LANG`, `LC_*`,
//! `BEADS_DIR`, `BD_CACHE_PATH` and the row's `--env` names, stdout and stderr
//! each capped at [`MAX_HOOK_PIPE_BYTES`], the request written while the
//! output is read, and the tree killed at the hook deadline or on a stop. The
//! trust check runs before every launch: the runner takes a
//! [`TrustedCacheFile`] by value. The run marks nothing; the hook marks what
//! it handled (`scope_announce`, `journal announce`) and must be idempotent.

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use bd_cache_model::{CacheError, FailingSince, JournalEntry, RunId, ScopeId, Seq, TickRunReason};
use sc_beads::Timestamp;
use sc_cron::process::{ChildSpec, TempCwd, supervise};
use serde::Serialize;
use serde_json::{Value, json};

use crate::payload::{BUILTIN_ROW, PostSyncHook};
use crate::text::{MAX_KEPT_STDERR_BYTES, cut, redact_and_cut};
use crate::trust::TrustedCacheFile;

/// The `protocol` member of the hook's request and of `bd_cache_rs.run`'s answer.
pub const RUN_PROTOCOL: &str = "bd-cache.run.v1";

/// The largest hook request, in bytes (REQ-BDC-0727).
pub const MAX_HOOK_REQUEST_BYTES: usize = 4 * 1024 * 1024;

/// The most escalated entries one request offers (REQ-BDC-0727).
pub const MAX_ESCALATIONS_OFFERED: usize = 500;

/// The most bytes of the hook's stdout, and of its stderr, read (REQ-BDC-0723).
pub const MAX_HOOK_PIPE_BYTES: usize = 1024 * 1024;

/// The consecutive failure from which the hook is backed off (REQ-BDC-0727).
pub const HOOK_BACKOFF_FROM: u32 = 3;

/// The longest wait of the backoff (REQ-BDC-0727).
pub const HOOK_BACKOFF_CAP: Duration = Duration::from_secs(3600);

/// One scope's result of a built-in run, as the hook and the reply see it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScopeResult {
    /// The scope.
    pub scope: ScopeId,
    /// How its sync went.
    pub result: ScopeOutcome,
    /// The error of a failed or outage result.
    pub error: Option<ScopeError>,
    /// The opaque token of the current failure, `None` while ok (REQ-BDC-0728).
    pub failing_since: Option<FailingSince>,
    /// Whether the current failure was marked notified.
    pub notified: bool,
    /// Rows stored by this sync.
    pub stored: u32,
}

/// The result of one scope's sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeOutcome {
    /// Synced.
    Ok,
    /// bd answered with an error, or a row was refused (no row stored).
    Failed,
    /// bd could not be reached; the scope's rows stay as they were (ADR-BDC-0005).
    Outage,
    /// Not started: the sync deadline came first, or the run was stopped.
    Skipped,
}

/// The error of a scope result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScopeError {
    /// The `BD_CACHE.*` code.
    pub code: String,
    /// What happened.
    pub message: String,
    /// True only when bd could not be reached (REQ-BDC-0729).
    pub outage: bool,
}

/// The drain's part of a hook request.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DrainSummary {
    /// Entries applied.
    pub applied: usize,
    /// Entries escalated by this drain.
    pub escalated: usize,
    /// The drain's error, when it failed.
    pub error: Option<ScopeError>,
}

/// A request ready to write, with what it offered.
#[derive(Debug)]
pub(crate) struct HookRequest {
    pub(crate) body: Vec<u8>,
    pub(crate) offered: Vec<Seq>,
}

/// The inputs of a hook request besides the escalations.
pub(crate) struct RequestParts<'a> {
    pub(crate) run_id: &'a RunId,
    pub(crate) now: &'a Timestamp,
    pub(crate) deadline: &'a Timestamp,
    pub(crate) cache_path: &'a Path,
    pub(crate) drain: &'a DrainSummary,
    pub(crate) scopes: &'a [ScopeResult],
    pub(crate) pending: (u64, Option<i64>),
}

/// The JSON of one offered escalation.
fn escalation_json(e: &JournalEntry, now: &Timestamp) -> Value {
    json!({
        "seq": e.seq.get(),
        "bead": e.bead.as_str(),
        "op": e.op.name(),
        "actor": e.actor.as_str(),
        "attempts": e.attempts,
        "error": {"code": e.last_error_code, "message": e.last_error},
        "created_at": e.created_at.as_str(),
        "first_offered_at": e.first_offered_at.as_ref().unwrap_or(now).as_str(),
    })
}

/// Builds the request: at most [`MAX_ESCALATIONS_OFFERED`] escalations,
/// oldest first, dropped from the end until the whole request fits in
/// `cap` bytes, and counted in `escalated_remaining` (REQ-BDC-0727).
pub(crate) fn build_request(
    parts: &RequestParts<'_>,
    escalated: &[JournalEntry],
    cap: usize,
) -> HookRequest {
    let entries: Vec<(Seq, String)> = escalated
        .iter()
        .take(MAX_ESCALATIONS_OFFERED)
        .map(|e| (e.seq, escalation_json(e, parts.now).to_string()))
        .collect();
    let envelope = |kept: usize, remaining: usize| {
        json!({
            "protocol": RUN_PROTOCOL,
            "kind": "post_sync",
            "run_id": parts.run_id,
            "now": parts.now.as_str(),
            "deadline": parts.deadline.as_str(),
            "cache_path": parts.cache_path.display().to_string(),
            "drain": parts.drain,
            "scopes": parts.scopes,
            "escalated": [],
            "escalated_truncated": remaining > 0,
            "escalated_remaining": remaining,
            "pending": {"count": parts.pending.0, "oldest_age_s": parts.pending.1},
            "_kept": kept,
        })
    };
    // The size of the request with `kept` entries: the envelope plus each
    // entry and its comma. The counts' digits change the envelope by a few
    // bytes, so it is measured with the final counts.
    let total = escalated.len();
    let mut kept = entries.len();
    loop {
        let base = envelope(kept, total - kept).to_string().len();
        let body: usize = entries.iter().take(kept).map(|(_, s)| s.len() + 1).sum();
        if base + body <= cap || kept == 0 {
            break;
        }
        kept -= 1;
    }
    let mut v = envelope(kept, total - kept);
    if let Value::Object(m) = &mut v {
        m.remove("_kept");
        let list: Vec<Value> = escalated
            .iter()
            .take(kept)
            .map(|e| escalation_json(e, parts.now))
            .collect();
        m.insert("escalated".to_owned(), Value::Array(list));
    }
    HookRequest {
        body: v.to_string().into_bytes(),
        offered: entries.iter().take(kept).map(|(s, _)| *s).collect(),
    }
}

/// What a hook that succeeded answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookSuccess {
    /// Its `result`, kept as JSON when it fits in 8 KiB, else cut text.
    pub result: Option<Value>,
    /// Its stderr, redacted and cut to 8 KiB.
    pub stderr: String,
}

/// A `TICK_RUN_FAILED` of the built-in row's hook step.
pub(crate) fn hook_failed(
    run_id: &RunId,
    reason: TickRunReason,
    exit_status: Option<i32>,
    stderr: String,
    detail: impl Into<String>,
) -> CacheError {
    CacheError::TickRunFailed {
        detail: detail.into(),
        row: BUILTIN_ROW.to_owned(),
        run_id: run_id.clone(),
        reason,
        exit_status,
        stderr,
        restored: None,
    }
}

/// Runs the hook once with `request`, killed when `expiry` completes.
///
/// # Errors
///
/// `TICK_RUN_FAILED` (row `bd-cache.sync.builtin`): `spawn` when it cannot
/// start, `timeout` when killed at `expiry`, `oversize` when a pipe went over
/// the cap, `exit` for a non-zero exit, malformed output or a wrong protocol.
pub(crate) async fn run_hook(
    hook: &PostSyncHook,
    trusted: TrustedCacheFile,
    beads_dir: Option<&Path>,
    run_id: &RunId,
    request: &[u8],
    expiry: impl Future<Output = ()>,
) -> Result<HookSuccess, CacheError> {
    let mut set: Vec<(OsString, OsString)> = vec![(
        "BD_CACHE_PATH".into(),
        trusted.path().as_os_str().to_owned(),
    )];
    if let Some(dir) = beads_dir {
        set.push(("BEADS_DIR".into(), dir.as_os_str().to_owned()));
    }
    let env = hook.env().child_env(set);
    let names = hook.env().names();
    let cwd = TempCwd::new().map_err(|e| {
        hook_failed(
            run_id,
            TickRunReason::Spawn,
            None,
            String::new(),
            format!("no temporary directory: {e}"),
        )
    })?;
    let target = hook.target();
    let spec = ChildSpec {
        program: hook.interpreter().path(),
        args: vec![
            "-I".into(),
            "-m".into(),
            "bd_cache_rs.run".into(),
            "post_sync".into(),
            target.clone().into(),
        ],
        cwd: cwd.path(),
        input: request,
        env,
    };
    let captured = supervise(&spec, MAX_HOOK_PIPE_BYTES, expiry)
        .await
        .map_err(|e| {
            hook_failed(
                run_id,
                TickRunReason::Spawn,
                None,
                String::new(),
                format!("the post-sync hook {target} could not start: {e}"),
            )
        })?;
    // Keep only what the run needs; the trust proof was for this launch.
    drop(trusted);
    judge(&captured, names, run_id, &target)
}

/// What one finished (or killed) hook process means (REQ-BDC-0723, 0729).
fn judge(
    captured: &sc_cron::process::Captured,
    names: &[String],
    run_id: &RunId,
    target: &str,
) -> Result<HookSuccess, CacheError> {
    let stderr = redact_and_cut(&String::from_utf8_lossy(&captured.stderr.bytes), names);
    let Some(status) = captured.status else {
        return Err(hook_failed(
            run_id,
            TickRunReason::Timeout,
            None,
            stderr,
            format!("the post-sync hook {target} was killed at its deadline"),
        ));
    };
    if captured.stdout.truncated || captured.stderr.truncated {
        return Err(hook_failed(
            run_id,
            TickRunReason::Oversize,
            status.code(),
            stderr,
            format!(
                "the post-sync hook {target} wrote more than {MAX_HOOK_PIPE_BYTES} bytes to a pipe"
            ),
        ));
    }
    let malformed = |why: &str, stderr: String| {
        hook_failed(
            run_id,
            TickRunReason::Exit,
            status.code(),
            stderr,
            format!("the post-sync hook {target} {why}"),
        )
    };
    let answer: Option<Value> = serde_json::from_slice(captured.stdout.bytes.trim_ascii()).ok();
    if !status.success() {
        let message = answer
            .as_ref()
            .and_then(|a| a.get("error"))
            .and_then(|e| e.get("message"))
            .and_then(Value::as_str)
            .map(|m| format!(": {}", cut(m, 512)))
            .unwrap_or_default();
        return Err(malformed(&format!("exited with {status}{message}"), stderr));
    }
    let Some(answer) = answer else {
        return Err(malformed(
            "exited 0 but did not print one JSON object",
            stderr,
        ));
    };
    if answer.get("protocol").and_then(Value::as_str) != Some(RUN_PROTOCOL) {
        return Err(malformed(
            &format!("answered without protocol {RUN_PROTOCOL}"),
            stderr,
        ));
    }
    if answer.get("ok") != Some(&Value::Bool(true)) {
        return Err(malformed("answered ok: false", stderr));
    }
    let result = answer.get("result").filter(|r| !r.is_null()).map(|r| {
        let text = r.to_string();
        if text.len() <= MAX_KEPT_STDERR_BYTES {
            r.clone()
        } else {
            Value::String(cut(&text, MAX_KEPT_STDERR_BYTES).to_owned())
        }
    });
    Ok(HookSuccess { result, stderr })
}

/// The hook's next attempt after a failure that makes `failures` in a row
/// (REQ-BDC-0727): from the third, one `period` of the row, doubling with
/// each further failure, capped at [`HOOK_BACKOFF_CAP`]; `None` before the
/// third. A row whose period is unknown backs off by the cap.
#[must_use]
pub fn next_attempt(failures: u32, period: Option<Duration>, now: &Timestamp) -> Option<Timestamp> {
    if failures < HOOK_BACKOFF_FROM {
        return None;
    }
    let doublings = (failures - HOOK_BACKOFF_FROM).min(20);
    let wait = period
        .map_or(HOOK_BACKOFF_CAP, |p| p.saturating_mul(1 << doublings))
        .min(HOOK_BACKOFF_CAP);
    let secs = i64::try_from(wait.as_secs()).unwrap_or(i64::MAX).max(1);
    Timestamp::from_unix(now.unix().saturating_add(secs)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_backoff_starts_at_the_third_failure_doubles_and_is_capped() {
        let now = Timestamp::parse("2026-10-26T00:00:00Z").unwrap();
        let period = Some(Duration::from_secs(60));
        assert_eq!(next_attempt(1, period, &now), None);
        assert_eq!(next_attempt(2, period, &now), None);
        let at = |n| next_attempt(n, period, &now).map(|t| t.unix() - now.unix());
        assert_eq!(at(3), Some(60));
        assert_eq!(at(4), Some(120));
        assert_eq!(at(5), Some(240));
        assert_eq!(at(30), Some(3600));
        assert_eq!(
            next_attempt(3, None, &now).map(|t| t.unix() - now.unix()),
            Some(3600)
        );
    }
}
