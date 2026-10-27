//! A sync row's run history (REQ-BDC-0731): the run id, the consecutive
//! failures and the first failure's time, kept as typed data.
//!
//! sc-cron keeps one last outcome per row and no run id or counter. The sync
//! therefore keeps its own record per row in the cache file (`sync_run`,
//! ADR-STORE-0006 as amended by sprint b-8), written through the cache:
//!
//! - a run is *started* before it does anything (the resolver hands the job
//!   the run id it minted), and *finished* when it returns: `ok` resets the
//!   count, `failed` adds one, `skipped` (the lease was held, or the stop
//!   check said to skip) leaves it;
//! - a run that never finished (sc-cron timed it out or aborted it, or the
//!   process died) is counted as a failure when the next run starts, and
//!   `schedule show` counts it at once when sc-cron has recorded its end;
//! - a row whose payload or trust check fails at resolve time is recorded
//!   as a failed run by the job the resolver returns in its place.
//!
//! [`RowHistory::of`] is the one reading of a record for `schedule show`.

use bd_cache_model::{CacheError, CacheStore, DrainLock, LeaseHolder, RunId, SyncRun, SyncRunEnd};
use sc_beads::{CacheBackend, Timestamp};
use sc_cron::Schedule;
use serde::Serialize;

use crate::lease::hold;
use crate::payload::SyncRow;
use crate::run::{SharedCache, blocking};

/// What `schedule show` reports of a row's runs (REQ-BDC-0731).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RowHistory {
    /// The latest run, finished or not.
    pub run_id: Option<RunId>,
    /// Consecutive failed runs, the latest one included.
    pub consecutive_failures: u32,
    /// When the first of them happened.
    pub first_failure_at: Option<Timestamp>,
}

impl RowHistory {
    /// The history of `row` from its run `record` and the current `lease`:
    /// a latest run that did not finish, holds no lease and whose end sc-cron
    /// recorded (a timeout, an abort) counts as one more failure.
    #[must_use]
    pub fn of(record: Option<&SyncRun>, row: &Schedule, lease: Option<&LeaseHolder>) -> Self {
        let Some(record) = record else {
            return Self::default();
        };
        let mut history = Self {
            run_id: Some(record.run_id.clone()),
            consecutive_failures: record.consecutive_failures,
            first_failure_at: record.first_failure_at.clone(),
        };
        let running = lease.is_some_and(|l| l.run_id == record.run_id);
        let ended_unfinished = record.finished_at.is_none()
            && !running
            && row
                .last_run_at
                .is_some_and(|at| at.as_datetime().timestamp() >= record.started_at.unix());
        if ended_unfinished {
            history.consecutive_failures = history.consecutive_failures.saturating_add(1);
            history.first_failure_at = history
                .first_failure_at
                .or_else(|| Some(record.started_at.clone()));
        }
        history
    }
}

/// Records one run of a sync row in its run record (module docs).
pub(crate) struct RunRecord<S: CacheStore, L: DrainLock, B: CacheBackend> {
    cache: SharedCache<S, L, B>,
    row: SyncRow,
    run_id: RunId,
}

impl<S, L, B> RunRecord<S, L, B>
where
    S: CacheStore + 'static,
    L: DrainLock + 'static,
    B: CacheBackend + 'static,
{
    pub(crate) fn new(cache: SharedCache<S, L, B>, row: SyncRow, run_id: RunId) -> Self {
        Self { cache, row, run_id }
    }

    /// The run's id.
    pub(crate) fn run_id(&self) -> &RunId {
        &self.run_id
    }

    /// Records the start. A failed write is logged: the run goes on, and
    /// only `schedule show`'s count is then behind.
    pub(crate) async fn start(&self) {
        let (cache, row, run_id) = (
            std::sync::Arc::clone(&self.cache),
            self.row,
            self.run_id.clone(),
        );
        let written = blocking(move || hold(&cache)?.sync_run_started(row.as_str(), &run_id)).await;
        if let Err(e) = written {
            log_failed(self.row, &self.run_id, "start", &e);
        }
    }

    /// Records the end (logged on failure, as [`RunRecord::start`]).
    pub(crate) async fn finish(&self, end: SyncRunEnd) {
        let (cache, row, run_id) = (
            std::sync::Arc::clone(&self.cache),
            self.row,
            self.run_id.clone(),
        );
        let written =
            blocking(move || hold(&cache)?.sync_run_finished(row.as_str(), &run_id, end)).await;
        if let Err(e) = written {
            log_failed(self.row, &self.run_id, "end", &e);
        }
    }
}

fn log_failed(row: SyncRow, run_id: &RunId, what: &str, e: &CacheError) {
    tracing::warn!(row = row.as_str(), run_id = %run_id, code = e.code(), detail = e.detail(),
        "the run's {what} could not be recorded in its run record");
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn row(last_run_at: Option<&str>) -> Schedule {
        let spec = sc_cron::ScheduleSpec::new(
            "s",
            "*/30 * * * * *",
            Duration::from_secs(5),
            serde_json::json!({}),
        )
        .unwrap();
        let at: sc_cron::Timestamp = "2026-10-26T10:00:00Z".parse().unwrap();
        Schedule {
            id: sc_cron::ScheduleId::new(1),
            generation: sc_cron::Generation::FIRST,
            spec,
            jitter: Duration::ZERO,
            created_at: at,
            next_run_at: Some(at),
            last_run_at: last_run_at.map(|t| t.parse().unwrap()),
            last_outcome: None,
            ended_at: None,
            stop_reason: None,
            enabled: true,
        }
    }

    fn ts(s: &str) -> Timestamp {
        Timestamp::parse(s).unwrap()
    }

    /// A latest run sc-cron ended (timed out) without its finish counts as
    /// one more failure from its start; planted negatives: the same run
    /// while it holds the lease, or before sc-cron recorded its end, does not.
    #[test]
    fn an_unfinished_run_counts_once_sc_cron_recorded_its_end() {
        let run = RunId::new("1-2-3").unwrap();
        let record = SyncRun::started(None, &run, &ts("2026-10-26T10:00:01Z"));
        let timed_out = row(Some("2026-10-26T10:00:05Z"));
        let h = RowHistory::of(Some(&record), &timed_out, None);
        assert_eq!(
            (
                h.run_id.as_ref(),
                h.consecutive_failures,
                h.first_failure_at
            ),
            (Some(&run), 1, Some(ts("2026-10-26T10:00:01Z")))
        );
        let lease = LeaseHolder {
            run_id: run.clone(),
            pid: 1,
            host: "h".into(),
            expires_at: ts("2026-10-26T10:01:00Z"),
        };
        assert_eq!(
            RowHistory::of(Some(&record), &timed_out, Some(&lease)).consecutive_failures,
            0
        );
        assert_eq!(
            RowHistory::of(Some(&record), &row(None), None).consecutive_failures,
            0
        );
        let ok = record.finished(&ts("2026-10-26T10:00:02Z"), SyncRunEnd::Ok);
        let h = RowHistory::of(Some(&ok), &timed_out, None);
        assert_eq!((h.run_id, h.consecutive_failures), (Some(run), 0));
        assert_eq!(
            RowHistory::of(None, &timed_out, None),
            RowHistory::default()
        );
    }
}
