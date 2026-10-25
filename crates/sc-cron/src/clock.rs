//! Wall-clock and monotonic time for the scheduler.
//!
//! The scheduler reads time only through [`Clock`], so tests drive it with
//! `ManualClock` (feature `test-utils`) and never wait on real time.
//! [`SystemClock`] sleeps in slices of at most 30 s and re-reads the wall
//! clock after each, so a wall-clock jump or a host suspend is noticed
//! within one slice.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// The scheduler's source of time.
///
/// `now` is the wall clock that cron occurrences are computed against.
/// `monotonic_now` is the timeline run deadlines are on: the worker times
/// runs out against it, so a wall-clock jump never cuts a run short, and a
/// [`Job`](crate::Job) compares its deadline with it rather than with
/// `Instant::now`, so it behaves the same under a manual clock.
/// `sleep_until` returns once `now` reaches `target` and is the only place
/// the scheduler waits on time.
#[async_trait]
pub trait Clock: Send + Sync {
    /// Returns the current wall-clock time in UTC.
    fn now(&self) -> DateTime<Utc>;

    /// Returns the current monotonic instant; deadlines are on this timeline.
    fn monotonic_now(&self) -> Instant {
        Instant::now()
    }

    /// Waits until [`Clock::now`] is at or after `target`.
    async fn sleep_until(&self, target: DateTime<Utc>);
}

/// The real clock: `chrono::Utc::now`, `Instant::now` and tokio timers.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

/// Longest single sleep, so wall-clock changes are noticed promptly.
pub(crate) const MAX_SLEEP_SLICE: Duration = Duration::from_secs(30);

pub(crate) fn system_sleep_step(now: DateTime<Utc>, target: DateTime<Utc>) -> Duration {
    (target - now)
        .to_std()
        .unwrap_or_default()
        .min(MAX_SLEEP_SLICE)
}

#[async_trait]
impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    async fn sleep_until(&self, target: DateTime<Utc>) {
        loop {
            let step = system_sleep_step(self.now(), target);
            if step.is_zero() {
                return;
            }
            tokio::time::sleep(step).await;
        }
    }
}

/// Sleeps on `clock` until its monotonic time reaches `deadline`.
///
/// The wait goes through [`Clock::sleep_until`], targeting the wall time at
/// which the remaining monotonic span will have passed, and re-checks the
/// monotonic clock after each wake.
pub(crate) async fn sleep_until_monotonic<C: Clock + ?Sized>(clock: &C, deadline: Instant) {
    loop {
        let remaining = deadline.saturating_duration_since(clock.monotonic_now());
        if remaining.is_zero() {
            return;
        }
        clock.sleep_until(wall_after(clock.now(), remaining)).await;
    }
}

/// `now + span`, saturating at the latest representable time.
pub(crate) fn wall_after(now: DateTime<Utc>, span: Duration) -> DateTime<Utc> {
    chrono::Duration::from_std(span)
        .ok()
        .and_then(|span| now.checked_add_signed(span))
        .unwrap_or(DateTime::<Utc>::MAX_UTC)
}

#[cfg(feature = "test-utils")]
pub use manual::ManualClock;

#[cfg(feature = "test-utils")]
mod manual {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};

    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use tokio::sync::watch;

    use super::Clock;

    /// Longest a test helper waits for the worker before panicking.
    const SETTLE_TIMEOUT: Duration = Duration::from_secs(10);

    /// One park: how many sleepers have blocked so far, and the time the last one saw.
    #[derive(Clone, Copy, Debug)]
    struct Park {
        count: u64,
        observed: DateTime<Utc>,
    }

    /// A clock that moves only when a test moves it.
    ///
    /// Every time a [`Clock::sleep_until`] caller blocks, the clock records
    /// a park together with the time that caller saw. The scheduler's worker
    /// is its only sleeper, so after [`ManualClock::advance_settled`]
    /// returns, the worker has seen the new time, handled every tick due at
    /// it, and is waiting again. Monotonic time moves forward with every
    /// positive step and never moves back.
    #[derive(Debug)]
    pub struct ManualClock {
        now: watch::Sender<DateTime<Utc>>,
        parks: watch::Sender<Park>,
        base: Instant,
        // Nanoseconds of monotonic time past `base`. `advance` takes `&self`
        // and the clock is shared across tasks, so the offset needs interior
        // mutability; an atomic is enough because it only ever grows.
        offset_nanos: AtomicU64,
    }

    impl ManualClock {
        /// Creates a clock reading `start`.
        #[must_use]
        pub fn new(start: DateTime<Utc>) -> Self {
            Self {
                now: watch::channel(start).0,
                parks: watch::channel(Park {
                    count: 0,
                    observed: start,
                })
                .0,
                base: Instant::now(),
                offset_nanos: AtomicU64::new(0),
            }
        }

        /// Moves both clocks forward by `step`.
        pub fn advance(&self, step: chrono::Duration) {
            self.advance_monotonic(step);
            self.now.send_modify(|now| *now += step);
        }

        /// Sets the wall clock; monotonic time moves only if `time` is later.
        pub fn set(&self, time: DateTime<Utc>) {
            self.advance_monotonic(time - self.now());
            self.now.send_replace(time);
        }

        /// Steps the wall clock to `time` without moving monotonic time.
        ///
        /// This models an NTP step or a manual clock change: cron
        /// occurrences follow the wall clock, run deadlines do not.
        pub fn jump_wall(&self, time: DateTime<Utc>) {
            self.now.send_replace(time);
        }

        fn advance_monotonic(&self, step: chrono::Duration) {
            let nanos = step
                .to_std()
                .map_or(0, |step| u64::try_from(step.as_nanos()).unwrap_or(u64::MAX));
            self.offset_nanos.fetch_add(nanos, Ordering::SeqCst);
        }

        /// Returns how many times a sleeper has blocked so far.
        #[must_use]
        pub fn parks(&self) -> u64 {
            self.parks.borrow().count
        }

        /// Waits until the park count exceeds `count`.
        ///
        /// # Panics
        ///
        /// Panics if that does not happen within ten seconds of real time,
        /// so a test fails instead of hanging.
        pub async fn wait_parks_above(&self, count: u64) {
            self.wait_park(count, DateTime::<Utc>::MIN_UTC).await;
        }

        /// Waits for a park after `count` by a sleeper that saw time `at` or later.
        ///
        /// # Panics
        ///
        /// Panics as [`ManualClock::wait_parks_above`] does.
        pub async fn wait_parked_at(&self, count: u64, at: DateTime<Utc>) {
            self.wait_park(count, at).await;
        }

        async fn wait_park(&self, count: u64, at: DateTime<Utc>) {
            let mut parks = self.parks.subscribe();
            let settled = tokio::time::timeout(
                SETTLE_TIMEOUT,
                parks.wait_for(|park| park.count > count && park.observed >= at),
            )
            .await;
            let last = *self.parks.borrow();
            assert!(
                matches!(settled, Ok(Ok(_))),
                "manual clock: expected a park above {count} at or after {at}, \
                 last was {} at {}",
                last.count,
                last.observed
            );
        }

        /// Advances by `step`, then waits for the worker to block again at the new time.
        ///
        /// A park recorded by a sleeper that still saw the old time does not
        /// count, so the wait cannot be satisfied before the worker has
        /// handled the new time.
        ///
        /// # Panics
        ///
        /// Panics as [`ManualClock::wait_parks_above`] does.
        pub async fn advance_settled(&self, step: chrono::Duration) {
            let count = self.parks();
            self.advance(step);
            self.wait_park(count, self.now()).await;
        }
    }

    #[async_trait]
    impl Clock for ManualClock {
        fn now(&self) -> DateTime<Utc> {
            *self.now.borrow()
        }

        fn monotonic_now(&self) -> Instant {
            self.base + Duration::from_nanos(self.offset_nanos.load(Ordering::SeqCst))
        }

        async fn sleep_until(&self, target: DateTime<Utc>) {
            let mut receiver = self.now.subscribe();
            loop {
                let observed = *receiver.borrow_and_update();
                if observed >= target {
                    return;
                }
                self.parks.send_modify(|park| {
                    park.count += 1;
                    park.observed = observed;
                });
                if receiver.changed().await.is_err() {
                    // The clock is gone; nothing will ever wake this sleeper.
                    std::future::pending::<()>().await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_sleep_step_is_capped_at_the_slice() {
        let now: DateTime<Utc> = "2026-10-19T00:00:00Z".parse().expect("timestamp");
        assert_eq!(
            system_sleep_step(now, now + chrono::Duration::hours(1)),
            MAX_SLEEP_SLICE
        );
        assert_eq!(
            system_sleep_step(now, now + chrono::Duration::seconds(10)),
            Duration::from_secs(10)
        );
        assert_eq!(
            system_sleep_step(now, now - chrono::Duration::seconds(1)),
            Duration::ZERO
        );
    }

    #[test]
    fn wall_after_saturates() {
        let now: DateTime<Utc> = "2026-10-19T00:00:00Z".parse().expect("timestamp");
        assert_eq!(
            wall_after(now, Duration::from_secs(1)),
            now + chrono::Duration::seconds(1)
        );
        assert_eq!(wall_after(now, Duration::MAX), DateTime::<Utc>::MAX_UTC);
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn manual_monotonic_time_moves_only_forward_with_the_clock() {
        let clock = ManualClock::new("2026-10-19T00:00:00Z".parse().expect("timestamp"));
        let initial = clock.monotonic_now();
        clock.advance(chrono::Duration::seconds(60));
        assert_eq!(clock.monotonic_now(), initial + Duration::from_secs(60));
        clock.set(clock.now() - chrono::Duration::seconds(30));
        assert_eq!(clock.monotonic_now(), initial + Duration::from_secs(60));
        clock.set(clock.now() + chrono::Duration::seconds(10));
        assert_eq!(clock.monotonic_now(), initial + Duration::from_secs(70));
    }

    #[cfg(feature = "test-utils")]
    #[tokio::test]
    async fn advance_settled_ignores_a_park_that_saw_the_old_time() {
        use std::sync::Arc;

        let clock = Arc::new(ManualClock::new(
            "2026-10-19T00:00:00Z".parse().expect("timestamp"),
        ));
        let far = clock.now() + chrono::Duration::hours(1);
        let sleeper = {
            let clock = Arc::clone(&clock);
            tokio::spawn(async move { clock.sleep_until(far).await })
        };
        clock.wait_parks_above(0).await;
        // The sleeper re-parks after seeing the new time; only that park counts.
        clock.advance_settled(chrono::Duration::seconds(5)).await;
        assert_eq!(clock.parks(), 2);
        clock.advance(chrono::Duration::hours(1));
        sleeper.await.expect("sleeper");
    }
}
