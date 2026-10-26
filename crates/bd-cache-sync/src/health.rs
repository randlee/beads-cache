//! What `cache health` reads of the sync rows (REQ-BDC-0732), and the one
//! surface the front ends run the api's commands through (ADR-BDC-0015
//! amendment 1).
//!
//! `bd-cache-api` judges health but cannot read the schedule table, which is
//! sc-cron's (ADR-BDC-0014). [`sync_facts`] reads it: the rows' state (no
//! row, both disabled, or the enabled row with its last run and history, the
//! reading `schedule show` makes, [`RowHistory::of`]), one period of that
//! row (`row_period`, REQ-BDC-0727) and the hook's consecutive failures. It
//! is a blocking read of the store and the cache, under no lease and with no
//! write, so health stays a read (contract §6).
//!
//! [`execute_api`] is the one call every front end makes for a command
//! `bd-cache-api` runs: `cache health` goes to
//! [`bd_cache_api::execute_health`] with the facts read now (a failure to
//! read them is passed on, and health reports it as degraded), every other
//! command to [`bd_cache_api::execute`]. So `bdc cache health` and
//! `cache.manage.health()` report the same sync state, and no front end can
//! forget to read it.

use bd_cache::Cache;
use bd_cache_api::{
    ApiError, CacheFacts, Data, Request, SyncFacts, SyncRowFacts, SyncRows, execute, execute_health,
};
use bd_cache_model::{CacheError, CacheStore, DrainLock, SyncMode, SyncRow};
use sc_beads::{CacheBackend, Timestamp};
use sc_cron::{Schedule, ScheduleStore};

use crate::compose::ScheduleTable;
use crate::history::RowHistory;
use crate::payload::SyncPayload;
use crate::schedule::{Rows, outcome_text, row_period};

/// The sync rows' state for `cache health` (module docs): read from `store`
/// (the cache file's schedule table) and from `cache` (the rows' run records,
/// the lease and the hook state).
///
/// # Errors
///
/// `STORE_FAILED` when the table or the cache's records cannot be read, or
/// a row's time is outside the cache's range.
pub fn sync_facts<St, S, L, B>(store: &St, cache: &Cache<S, L, B>) -> Result<SyncFacts, CacheError>
where
    St: ScheduleStore + ?Sized,
    S: CacheStore,
    L: DrainLock,
    B: CacheBackend,
{
    let rows = Rows::read(store)?;
    let enabled = match rows.mode() {
        SyncMode::Builtin => rows.builtin.as_ref().map(|r| (SyncRow::Builtin, r)),
        SyncMode::Python => rows.python.as_ref().map(|r| (SyncRow::Python, r)),
        SyncMode::None => None,
    };
    let state = match enabled {
        Some((kind, row)) => SyncRows::Enabled(row_facts(cache, kind, row)?),
        None if rows.builtin.is_some() || rows.python.is_some() => SyncRows::AllDisabled,
        None => SyncRows::NoRows,
    };
    let names_hook = rows.builtin.as_ref().is_some_and(|r| {
        matches!(SyncPayload::decode(&r.spec.payload), Ok(SyncPayload::Builtin(b)) if b.hook.is_some())
    });
    let hook_failures = if names_hook {
        Some(cache.hook_state()?.consecutive_failures)
    } else {
        None
    };
    Ok(SyncFacts {
        rows: state,
        hook_failures,
    })
}

/// Runs one decoded request of a command `bd-cache-api` runs (not a
/// front-end verb, [`bd_cache_api::Command::is_front_end`]) over `cache`:
/// `cache health` through [`bd_cache_api::execute_health`] with the sync
/// rows' state read now from `table` ([`sync_facts`]; a failure reading it
/// is passed to health, which reports it as `sync_state_unreadable`), every
/// other command through [`bd_cache_api::execute`] with `facts`. The one
/// call of every front end (module docs).
///
/// # Errors
///
/// The command's errors, as [`bd_cache_api::execute`] and
/// [`bd_cache_api::execute_health`] return them.
pub fn execute_api<S, L, B>(
    table: &ScheduleTable,
    cache: &mut Cache<S, L, B>,
    request: &Request,
    facts: &CacheFacts,
) -> Result<Data, ApiError>
where
    S: CacheStore,
    L: DrainLock,
    B: CacheBackend,
{
    match request {
        Request::CacheHealth(r) => {
            let sync = table
                .store()
                .and_then(|store| sync_facts(store.as_ref(), cache));
            execute_health(cache, *r, sync)
        }
        _ => execute(cache, request, facts),
    }
}

/// The enabled row `row` of kind `kind`, with its history as `schedule show` reads it.
fn row_facts<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    kind: SyncRow,
    row: &Schedule,
) -> Result<SyncRowFacts, CacheError> {
    let record = cache.sync_run(kind.as_str())?;
    let lease = cache.lease()?;
    let history = RowHistory::of(record.as_ref(), row, lease.as_ref());
    Ok(SyncRowFacts {
        row: kind,
        created_at: cache_time(&row.created_at)?,
        period: row_period(row),
        last_run_at: row.last_run_at.as_ref().map(cache_time).transpose()?,
        outcome: row.last_outcome.as_ref().map(outcome_text),
        run_id: history.run_id.map(|id| id.as_str().to_owned()),
        consecutive_failures: history.consecutive_failures,
    })
}

/// A sc-cron time as the cache's [`Timestamp`] (whole seconds, UTC).
fn cache_time(at: &sc_cron::Timestamp) -> Result<Timestamp, CacheError> {
    let secs = at.as_datetime().timestamp();
    Timestamp::from_unix(secs).map_err(|e| {
        CacheError::store_failed(
            format!("schedule store (sc_cron_schedule): a time of {secs} s is out of range: {e}"),
            e,
        )
    })
}
