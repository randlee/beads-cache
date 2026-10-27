//! Sync scopes are named bd filters (ADR-BDC-0013): [`ScopeName`],
//! [`ScopeId`], [`ScopeFilter`] and [`Scope`].
//!
//! A scope is a validated name plus a bd filter: `All`, or criteria over
//! labels (all of, any of), status, type and assignee. The types are
//! validated once, here; the front ends take them and never re-validate.
//!
//! - [`ScopeName`] keeps the phase-a name rule, so every name a v3 file holds
//!   stays valid. A scope's name is never passed to bd; its filter's labels are.
//! - [`ScopeFilter`] is the cache's filter, `status` included. It converts to
//!   the backend's [`sc_beads::ScopeFilter`] with [`ScopeFilter::backend`],
//!   which drops `status`: bd is never sent one, and the store applies the
//!   status criterion to membership.
//! - [`Scope`] is the registry's row: `{ id, filter }`, where a `None` filter
//!   is a phase-a scope, a label scope whose label is
//!   [`LEGACY_SCOPE_PREFIX`](crate::LEGACY_SCOPE_PREFIX) followed by the name.
//!
//! The store keeps a scope's name in the v3 `phase` columns; the column's
//! name is kept for format compatibility (ADR-STORE-0001) and says nothing
//! about what a scope is.

use std::borrow::Borrow;
use std::collections::BTreeSet;
use std::fmt;

use sc_beads::cache::check_filter_word;
use sc_beads::{Bead, InvalidValue};
use serde::{Deserialize, Serialize, Serializer};

use crate::compat::legacy_label;

/// The text of [`SCOPE_NAME_RULE`], as a literal: `concat!` builds every
/// message of this crate that states the rule from this one text (RBP-F037).
macro_rules! scope_name_rule {
    () => {
        "one or more ASCII letters, digits, '-', '_' or '.'"
    };
}

/// The literal of [`MAX_SCOPE_NAME_LEN`], for the messages that state it.
macro_rules! max_scope_name_len {
    () => {
        64
    };
}

/// The literal of [`MAX_FILTER_WORDS`], for the messages that state it.
macro_rules! max_filter_words {
    () => {
        32
    };
}

/// The literal of [`MAX_FILTER_WORD_BYTES`], for the messages that state it.
macro_rules! max_filter_word_bytes {
    () => {
        256
    };
}

/// The longest name a newly registered scope may have, in bytes.
///
/// A scope's name is part of a file name (the tick's per-scope files), a
/// `meta` key (`sync_alert:<name>`) and part of every log line
/// about the scope; 64 bytes stays far under every platform's file-name limit
/// (255) with the prefix and suffix added. Names already stored are read as
/// they are (ADR-BDC-0013): the cap applies only when a scope is registered.
pub const MAX_SCOPE_NAME_LEN: usize = max_scope_name_len!();

/// The most filter words (labels, statuses, type, assignee) a newly
/// registered scope's filter may have.
///
/// Each word becomes part of one bd argv; 32 is far more than any scope
/// needs and keeps an argv well inside Windows' 32 767-character command
/// line with [`MAX_FILTER_WORD_BYTES`] words.
pub const MAX_FILTER_WORDS: usize = max_filter_words!();

/// The most scopes the registry holds.
///
/// Every registration (a sync of a new name, `scopes add`, a configured
/// scope) adds a registry row, a `sync_state` row and an alert key, and each
/// tick and `cache status` reads them all; 1024 is far past any repository's
/// scopes and bounds a stream of on-demand syncs of new names. A registration
/// past it is `BAD_ARGS` ([`CacheError::registry_full`](crate::CacheError::registry_full)).
pub const MAX_REGISTERED_SCOPES: usize = 1024;

/// The longest filter word of a newly registered scope, in bytes.
pub const MAX_FILTER_WORD_BYTES: usize = max_filter_word_bytes!();

/// The rule a scope name follows, in words: the one text the validator and
/// every fix that states the rule use (RBP-F037). Other crates read it as
/// [`ScopeName::RULE`].
pub(crate) const SCOPE_NAME_RULE: &str = scope_name_rule!();

/// A scope's name, as the store, the journal and every front end write it.
///
/// Nonempty ASCII letters, digits, `-`, `_` and `.`, with no length cap: the
/// phase-a rule, so every name a v3 file holds stays valid and none is
/// refused or renamed (ADR-BDC-0013). The name is safe in meta keys. A
/// scope registered from now on is also held to [`MAX_SCOPE_NAME_LEN`]
/// ([`Scope::check_new`]); a stored name is read as it is.
///
/// # Examples
///
/// ```
/// use bd_cache_model::ScopeName;
/// assert_eq!(ScopeName::new("bk").unwrap().as_str(), "bk");
/// assert!(ScopeName::new("").is_err());
/// assert!(ScopeName::new("b k").is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScopeName(String);

impl ScopeName {
    /// The rule a scope name follows, in words: the text the validator's
    /// reason and every fix about a scope name state.
    ///
    /// # Examples
    ///
    /// ```
    /// use bd_cache_model::ScopeName;
    /// let err = ScopeName::new("b k").unwrap_err();
    /// assert!(err.reason.contains(ScopeName::RULE));
    /// ```
    pub const RULE: &'static str = SCOPE_NAME_RULE;

    /// Validates a scope name.
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] for an empty name or one with other characters.
    pub fn new(name: impl Into<String>) -> Result<Self, InvalidValue> {
        let name = name.into();
        let ok = !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
        if ok {
            Ok(Self(name))
        } else {
            Err(InvalidValue {
                kind: "scope name",
                value: name,
                reason: concat!("expected ", scope_name_rule!()),
            })
        }
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ScopeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ScopeName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// `Hash` and `Eq` are the name's, so a map keyed by `ScopeName` is looked up by `&str`.
impl Borrow<str> for ScopeName {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl Serialize for ScopeName {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

/// A sync scope, by name (ADR-BDC-0013): `ScopeId(ScopeName)`.
///
/// The name is taken as written: no prefix is added or stripped. Only the
/// 0.1 compatibility paths read a legacy-prefixed argument as the phase-a
/// scope it names ([`legacy_scope`](crate::legacy_scope)).
///
/// # Examples
///
/// ```
/// use bd_cache_model::ScopeId;
/// let x = ScopeId::new("x").unwrap();
/// assert_eq!(x.name(), "x");
/// assert_ne!(ScopeId::new("lane-x").ok(), Some(x));
/// assert!(ScopeId::new("").is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScopeId(ScopeName);

impl ScopeId {
    /// The scope named `name`, validated as a [`ScopeName`].
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] when `name` is not a valid [`ScopeName`].
    pub fn new(name: impl Into<String>) -> Result<Self, InvalidValue> {
        ScopeName::new(name).map(Self)
    }

    /// The scope with this name.
    #[must_use]
    pub fn from_name(name: ScopeName) -> Self {
        Self(name)
    }

    /// The scope's validated name.
    #[must_use]
    pub fn scope_name(&self) -> &ScopeName {
        &self.0
    }

    /// The scope's name as text.
    #[must_use]
    pub fn name(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for ScopeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl AsRef<str> for ScopeId {
    fn as_ref(&self) -> &str {
        self.name()
    }
}

impl Serialize for ScopeId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.name())
    }
}

/// The unvalidated keys of a scope filter, as `.beads/bd-cache.toml`'s
/// `[[scope]]` tables, the store's `scope.filter` JSON and a request give
/// them; [`ScopeFilter::from_parts`] validates them.
///
/// # Examples
///
/// ```
/// use bd_cache_model::{FilterParts, ScopeFilter};
/// let parts = FilterParts { labels_all: vec!["area".into()], ..FilterParts::default() };
/// assert!(ScopeFilter::from_parts(parts).is_ok());
/// assert!(ScopeFilter::from_parts(FilterParts::default()).is_err());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilterParts {
    /// Every bead (`all = true`); refused together with any criterion.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub all: bool,
    /// Labels a bead must all carry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels_all: Vec<String>,
    /// Labels of which a bead must carry at least one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels_any: Vec<String>,
    /// bd statuses a member must have; applied to membership by the store.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub status: Vec<String>,
    /// The issue type (`type` in the configuration).
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub issue_type: Option<String>,
    /// The assignee.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
}

/// The criteria of a [`ScopeFilter::Criteria`]: at least one is set.
///
/// Built only by [`ScopeFilter::from_parts`], so a criteria filter with every
/// field empty cannot exist.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Criteria {
    labels_all: BTreeSet<String>,
    labels_any: BTreeSet<String>,
    status: BTreeSet<String>,
    issue_type: Option<String>,
    assignee: Option<String>,
}

impl Criteria {
    /// Labels a bead must all carry.
    #[must_use]
    pub fn labels_all(&self) -> &BTreeSet<String> {
        &self.labels_all
    }

    /// Labels of which a bead must carry at least one (empty: no such criterion).
    #[must_use]
    pub fn labels_any(&self) -> &BTreeSet<String> {
        &self.labels_any
    }

    /// The statuses a member must have (empty: any status).
    #[must_use]
    pub fn status(&self) -> &BTreeSet<String> {
        &self.status
    }

    /// The issue type.
    #[must_use]
    pub fn issue_type(&self) -> Option<&str> {
        self.issue_type.as_deref()
    }

    /// The assignee.
    #[must_use]
    pub fn assignee(&self) -> Option<&str> {
        self.assignee.as_deref()
    }
}

/// A scope's validated bd filter (ADR-BDC-0013): every bead, or criteria.
///
/// # Examples
///
/// ```
/// use bd_cache_model::{FilterParts, ScopeFilter};
/// let open_bugs = ScopeFilter::from_parts(FilterParts {
///     labels_all: vec!["area".into()],
///     status: vec!["open".into()],
///     issue_type: Some("bug".into()),
///     ..FilterParts::default()
/// })?;
/// let mut row = sc_beads::testing::bead("b1");
/// row.labels = vec!["area".into()];
/// row.issue_type = Some("bug".into());
/// assert!(open_bugs.holds(&row));
/// row.status = Some("closed".into());
/// assert!(!open_bugs.holds(&row) && open_bugs.selects(&row));
/// // The backend's filter has no status: bd is never sent one.
/// assert_eq!(open_bugs.backend().labels_all(), vec!["area"]);
///
/// // All with a criterion, and a criteria filter with no criterion, are refused.
/// let both = FilterParts { all: true, status: vec!["open".into()], ..FilterParts::default() };
/// assert!(ScopeFilter::from_parts(both).is_err());
/// # Ok::<(), sc_beads::InvalidValue>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScopeFilter {
    /// Every bead (`bd list --all`).
    All,
    /// Beads matching every set criterion.
    Criteria(Criteria),
}

impl ScopeFilter {
    /// Validates `parts` into a filter: the one constructor.
    ///
    /// Each label, status, type and assignee value must be one bd word
    /// (nonempty, no leading `-`, no `,`, no control character:
    /// [`sc_beads::cache::check_filter_word`], the one statement of the rule).
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] for `all` together with a criterion, a criteria
    /// filter with every field empty, or a value bd would misread.
    pub fn from_parts(parts: FilterParts) -> Result<Self, InvalidValue> {
        let FilterParts {
            all,
            labels_all,
            labels_any,
            status,
            issue_type,
            assignee,
        } = parts;
        let none = labels_all.is_empty()
            && labels_any.is_empty()
            && status.is_empty()
            && issue_type.is_none()
            && assignee.is_none();
        if all {
            return if none {
                Ok(Self::All)
            } else {
                Err(InvalidValue {
                    kind: "scope filter",
                    value: "all".to_owned(),
                    reason: "all = true cannot be combined with a criterion",
                })
            };
        }
        if none {
            return Err(InvalidValue {
                kind: "scope filter",
                value: String::new(),
                reason: "sets no criterion: give labels_all, labels_any, status, type or \
                         assignee, or all = true",
            });
        }
        for l in labels_all.iter().chain(&labels_any) {
            check_filter_word("scope label", l)?;
        }
        for s in &status {
            check_filter_word("scope status", s)?;
        }
        if let Some(t) = &issue_type {
            check_filter_word("scope type", t)?;
        }
        if let Some(a) = &assignee {
            check_filter_word("scope assignee", a)?;
        }
        Ok(Self::Criteria(Criteria {
            labels_all: labels_all.into_iter().collect(),
            labels_any: labels_any.into_iter().collect(),
            status: status.into_iter().collect(),
            issue_type,
            assignee,
        }))
    }

    /// The label filter of one label: `labels_all = [label]`.
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] when `label` cannot be one bd word.
    pub fn label(label: impl Into<String>) -> Result<Self, InvalidValue> {
        Self::from_parts(FilterParts {
            labels_all: vec![label.into()],
            ..FilterParts::default()
        })
    }

    /// The filter's keys, the inverse of [`ScopeFilter::from_parts`] (what the
    /// store writes as `scope.filter` JSON).
    #[must_use]
    pub fn parts(&self) -> FilterParts {
        match self {
            Self::All => FilterParts {
                all: true,
                ..FilterParts::default()
            },
            Self::Criteria(c) => FilterParts {
                all: false,
                labels_all: c.labels_all.iter().cloned().collect(),
                labels_any: c.labels_any.iter().cloned().collect(),
                status: c.status.iter().cloned().collect(),
                issue_type: c.issue_type.clone(),
                assignee: c.assignee.clone(),
            },
        }
    }

    /// The backend's filter: the one conversion (ADR-BDC-0013).
    ///
    /// Drops `status`, which bd is never sent; a criteria filter whose only
    /// criterion is `status` becomes [`sc_beads::ScopeFilter::All`]. Never an
    /// empty criteria filter.
    #[must_use]
    pub fn backend(&self) -> sc_beads::ScopeFilter {
        match self {
            Self::All => sc_beads::ScopeFilter::All,
            Self::Criteria(c) => {
                if c.labels_all.is_empty()
                    && c.labels_any.is_empty()
                    && c.issue_type.is_none()
                    && c.assignee.is_none()
                {
                    sc_beads::ScopeFilter::All
                } else {
                    sc_beads::ScopeFilter::Criteria {
                        labels_all: c.labels_all.iter().cloned().collect(),
                        labels_any: c.labels_any.iter().cloned().collect(),
                        issue_type: c.issue_type.clone(),
                        assignee: c.assignee.clone(),
                    }
                }
            }
        }
    }

    /// True when bd's list of the filter returns `row`, in any status
    /// (labels, type and assignee).
    ///
    /// Equal to `self.backend().selects(row)`, read over the criteria in
    /// place, so a membership test allocates nothing (a unit test keeps the
    /// two equal).
    #[must_use]
    pub fn selects(&self, row: &Bead) -> bool {
        match self {
            Self::All => true,
            Self::Criteria(c) => {
                c.labels_all.iter().all(|l| row.has_label(l))
                    && (c.labels_any.is_empty() || c.labels_any.iter().any(|l| row.has_label(l)))
                    && c.issue_type
                        .as_deref()
                        .is_none_or(|t| row.issue_type.as_deref() == Some(t))
                    && c.assignee
                        .as_deref()
                        .is_none_or(|a| row.assignee.as_deref() == Some(a))
            }
        }
    }

    /// The criterion `row` fails, when bd's list of the filter would not
    /// return it: `labels`, `type` or `assignee` (a `BD_CONTRACT` field).
    #[must_use]
    pub fn unselected_by(&self, row: &Bead) -> Option<&'static str> {
        let Self::Criteria(c) = self else {
            return None;
        };
        let labels = c.labels_all.iter().all(|l| row.has_label(l))
            && (c.labels_any.is_empty() || c.labels_any.iter().any(|l| row.has_label(l)));
        if !labels {
            return Some("labels");
        }
        if c.issue_type
            .as_deref()
            .is_some_and(|t| row.issue_type.as_deref() != Some(t))
        {
            return Some("type");
        }
        if c.assignee
            .as_deref()
            .is_some_and(|a| row.assignee.as_deref() != Some(a))
        {
            return Some("assignee");
        }
        None
    }

    /// How many filter words the filter holds: its labels, statuses, type and assignee.
    #[must_use]
    pub fn word_count(&self) -> usize {
        match self {
            Self::All => 0,
            Self::Criteria(c) => {
                c.labels_all.len()
                    + c.labels_any.len()
                    + c.status.len()
                    + usize::from(c.issue_type.is_some())
                    + usize::from(c.assignee.is_some())
            }
        }
    }

    /// Every filter word, in no particular order.
    fn words(&self) -> impl Iterator<Item = &str> {
        let c = match self {
            Self::All => None,
            Self::Criteria(c) => Some(c),
        };
        c.into_iter().flat_map(|c| {
            c.labels_all
                .iter()
                .chain(&c.labels_any)
                .chain(&c.status)
                .map(String::as_str)
                .chain(c.issue_type.as_deref())
                .chain(c.assignee.as_deref())
        })
    }

    /// True when `row`'s status is in the status criterion, or there is none.
    #[must_use]
    pub fn admits_status(&self, row: &Bead) -> bool {
        match self {
            Self::All => true,
            Self::Criteria(c) => {
                c.status.is_empty() || row.status.as_deref().is_some_and(|s| c.status.contains(s))
            }
        }
    }

    /// True when `row` is a member: bd selects it and its status is in the
    /// status criterion, when there is one (the store's membership rule).
    #[must_use]
    pub fn holds(&self, row: &Bead) -> bool {
        self.admits_status(row) && self.selects(row)
    }
}

impl Serialize for ScopeFilter {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.parts().serialize(s)
    }
}

/// One scope of the registry: its name and its filter (ADR-BDC-0013).
///
/// `filter: None` is a phase-a scope: the beads carrying the label
/// [`LEGACY_SCOPE_PREFIX`](crate::LEGACY_SCOPE_PREFIX) followed by the name (a stored `x` syncs the label
/// `<prefix>x`); `Some` is a scope created in phase B with a filter.
///
/// # Examples
///
/// ```
/// use bd_cache_model::{LEGACY_SCOPE_PREFIX, Scope, ScopeFilter, ScopeId};
/// let legacy = Scope::legacy(ScopeId::new("bk")?);
/// let mut row = sc_beads::testing::bead("b1");
/// row.labels = vec![format!("{LEGACY_SCOPE_PREFIX}bk")];
/// assert!(legacy.holds(&row));
/// let area = Scope::new(ScopeId::new("area")?, ScopeFilter::label("area")?);
/// assert!(!area.holds(&row));
/// # Ok::<(), sc_beads::InvalidValue>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Scope {
    /// The scope's name.
    pub id: ScopeId,
    /// The filter; `None` = a phase-a label scope.
    pub filter: Option<ScopeFilter>,
}

impl Scope {
    /// A scope with a filter.
    #[must_use]
    pub fn new(id: ScopeId, filter: ScopeFilter) -> Self {
        Self {
            id,
            filter: Some(filter),
        }
    }

    /// A phase-a scope: the label [`LEGACY_SCOPE_PREFIX`](crate::LEGACY_SCOPE_PREFIX) followed by the name.
    #[must_use]
    pub fn legacy(id: ScopeId) -> Self {
        Self { id, filter: None }
    }

    /// The scope a sync of an unregistered name registers on demand: a label
    /// scope with that one label (REQ-BDC-0909 as amended by ADR-BDC-0013).
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] when the name cannot be one bd label word (it starts
    /// with `-`).
    pub fn on_demand(id: ScopeId) -> Result<Self, InvalidValue> {
        let filter = ScopeFilter::label(id.name())?;
        Ok(Self::new(id, filter))
    }

    /// True for a phase-a scope (no stored filter).
    #[must_use]
    pub fn is_legacy(&self) -> bool {
        self.filter.is_none()
    }

    /// The backend filter the sync lists (REQ-BDC-0501 as amended).
    #[must_use]
    pub fn backend_filter(&self) -> sc_beads::ScopeFilter {
        match &self.filter {
            None => sc_beads::ScopeFilter::label(legacy_label(&self.id)),
            Some(f) => f.backend(),
        }
    }

    /// True when `row` is a member (a scoped upsert's membership rule).
    ///
    /// A phase-a scope's rule is its backend filter's (the one label); a
    /// filter scope's is its filter with the status criterion.
    #[must_use]
    pub fn holds(&self, row: &Bead) -> bool {
        match &self.filter {
            None => self.backend_filter().selects(row),
            Some(f) => f.holds(row),
        }
    }

    /// True when a row an ingest declares a member is one: its status is in
    /// the scope's status criterion, when there is one (REQ-BDC-0726; the
    /// snapshot declares membership, and the store applies status to it).
    #[must_use]
    pub fn admits(&self, row: &Bead) -> bool {
        self.filter.as_ref().is_none_or(|f| f.admits_status(row))
    }

    /// True when bd's list of the scope returns `row`, in any status.
    #[must_use]
    pub fn selects(&self, row: &Bead) -> bool {
        match &self.filter {
            None => self.backend_filter().selects(row),
            Some(f) => f.selects(row),
        }
    }

    /// The criterion `row` fails when bd's list of the scope would not
    /// return it (`labels`, `type` or `assignee`); `None` when it is selected.
    #[must_use]
    pub fn unselected_by(&self, row: &Bead) -> Option<&'static str> {
        match &self.filter {
            None => (!self.selects(row)).then_some("labels"),
            Some(f) => f.unselected_by(row),
        }
    }

    /// Checks the limits a newly registered scope is held to (ADR-BDC-0013,
    /// B7Q1-045): a name of at most [`MAX_SCOPE_NAME_LEN`] bytes, at most
    /// [`MAX_FILTER_WORDS`] filter words, each at most
    /// [`MAX_FILTER_WORD_BYTES`]. `scopes add` and `set`, the configuration's
    /// `[[scope]]` tables and `scopes` list, and on-demand registration call
    /// it; a scope already stored is read as it is.
    ///
    /// # Examples
    ///
    /// ```
    /// use bd_cache_model::{MAX_SCOPE_NAME_LEN, Scope, ScopeId};
    /// assert!(Scope::legacy(ScopeId::new("bk")?).check_new().is_ok());
    /// let long = ScopeId::new("x".repeat(MAX_SCOPE_NAME_LEN + 1))?;
    /// assert!(Scope::legacy(long).check_new().is_err());
    /// # Ok::<(), sc_beads::InvalidValue>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] naming the limit the scope is over.
    pub fn check_new(&self) -> Result<(), InvalidValue> {
        if self.id.name().len() > MAX_SCOPE_NAME_LEN {
            return Err(InvalidValue {
                kind: "scope name",
                value: self.id.name().to_owned(),
                reason: concat!(
                    "is longer than ",
                    max_scope_name_len!(),
                    " bytes, the limit for a new scope"
                ),
            });
        }
        let Some(filter) = &self.filter else {
            return Ok(());
        };
        if filter.word_count() > MAX_FILTER_WORDS {
            return Err(InvalidValue {
                kind: "scope filter",
                value: format!("{} words", filter.word_count()),
                reason: concat!("has more than ", max_filter_words!(), " words"),
            });
        }
        match filter.words().find(|w| w.len() > MAX_FILTER_WORD_BYTES) {
            Some(word) => Err(InvalidValue {
                kind: "scope filter word",
                value: word.to_owned(),
                reason: concat!("is longer than ", max_filter_word_bytes!(), " bytes"),
            }),
            None => Ok(()),
        }
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.id, f)
    }
}

impl Serialize for Scope {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("Scope", 2)?;
        st.serialize_field("name", &self.id)?;
        st.serialize_field("filter", &self.filter)?;
        st.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LEGACY_SCOPE_PREFIX;

    /// The validator states the one rule (RBP-F037).
    #[test]
    fn the_validator_states_the_scope_name_rule() {
        for bad in ["", "b k", "a/b"] {
            let err = ScopeName::new(bad).unwrap_err();
            assert!(
                err.reason.contains(SCOPE_NAME_RULE),
                "{bad:?}: {}",
                err.reason
            );
        }
        assert_eq!(ScopeName::RULE, SCOPE_NAME_RULE);
        // The phase-a rule: a name a v3 file may hold stays valid.
        for ok in ["bk", "-x", "a.b_c-1", "None"] {
            assert!(ScopeName::new(ok).is_ok(), "{ok}");
        }
    }

    /// A criteria filter with every field empty, and `all` with a criterion,
    /// are refused at construction (ADR-BDC-0013, AC3).
    #[test]
    fn an_empty_criteria_filter_and_all_with_a_criterion_are_refused() {
        assert!(ScopeFilter::from_parts(FilterParts::default()).is_err());
        for parts in [
            FilterParts {
                all: true,
                labels_all: vec!["a".into()],
                ..FilterParts::default()
            },
            FilterParts {
                all: true,
                status: vec!["open".into()],
                ..FilterParts::default()
            },
            FilterParts {
                all: true,
                assignee: Some("dev".into()),
                ..FilterParts::default()
            },
        ] {
            assert!(ScopeFilter::from_parts(parts.clone()).is_err(), "{parts:?}");
        }
        assert_eq!(
            ScopeFilter::from_parts(FilterParts {
                all: true,
                ..FilterParts::default()
            }),
            Ok(ScopeFilter::All)
        );
    }

    /// Every filter value is one bd word.
    #[test]
    fn filter_values_that_bd_would_misread_are_refused() {
        for bad in ["", " ", "-l", "a,b", "a\nb"] {
            assert!(ScopeFilter::label(bad).is_err(), "{bad:?}");
        }
        assert!(ScopeFilter::label("area-x").is_ok());
    }

    /// The conversion drops `status` and makes a status-only filter `All`.
    #[test]
    fn the_backend_filter_never_carries_status() {
        let status_only = ScopeFilter::from_parts(FilterParts {
            status: vec!["open".into()],
            ..FilterParts::default()
        })
        .unwrap();
        assert_eq!(status_only.backend(), sc_beads::ScopeFilter::All);
        let typed = ScopeFilter::from_parts(FilterParts {
            status: vec!["open".into()],
            issue_type: Some("bug".into()),
            ..FilterParts::default()
        })
        .unwrap();
        assert_eq!(
            typed.backend(),
            sc_beads::ScopeFilter::Criteria {
                labels_all: vec![],
                labels_any: vec![],
                issue_type: Some("bug".into()),
                assignee: None,
            }
        );
    }

    /// The stored JSON round-trips through the parts.
    #[test]
    fn a_filter_round_trips_through_its_parts() {
        let f = ScopeFilter::from_parts(FilterParts {
            labels_all: vec!["b".into(), "a".into()],
            labels_any: vec!["c".into()],
            status: vec!["open".into()],
            issue_type: Some("bug".into()),
            assignee: Some("dev".into()),
            all: false,
        })
        .unwrap();
        let json = serde_json::to_string(&f).unwrap();
        let back: FilterParts = serde_json::from_str(&json).unwrap();
        assert_eq!(ScopeFilter::from_parts(back).unwrap(), f);
        assert_eq!(
            serde_json::to_string(&ScopeFilter::All).unwrap(),
            r#"{"all":true}"#
        );
    }

    /// A phase-a scope syncs its legacy label; an on-demand scope its own name.
    #[test]
    fn legacy_and_on_demand_scopes_list_their_labels() {
        let x = ScopeId::new("x").unwrap();
        assert_eq!(
            Scope::legacy(x.clone()).backend_filter(),
            sc_beads::ScopeFilter::label(format!("{LEGACY_SCOPE_PREFIX}x"))
        );
        assert_eq!(
            Scope::on_demand(x).unwrap().backend_filter().labels_all(),
            vec!["x"]
        );
        assert!(Scope::on_demand(ScopeId::new("-x").unwrap()).is_err());
    }

    /// One case of the selection test: labels, type, assignee, and the
    /// criterion the row fails.
    type Case<'a> = (
        &'a [&'a str],
        Option<&'a str>,
        Option<&'a str>,
        Option<&'a str>,
    );

    /// `selects` reads the criteria in place and equals the backend
    /// filter's answer; `unselected_by` names the criterion a row fails.
    #[test]
    fn selects_equals_the_backend_filter_and_names_the_failed_criterion() {
        let f = ScopeFilter::from_parts(FilterParts {
            labels_all: vec!["a".into()],
            labels_any: vec!["b".into(), "c".into()],
            issue_type: Some("bug".into()),
            assignee: Some("dev".into()),
            ..FilterParts::default()
        })
        .unwrap();
        let mut row = sc_beads::testing::bead("b1");
        let cases: [Case<'_>; 5] = [
            (&["a", "c"], Some("bug"), Some("dev"), None),
            (&["a"], Some("bug"), Some("dev"), Some("labels")),
            (&["c"], Some("bug"), Some("dev"), Some("labels")),
            (&["a", "b"], Some("task"), Some("dev"), Some("type")),
            (&["a", "b"], Some("bug"), None, Some("assignee")),
        ];
        for (labels, t, a, failed) in cases {
            row.labels = labels.iter().map(|l| (*l).to_owned()).collect();
            row.issue_type = t.map(str::to_owned);
            row.assignee = a.map(str::to_owned);
            assert_eq!(f.selects(&row), f.backend().selects(&row), "{labels:?}");
            assert_eq!(f.unselected_by(&row), failed, "{labels:?}");
        }
        assert!(ScopeFilter::All.selects(&row));
        assert_eq!(ScopeFilter::All.unselected_by(&row), None);
    }

    /// A new scope is held to the name, word-count and word-length caps;
    /// a stored name of any length still validates (ADR-BDC-0013).
    #[test]
    fn a_new_scope_over_the_caps_is_refused_and_a_stored_name_still_reads() {
        let long = "x".repeat(MAX_SCOPE_NAME_LEN + 1);
        let id = ScopeId::new(long.clone()).expect("a stored name of any length reads");
        assert!(Scope::legacy(id).check_new().is_err());
        let ok = ScopeId::new("x".repeat(MAX_SCOPE_NAME_LEN)).unwrap();
        assert!(Scope::legacy(ok.clone()).check_new().is_ok());
        let many = ScopeFilter::from_parts(FilterParts {
            labels_all: (0..=MAX_FILTER_WORDS).map(|i| format!("l{i}")).collect(),
            ..FilterParts::default()
        })
        .unwrap();
        assert!(Scope::new(ok.clone(), many).check_new().is_err());
        let wide = ScopeFilter::label("w".repeat(MAX_FILTER_WORD_BYTES + 1)).unwrap();
        assert!(Scope::new(ok.clone(), wide).check_new().is_err());
        let fits = ScopeFilter::label("w".repeat(MAX_FILTER_WORD_BYTES)).unwrap();
        assert!(Scope::new(ok, fits).check_new().is_ok());
    }
}
