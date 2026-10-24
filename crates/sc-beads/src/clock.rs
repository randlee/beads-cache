//! Time: the [`Clock`] port, [`SystemClock`], per-call [`Deadline`]s and RFC 3339 helpers.
//!
//! Every component that reads time takes an `Arc<dyn Clock>` so tests can drive
//! it with `testing::ManualClock`. Two readings are kept apart:
//!
//! - [`Clock::now_utc`] is wall-clock time as a [`Timestamp`]: RFC 3339 UTC
//!   with second precision (`2026-10-23T10:00:00Z`), the format of every
//!   timestamp the cache stores (REQ-BDC-0107).
//! - [`Clock::now_mono`] is a monotonic [`Instant`], the basis of
//!   [`Deadline`]s.
//!
//! A [`Deadline`] is an instant a backend call must stop at. The cache builds
//! one per call from the caller's hard deadline and its own per-call cap
//! ([`Deadline::earliest`]); a backend implementation stops at the deadline it
//! is handed and applies no cap of its own.

use std::fmt;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::cache::InvalidValue;

/// An instant as RFC 3339 UTC with second precision: `YYYY-MM-DDTHH:MM:SSZ`.
///
/// The one timestamp type of the clock, the store and the journal. Any
/// timezone-aware RFC 3339 input is normalized to UTC seconds (fractions are
/// truncated), so the text orders as the instants do.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use sc_beads::Timestamp;
/// let t = Timestamp::parse("2026-10-23T12:00:00.9+02:00").unwrap();
/// assert_eq!(t.as_str(), "2026-10-23T10:00:00Z");
/// assert_eq!(t.minus(Duration::from_secs(120)).as_str(), "2026-10-23T09:58:00Z");
/// assert!(Timestamp::parse("yesterday").is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Timestamp(String);

impl Timestamp {
    /// Parses any timezone-aware RFC 3339 date-time.
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] for anything else.
    pub fn parse(s: &str) -> Result<Self, InvalidValue> {
        let invalid = |reason| InvalidValue {
            kind: "timestamp",
            value: s.to_owned(),
            reason,
        };
        let secs = rfc3339_to_unix(s)
            .ok_or_else(|| invalid("expected an RFC 3339 date-time with a timezone"))?;
        if (Self::MIN_UNIX..=Self::MAX_UNIX).contains(&secs) {
            Ok(Self(unix_to_rfc3339(secs)))
        } else {
            Err(invalid("outside years 0000..=9999 once normalized to UTC"))
        }
    }

    /// The instant `secs` after the Unix epoch.
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] when the instant is outside years 0000..=9999 (UTC),
    /// which the four-digit RFC 3339 year cannot represent.
    pub fn from_unix(secs: i64) -> Result<Self, InvalidValue> {
        if (Self::MIN_UNIX..=Self::MAX_UNIX).contains(&secs) {
            Ok(Self(unix_to_rfc3339(secs)))
        } else {
            Err(InvalidValue {
                kind: "timestamp",
                value: secs.to_string(),
                reason: "outside years 0000..=9999",
            })
        }
    }

    /// Seconds since the Unix epoch of `0000-01-01T00:00:00Z`, the earliest instant.
    pub const MIN_UNIX: i64 = -62_167_219_200;
    /// Seconds since the Unix epoch of `9999-12-31T23:59:59Z`, the latest instant.
    pub const MAX_UNIX: i64 = 253_402_300_799;

    /// Seconds since the Unix epoch.
    ///
    /// # Panics
    ///
    /// Never: the text is built by [`Timestamp::from_unix`], which only
    /// renders instants this parser reads back.
    #[must_use]
    #[expect(
        clippy::expect_used,
        reason = "invariant: the stored text is a normalized in-range RFC 3339 instant"
    )]
    pub fn unix(&self) -> i64 {
        rfc3339_to_unix(&self.0).expect("Timestamp text is always valid RFC 3339")
    }

    /// This instant moved back by `d` (whole seconds), stopping at
    /// `0000-01-01T00:00:00Z`.
    #[must_use]
    pub fn minus(&self, d: Duration) -> Self {
        let secs = self
            .unix()
            .saturating_sub(i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
            .max(Self::MIN_UNIX);
        Self(unix_to_rfc3339(secs))
    }

    /// The text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for Timestamp {
    type Error = InvalidValue;
    fn try_from(value: String) -> Result<Self, InvalidValue> {
        Self::parse(&value)
    }
}

impl From<Timestamp> for String {
    fn from(t: Timestamp) -> String {
        t.0
    }
}

/// A source of wall-clock and monotonic time.
///
/// Implementations must be cheap to call and thread safe. The cache, its store
/// and its backend share one `Arc<dyn Clock>`.
pub trait Clock: Send + Sync {
    /// Current UTC time, e.g. `2026-10-23T10:00:00Z`.
    fn now_utc(&self) -> Timestamp;
    /// Current monotonic instant.
    fn now_mono(&self) -> Instant;
}

/// The real clock: [`SystemTime`] for UTC and [`Instant`] for monotonic time.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SystemClock;

impl SystemClock {
    /// Creates the system clock.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Clock for SystemClock {
    fn now_utc(&self) -> Timestamp {
        // A system clock before 1970 is a host misconfiguration; report the epoch
        // rather than failing every caller.
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        // A clock past year 9999 is a host misconfiguration; report the last instant.
        Timestamp(unix_to_rfc3339(secs.min(Timestamp::MAX_UNIX)))
    }

    fn now_mono(&self) -> Instant {
        Instant::now()
    }
}

/// The instant a backend call must stop at.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use sc_beads::{Clock, Deadline, SystemClock};
///
/// let clock = SystemClock;
/// let d = Deadline::after(&clock, Duration::ZERO);
/// assert!(d.expired(&clock));
/// assert_eq!(d.remaining(&clock), Duration::ZERO);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Deadline(Instant);

impl Deadline {
    /// A deadline at the given monotonic instant.
    #[must_use]
    pub fn at(instant: Instant) -> Self {
        Self(instant)
    }

    /// A deadline `d` after the clock's current monotonic time.
    ///
    /// A duration too large to represent saturates to roughly 100 years ahead.
    #[must_use]
    pub fn after(clock: &dyn Clock, d: Duration) -> Self {
        let now = clock.now_mono();
        Self(
            now.checked_add(d)
                .or_else(|| now.checked_add(FAR_FUTURE))
                .unwrap_or(now),
        )
    }

    /// The earlier of the two deadlines.
    #[must_use]
    pub fn earliest(self, other: Deadline) -> Deadline {
        if other.0 < self.0 {
            other
        } else {
            self
        }
    }

    /// Time left before the deadline, zero once it has passed.
    #[must_use]
    pub fn remaining(&self, clock: &dyn Clock) -> Duration {
        self.0.saturating_duration_since(clock.now_mono())
    }

    /// True once the clock has reached the deadline.
    #[must_use]
    pub fn expired(&self, clock: &dyn Clock) -> bool {
        clock.now_mono() >= self.0
    }

    /// The deadline's monotonic instant.
    #[must_use]
    pub fn instant(&self) -> Instant {
        self.0
    }
}

/// Saturation point for [`Deadline::after`]: far enough that no real call reaches it.
const FAR_FUTURE: Duration = Duration::from_hours(100 * 365 * 24);

const SECS_PER_DAY: i64 = 86_400;

/// Formats Unix seconds as RFC 3339 UTC with second precision (`YYYY-MM-DDTHH:MM:SSZ`).
///
/// # Examples
///
/// ```
/// assert_eq!(sc_beads::unix_to_rfc3339(0), "1970-01-01T00:00:00Z");
/// assert_eq!(sc_beads::unix_to_rfc3339(1_792_749_600), "2026-10-23T10:00:00Z");
/// ```
#[must_use]
pub fn unix_to_rfc3339(secs: i64) -> String {
    let days = secs.div_euclid(SECS_PER_DAY);
    let rem = secs.rem_euclid(SECS_PER_DAY);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Parses an RFC 3339 date-time with a `Z` or `±HH:MM` offset into Unix seconds.
///
/// Fractional seconds are accepted and truncated. Returns `None` for anything
/// that is not a timezone-aware RFC 3339 date-time (the `date-time` format of
/// `bd schema`).
///
/// # Examples
///
/// ```
/// assert_eq!(sc_beads::rfc3339_to_unix("2026-10-23T10:00:00Z"), Some(1_792_749_600));
/// assert_eq!(sc_beads::rfc3339_to_unix("2026-10-23T12:00:00.5+02:00"), Some(1_792_749_600));
/// assert_eq!(sc_beads::rfc3339_to_unix("yesterday"), None);
/// assert_eq!(sc_beads::rfc3339_to_unix("2026-10-23T10:00:00"), None);
/// ```
#[must_use]
pub fn rfc3339_to_unix(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    let num = |from: usize, len: usize| -> Option<i64> {
        let part = b.get(from..from + len)?;
        if !part.iter().all(u8::is_ascii_digit) {
            return None;
        }
        part.iter()
            .try_fold(0i64, |acc, d| Some(acc * 10 + i64::from(d - b'0')))
    };
    let sep = |at: usize, allowed: &[u8]| b.get(at).is_some_and(|c| allowed.contains(c));
    if !(sep(4, b"-") && sep(7, b"-") && sep(10, b"Tt ") && sep(13, b":") && sep(16, b":")) {
        return None;
    }
    let (year, month, day) = (num(0, 4)?, num(5, 2)?, num(8, 2)?);
    let (hour, minute, second) = (num(11, 2)?, num(14, 2)?, num(17, 2)?);
    if !(1..=12).contains(&month)
        || day < 1
        || day > days_in_month(year, month)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let mut i = 19;
    if sep(i, b".") {
        i += 1;
        let start = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return None;
        }
    }
    let offset = match b.get(i) {
        Some(b'Z' | b'z') if i + 1 == b.len() => 0,
        Some(sign @ (b'+' | b'-')) if i + 6 == b.len() && sep(i + 3, b":") => {
            let (oh, om) = (num(i + 1, 2)?, num(i + 4, 2)?);
            if oh > 23 || om > 59 {
                return None;
            }
            let o = oh * 3600 + om * 60;
            if *sign == b'+' {
                o
            } else {
                -o
            }
        }
        _ => return None,
    };
    let days = days_from_civil(year, month, day);
    Some(days * SECS_PER_DAY + hour * 3600 + minute * 60 + second - offset)
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        2 if is_leap(y) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

// Howard Hinnant's days_from_civil / civil_from_days (proleptic Gregorian).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_across_leap_years_and_epoch() {
        for secs in [
            -86_400,
            0,
            951_782_400,
            1_709_164_800,
            1_792_749_600,
            4_102_444_799,
        ] {
            assert_eq!(
                rfc3339_to_unix(&unix_to_rfc3339(secs)),
                Some(secs),
                "{secs}"
            );
        }
        assert_eq!(unix_to_rfc3339(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn rejects_malformed_and_naive_times() {
        for bad in [
            "",
            "2026-13-01T00:00:00Z",
            "2026-02-30T00:00:00Z",
            "2026-10-23T24:00:00Z",
            "2026-10-23T10:00:00",
            "2026-10-23T10:00:00.Z",
            "2026-10-23T10:00:00+0200",
            "2026-10-23T10:00:00Zx",
            "2026-10-23",
        ] {
            assert_eq!(rfc3339_to_unix(bad), None, "{bad}");
        }
    }

    #[test]
    fn deadline_earliest_and_remaining() {
        let clock = SystemClock;
        let near = Deadline::after(&clock, Duration::from_millis(10));
        let far = Deadline::after(&clock, Duration::from_secs(60));
        assert_eq!(near.earliest(far), near);
        assert_eq!(far.earliest(near), near);
        assert!(far.remaining(&clock) > Duration::from_secs(59));
        assert!(!far.expired(&clock));
        let huge = Deadline::after(&clock, Duration::MAX);
        assert!(!huge.expired(&clock));
    }

    #[test]
    fn timestamps_are_bounded_to_four_digit_years() {
        let first = Timestamp::parse("0000-01-01T00:00:00Z").unwrap();
        assert_eq!(first.unix(), Timestamp::MIN_UNIX);
        let last = Timestamp::parse("9999-12-31T23:59:59Z").unwrap();
        assert_eq!(last.unix(), Timestamp::MAX_UNIX);
        // Valid text that normalizes outside the range is refused.
        assert!(Timestamp::parse("0000-01-01T00:00:00+00:01").is_err());
        assert!(Timestamp::parse("9999-12-31T23:59:59-00:01").is_err());
        assert!(Timestamp::from_unix(Timestamp::MIN_UNIX - 1).is_err());
        assert!(Timestamp::from_unix(Timestamp::MAX_UNIX + 1).is_err());
        assert_eq!(
            Timestamp::from_unix(0).unwrap().as_str(),
            "1970-01-01T00:00:00Z"
        );
        assert_eq!(
            first.minus(Duration::from_secs(1)),
            first,
            "minus stops at the first instant"
        );
        assert!(SystemClock.now_utc() < last);
    }
}
