//! Backend call kinds and their classified outcomes.

use crate::error::CacheError;

/// Which call failed, for mapping backend errors ([`CacheError::from_backend`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallKind {
    /// The backend check (`bd version` + `bd schema`).
    Check,
    /// The drain's batched pre-read.
    PreRead,
    /// A drain write.
    Write,
    /// A drain read-back.
    ReadBack,
    /// A sync scope list.
    Sync,
    /// A caller's fresh read (`refresh`) or a sync's stale re-read.
    Refresh,
    /// A cache-miss fill.
    MissFill,
    /// `bd blocked` / `bd ready` for a ready answer or base.
    Ready,
}

impl CallKind {
    /// Calls whose answer proves bd answers lists, so they clear an outage.
    #[must_use]
    pub fn is_list(self) -> bool {
        !matches!(self, Self::Check | Self::Write)
    }

    /// Calls for which an unreachable backend is an outage (ADR-BDC-0005).
    ///
    /// A write that fails or times out may have reached bd, so it counts an
    /// attempt instead; a failed read-back marks the entry applied and stale.
    #[must_use]
    pub fn unavailable_is_outage(self) -> bool {
        !matches!(self, Self::Write | Self::ReadBack)
    }
}

/// Outcome of one backend call after the cache's classification.
///
/// `Unavailable { timed_out: true }` when the caller's hard deadline has
/// expired is a cut, not an outage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallOutcome {
    /// The call answered.
    Ok,
    /// The caller's hard deadline cut the call: neither an outage nor an attempt.
    Cut,
    /// The backend is unreachable; holds the recorded error text (`CODE: detail`).
    Outage(String),
    /// Any other failure, typed.
    Error(CacheError),
}
