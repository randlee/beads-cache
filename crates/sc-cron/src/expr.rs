//! [`CronExpr`]: a parsed cron expression, and next-run computation.
//!
//! The grammar is the `cron` crate's, in UTC: six or seven fields with
//! seconds first and year optional, or `@yearly`, `@monthly`, `@weekly`,
//! `@daily`, `@hourly`. A five-field expression is refused rather than read
//! with a shifted meaning. [`CronExpr::parse`] checks syntax once; whether
//! an expression still has a future occurrence depends on the time, so
//! [`Scheduler::start`](crate::Scheduler::start) checks that.

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::CronError;
use crate::limits::MAX_EXPR_LEN;

/// A syntactically valid cron expression, kept with its original text.
///
/// Two expressions are equal when their text is equal.
///
/// # Examples
///
/// ```
/// use sc_cron::CronExpr;
///
/// let expr = CronExpr::parse("0 */5 * * * *")?;
/// assert_eq!(expr.as_str(), "0 */5 * * * *");
/// assert!(CronExpr::parse("*/5 * * * *").is_err()); // five fields
/// # Ok::<(), sc_cron::CronError>(())
/// ```
#[derive(Clone)]
pub struct CronExpr {
    text: String,
    schedule: cron::Schedule,
}

impl CronExpr {
    /// Parses `text` (surrounding whitespace is ignored but kept).
    ///
    /// # Errors
    ///
    /// [`CronError::InvalidSpec`] if `text` is longer than
    /// [`MAX_EXPR_LEN`](crate::MAX_EXPR_LEN) bytes;
    /// [`CronError::BadExpression`] if it has five fields or does not parse.
    pub fn parse(text: &str) -> Result<Self, CronError> {
        if text.len() > MAX_EXPR_LEN {
            return Err(CronError::InvalidSpec(format!(
                "expression is longer than {MAX_EXPR_LEN} bytes"
            )));
        }
        let trimmed = text.trim();
        if !trimmed.starts_with('@') && trimmed.split_whitespace().count() == 5 {
            return Err(CronError::BadExpression(
                "five-field expressions are not accepted; use 6 or 7 fields, seconds first, \
                 or @hourly..@yearly"
                    .into(),
            ));
        }
        let schedule = cron::Schedule::from_str(trimmed)
            .map_err(|error| CronError::BadExpression(error.to_string()))?;
        Ok(Self {
            text: text.to_owned(),
            schedule,
        })
    }

    /// Returns the expression exactly as given.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub(crate) fn schedule(&self) -> &cron::Schedule {
        &self.schedule
    }

    /// Refuses an expression with no occurrence after `now`.
    pub(crate) fn require_future(&self, now: DateTime<Utc>) -> Result<(), CronError> {
        if self.schedule.after(&now).next().is_none() {
            return Err(CronError::BadExpression(
                "expression has no future occurrence".into(),
            ));
        }
        Ok(())
    }

    /// The first jittered run strictly after `after`.
    ///
    /// This is `occurrence + jitter` for the first occurrence after
    /// `after - jitter`, so a run whose jitter window has opened but not
    /// closed is still found: a restart inside that window still fires it.
    pub(crate) fn next_run(&self, jitter: Duration, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let jitter = chrono::Duration::from_std(jitter).ok()?;
        let base = after.checked_sub_signed(jitter)?;
        self.schedule
            .after(&base)
            .next()
            .and_then(|occurrence| occurrence.checked_add_signed(jitter))
    }
}

impl FromStr for CronExpr {
    type Err = CronError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

impl PartialEq for CronExpr {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl Eq for CronExpr {}

impl fmt::Debug for CronExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("CronExpr").field(&self.text).finish()
    }
}

impl fmt::Display for CronExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::clone_on_ref_ptr,
    clippy::unused_self,
    clippy::duration_suboptimal_units,
    clippy::unchecked_time_subtraction,
    reason = "test code: a failed expectation is the test failure"
)]
mod tests {
    use super::*;

    fn at(text: &str) -> DateTime<Utc> {
        text.parse().expect("timestamp")
    }

    fn expr(text: &str) -> CronExpr {
        CronExpr::parse(text).expect("expression")
    }

    #[test]
    fn next_run_adds_the_jitter_to_the_occurrence() {
        assert_eq!(
            expr("@hourly").next_run(Duration::from_secs(600), at("2026-10-19T00:30:00Z")),
            Some(at("2026-10-19T01:10:00Z"))
        );
    }

    #[test]
    fn next_run_finds_an_open_jitter_window() {
        assert_eq!(
            expr("@hourly").next_run(Duration::from_secs(600), at("2026-10-19T10:00:30Z")),
            Some(at("2026-10-19T10:10:00Z"))
        );
    }

    #[test]
    fn next_run_is_strictly_after() {
        assert_eq!(
            expr("0 * * * * *").next_run(Duration::ZERO, at("2026-10-19T00:01:00Z")),
            Some(at("2026-10-19T00:02:00Z"))
        );
    }

    #[test]
    fn next_run_is_none_after_the_last_occurrence() {
        assert_eq!(
            expr("0 0 0 20 10 * 2026").next_run(Duration::ZERO, at("2026-10-20T00:00:00Z")),
            None
        );
    }

    #[test]
    fn an_over_long_expression_is_invalid_spec() {
        let long = format!("0 * * * * *{}", " ".repeat(MAX_EXPR_LEN));
        assert!(matches!(
            CronExpr::parse(&long),
            Err(CronError::InvalidSpec(_))
        ));
    }
}
