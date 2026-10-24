//! The text form of every timestamp the scheduler hands to a store.
//!
//! Stores read no clock, so every instant crosses the [`ScheduleStore`]
//! boundary as an argument. The scheduler writes RFC 3339 UTC with a fixed
//! six-digit fraction (`2026-10-19T00:00:00.000000Z`), so the text sorts in
//! time order; stores keep the text as given.
//!
//! [`ScheduleStore`]: crate::ScheduleStore

use chrono::{DateTime, SecondsFormat, Utc};

/// Formats `at` in the fixed-width form stores keep.
pub(crate) fn format(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(SecondsFormat::Micros, true)
}

/// Parses an RFC 3339 timestamp into UTC.
pub(crate) fn parse(text: &str) -> Result<DateTime<Utc>, chrono::ParseError> {
    DateTime::parse_from_rfc3339(text).map(|at| at.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_is_fixed_width_and_round_trips() {
        let at = parse("2026-10-19T01:02:03Z").expect("valid timestamp");
        let text = format(at);
        assert_eq!(text, "2026-10-19T01:02:03.000000Z");
        assert_eq!(parse(&text).expect("round trip"), at);
    }

    #[test]
    fn parse_normalizes_offsets_to_utc() {
        let at = parse("2026-10-19T03:00:00+02:00").expect("valid timestamp");
        assert_eq!(format(at), "2026-10-19T01:00:00.000000Z");
    }
}
