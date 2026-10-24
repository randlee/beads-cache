//! Sync scopes (ADR-BDC-0001): [`ScopeId`] and [`PhaseName`].

use std::fmt;

use sc_beads::{Bead, InvalidValue, ScopeFilter};
use serde::{Serialize, Serializer};

/// A phase name, the `<x>` of a `phase-<x>` label and the v3 `phase` column.
///
/// Nonempty ASCII letters, digits, `-`, `_` and `.`, so the label is one
/// bd argument and the name is safe in meta keys (`sync_alert:<x>`).
///
/// # Examples
///
/// ```
/// use bd_cache_model::PhaseName;
/// assert_eq!(PhaseName::new("bk").unwrap().as_str(), "bk");
/// assert!(PhaseName::new("").is_err());
/// assert!(PhaseName::new("b k").is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhaseName(String);

impl PhaseName {
    /// Validates a phase name.
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
                kind: "phase name",
                value: name,
                reason: "expected ASCII letters, digits, '-', '_' or '.'",
            })
        }
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PhaseName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for PhaseName {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

/// A sync scope. The only kind in phase a is a `phase-<x>` label.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ScopeId {
    /// The beads labelled `phase-<x>`.
    Phase(PhaseName),
}

impl ScopeId {
    /// The phase scope `<x>`.
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] when `name` is not a valid [`PhaseName`].
    pub fn phase(name: &str) -> Result<Self, InvalidValue> {
        PhaseName::new(name).map(Self::Phase)
    }

    /// Parses `phase-<x>` or a bare `<x>`; `None` when `<x>` is not a valid phase name.
    ///
    /// # Examples
    ///
    /// ```
    /// use bd_cache_model::ScopeId;
    /// assert_eq!(ScopeId::parse("phase-bk"), ScopeId::phase("bk").ok());
    /// assert_eq!(ScopeId::parse("bk"), ScopeId::phase("bk").ok());
    /// assert_eq!(ScopeId::parse("phase-"), None);
    /// ```
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let name = s.trim();
        let name = name.strip_prefix("phase-").unwrap_or(name);
        Self::phase(name).ok()
    }

    /// The scope's phase name.
    #[must_use]
    pub fn phase_name(&self) -> &PhaseName {
        match self {
            Self::Phase(x) => x,
        }
    }

    /// The scope's name as text: the phase `<x>`.
    #[must_use]
    pub fn name(&self) -> &str {
        self.phase_name().as_str()
    }

    /// The label selecting the scope: `phase-<x>`.
    #[must_use]
    pub fn label(&self) -> String {
        format!("phase-{}", self.name())
    }

    /// The backend filter: `Phase(x)` is `Label("phase-x")`.
    #[must_use]
    pub fn filter(&self) -> ScopeFilter {
        ScopeFilter::Label(self.label())
    }

    /// The first valid `phase-<x>` label (the v3 `phase` column).
    #[must_use]
    pub fn from_labels(labels: &[String]) -> Option<ScopeId> {
        labels
            .iter()
            .filter_map(|l| l.strip_prefix("phase-"))
            .find_map(|x| Self::phase(x).ok())
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
