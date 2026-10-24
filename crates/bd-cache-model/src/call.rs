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

/// Why one backend call failed, after the cache's classification.
///
/// `call` returns `Result<T, CallFailure>`: an answer is `Ok`, every failure
/// is one of these. `Unavailable { timed_out: true }` when the caller's hard
/// deadline has expired is a cut, not an outage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallFailure {
    /// The caller's hard deadline cut the call: neither an outage nor an attempt.
    Cut,
    /// The backend is unreachable; the outage has been recorded with this error
    /// (always `BD_UNAVAILABLE`).
    Outage(CacheError),
    /// Any other failure, typed. On a write this counts an attempt.
    Error(CacheError),
}

impl CallFailure {
    /// The typed error for a caller that does not distinguish the cases:
    /// a cut becomes `BD_UNAVAILABLE` with `detail`.
    #[must_use]
    pub fn into_error(self, detail: impl Into<String>) -> CacheError {
        match self {
            Self::Cut => CacheError::BdUnavailable {
                detail: detail.into(),
                source: None,
            },
            Self::Outage(e) | Self::Error(e) => e,
        }
    }
}
