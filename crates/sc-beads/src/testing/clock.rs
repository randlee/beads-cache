//! [`ManualClock`]: a clock tests move by hand.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::lock;
use crate::clock::{rfc3339_to_unix, Clock, Timestamp};

/// A [`Clock`] that only moves when told to.
///
/// Monotonic time starts at the instant of construction and advances only
/// through [`ManualClock::advance`]. UTC time starts at the given RFC 3339
/// instant and advances with it; [`ManualClock::set_utc`] re-anchors it
/// without moving monotonic time. Share it as `Arc<ManualClock>`, which
/// coerces to `Arc<dyn Clock>`.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use sc_beads::{Clock, Deadline};
/// use sc_beads::testing::ManualClock;
///
/// let clock = ManualClock::new("2026-10-23T10:00:00Z");
/// let d = Deadline::after(&clock, Duration::from_millis(500));
/// clock.advance(Duration::from_millis(499));
/// assert!(!d.expired(&clock));
/// clock.advance(Duration::from_millis(1));
/// assert!(d.expired(&clock));
/// clock.advance(Duration::from_secs(60));
/// assert_eq!(clock.now_utc().as_str(), "2026-10-23T10:01:00Z");
/// ```
#[derive(Debug)]
pub struct ManualClock {
    state: Mutex<State>,
}

#[derive(Debug)]
struct State {
    origin: Instant,
    elapsed: Duration,
    /// UTC seconds at `anchor_elapsed`.
    utc_anchor: i64,
    anchor_elapsed: Duration,
}

impl ManualClock {
    /// A clock whose UTC time starts at `utc` (RFC 3339).
    ///
    /// # Panics
    ///
    /// When `utc` is not an RFC 3339 date-time (a test bug).
    #[must_use]
    #[expect(clippy::panic, reason = "an invalid start time is a test bug")]
    pub fn new(utc: &str) -> Self {
        let Some(utc_anchor) = rfc3339_to_unix(utc) else {
            panic!("ManualClock::new: {utc:?} is not RFC 3339");
        };
        Self {
            state: Mutex::new(State {
                origin: Instant::now(),
                elapsed: Duration::ZERO,
                utc_anchor,
                anchor_elapsed: Duration::ZERO,
            }),
        }
    }

    /// Moves both monotonic and UTC time forward by `d`.
    pub fn advance(&self, d: Duration) {
        let mut s = lock(&self.state);
        s.elapsed += d;
    }

    /// Sets UTC time to `utc` from now on; monotonic time does not move.
    ///
    /// # Panics
    ///
    /// When `utc` is not an RFC 3339 date-time (a test bug).
    #[expect(clippy::panic, reason = "an invalid time is a test bug")]
    pub fn set_utc(&self, utc: &str) {
        let Some(secs) = rfc3339_to_unix(utc) else {
            panic!("ManualClock::set_utc: {utc:?} is not RFC 3339");
        };
        let mut s = lock(&self.state);
        s.utc_anchor = secs;
        s.anchor_elapsed = s.elapsed;
    }

    /// Monotonic time elapsed since construction.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        lock(&self.state).elapsed
    }
}

impl Default for ManualClock {
    /// Starts at `2026-10-23T10:00:00Z`.
    fn default() -> Self {
        Self::new("2026-10-23T10:00:00Z")
    }
}

impl Clock for ManualClock {
    fn now_utc(&self) -> Timestamp {
        let s = lock(&self.state);
        let since_anchor = s.elapsed.saturating_sub(s.anchor_elapsed).as_secs();
        Timestamp::from_unix(s.utc_anchor + i64::try_from(since_anchor).unwrap_or(i64::MAX / 2))
    }

    fn now_mono(&self) -> Instant {
        let s = lock(&self.state);
        s.origin + s.elapsed
    }
}
