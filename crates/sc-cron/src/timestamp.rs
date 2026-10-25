//! [`Timestamp`]: the validated instant that crosses the store port.
//!
//! Stores read no clock, so every instant crosses the
//! [`ScheduleStore`](crate::ScheduleStore) boundary as an argument. A
//! `Timestamp` is a UTC instant at microsecond precision. It is valid by
//! construction, so no layer re-validates it. Text exists only at a storage
//! boundary: [`Display`](std::fmt::Display) writes RFC 3339 with a fixed
//! six-digit fraction (`2026-10-19T00:00:00.000000Z`), which sorts in time
//! order, and [`FromStr`] reads any RFC 3339 form back.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, SecondsFormat, Timelike, Utc};

use crate::CronError;

/// A UTC instant at microsecond precision.
///
/// # Examples
///
/// ```
/// use sc_cron::Timestamp;
///
/// let at: Timestamp = "2026-10-19T03:00:00+02:00".parse()?;
/// assert_eq!(at.to_string(), "2026-10-19T01:00:00.000000Z");
/// # Ok::<(), sc_cron::CronError>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(DateTime<Utc>);

impl Timestamp {
    /// Wraps `at`, dropping any precision finer than a microsecond.
    ///
    /// Truncating here means a value stored and read back compares equal.
    #[must_use]
    pub fn from_datetime(at: DateTime<Utc>) -> Self {
        let nanos = at.nanosecond();
        Self(at.with_nanosecond(nanos - nanos % 1_000).unwrap_or(at))
    }

    /// Returns the instant.
    #[must_use]
    pub const fn as_datetime(self) -> DateTime<Utc> {
        self.0
    }
}

impl From<DateTime<Utc>> for Timestamp {
    fn from(at: DateTime<Utc>) -> Self {
        Self::from_datetime(at)
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.to_rfc3339_opts(SecondsFormat::Micros, true))
    }
}

impl FromStr for Timestamp {
    type Err = CronError;

    /// Parses RFC 3339 in any offset, normalized to UTC.
    ///
    /// # Errors
    ///
    /// [`CronError::InvalidSpec`] if `text` is not RFC 3339.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        DateTime::parse_from_rfc3339(text)
            .map(|at| Self::from_datetime(at.with_timezone(&Utc)))
            .map_err(|error| {
                CronError::InvalidSpec(format!("timestamp {text:?} is not RFC 3339: {error}"))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_is_fixed_width_and_round_trips() {
        let at: Timestamp = "2026-10-19T01:02:03Z".parse().expect("valid timestamp");
        let text = at.to_string();
        assert_eq!(text, "2026-10-19T01:02:03.000000Z");
        assert_eq!(text.parse::<Timestamp>().expect("round trip"), at);
    }

    #[test]
    fn precision_below_a_microsecond_is_dropped() {
        let at: Timestamp = "2026-10-19T01:02:03.123456789Z".parse().expect("timestamp");
        assert_eq!(at.to_string(), "2026-10-19T01:02:03.123456Z");
        assert_eq!(at.to_string().parse::<Timestamp>().expect("round trip"), at);
    }

    #[test]
    fn garbage_is_invalid_spec() {
        assert!(matches!(
            "yesterday".parse::<Timestamp>(),
            Err(CronError::InvalidSpec(_))
        ));
    }
}
