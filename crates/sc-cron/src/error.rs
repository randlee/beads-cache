//! The crate's one error type, with stable codes and preserved causes.
//!
//! A caller needs to know whether its request was wrong (a bad expression
//! or spec, or a name already running), whether storage failed and whether
//! retrying can help, or whether it acted on a stale generation.
//! [`CronError::code`] gives a stable string for each case, and
//! [`CronError::recovery`] says what to do. A store failure keeps its
//! underlying error as [`std::error::Error::source`] and is classified by
//! [`StoreErrorKind`], so a busy database can be told from a corrupt one
//! (ADR-CRON-0001).

use std::error::Error;
use std::fmt;
use std::sync::Arc;

use crate::{Generation, ScheduleId, ScheduleName};

/// Every failure the scheduler and its stores report.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum CronError {
    /// The cron expression does not parse or has no future occurrence.
    BadExpression(String),
    /// A schedule field other than the expression is invalid or over a limit.
    InvalidSpec(String),
    /// A schedule with this name is already active.
    AlreadyActive(ScheduleName),
    /// The schedule store failed.
    Store(StoreError),
    /// `(id, generation)` no longer names the active schedule.
    Fenced {
        /// The schedule the caller named.
        id: ScheduleId,
        /// The stale generation the caller named.
        generation: Generation,
    },
}

impl CronError {
    /// Returns the stable machine-readable code for this error.
    ///
    /// One of `BAD_EXPRESSION`, `INVALID_SPEC`, `ALREADY_ACTIVE`,
    /// `STORE_FAILED`, `FENCED`.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::BadExpression(_) => "BAD_EXPRESSION",
            Self::InvalidSpec(_) => "INVALID_SPEC",
            Self::AlreadyActive(_) => "ALREADY_ACTIVE",
            Self::Store(_) => "STORE_FAILED",
            Self::Fenced { .. } => "FENCED",
        }
    }

    /// Returns what a caller can do about this error.
    #[must_use]
    pub const fn recovery(&self) -> &'static str {
        match self {
            Self::BadExpression(_) => {
                "use 6 or 7 fields (seconds first) or @hourly..@yearly, with a future occurrence"
            }
            Self::InvalidSpec(_) => "correct the named field; see the limits in sc_cron's docs",
            Self::AlreadyActive(_) => "stop the active schedule first, or choose another name",
            Self::Store(error) => error.kind().recovery(),
            Self::Fenced { .. } => "re-read the schedule and act on its current generation",
        }
    }
}

impl fmt::Display for CronError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadExpression(message) => write!(f, "invalid cron expression: {message}"),
            Self::InvalidSpec(message) => write!(f, "invalid schedule: {message}"),
            Self::AlreadyActive(name) => write!(f, "schedule {name} is already active"),
            Self::Store(error) => write!(f, "schedule store failed: {error}"),
            Self::Fenced { id, generation } => write!(
                f,
                "schedule {id} generation {generation} is no longer the active schedule"
            ),
        }
    }
}

impl Error for CronError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            _ => None,
        }
    }
}

impl From<StoreError> for CronError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

/// What kind of store failure happened, so a caller can decide to retry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StoreErrorKind {
    /// Another connection holds the lock; retrying later can succeed.
    Busy,
    /// Stored data is malformed or the file is not a database.
    Corrupt,
    /// The file holds an unknown schema version, or tables that are not sc-cron's.
    SchemaMismatch,
    /// The file cannot be opened, read or written (I/O, permissions, full disk).
    Unavailable,
    /// Any other failure.
    Other,
}

impl StoreErrorKind {
    /// Returns the stable lower-case name of this kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Busy => "busy",
            Self::Corrupt => "corrupt",
            Self::SchemaMismatch => "schema_mismatch",
            Self::Unavailable => "unavailable",
            Self::Other => "other",
        }
    }

    /// Returns what a caller can do about this kind of failure.
    #[must_use]
    pub const fn recovery(self) -> &'static str {
        match self {
            Self::Busy => "retry later; another connection holds the database lock",
            Self::Corrupt => "repair or remove the named row, or restore the database file",
            Self::SchemaMismatch => "open the schedule store on its own sc-cron database file",
            Self::Unavailable => "check the database path, permissions and free disk space",
            Self::Other => "see the source error",
        }
    }
}

impl fmt::Display for StoreErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A classified store failure with its context and underlying cause.
///
/// Store implementations outside this crate build one with
/// [`StoreError::new`] and [`StoreError::with_source`].
#[derive(Clone, Debug)]
pub struct StoreError {
    kind: StoreErrorKind,
    context: String,
    source: Option<Arc<dyn Error + Send + Sync>>,
}

impl StoreError {
    /// Creates a failure of `kind`; `context` names the operation and row.
    #[must_use]
    pub fn new(kind: StoreErrorKind, context: impl Into<String>) -> Self {
        Self {
            kind,
            context: context.into(),
            source: None,
        }
    }

    /// Attaches the underlying error, reported by [`Error::source`].
    #[must_use]
    pub fn with_source(mut self, source: impl Error + Send + Sync + 'static) -> Self {
        self.source = Some(Arc::new(source));
        self
    }

    /// Returns the failure kind.
    #[must_use]
    pub const fn kind(&self) -> StoreErrorKind {
        self.kind
    }

    /// Returns the operation and row the failure concerns.
    #[must_use]
    pub fn context(&self) -> &str {
        &self.context
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.context, self.kind)?;
        if let Some(source) = &self.source {
            write!(f, ": {source}")?;
        }
        Ok(())
    }
}

impl Error for StoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
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

    #[test]
    fn codes_are_stable_and_every_error_has_recovery_text() {
        let name = ScheduleName::new("sync").expect("name");
        let cases = [
            (CronError::BadExpression(String::new()), "BAD_EXPRESSION"),
            (CronError::InvalidSpec(String::new()), "INVALID_SPEC"),
            (CronError::AlreadyActive(name), "ALREADY_ACTIVE"),
            (
                CronError::Store(StoreError::new(StoreErrorKind::Busy, "x")),
                "STORE_FAILED",
            ),
            (
                CronError::Fenced {
                    id: ScheduleId::new(1),
                    generation: Generation::FIRST,
                },
                "FENCED",
            ),
        ];
        for (error, code) in cases {
            assert_eq!(error.code(), code);
            assert!(!error.recovery().is_empty());
        }
    }

    #[test]
    fn store_errors_keep_their_source_and_kind() {
        let io = std::io::Error::other("disk on fire");
        let error = CronError::from(
            StoreError::new(StoreErrorKind::Unavailable, "failed to open").with_source(io),
        );
        let store = error.source().expect("store error is the source");
        assert_eq!(
            store.source().map(ToString::to_string).as_deref(),
            Some("disk on fire")
        );
        assert_eq!(
            error.to_string(),
            "schedule store failed: failed to open (unavailable): disk on fire"
        );
        assert_eq!(error.recovery(), StoreErrorKind::Unavailable.recovery());
    }

    #[test]
    fn fenced_names_the_id_and_generation() {
        let error = CronError::Fenced {
            id: ScheduleId::new(7),
            generation: Generation::FIRST,
        };
        assert_eq!(
            error.to_string(),
            "schedule 7 generation 1 is no longer the active schedule"
        );
    }
}
