//! What `cache health` reads of the sync rows (REQ-BDC-0732).
//!
//! `bd-cache-api` judges health but cannot read the schedule table, which is
//! sc-cron's (ADR-BDC-0014). [`sync_facts`] reads it for every front end, so
//! `bdc cache health` and `cache.manage.health()` report the same sync state:
//! the mode, the enabled row's last run and history (the reading
//! `schedule show` makes, [`RowHistory::of`]), one period of that row
//! ([`row_period`], REQ-BDC-0727) and the hook's consecutive failures. It is
//! a blocking read of the store and the cache, under no lease and with no
//! write, so health stays a read (contract §6).

use bd_cache::Cache;
use bd_cache_api::{SyncFacts, SyncRowFacts, data::SyncModeData};
use bd_cache_model::{CacheError, CacheStore, DrainLock};
use sc_beads::{CacheBackend, Timestamp};
use sc_cron::{Schedule, ScheduleStore};

use crate::history::RowHistory;
use crate::payload::{SyncPayload, SyncRow};
use crate::schedule::{Rows, SyncMode, outcome_text, row_period};

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
    let mode = rows.mode();
    let enabled = match mode {
        SyncMode::Builtin => rows.builtin.as_ref().map(|r| (SyncRow::Builtin, r)),
        SyncMode::Python => rows.python.as_ref().map(|r| (SyncRow::Python, r)),
        SyncMode::None => None,
    };
    let enabled = match enabled {
        Some((kind, row)) => Some(row_facts(cache, kind, row)?),
        None => None,
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
        mode: match mode {
            SyncMode::Builtin => SyncModeData::Builtin,
            SyncMode::Python => SyncModeData::Python,
            SyncMode::None => SyncModeData::None,
        },
        has_rows: rows.builtin.is_some() || rows.python.is_some(),
        enabled,
        hook_failures,
    })
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
        row: match kind {
            SyncRow::Builtin => "builtin",
            SyncRow::Python => "python",
        },
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
