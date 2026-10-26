//! A sync row's run history in its recorded outcome (REQ-BDC-0731).
//!
//! sc-cron keeps one last outcome per row and no run counter. `schedule show`
//! reports each row's last run id, its consecutive failures and the first
//! failure's time, so the sync jobs write them at the front of a failed
//! outcome's text: `run=<run id> failures=<n> since=<time>: <message>`. The
//! job of a launch reads the row's previous outcome (the resolver hands it
//! over) and counts on; an `Ok` outcome resets the count.

use bd_cache_model::RunId;
use sc_cron::RunOutcome;

/// What a row's last outcome says of its runs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RowHistory {
    /// The last failed run, when the last outcome is a failure.
    pub run_id: Option<String>,
    /// Consecutive failures up to the last outcome.
    pub failures: u32,
    /// When the first of them was recorded.
    pub since: Option<String>,
}

impl RowHistory {
    /// Reads the history a job wrote into `outcome`.
    #[must_use]
    pub fn of(outcome: Option<&RunOutcome>) -> Self {
        let text = match outcome {
            Some(RunOutcome::Failed(text)) => text,
            Some(RunOutcome::TimedOut) => {
                return Self {
                    failures: 1,
                    ..Self::default()
                };
            }
            _ => return Self::default(),
        };
        let mut out = Self {
            failures: 1,
            ..Self::default()
        };
        let head = text.split_once(": ").map_or("", |(h, _)| h);
        for part in head.split(' ') {
            match part.split_once('=') {
                Some(("run", v)) => out.run_id = Some(v.to_owned()),
                Some(("failures", v)) => out.failures = v.parse().unwrap_or(1),
                Some(("since", v)) => out.since = Some(v.to_owned()),
                _ => {}
            }
        }
        out
    }

    /// The failed outcome of `run_id` at `now`, counting on from `self`.
    #[must_use]
    pub fn failed(&self, run_id: &RunId, now: &str, message: &str) -> RunOutcome {
        let failures = self.failures.saturating_add(1);
        let since = self.since.as_deref().unwrap_or(now);
        RunOutcome::Failed(format!(
            "run={run_id} failures={failures} since={since}: {message}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_count_on_and_an_ok_resets_them() {
        let run = RunId::new("1-2-3").unwrap();
        let first = RowHistory::default().failed(&run, "2026-10-26T00:00:00Z", "exit 1");
        let h = RowHistory::of(Some(&first));
        assert_eq!((h.failures, h.since.as_deref(), h.run_id.as_deref()),
            (1, Some("2026-10-26T00:00:00Z"), Some("1-2-3")));
        let second = h.failed(&run, "2026-10-26T00:01:00Z", "exit 1");
        let h = RowHistory::of(Some(&second));
        assert_eq!((h.failures, h.since.as_deref()), (2, Some("2026-10-26T00:00:00Z")));
        assert_eq!(RowHistory::of(Some(&RunOutcome::Ok)), RowHistory::default());
        assert_eq!(RowHistory::of(Some(&RunOutcome::Failed("plain".into()))).failures, 1);
    }
}
