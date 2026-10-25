//! Sync scopes (ADR-BDC-0001, REQ-BDC-0702): [`ScopeId`] and [`Scope`].
//!
//! A scope is a name plus a bd filter: labels (all or any), status, type,
//! assignee, or every bead (`--all`). The built-in sync runs `bd list` with
//! the filter ([`sc_beads::ScopeFilter`]); a bead belongs to the scope while
//! it [matches](Scope::holds) the filter. Scopes are set in the repository
//! configuration (`.beads/bd-cache.toml`, [`RepoConfig`](crate::RepoConfig)).
//! A scope id that is not configured names the label of the same name
//! ([`Scope::label`]), so syncing it registers it on demand (REQ-BDC-0909).
//!
//! The store keeps a scope's name in the v3 `phase` columns; the name of that
//! column is kept for format compatibility (ADR-STORE-0001) and says nothing
//! about what a scope is.

use std::fmt;

use sc_beads::{Bead, FilterCriteria, InvalidValue, ScopeFilter};
use serde::{Serialize, Serializer};

/// The text of [`SCOPE_NAME_RULE`], as a literal: `concat!` builds every
/// message of this crate that states the rule from this one text (RBP-F037).
macro_rules! scope_name_rule {
    () => {
        "one or more ASCII letters, digits, '-', '_' or '.', not starting with '-'"
    };
}

/// The rule a scope name follows, in words: the one text the validator and
/// every fix that states the rule use (RBP-F037). Other crates read it as
/// [`ScopeId::RULE`].
pub(crate) const SCOPE_NAME_RULE: &str = scope_name_rule!();

/// A scope's name: what the store, the journal and every front end call it.
///
/// Nonempty ASCII letters, digits, `-`, `_` and `.`, not starting with `-`,
/// so the name is one bd argument when it is used as a label and is safe in
/// meta keys.
///
/// # Examples
///
/// ```
/// use bd_cache_model::ScopeId;
/// assert_eq!(ScopeId::new("bk").unwrap().name(), "bk");
/// assert!(ScopeId::new("").is_err());
/// assert!(ScopeId::new("b k").is_err());
/// assert!(ScopeId::new("-x").is_err());
/// assert_eq!(ScopeId::parse(" open-bugs "), ScopeId::new("open-bugs").ok());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScopeId(String);

impl ScopeId {
    /// The rule a scope name follows, in words: the text the validator's
    /// reason and every fix about a scope name state.
    ///
    /// # Examples
    ///
    /// ```
    /// use bd_cache_model::ScopeId;
    /// let err = ScopeId::new("b k").unwrap_err();
    /// assert!(err.reason.contains(ScopeId::RULE));
    /// ```
    pub const RULE: &'static str = SCOPE_NAME_RULE;

    /// Validates a scope name.
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] for an empty name, one with other characters, or one
    /// that starts with `-`.
    pub fn new(name: impl Into<String>) -> Result<Self, InvalidValue> {
        let name = name.into();
        let ok = !name.is_empty()
            && !name.starts_with('-')
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

    /// Parses a scope name with surrounding whitespace trimmed; `None` when
    /// it is not a valid name.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::new(s.trim()).ok()
    }

    /// The name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ScopeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for ScopeId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

/// A scope: its name and the bd filter that selects its beads (ADR-BDC-0001).
///
/// # Examples
///
/// ```
/// use bd_cache_model::{Scope, ScopeId};
/// use sc_beads::ScopeFilter;
/// let all = Scope::new(ScopeId::new("everything")?, ScopeFilter::All)?;
/// assert!(all.holds(&sc_beads::testing::bead("b1")));
/// let label = Scope::label(ScopeId::new("area")?);
/// assert_eq!(label.filter(), &ScopeFilter::Label("area".into()));
/// assert!(!label.holds(&sc_beads::testing::bead("b1")));
/// # Ok::<(), sc_beads::InvalidValue>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Scope {
    id: ScopeId,
    filter: ScopeFilter,
}

impl Scope {
    /// A scope named `id` that selects the beads of `filter`.
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] when a filter value cannot be passed to bd as one
    /// word (empty, a comma, a control character, a leading `-`), or when a
    /// criteria filter sets no criterion.
    pub fn new(id: ScopeId, filter: ScopeFilter) -> Result<Self, InvalidValue> {
        check_filter(&filter)?;
        Ok(Self { id, filter })
    }

    /// The scope a scope id names when it is not configured: the beads that
    /// carry the label of the same name (REQ-BDC-0909).
    #[must_use]
    pub fn label(id: ScopeId) -> Self {
        // A scope name is always a valid label value (its rule is stricter).
        let filter = ScopeFilter::Label(id.name().to_owned());
        Self { id, filter }
    }

    /// The scope's name.
    #[must_use]
    pub fn id(&self) -> &ScopeId {
        &self.id
    }

    /// The scope's bd filter.
    #[must_use]
    pub fn filter(&self) -> &ScopeFilter {
        &self.filter
    }

    /// True when `row` belongs to the scope: the filter
    /// [matches](ScopeFilter::matches) it, its status criterion included.
    #[must_use]
    pub fn holds(&self, row: &Bead) -> bool {
        self.filter.matches(row)
    }

    /// True when bd's list of the scope returns `row`, in any status
    /// ([`ScopeFilter::selects`]).
    #[must_use]
    pub fn selects(&self, row: &Bead) -> bool {
        self.filter.selects(row)
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.id, f)
    }
}

/// `InvalidValue` for one filter value that cannot be one bd argument.
fn check_value(kind: &'static str, value: &str) -> Result<(), InvalidValue> {
    let bad = if value.trim().is_empty() {
        Some("is empty")
    } else if value.starts_with('-') {
        Some("starts with '-'")
    } else if value.contains(',') {
        Some("contains ',' (bd splits a list on it)")
    } else if value.chars().any(char::is_control) {
        Some("contains a control character")
    } else {
        None
    };
    match bad {
        Some(reason) => Err(InvalidValue {
            kind,
            value: value.to_owned(),
            reason,
        }),
        None => Ok(()),
    }
}

/// Checks every value of `filter` (see [`Scope::new`]).
fn check_filter(filter: &ScopeFilter) -> Result<(), InvalidValue> {
    if let ScopeFilter::Match(c) = filter
        && *c == FilterCriteria::default()
    {
        return Err(InvalidValue {
            kind: "scope filter",
            value: String::new(),
            reason: "sets no criterion: give labels, labels_any, status, type or assignee, or all = true",
        });
    }
    for label in filter.labels().iter().chain(filter.labels_any()) {
        check_value("scope label", label)?;
    }
    for status in filter.statuses() {
        check_value("scope status", status)?;
    }
    if let Some(t) = filter.issue_type() {
        check_value("scope type", t)?;
    }
    if let Some(a) = filter.assignee() {
        check_value("scope assignee", a)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The validator states the one rule (RBP-F037).
    #[test]
    fn the_validator_states_the_scope_name_rule() {
        for bad in ["", "b k", "a/b", "-x"] {
            let err = ScopeId::new(bad).unwrap_err();
            assert!(
                err.reason.contains(SCOPE_NAME_RULE),
                "{bad:?}: {}",
                err.reason
            );
        }
        assert_eq!(ScopeId::RULE, SCOPE_NAME_RULE);
    }

    /// A scope id is its name: no prefix is added or stripped (ANC-009).
    #[test]
    fn a_scope_id_is_its_name_with_no_prefix() {
        let id = ScopeId::parse("bk").unwrap();
        assert_eq!(id.name(), "bk");
        assert_eq!(Scope::label(id).filter(), &ScopeFilter::Label("bk".into()));
        assert_ne!(ScopeId::parse("x-bk"), ScopeId::parse("bk"));
    }

    /// Every filter value is one bd word, and a criteria filter sets one criterion.
    #[test]
    fn filter_values_that_bd_would_misread_are_refused() {
        let id = || ScopeId::new("s").unwrap();
        for bad in ["", "-l", "a,b", "a\nb"] {
            assert!(
                Scope::new(id(), ScopeFilter::Label(bad.into())).is_err(),
                "{bad:?}"
            );
        }
        let none = ScopeFilter::Match(FilterCriteria::default());
        assert!(Scope::new(id(), none).is_err());
        let typed = ScopeFilter::Match(FilterCriteria {
            issue_type: Some("bug".into()),
            ..FilterCriteria::default()
        });
        assert!(Scope::new(id(), typed).is_ok());
        assert!(Scope::new(id(), ScopeFilter::All).is_ok());
    }
}
