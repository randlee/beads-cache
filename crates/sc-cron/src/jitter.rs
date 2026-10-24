//! Per-schedule jitter, so schedules with one expression do not all fire at once.
//!
//! When a schedule is started the scheduler draws one offset from a
//! [`JitterSource`] and stores it with the schedule. Every occurrence then
//! fires at `occurrence + jitter`, and the offset survives a restart. The
//! upper bound is half the expression's period, capped at 30 minutes, so a
//! jittered run never drifts into the following occurrence.

#[cfg(feature = "test-utils")]
use std::collections::VecDeque;
#[cfg(feature = "test-utils")]
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use chrono::{DateTime, Utc};

/// Largest jitter any schedule gets.
pub(crate) const MAX_JITTER: Duration = Duration::from_secs(30 * 60);

/// Source of the random offset drawn when a schedule starts.
pub trait JitterSource: Send + Sync {
    /// Returns an offset in `[0, max)`, or zero when `max` is zero.
    ///
    /// The scheduler clamps a larger value to `max`.
    fn jitter(&self, max: Duration) -> Duration;
}

/// Uniform jitter at millisecond resolution from the operating system's RNG.
///
/// If the operating system cannot supply random bytes the offset is zero:
/// jitter spreads load, it is not a correctness property.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemJitter;

impl JitterSource for SystemJitter {
    fn jitter(&self, max: Duration) -> Duration {
        let upper = u64::try_from(max.as_millis()).unwrap_or(u64::MAX);
        if upper == 0 {
            return Duration::ZERO;
        }
        // Rejection sampling: drop the low values that would bias `% upper`.
        let threshold = upper.wrapping_neg() % upper;
        loop {
            match getrandom::u64() {
                Ok(value) if value >= threshold => {
                    return Duration::from_millis(value % upper);
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!(subsystem = "sc_cron", %error, "no random source; jitter is zero");
                    return Duration::ZERO;
                }
            }
        }
    }
}

/// Jitter that replays preset offsets in order, then returns zero.
///
/// It also records every `max` it was asked for, so a test can check the
/// bound the scheduler computed.
#[cfg(feature = "test-utils")]
#[derive(Debug, Default)]
pub struct SeqJitter {
    values: Mutex<VecDeque<Duration>>,
    calls: Mutex<Vec<Duration>>,
}

#[cfg(feature = "test-utils")]
impl SeqJitter {
    /// Creates a source that returns `values` in order.
    #[must_use]
    pub fn new(values: impl IntoIterator<Item = Duration>) -> Self {
        Self {
            values: Mutex::new(values.into_iter().collect()),
            calls: Mutex::new(Vec::new()),
        }
    }

    /// Queues one more offset.
    pub fn push(&self, value: Duration) {
        self.values
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push_back(value);
    }

    /// Returns every `max` passed to [`JitterSource::jitter`] so far.
    #[must_use]
    pub fn calls(&self) -> Vec<Duration> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

#[cfg(feature = "test-utils")]
impl JitterSource for SeqJitter {
    fn jitter(&self, max: Duration) -> Duration {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(max);
        self.values
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .unwrap_or_default()
    }
}

/// The jitter bound for `schedule` as of `now`: half the gap between its next
/// two occurrences, capped at [`MAX_JITTER`]; zero with fewer than two.
pub(crate) fn bound(schedule: &cron::Schedule, now: DateTime<Utc>) -> Duration {
    let mut occurrences = schedule.after(&now);
    match (occurrences.next(), occurrences.next()) {
        (Some(first), Some(second)) => ((second - first) / 2)
            .to_std()
            .unwrap_or_default()
            .min(MAX_JITTER),
        _ => Duration::ZERO,
    }
}

/// Draws a jitter for `schedule` from `source`, clamped to the bound.
pub(crate) fn draw(
    schedule: &cron::Schedule,
    now: DateTime<Utc>,
    source: &dyn JitterSource,
) -> Duration {
    let max = bound(schedule, now);
    source.jitter(max).min(max)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    fn at(text: &str) -> DateTime<Utc> {
        text.parse().expect("timestamp")
    }

    #[test]
    fn bound_is_half_the_period_capped_at_thirty_minutes() {
        let now = at("2026-10-19T00:00:00Z");
        for (expr, expected) in [
            ("0 * * * * *", Duration::from_secs(30)),
            ("@hourly", MAX_JITTER),
            ("@daily", MAX_JITTER),
            ("*/10 * * * * *", Duration::from_secs(5)),
        ] {
            let schedule = cron::Schedule::from_str(expr).expect("expression");
            assert_eq!(bound(&schedule, now), expected, "{expr}");
        }
    }

    #[test]
    fn bound_is_zero_for_a_last_occurrence() {
        let schedule = cron::Schedule::from_str("0 0 0 20 10 * 2026").expect("expression");
        assert_eq!(bound(&schedule, at("2026-10-19T00:00:00Z")), Duration::ZERO);
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn draw_clamps_an_out_of_range_source() {
        let schedule = cron::Schedule::from_str("0 * * * * *").expect("expression");
        let source = SeqJitter::new([Duration::from_secs(600)]);
        assert_eq!(
            draw(&schedule, at("2026-10-19T00:00:00Z"), &source),
            Duration::from_secs(30)
        );
        assert_eq!(source.calls(), [Duration::from_secs(30)]);
    }

    #[test]
    fn system_jitter_stays_below_the_bound() {
        assert_eq!(SystemJitter.jitter(Duration::ZERO), Duration::ZERO);
        for _ in 0..200 {
            assert!(SystemJitter.jitter(Duration::from_millis(7)) < Duration::from_millis(7));
        }
    }
}
