//! Sync scopes (ADR-BDC-0001): [`ScopeId`].

use std::fmt;

use sc_beads::{Bead, ScopeFilter};
use serde::{Serialize, Serializer};

/// A sync scope. The only kind in phase a is a `phase-<x>` label.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ScopeId {
    /// The beads labelled `phase-<x>`; holds `<x>`.
    Phase(String),
}

impl ScopeId {
    /// The phase scope `<x>`.
    #[must_use]
    pub fn phase(name: impl Into<String>) -> Self {
        Self::Phase(name.into())
    }

    /// Parses `phase-<x>` or a bare `<x>`; `None` for an empty name.
    ///
    /// # Examples
    ///
    /// ```
    /// use bd_cache_model::ScopeId;
    /// assert_eq!(ScopeId::parse("phase-bk"), Some(ScopeId::phase("bk")));
    /// assert_eq!(ScopeId::parse("bk"), Some(ScopeId::phase("bk")));
    /// assert_eq!(ScopeId::parse("phase-"), None);
    /// ```
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let name = s.strip_prefix("phase-").unwrap_or(s).trim();
        (!name.is_empty()).then(|| Self::Phase(name.to_owned()))
    }

    /// The scope's name: the phase `<x>` (the v3 `phase` column value).
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Phase(x) => x,
        }
    }

    /// The label selecting the scope: `phase-<x>`.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Phase(x) => format!("phase-{x}"),
        }
    }

    /// The backend filter: `Phase(x)` is `Label("phase-x")`.
    #[must_use]
    pub fn filter(&self) -> ScopeFilter {
        ScopeFilter::Label(self.label())
    }

    /// The first `phase-<x>` label (the v3 `phase` column).
    #[must_use]
    pub fn from_labels(labels: &[String]) -> Option<ScopeId> {
        labels
            .iter()
            .find_map(|l| l.strip_prefix("phase-"))
            .filter(|x| !x.is_empty())
            .map(|x| Self::Phase(x.to_owned()))
    }

    /// True when the row carries the scope's label (scope membership on a scoped upsert).
    #[must_use]
    pub fn holds(&self, row: &Bead) -> bool {
        row.has_label(&self.label())
    }
}

impl fmt::Display for ScopeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl Serialize for ScopeId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.name())
    }
}
