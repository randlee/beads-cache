//! [`CronExpr`]: a parsed cron expression, and next-run computation.
//!
//! The grammar is the `cron` crate's, in UTC: six or seven fields with
//! seconds first and year optional, or `@yearly`, `@monthly`, `@weekly`,
//! `@daily`, `@hourly`. A standard five-field crontab expression (minute
//! first) is also accepted (REQ-CRON-0015). It is read with crontab's
//! meaning, not the `cron` crate's: the day-of-week field counts Sunday as 0
//! (or 7) and Monday as 1, where the `cron` crate counts Sunday as 1, and a
//! day matches when the day-of-month or the day-of-week field matches if both
//! are restricted. [`CronExpr::parse`] checks syntax once; whether an
//! expression still has a future occurrence depends on the time, so
//! [`Scheduler::start`](crate::Scheduler::start) checks that.

use std::borrow::Cow;
use std::fmt;
use std::iter::Peekable;
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
/// assert!(CronExpr::parse("*/5 * * * *").is_ok()); // five fields, minute first
/// assert!(CronExpr::parse("*/5 * * *").is_err()); // four fields
/// # Ok::<(), sc_cron::CronError>(())
/// ```
#[derive(Clone)]
pub struct CronExpr {
    text: String,
    /// The expression's occurrences are the union of these `cron` schedules:
    /// one, or two for a five-field expression that restricts both the day of
    /// month and the day of week. Crontab matches a day when either field
    /// does, where the `cron` crate requires both.
    schedules: Vec<cron::Schedule>,
}

/// Occurrences of the union of a `CronExpr`'s schedules after an instant, in
/// time order, each once.
struct Occurrences<'a> {
    sources: Vec<Peekable<cron::ScheduleIterator<'a, Utc>>>,
}

impl Iterator for Occurrences<'_> {
    type Item = DateTime<Utc>;

    fn next(&mut self) -> Option<Self::Item> {
        let earliest = self
            .sources
            .iter_mut()
            .filter_map(|source| source.peek().copied())
            .min()?;
        // Every source due at `earliest` moves past it, so it is yielded once.
        for source in &mut self.sources {
            if source.peek() == Some(&earliest) {
                source.next();
            }
        }
        Some(earliest)
    }
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
        let bad = |error: &dyn fmt::Display| CronError::BadExpression(error.to_string());
        let schedules = match crontab_fields(trimmed) {
            Some(fields) => crontab_schedules(&fields).map_err(CronError::BadExpression)?,
            None => vec![cron::Schedule::from_str(trimmed).map_err(|e| bad(&e))?],
        };
        Ok(Self {
            text: text.to_owned(),
            schedules,
        })
    }

    /// Returns the expression exactly as given.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The occurrences strictly after `after`, in time order.
    pub(crate) fn occurrences_after(
        &self,
        after: &DateTime<Utc>,
    ) -> impl Iterator<Item = DateTime<Utc>> {
        Occurrences {
            sources: self
                .schedules
                .iter()
                .map(|schedule| schedule.after(after).peekable())
                .collect(),
        }
    }

    /// Refuses an expression with no occurrence after `now`.
    pub(crate) fn require_future(&self, now: DateTime<Utc>) -> Result<(), CronError> {
        if self.occurrences_after(&now).next().is_none() {
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
        self.occurrences_after(&base)
            .next()
            .and_then(|occurrence| occurrence.checked_add_signed(jitter))
    }
}

/// Returns the five fields of a standard crontab expression, or `None` for
/// any other form (six or seven fields, or an `@` macro), which the `cron`
/// crate reads as it is.
fn crontab_fields(trimmed: &str) -> Option<[&str; 5]> {
    if trimmed.starts_with('@') {
        return None;
    }
    let mut fields = trimmed.split_whitespace();
    let found = [
        fields.next()?,
        fields.next()?,
        fields.next()?,
        fields.next()?,
        fields.next()?,
    ];
    fields.next().is_none().then_some(found)
}

/// Builds the schedules for a five-field expression with crontab's meaning.
fn crontab_schedules(fields: &[&str; 5]) -> Result<Vec<cron::Schedule>, String> {
    let [minute, hour, day_of_month, month, day_of_week] = *fields;
    let schedule = |day: &str, weekday: &str| {
        // The `cron` crate wants seconds first; crontab fires at second 0.
        let text = format!("0 {minute} {hour} {day} {month} {weekday}");
        cron::Schedule::from_str(&text).map_err(|error| error.to_string())
    };
    let weekdays = crontab_weekdays(day_of_week)?;
    let open = |field: &str| field.starts_with('*') || field == "?";
    if open(day_of_month) || open(day_of_week) {
        Ok(vec![schedule(day_of_month, &weekdays)?])
    } else {
        Ok(vec![
            schedule(day_of_month, "*")?,
            schedule("*", &weekdays)?,
        ])
    }
}

/// Day names in crontab order: 0 and 7 are Sunday, 1 is Monday.
const WEEKDAYS: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];

/// Rewrites a crontab day-of-week field as a list of day names.
///
/// Names are independent of the `cron` crate's numbering (it counts Sunday
/// as 1), so numbers are translated here, and a range or step is expanded to
/// the days it covers. An open field (`*` or `?`) is returned as it is.
fn crontab_weekdays(field: &str) -> Result<Cow<'_, str>, String> {
    if field == "*" || field == "?" {
        return Ok(Cow::Borrowed(field));
    }
    let bad = |why: &str| format!("day-of-week {field:?}: {why}; use six fields for other forms");
    let value = |text: &str| -> Result<usize, String> {
        let upper = text.to_ascii_uppercase();
        match WEEKDAYS.iter().position(|name| *name == upper) {
            Some(day) => Ok(day),
            None => text
                .parse::<usize>()
                .ok()
                .filter(|day| *day <= 7)
                .ok_or_else(|| bad("expected 0-7 or SUN-SAT")),
        }
    };
    let mut days = [false; 7];
    for part in field.split(',') {
        let (range, step) = match part.split_once('/') {
            Some((range, step)) => {
                let step = step
                    .parse::<usize>()
                    .ok()
                    .filter(|step| *step > 0)
                    .ok_or_else(|| bad("the step must be a positive number"))?;
                (range, step)
            }
            None => (part, 1),
        };
        let (from, to) = match range.split_once('-') {
            Some((from, to)) => (value(from)?, value(to)?),
            None if range == "*" => (0, 6),
            None if step > 1 => return Err(bad("a step needs a range or *")),
            None => {
                let day = value(range)?;
                (day, day)
            }
        };
        if from > to {
            return Err(bad("a range must not run backwards"));
        }
        for day in (from..=to).step_by(step) {
            if let Some(seen) = days.get_mut(day % 7) {
                *seen = true;
            }
        }
    }
    let names: Vec<&str> = WEEKDAYS
        .iter()
        .zip(days)
        .filter_map(|(name, seen)| seen.then_some(*name))
        .collect();
    Ok(Cow::Owned(names.join(",")))
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
    fn crontab_weekdays_become_names_in_crontab_numbering() {
        let names = |field| crontab_weekdays(field).expect("field").into_owned();
        assert_eq!(names("*"), "*");
        assert_eq!(names("?"), "?");
        assert_eq!(names("1-5"), "MON,TUE,WED,THU,FRI");
        assert_eq!(names("0"), "SUN");
        assert_eq!(names("7"), "SUN");
        assert_eq!(names("0,7"), "SUN");
        assert_eq!(names("5-7"), "SUN,FRI,SAT");
        assert_eq!(names("*/2"), "SUN,TUE,THU,SAT");
        assert_eq!(names("1-6/2"), "MON,WED,FRI");
        assert_eq!(names("sat,Mon"), "MON,SAT");
        assert_eq!(names("tue-thu"), "TUE,WED,THU");
    }

    #[test]
    fn restricted_day_of_month_and_week_are_the_union_without_repeats() {
        // The 13th or a Friday; 2026-11-13 is both.
        let either = expr("0 0 13 * 5");
        let after = at("2026-11-12T12:00:00Z");
        let next: Vec<_> = either.occurrences_after(&after).take(3).collect();
        assert_eq!(
            next,
            [
                at("2026-11-13T00:00:00Z"),
                at("2026-11-20T00:00:00Z"),
                at("2026-11-27T00:00:00Z"),
            ]
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
