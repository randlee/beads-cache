//! `x_cache` and the shared cache facts: [`OpError`], [`OutageData`], row facts (contract §5.1).
//!
//! In `x_cache` every member is always present, and `null` means unknown
//! (contract §1 rule 3). Timestamps the cache produces are UTC
//! `YYYY-MM-DDTHH:MM:SS.sssZ` ([`cache_time`]).

use std::collections::BTreeMap;

use bd_cache_model::{CacheError, Outage, ReadyBasis, RowView, ScopeId};
use sc_beads::{BeadId, Timestamp};
use serde::Serialize;

use crate::error::CODE_PREFIX;

/// A cache-produced timestamp in the contract's form, `YYYY-MM-DDTHH:MM:SS.sssZ`.
///
/// # Examples
///
/// ```
/// let t = sc_beads::Timestamp::parse("2026-10-23T12:00:00+02:00").unwrap();
/// assert_eq!(bd_cache_api::data::cache_time(&t), "2026-10-23T10:00:00.000Z");
/// ```
#[must_use]
pub fn cache_time(t: &Timestamp) -> String {
    let s = t.as_str();
    match s.strip_suffix('Z') {
        Some(head) if !head.contains('.') => format!("{head}.000Z"),
        _ => s.to_owned(),
    }
}

pub(crate) fn opt_time(t: Option<&Timestamp>) -> Option<String> {
    t.map(cache_time)
}

/// A typed error inside `data`: a stored failure, an escalation reason, a
/// sync failure in a result list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpError {
    /// The `BD_CACHE.*` code.
    pub code: String,
    /// What happened.
    pub message: String,
}

impl OpError {
    /// Decodes stored text in either writer's form, `CODE: detail`.
    ///
    /// Text with no `BD_CACHE.*` prefix keeps its whole text as the message,
    /// under `BD_CACHE.INTERNAL`: its code is unknown, never invented.
    ///
    /// # Examples
    ///
    /// ```
    /// use bd_cache_api::data::OpError;
    /// let e = OpError::from_stored("BD_CACHE.BD_UNAVAILABLE: bd timed out");
    /// assert_eq!((e.code.as_str(), e.message.as_str()), ("BD_CACHE.BD_UNAVAILABLE", "bd timed out"));
    /// ```
    #[must_use]
    pub fn from_stored(text: &str) -> Self {
        match text.split_once(": ") {
            Some((code, message)) if is_code(code) => Self {
                code: code.to_owned(),
                message: message.to_owned(),
            },
            _ => Self {
                code: format!("{CODE_PREFIX}INTERNAL"),
                message: text.to_owned(),
            },
        }
    }

    /// Stored text with its code kept apart (b-1's `last_error_code`).
    pub(crate) fn from_parts(code: Option<&str>, text: &str) -> Self {
        match code {
            Some(code) => Self {
                code: code.to_owned(),
                message: text
                    .strip_prefix(code)
                    .and_then(|t| t.strip_prefix(": "))
                    .unwrap_or(text)
                    .to_owned(),
            },
            None => Self::from_stored(text),
        }
    }

    /// The error of a failed core call.
    #[must_use]
    pub fn of(e: &CacheError) -> Self {
        Self {
            code: e.code().to_owned(),
            message: e.detail().to_owned(),
        }
    }
}

/// True for `BD_CACHE.<UPPER_SNAKE>`.
fn is_code(s: &str) -> bool {
    s.strip_prefix(CODE_PREFIX).is_some_and(|rest| {
        !rest.is_empty()
            && rest
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    })
}

/// The backend outage (ADR-BDC-0005): `{since, last_error}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OutageData {
    /// When it started.
    pub since: String,
    /// The latest failure.
    pub last_error: OpError,
}

impl OutageData {
    /// The api form of the stored outage.
    #[must_use]
    pub fn of(o: &Outage) -> Self {
        Self {
            since: cache_time(&o.since),
            last_error: OpError::from_stored(&o.last_error),
        }
    }
}

pub(crate) fn outage(o: Option<&Outage>) -> Option<OutageData> {
    o.map(OutageData::of)
}

/// What the cache knows of one row (`x_cache.rows[id]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RowFacts {
    /// A read-back failed; the next sync re-reads it.
    pub stale: bool,
    /// The blockers: `null` unknown, `[]` not blocked.
    pub blocked_by: Option<Vec<BeadId>>,
}

/// `x_cache.rows`, keyed by bead id.
pub type Rows = BTreeMap<String, RowFacts>;

pub(crate) fn rows<'a>(views: impl IntoIterator<Item = &'a RowView>) -> Rows {
    views
        .into_iter()
        .map(|r| {
            (
                r.bead.id.as_str().to_owned(),
                RowFacts {
                    stale: r.stale,
                    blocked_by: r.blocked.clone(),
                },
            )
        })
        .collect()
}

/// `x_cache` on reads: `{scope, last_ok_at, outage, rows}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadXCache {
    /// The scope read, by its name as written (ADR-BDC-0013); `null` when the bead has none.
    pub scope: Option<String>,
    /// The scope's last successful sync.
    pub last_ok_at: Option<String>,
    /// The outage, while one holds.
    pub outage: Option<OutageData>,
    /// Facts about each row returned.
    pub rows: Rows,
}

impl ReadXCache {
    pub(crate) fn new(
        scope: Option<&ScopeId>,
        last_ok_at: Option<&Timestamp>,
        outage_now: Option<&Outage>,
        views: &[RowView],
    ) -> Self {
        Self {
            scope: scope.map(|s| s.name().to_owned()),
            last_ok_at: opt_time(last_ok_at),
            outage: outage(outage_now),
            rows: rows(views),
        }
    }
}

/// `x_cache` of `issues get`: the read facts plus `pending`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GetXCache {
    /// The read facts.
    #[serde(flatten)]
    pub read: ReadXCache,
    /// The bead's own journal entries not yet applied.
    pub pending: u32,
}

/// What a `ready` answer is based on: `{kind, fetched_at?, stale?}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Basis {
    /// bd answered within `fresh_within_ms`.
    Fresh {
        /// When bd's answer was fetched.
        fetched_at: String,
    },
    /// The stored base.
    Cached {
        /// When the base was fetched.
        fetched_at: String,
        /// A later refresh failed.
        stale: bool,
    },
    /// No base yet.
    Unknown,
}

impl Basis {
    pub(crate) fn of(b: &ReadyBasis) -> Self {
        match b {
            ReadyBasis::Fresh { fetched_at } => Self::Fresh {
                fetched_at: cache_time(fetched_at),
            },
            ReadyBasis::Cached { fetched_at, stale } => Self::Cached {
                fetched_at: cache_time(fetched_at),
                stale: *stale,
            },
            ReadyBasis::Unknown => Self::Unknown,
        }
    }
}

/// Why a requested fresh `ready` fell back to the cached answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadFallback {
    /// `fresh_within_ms` ran out.
    TimedOut,
    /// bd could not answer at all.
    Unavailable,
    /// bd answered with an error.
    Failed,
}

/// The core's cut text: a fresh read the caller's limit ran out on.
const CUT_MARK: &str = "no answer within";

impl ReadFallback {
    /// Classifies the core's `read_fallback` text, `CODE: detail; hint`, by
    /// whether its error was an outage (`outage`, the core's
    /// [`CacheError::outage`] of it, REQ-BDC-0729): an answered
    /// `BD_LIST_FAILED` is `failed`, next to `details.outage: false`.
    pub(crate) fn of(text: &str, outage: bool) -> (Self, OpError) {
        let error = OpError::from_stored(text);
        let kind = if outage && error.message.contains(CUT_MARK) {
            Self::TimedOut
        } else if outage {
            Self::Unavailable
        } else {
            Self::Failed
        };
        (kind, error)
    }
}

/// `x_cache` of `ready`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadyXCache {
    /// The read facts.
    #[serde(flatten)]
    pub read: ReadXCache,
    /// The basis of the answer.
    pub basis: Basis,
    /// Journaled writes adjusted the base.
    pub locally_adjusted: bool,
    /// Why a fresh answer fell back; `null` when none was asked or it came.
    pub read_fallback: Option<ReadFallback>,
    /// The failure behind `read_fallback`.
    pub read_fallback_error: Option<OpError>,
    /// Ready per bd, but not cached yet: left out of `items`.
    pub missing_rows: Vec<BeadId>,
}

/// `x_cache` on writes: the journal entry (contract §1 rule 4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WriteXCache {
    /// The entry.
    pub seq: i64,
    /// `pending`, `applied`, `escalated` or `resolved`.
    pub state: &'static str,
    /// The key already journaled this write.
    pub duplicate: bool,
    /// The caller's key.
    pub idempotency_key: String,
}

/// `x_cache` of `dependencies list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DepsXCache {
    /// The outage, while one holds.
    pub outage: Option<OutageData>,
    /// Requested beads the cache does not hold.
    pub not_cached: Vec<BeadId>,
}

/// `x_cache` of `cache refresh issues`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RowsXCache {
    /// Facts about each row returned.
    pub rows: Rows,
}

/// `x_cache` of `context`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContextXCache {
    /// The cache file.
    pub cache_path: String,
    /// [`CONTRACT_VERSION`](crate::CONTRACT_VERSION).
    pub contract_version: &'static str,
    /// The store's `user_version`.
    pub store_schema_version: i64,
    /// The recorded `bd schema` sha, once fingerprinted.
    pub bd_schema_sha256: Option<String>,
    /// The recorded `bd schema` version, once fingerprinted.
    pub bd_schema_version: Option<String>,
    /// The recorded schema equals the pinned model's; `null` before the first fingerprint.
    pub model_current: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `read_fallback` follows the core's outage flag, not the code: an
    /// answered `BD_LIST_FAILED` is `failed` (B7Q1-015).
    #[test]
    fn the_fallback_kind_follows_the_outage_flag() {
        let cut = "BD_CACHE.BD_UNAVAILABLE: bd blocked / bd ready: no answer within 5ms; retry";
        assert_eq!(ReadFallback::of(cut, true).0, ReadFallback::TimedOut);
        let down = "BD_CACHE.BD_UNAVAILABLE: refused; retry";
        assert_eq!(ReadFallback::of(down, true).0, ReadFallback::Unavailable);
        let answered = "BD_CACHE.BD_LIST_FAILED: exit 1; retry";
        assert_eq!(ReadFallback::of(answered, false).0, ReadFallback::Failed);
        let transport = "BD_CACHE.BD_LIST_FAILED: stdout exceeded; retry";
        assert_eq!(
            ReadFallback::of(transport, true).0,
            ReadFallback::Unavailable
        );
    }
}
