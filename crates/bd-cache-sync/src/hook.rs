//! The post-sync hook (REQ-BDC-0704..0706 amended, ANC-004) and the Python
//! rows (ANC-003), both run through `sc-cron`'s Python runner.
//!
//! beads-cache sends no messages (ADR-BDC-0010). After each run of the
//! built-in sync, the job calls the optional post-sync hook its row names
//! ([`HookSpec`](crate::HookSpec)) as `<interpreter> -I -m <module>`, event
//! `cron.tick`, with the request [`post_sync_request`] builds as the
//! `payload` of `sc-cron`'s stdin object:
//!
//! ```json
//! {"kind": "bd-cache.post-sync.v1", "repo_root": "/repo",
//!  "run": {"drain": {...}, "scopes": [{"scope", "result", "error",
//!          "deferred", "failing_since", "notified"}], "outage": {...},
//!          "stopped": false, "busy": false, "poisoned": false},
//!  "escalations": [{"seq", "idempotency_key", "scope", "bead", "op",
//!                   "actor", "attempts", "last_error", "created_at"}],
//!  "escalations_omitted": 0, "pending": 3}
//! ```
//!
//! The hook does what the consumer wants (beads-cache does nothing with its
//! output) and marks what it handled: `bdc cache journal announce <seq>` for
//! an escalation, and the compare-and-set `bdc cache scopes announce <scope>
//! --since <failing_since>` for a scope's failure. Anything it does not mark
//! is offered again on the next run (REQ-BDC-0706).
//!
//! Every hook is killed at its timeout capped by the time left before the
//! run's deadline (`sc_cron::kill_budget`, NFR-BDC-0004), and a read never
//! waits for it: the job releases the cache before the hook starts
//! (REQ-BDC-0001).

use std::path::Path;
use std::time::{Duration, Instant};

use bd_cache_model::JournalEntry;
use sc_cron::{
    Clock, HookCall, HookEvent, HookOutput, HookStatus, PythonHook, RunTimeout, ScheduleName,
};
use serde::Serialize;
use serde_json::{Value, json};

use crate::report::SyncRun;
use crate::text::clip;

/// The `kind` of the post-sync hook's request.
pub const POST_SYNC_KIND: &str = "bd-cache.post-sync.v1";

/// The most escalations one request offers; the rest are counted in
/// `escalations_omitted` and offered on later runs, once these are marked.
///
/// A hook that handles them one `journal announce` at a time cannot do many
/// more within its few seconds, and the request stays a small stdin write.
pub const MAX_HOOK_ESCALATIONS: usize = 100;

/// The most bytes of a hook's stderr kept in a recorded outcome.
const MAX_STDERR_BYTES: usize = 512;

/// One escalated entry as the hook's request offers it.
#[derive(Debug, Serialize)]
struct Offered<'a> {
    seq: i64,
    idempotency_key: &'a str,
    scope: Option<&'a str>,
    bead: &'a str,
    op: &'a str,
    actor: &'a str,
    attempts: u32,
    last_error: Option<&'a str>,
    created_at: &'a str,
}

impl<'a> Offered<'a> {
    fn of(e: &'a JournalEntry) -> Self {
        Self {
            seq: e.seq.get(),
            idempotency_key: e.idem_key.as_str(),
            scope: e.scope.as_ref().map(bd_cache_model::ScopeId::name),
            bead: e.bead.as_str(),
            op: e.op.name(),
            actor: e.actor.as_str(),
            attempts: e.attempts,
            last_error: e.last_error.as_deref(),
            created_at: e.created_at.as_str(),
        }
    }
}

/// The post-sync hook's request: the run, the escalations not yet marked
/// (at most [`MAX_HOOK_ESCALATIONS`]) and the pending count (ANC-004).
///
/// # Examples
///
/// ```
/// use bd_cache_sync::{SyncRun, post_sync_request};
/// let request = post_sync_request(&SyncRun::busy(), &[], 2, std::path::Path::new("/r"));
/// assert_eq!(request["kind"], "bd-cache.post-sync.v1");
/// assert_eq!(request["pending"], 2);
/// assert_eq!(request["run"]["busy"], true);
/// ```
#[must_use]
pub fn post_sync_request(
    run: &SyncRun,
    escalations: &[JournalEntry],
    pending: u64,
    repo_root: &Path,
) -> Value {
    let offered: Vec<Offered<'_>> = escalations
        .iter()
        .take(MAX_HOOK_ESCALATIONS)
        .map(Offered::of)
        .collect();
    json!({
        "kind": POST_SYNC_KIND,
        "repo_root": repo_root.to_string_lossy(),
        "run": run,
        "escalations": offered,
        "escalations_omitted": escalations.len().saturating_sub(MAX_HOOK_ESCALATIONS),
        "pending": pending,
    })
}

/// How a hook run ended, as the job reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum HookRun {
    /// It exited 0.
    Ok,
    /// It could not start, or exited non-zero, with what it said.
    Failed {
        /// The failure, with the start of its stderr.
        detail: String,
    },
    /// It was killed at its timeout, capped by the run's deadline (NFR-BDC-0004).
    TimedOut,
}

impl HookRun {
    /// The run of a hook that `invoke` returned.
    fn of(result: Result<HookOutput, sc_cron::CronError>) -> Self {
        match result {
            Err(e) => Self::Failed {
                detail: e.to_string(),
            },
            Ok(out) => match out.status {
                HookStatus::TimedOut => Self::TimedOut,
                HookStatus::Exited(status) if status.success() => Self::Ok,
                HookStatus::Exited(status) => {
                    let stderr = clip(out.stderr.trim(), MAX_STDERR_BYTES);
                    Self::Failed {
                        detail: if stderr.is_empty() {
                            format!("exited with {status}")
                        } else {
                            format!("exited with {status}: {stderr}")
                        },
                    }
                }
            },
        }
    }
}

/// Runs `hook` once for `cron.tick` with `payload`, killed at `timeout`
/// capped by the time left before `deadline` on `clock`'s monotonic timeline
/// (NFR-BDC-0004). A deadline already passed starts nothing ([`HookRun::TimedOut`]).
pub async fn run_hook(
    hook: &PythonHook,
    clock: &dyn Clock,
    schedule: &ScheduleName,
    payload: &Value,
    timeout: Duration,
    deadline: Instant,
) -> HookRun {
    let Ok(timeout) = RunTimeout::new(timeout) else {
        return HookRun::TimedOut;
    };
    let call = HookCall {
        event: HookEvent::Tick,
        schedule,
        payload,
        timeout,
        deadline,
    };
    HookRun::of(hook.invoke(clock, &call).await)
}
