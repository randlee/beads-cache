//! The built-in sync run (REQ-BDC-0703 as amended, REQ-BDC-0720, 0722, 0727; ADR-SYNC-0008).
//!
//! [`run_builtin`] is the one Rust code of the built-in row: `cron run`,
//! `cron run --once`, `bdc cache tick` and `PyO3` `Cache.tick()` all call it.
//! Steps, each logged as one structured event with the row, the `run_id` and
//! the outcome:
//!
//! 1. **lease**: [`TickLease::take`]; a live lease held elsewhere makes the
//!    run do nothing and reply `skipped: "in_flight"` (REQ-BDC-0722);
//! 2. **drain**, with the sync deadline ([`Budget`]);
//! 3. **scope**: each registered scope is synced by its filter
//!    (ADR-BDC-0013), rows stored through the strict decoder, all or nothing
//!    per scope, the newer-row guard kept; an unreachable bd is an `outage`
//!    result and the scope's rows stay as they were (ADR-BDC-0005); a scope
//!    not started before the sync deadline, or after a stop, is `skipped`;
//! 4. **hook**: the post-sync hook, when one is named and not backed off,
//!    with at least `hook_reserve_ms` left (REQ-BDC-0727); no write
//!    transaction and no drain lock is held while it runs;
//! 5. **reply**: the lease is released and the reply returned.
//!
//! The blocking parts (lease, drain, syncs, bookkeeping) run on tokio's
//! blocking pool with the cache's mutex held; the hook runs on the async side
//! with the mutex free, so an aborted run drops the hook's supervisor and
//! kills its process tree, and the lease guard's drop releases the lease.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use bd_cache::Cache;
use bd_cache_model::{
    CacheError, CacheStore, DrainBudget, DrainLock, DrainReport, HARD_MARGIN, LeaseHolder, RunId,
    Scope,
};
use sc_beads::{CacheBackend, Clock, Deadline, Timestamp};
use serde::Serialize;
use serde_json::{Value, json};

use crate::control::{Budget, StopFlag};
use crate::hook::{
    DrainSummary, HookSuccess, MAX_HOOK_REQUEST_BYTES, RequestParts, ScopeError, ScopeOutcome,
    ScopeResult, build_request, next_attempt, run_hook,
};
use crate::lease::{LeaseTaken, TickLease, log_release, new_run_id};
use crate::payload::BuiltinPayload;
use crate::trust::TrustedCacheFile;

/// A cache shared by a run's blocking parts, its lease guard and its host.
pub type SharedCache<S, L, B> = Arc<Mutex<Cache<S, L, B>>>;

/// What a run knows besides its payload.
#[derive(Debug, Clone)]
pub struct RunContext {
    /// The row that runs, for the logs and the reply.
    pub row: String,
    /// One period of the row at this run (REQ-BDC-0727), for the hook's backoff.
    pub period: Option<Duration>,
    /// The cache file (the hook's `BD_CACHE_PATH`, and the trust check's subject).
    pub cache_path: PathBuf,
    /// The beads directory given to the hook (`BEADS_DIR`), when set.
    pub beads_dir: Option<PathBuf>,
    /// Set on a signal: the run starts no further step (and no hook).
    pub stop: StopFlag,
    /// Set to kill a running hook with its tree at once. `tick` passes its
    /// `stop` flag here; `cron run` passes a flag of its own, so a hook in
    /// flight is waited for up to `--shutdown-timeout-ms` and then killed by
    /// the abort of its run (REQ-BDC-0730).
    pub kill: StopFlag,
}

/// How a built-in run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// It ran every step (a scope or the hook may still have failed).
    Ok,
    /// Another run held the lease; nothing ran.
    Skipped,
    /// A stop flag ended it early.
    Stopped,
}

/// What the hook step did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HookStep {
    /// The row names no hook.
    None,
    /// Backed off after consecutive failures (REQ-BDC-0727).
    BackedOff,
    /// It ran and answered `ok: true`.
    Ok,
    /// It failed: `error` holds the `TICK_RUN_FAILED`.
    Failed,
    /// The run was stopped before the hook.
    Skipped,
}

/// The hook step of a built-in run's reply.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HookReport {
    /// What the step did.
    pub status: HookStep,
    /// The hook's `module:function`, when one is named.
    pub module: Option<String>,
    /// Escalations offered.
    pub offered: usize,
    /// Escalations left out of the request.
    pub escalated_remaining: usize,
    /// The hook's result.
    pub result: Option<Value>,
    /// The hook's stderr, redacted and cut to 8 KiB.
    pub stderr: Option<String>,
    /// The `TICK_RUN_FAILED` of a failed hook, as an error object; the run's
    /// own reply is still `ok: true` (REQ-BDC-0729).
    pub error: Option<Value>,
    /// When the hook is next called, while backed off.
    pub next_attempt_at: Option<Timestamp>,
}

impl HookReport {
    fn none(module: Option<String>, status: HookStep) -> Self {
        Self {
            status,
            module,
            offered: 0,
            escalated_remaining: 0,
            result: None,
            stderr: None,
            error: None,
            next_attempt_at: None,
        }
    }
}

/// The reply of a built-in run (REQ-SYNC-0001 as amended): every step, no
/// `refused` or `pass-through`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BuiltinReply {
    /// The run's id; also in the lease row and the hook request.
    pub run_id: RunId,
    /// The row that ran.
    pub row: String,
    /// How it ended.
    pub outcome: RunStatus,
    /// `in_flight` when another run held the lease.
    pub skipped: Option<&'static str>,
    /// The live holder of a skipped run (pid, host, expiry).
    pub holder: Option<Value>,
    /// The expired holder this run took the lease over from.
    pub took_over: Option<Value>,
    /// The drain.
    pub drain: Option<DrainReport>,
    /// Each scope's result.
    pub scopes: Vec<ScopeResult>,
    /// The hook step.
    pub hook: HookReport,
}

/// The public view of a lease holder: REQ-BDC-0729's keys only.
pub(crate) fn holder_json(h: &LeaseHolder) -> Value {
    json!({"pid": h.pid, "host": h.host, "expires_at": h.expires_at.as_str()})
}

/// A `CacheError` as the contract's error object, `{code, message, details,
/// suggested_action}`, with the `*_reason` keys named `reason` as
/// `bd-cache-api` names them (REQ-BDC-0729).
pub(crate) fn error_object(e: &CacheError) -> Value {
    let mut details = serde_json::Map::new();
    if let Value::Object(m) = e.to_json() {
        for (k, v) in m {
            match k.as_str() {
                "ok" | "code" | "detail" | "fix" => {}
                "run_reason" | "schedule_reason" | "ingest_reason" => {
                    details.insert("reason".to_owned(), v);
                }
                _ => {
                    details.insert(k, v);
                }
            }
        }
    }
    json!({
        "code": e.code(),
        "message": e.detail(),
        "details": details,
        "suggested_action": e.fix(),
    })
}

/// Takes the cache's mutex; a poisoned one is an error, never trusted.
pub(crate) fn hold<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Mutex<Cache<S, L, B>>,
) -> Result<MutexGuard<'_, Cache<S, L, B>>, CacheError> {
    cache.lock().map_err(|poisoned| CacheError::StoreFailed {
        detail: format!(
            "the cache mutex was poisoned by a panic in an earlier run ({poisoned}); restart the process"
        ),
        source: None,
    })
}

/// Runs `f` on tokio's blocking pool; a panic there is a store error.
pub(crate) async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, CacheError> + Send + 'static,
) -> Result<T, CacheError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| CacheError::StoreFailed {
            detail: format!("a blocking step of the run panicked or was cancelled: {e}"),
            source: None,
        })?
}

/// Everything the blocking first half hands the hook step.
struct Prepared {
    lease: TickLease,
    reply: BuiltinReply,
    clock: Arc<dyn Clock>,
    budget: Budget,
    request: Option<Vec<u8>>,
}

/// Runs the built-in row once (module docs).
///
/// # Errors
///
/// A store error, or an error of the cache that is not a backend failure; a
/// backend failure is a scope result, and a hook failure is in
/// [`HookReport::error`]. The lease is released on every path.
pub async fn run_builtin<S, L, B>(
    cache: &SharedCache<S, L, B>,
    payload: &BuiltinPayload,
    ctx: &RunContext,
) -> Result<BuiltinReply, CacheError>
where
    S: CacheStore + 'static,
    L: DrainLock + 'static,
    B: CacheBackend + 'static,
{
    let prepared = {
        let shared = Arc::clone(cache);
        let payload = payload.clone();
        let ctx = ctx.clone();
        blocking(move || prepare(&shared, &payload, &ctx)).await?
    };
    let mut prepared = match prepared {
        Ok(p) => p,
        Err(skipped) => return Ok(*skipped),
    };
    let fallback = Arc::clone(cache);
    prepared.lease = prepared.lease.with_fallback(move |run_id| {
        let mut guard = fallback
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        log_release(&run_id, guard.lease_release(&run_id));
    });
    let outcome = match (&payload.hook, prepared.request.take()) {
        (Some(hook), Some(request)) if !ctx.stop.is_stopped() => {
            Some(call_hook(hook, &prepared, ctx, &request).await)
        }
        _ => None,
    };
    if outcome.is_none() && payload.hook.is_some() && prepared.reply.hook.status == HookStep::Ok {
        prepared.reply.hook.status = HookStep::Skipped;
        prepared.reply.outcome = RunStatus::Stopped;
    }
    let shared = Arc::clone(cache);
    let period = ctx.period;
    let row = ctx.row.clone();
    blocking(move || finish(&shared, prepared, outcome, period, &row)).await
}

/// The trust check, then the hook, killed at the hook deadline or on a stop.
async fn call_hook(
    hook: &crate::payload::PostSyncHook,
    prepared: &Prepared,
    ctx: &RunContext,
    request: &[u8],
) -> Result<HookSuccess, CacheError> {
    let run_id = prepared.lease.run_id();
    let trusted = TrustedCacheFile::check(&ctx.cache_path).map_err(|e| {
        crate::hook::hook_failed(
            run_id,
            bd_cache_model::TickRunReason::Untrusted,
            None,
            String::new(),
            e.to_string(),
        )
    })?;
    let left = prepared.budget.hook_deadline().remaining(&*prepared.clock);
    let stop = ctx.kill.clone();
    let expiry = async move {
        tokio::select! {
            () = tokio::time::sleep(left) => {}
            () = stop.wait() => {}
        }
    };
    run_hook(
        hook,
        trusted,
        ctx.beads_dir.as_deref(),
        run_id,
        request,
        expiry,
    )
    .await
}

/// Steps 1 to 3 and the hook's request, with the cache's mutex held.
fn prepare<S: CacheStore, L: DrainLock, B: CacheBackend>(
    shared: &Mutex<Cache<S, L, B>>,
    payload: &BuiltinPayload,
    ctx: &RunContext,
) -> Result<Result<Prepared, Box<BuiltinReply>>, CacheError> {
    let mut guard = hold(shared)?;
    let cache = &mut *guard;
    let clock = cache.clock();
    let budget = Budget::new(&*clock, &payload.budget);
    let run_id = new_run_id(&*clock);
    let module = payload
        .hook
        .as_ref()
        .map(crate::payload::PostSyncHook::target);
    let mut reply = BuiltinReply {
        run_id: run_id.clone(),
        row: ctx.row.clone(),
        outcome: RunStatus::Ok,
        skipped: None,
        holder: None,
        took_over: None,
        drain: None,
        scopes: Vec::new(),
        hook: HookReport::none(module.clone(), HookStep::None),
    };
    let lease = match TickLease::take(cache, run_id, budget.hook_deadline())? {
        LeaseTaken::Held(holder) => {
            step(&ctx.row, &reply.run_id, "lease", "in_flight");
            reply.outcome = RunStatus::Skipped;
            reply.skipped = Some("in_flight");
            reply.holder = Some(holder_json(&holder));
            reply.hook.status = HookStep::Skipped;
            return Ok(Err(Box::new(reply)));
        }
        LeaseTaken::Taken(lease) => lease,
    };
    reply.took_over = lease.took_over().map(holder_json);
    step(
        &ctx.row,
        &reply.run_id,
        "lease",
        if reply.took_over.is_some() {
            "took_over"
        } else {
            "taken"
        },
    );
    match steps(cache, payload, ctx, &budget, &mut reply) {
        Ok(request) => Ok(Ok(Prepared {
            lease,
            reply,
            clock,
            budget,
            request,
        })),
        Err(e) => {
            lease.release(cache);
            Err(e)
        }
    }
}

/// Drain, scopes and the hook's request; `Ok(None)` when no hook is called.
fn steps<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    payload: &BuiltinPayload,
    ctx: &RunContext,
    budget: &Budget,
    reply: &mut BuiltinReply,
) -> Result<Option<Vec<u8>>, CacheError> {
    let clock = cache.clock();
    let sync = budget.sync_deadline();
    let mut drain = DrainSummary::default();
    if ctx.stop.is_stopped() {
        reply.outcome = RunStatus::Stopped;
    } else {
        let margin = HARD_MARGIN.min(sync.remaining(&*clock) / 2);
        let gate = Deadline::at(sync.instant().checked_sub(margin).unwrap_or(sync.instant()));
        match cache.drain(&DrainBudget {
            start_gate: gate,
            hard: sync,
        }) {
            Ok(report) => {
                drain.applied = report.applied.len();
                drain.escalated = report.escalated.len();
                step(
                    &ctx.row,
                    &reply.run_id,
                    "drain",
                    if report.busy { "busy" } else { "ok" },
                );
                reply.drain = Some(report);
            }
            Err(e) => {
                step(&ctx.row, &reply.run_id, "drain", e.code());
                drain.error = Some(scope_error(&e));
            }
        }
    }
    for scope in cache.scopes()? {
        let result = if ctx.stop.is_stopped() {
            reply.outcome = RunStatus::Stopped;
            skipped(&scope)
        } else if sync.expired(&*clock) {
            skipped(&scope)
        } else {
            sync_one(cache, &scope, sync)?
        };
        step_scope(&ctx.row, &reply.run_id, &result);
        reply.scopes.push(result);
    }
    let Some(hook) = &payload.hook else {
        return Ok(None);
    };
    if ctx.stop.is_stopped() {
        reply.hook.status = HookStep::Skipped;
        reply.outcome = RunStatus::Stopped;
        return Ok(None);
    }
    let now = clock.now_utc();
    let state = cache.hook_state()?;
    if let Some(next) = state
        .next_attempt_at
        .as_ref()
        .filter(|n| n.unix() > now.unix())
    {
        step(&ctx.row, &reply.run_id, "hook", "backed_off");
        reply.hook.status = HookStep::BackedOff;
        reply.hook.next_attempt_at = Some(next.clone());
        return Ok(None);
    }
    let escalated = cache.unannounced_escalations()?;
    let pending = cache.pending_entries()?;
    let oldest = pending
        .iter()
        .map(|e| now.unix().saturating_sub(e.created_at.unix()))
        .max();
    let left = budget.hook_deadline().remaining(&*clock);
    let deadline = Timestamp::from_unix(
        now.unix()
            .saturating_add(i64::try_from(left.as_secs()).unwrap_or(i64::MAX)),
    )
    .unwrap_or_else(|_| now.clone());
    let request = build_request(
        &RequestParts {
            run_id: &reply.run_id,
            now: &now,
            deadline: &deadline,
            cache_path: &ctx.cache_path,
            drain: &drain,
            scopes: &reply.scopes,
            pending: (u64::try_from(pending.len()).unwrap_or(u64::MAX), oldest),
        },
        &escalated,
        MAX_HOOK_REQUEST_BYTES,
    );
    cache.mark_offered(&request.offered)?;
    reply.hook.status = HookStep::Ok;
    reply.hook.module = Some(hook.target());
    reply.hook.offered = request.offered.len();
    reply.hook.escalated_remaining = escalated.len().saturating_sub(request.offered.len());
    Ok(Some(request.body))
}

/// One scope's sync and its result (REQ-BDC-0727, 0729).
fn sync_one<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    scope: &Scope,
    deadline: Deadline,
) -> Result<ScopeResult, CacheError> {
    let (result, error, stored) = match cache.sync(&scope.id, deadline) {
        Ok(report) if report.deferred => (ScopeOutcome::Skipped, None, 0),
        Ok(report) => match &report.outage {
            Some(outage) => (
                ScopeOutcome::Outage,
                Some(ScopeError {
                    code: "BD_CACHE.BD_UNAVAILABLE".to_owned(),
                    message: outage.last_error.clone(),
                    outage: true,
                }),
                report.stored,
            ),
            None => (ScopeOutcome::Ok, None, report.stored),
        },
        Err(e @ CacheError::SyncFailed { .. }) => (ScopeOutcome::Failed, Some(scope_error(&e)), 0),
        Err(e) => return Err(e),
    };
    let state = cache.sync_state(&scope.id)?;
    Ok(ScopeResult {
        scope: scope.id.clone(),
        result,
        error,
        notified: state.notified(),
        failing_since: state.failing_since,
        stored,
    })
}

fn skipped(scope: &Scope) -> ScopeResult {
    ScopeResult {
        scope: scope.id.clone(),
        result: ScopeOutcome::Skipped,
        error: None,
        failing_since: None,
        notified: false,
        stored: 0,
    }
}

fn scope_error(e: &CacheError) -> ScopeError {
    ScopeError {
        code: e.code().to_owned(),
        message: e.to_string(),
        outage: e.outage().unwrap_or(false),
    }
}

/// Records the hook's outcome, releases the lease and finishes the reply.
fn finish<S: CacheStore, L: DrainLock, B: CacheBackend>(
    shared: &Mutex<Cache<S, L, B>>,
    prepared: Prepared,
    outcome: Option<Result<HookSuccess, CacheError>>,
    period: Option<Duration>,
    row: &str,
) -> Result<BuiltinReply, CacheError> {
    let Prepared {
        lease, mut reply, ..
    } = prepared;
    let mut guard = hold(shared)?;
    let cache = &mut *guard;
    let recorded = record_hook(cache, &mut reply, outcome, period, row);
    let released = lease.release(cache);
    step(
        row,
        &reply.run_id,
        "reply",
        if released {
            "lease_released"
        } else {
            "lease_left_to_expiry"
        },
    );
    recorded.map(|()| reply)
}

fn record_hook<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    reply: &mut BuiltinReply,
    outcome: Option<Result<HookSuccess, CacheError>>,
    period: Option<Duration>,
    row: &str,
) -> Result<(), CacheError> {
    match outcome {
        None => Ok(()),
        Some(Ok(success)) => {
            step(row, &reply.run_id, "hook", "ok");
            reply.hook.status = HookStep::Ok;
            reply.hook.result = success.result;
            reply.hook.stderr = Some(success.stderr);
            cache.hook_succeeded()
        }
        Some(Err(e)) => {
            step(row, &reply.run_id, "hook", "failed");
            let now = cache.clock().now_utc();
            let failures = cache.hook_state()?.consecutive_failures.saturating_add(1);
            let next = next_attempt(failures, period, &now);
            let state = cache.hook_failed(&e.to_string(), next.as_ref())?;
            reply.hook.status = HookStep::Failed;
            if let CacheError::TickRunFailed { stderr, .. } = &e {
                reply.hook.stderr = Some(stderr.clone());
            }
            reply.hook.error = Some(error_object(&e));
            reply.hook.next_attempt_at = state.next_attempt_at;
            Ok(())
        }
    }
}

/// One structured event per run step (REQ-BDC-0730).
fn step(row: &str, run_id: &RunId, step: &str, outcome: &str) {
    tracing::info!(row, run_id = %run_id, step, outcome, "sync run step");
}

fn step_scope(row: &str, run_id: &RunId, r: &ScopeResult) {
    let outcome = match r.result {
        ScopeOutcome::Ok => "ok",
        ScopeOutcome::Failed => "failed",
        ScopeOutcome::Outage => "outage",
        ScopeOutcome::Skipped => "skipped",
    };
    tracing::info!(row, run_id = %run_id, step = "scope", scope = %r.scope, outcome, "sync run step");
}
