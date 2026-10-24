//! Cron expression parsing and next-run computation.
//!
//! The grammar is the `cron` crate's, in UTC: six or seven fields with
//! seconds first and year optional, or `@yearly`, `@monthly`, `@weekly`,
//! `@daily`, `@hourly`. A five-field expression is refused rather than read
//! with a shifted meaning, and so is an expression with no future occurrence.

use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::CronError;

/// Parses `expr` and requires an occurrence after `now`.
pub(crate) fn parse(expr: &str, now: DateTime<Utc>) -> Result<cron::Schedule, CronError> {
    let schedule = parse_syntax(expr)?;
    if schedule.after(&now).next().is_none() {
        return Err(CronError::BadExpression(
            "expression has no future occurrence".into(),
        ));
    }
    Ok(schedule)
}

/// Parses `expr` without the future-occurrence check, for stored schedules.
pub(crate) fn parse_syntax(expr: &str) -> Result<cron::Schedule, CronError> {
    let trimmed = expr.trim();
    if !trimmed.starts_with('@') && trimmed.split_whitespace().count() == 5 {
        return Err(CronError::BadExpression(
            "five-field expressions are not accepted; use 6 or 7 fields, seconds first, \
             or @hourly..@yearly"
                .into(),
        ));
    }
    cron::Schedule::from_str(trimmed).map_err(|error| CronError::BadExpression(error.to_string()))
}

/// The first jittered run strictly after `after`.
///
/// This is `occurrence + jitter` for the first occurrence after
/// `after - jitter`, so a run whose jitter window has opened but not closed
/// is still found: a restart inside that window still fires it.
pub(crate) fn next_run(
    schedule: &cron::Schedule,
    jitter: Duration,
    after: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    let jitter = chrono::Duration::from_std(jitter).ok()?;
    let base = after.checked_sub_signed(jitter)?;
    schedule
        .after(&base)
        .next()
        .and_then(|occurrence| occurrence.checked_add_signed(jitter))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> DateTime<Utc> {
        text.parse().expect("timestamp")
    }

    #[test]
    fn next_run_adds_the_jitter_to_the_occurrence() {
        let schedule = parse_syntax("@hourly").expect("expression");
        let jitter = Duration::from_secs(600);
        assert_eq!(
            next_run(&schedule, jitter, at("2026-10-19T00:30:00Z")),
            Some(at("2026-10-19T01:10:00Z"))
        );
    }

    #[test]
    fn next_run_finds_an_open_jitter_window() {
        let schedule = parse_syntax("@hourly").expect("expression");
        let jitter = Duration::from_secs(600);
        assert_eq!(
            next_run(&schedule, jitter, at("2026-10-19T10:00:30Z")),
            Some(at("2026-10-19T10:10:00Z"))
        );
    }

    #[test]
    fn next_run_is_strictly_after() {
        let schedule = parse_syntax("0 * * * * *").expect("expression");
        assert_eq!(
            next_run(&schedule, Duration::ZERO, at("2026-10-19T00:01:00Z")),
            Some(at("2026-10-19T00:02:00Z"))
        );
    }

    #[test]
    fn next_run_is_none_after_the_last_occurrence() {
        let schedule = parse_syntax("0 0 0 20 10 * 2026").expect("expression");
        assert_eq!(
            next_run(&schedule, Duration::ZERO, at("2026-10-20T00:00:00Z")),
            None
        );
    }
}
