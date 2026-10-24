//! Per-schedule jitter, so schedules with one expression do not all fire at once.
//!
//! When a schedule is started the scheduler draws one offset from a
//! [`JitterSource`] and stores it with the schedule. Every occurrence then
//! fires at `occurrence + jitter`, and the offset survives a restart. The
//! upper bound is half the expression's period, capped at 30 minutes, so a
//! jittered run never drifts into the following occurrence.

use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::CronExpr;

/// Largest jitter any schedule gets.
pub(crate) const MAX_JITTER: Duration = Duration::from_secs(30 * 60);

/// Source of the random offset drawn when a schedule starts.
pub trait JitterSource: Send + Sync {
    /// Returns an offset in `[0, max)`, or zero when `max` is zero.
    ///
    /// The scheduler clamps a larger value to `max` and drops precision
    /// finer than a millisecond.
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

#[cfg(feature = "test-utils")]
pub use seq::SeqJitter;

#[cfg(feature = "test-utils")]
mod seq {
    use std::collections::VecDeque;
    use std::sync::{Mutex, PoisonError};
    use std::time::Duration;

    use super::JitterSource;

    /// Jitter that replays preset offsets in order, then returns zero.
    ///
    /// It also records every `max` it was asked for, so a test can check the
    /// bound the scheduler computed.
    #[derive(Debug, Default)]
    pub struct SeqJitter {
        // `JitterSource::jitter` takes `&self` and the source is shared
        // across threads (`Send + Sync`), so the queue and the call log need
        // interior mutability. One mutex covers both: every call reads one
        // and appends to the other.
        state: Mutex<SeqState>,
    }

    #[derive(Debug, Default)]
    struct SeqState {
        values: VecDeque<Duration>,
        calls: Vec<Duration>,
    }

    impl SeqJitter {
        /// Creates a source that returns `values` in order.
        #[must_use]
        pub fn new(values: impl IntoIterator<Item = Duration>) -> Self {
            Self {
                state: Mutex::new(SeqState {
                    values: values.into_iter().collect(),
                    calls: Vec::new(),
                }),
            }
        }

        /// Queues one more offset.
        pub fn push(&self, value: Duration) {
            self.lock().values.push_back(value);
        }

        /// Returns every `max` passed to [`JitterSource::jitter`] so far.
        #[must_use]
        pub fn calls(&self) -> Vec<Duration> {
            self.lock().calls.clone()
        }

        fn lock(&self) -> std::sync::MutexGuard<'_, SeqState> {
            // The state is two plain collections; a panic mid-update leaves
            // them usable, so poisoning is ignored.
            self.state.lock().unwrap_or_else(PoisonError::into_inner)
        }
    }

    impl JitterSource for SeqJitter {
        fn jitter(&self, max: Duration) -> Duration {
            let mut state = self.lock();
            state.calls.push(max);
            state.values.pop_front().unwrap_or_default()
        }
    }
}

/// The jitter bound for `expr` as of `now`: half the gap between its next
/// two occurrences, capped at [`MAX_JITTER`]; zero with fewer than two.
pub(crate) fn bound(expr: &CronExpr, now: DateTime<Utc>) -> Duration {
    let mut occurrences = expr.schedule().after(&now);
    match (occurrences.next(), occurrences.next()) {
        (Some(first), Some(second)) => ((second - first) / 2)
            .to_std()
            .unwrap_or_default()
            .min(MAX_JITTER),
        _ => Duration::ZERO,
    }
}

/// Draws a jitter for `expr` from `source`: clamped to the bound and
/// truncated to whole milliseconds, so it round-trips through a store.
pub(crate) fn draw(expr: &CronExpr, now: DateTime<Utc>, source: &dyn JitterSource) -> Duration {
    let max = bound(expr, now);
    let drawn = source.jitter(max).min(max);
    Duration::from_millis(u64::try_from(drawn.as_millis()).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> DateTime<Utc> {
        text.parse().expect("timestamp")
    }

    fn expr(text: &str) -> CronExpr {
        CronExpr::parse(text).expect("expression")
    }

    #[test]
    fn bound_is_half_the_period_capped_at_thirty_minutes() {
        let now = at("2026-10-19T00:00:00Z");
        for (text, expected) in [
            ("0 * * * * *", Duration::from_secs(30)),
            ("@hourly", MAX_JITTER),
            ("@daily", MAX_JITTER),
            ("*/10 * * * * *", Duration::from_secs(5)),
        ] {
            assert_eq!(bound(&expr(text), now), expected, "{text}");
        }
    }

    #[test]
    fn bound_is_zero_for_a_last_occurrence() {
        assert_eq!(
            bound(&expr("0 0 0 20 10 * 2026"), at("2026-10-19T00:00:00Z")),
            Duration::ZERO
        );
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn draw_clamps_and_truncates_an_out_of_range_source() {
        let source = SeqJitter::new([Duration::from_secs(600), Duration::from_micros(1500)]);
        let now = at("2026-10-19T00:00:00Z");
        assert_eq!(
            draw(&expr("0 * * * * *"), now, &source),
            Duration::from_secs(30)
        );
        assert_eq!(
            draw(&expr("0 * * * * *"), now, &source),
            Duration::from_millis(1)
        );
        assert_eq!(source.calls(), [Duration::from_secs(30); 2]);
    }

    #[test]
    fn system_jitter_stays_below_the_bound() {
        assert_eq!(SystemJitter.jitter(Duration::ZERO), Duration::ZERO);
        for _ in 0..200 {
            assert!(SystemJitter.jitter(Duration::from_millis(7)) < Duration::from_millis(7));
        }
    }
}
