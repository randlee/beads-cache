//! What one sync run did (REQ-BDC-0703, 0704; ADR-BDC-0005): [`SyncRun`].
//!
//! The run's reply carries each scope's result and error. There is no
//! `refused` or `pass-through` outcome: beads-cache sends no messages, so
//! nothing can be undelivered (ADR-BDC-0010). A post-sync hook gets the same
//! data in its request ([`crate::hook`]).

use bd_cache_model::{CacheError, DrainReport, Outage, ScopeId, SyncReport, SyncState};
use sc_beads::Timestamp;
use serde::Serialize;

/// A scope's sync error, as the run reports it: the stable code, the detail,
/// and whether bd could not be reached (ADR-BDC-0005).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScopeError {
    /// The `BD_CACHE.*` code.
    pub code: String,
    /// What happened.
    pub message: String,
    /// True only when bd could not be reached; `false` when it answered
    /// with a failure ([`CacheError::outage`]).
    pub outage: bool,
}

impl ScopeError {
    /// The run's view of a sync error.
    #[must_use]
    pub fn of(e: &CacheError) -> Self {
        Self {
            code: e.code().to_owned(),
            message: e.detail().to_owned(),
            outage: e.outage().unwrap_or(false),
        }
    }
}

/// One scope's part of a run.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScopeResult {
    /// The scope.
    pub scope: ScopeId,
    /// The sync's report; `None` when the sync failed or was not started.
    pub result: Option<SyncReport>,
    /// The sync's error; `None` when it passed, was deferred or hit an outage
    /// (an outage is in `result.outage`, never an error, ADR-BDC-0005).
    pub error: Option<ScopeError>,
    /// The sync was not started, or its deadline cut it, before the run's
    /// end (REQ-BDC-0703); it runs again on the next run.
    pub deferred: bool,
    /// When the scope's current run of failures and outages began; `None`
    /// while it syncs (REQ-BDC-0704).
    pub failing_since: Option<Timestamp>,
    /// True when a post-sync hook marked this run of failures handled
    /// (`scopes announce --since`, REQ-BDC-0705, 0706).
    pub notified: bool,
}

impl ScopeResult {
    /// A scope's result with its failure state read from the store.
    #[must_use]
    pub(crate) fn new(
        scope: ScopeId,
        result: Option<SyncReport>,
        error: Option<ScopeError>,
        deferred: bool,
        state: &SyncState,
    ) -> Self {
        Self {
            scope,
            result,
            error,
            deferred,
            failing_since: state.failing_since.clone(),
            notified: state.notified(),
        }
    }
}

/// The result of one run of the built-in sync.
///
/// # Examples
///
/// ```
/// use bd_cache_sync::SyncRun;
/// let run = SyncRun::busy();
/// assert!(run.busy && run.scopes.is_empty());
/// assert_eq!(serde_json::to_value(&run).unwrap()["busy"], true);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SyncRun {
    /// The drain; `None` when the run did not get as far.
    pub drain: Option<DrainReport>,
    /// One result per configured scope, in configuration order.
    pub scopes: Vec<ScopeResult>,
    /// The outage, while one holds (ADR-BDC-0005).
    pub outage: Option<Outage>,
    /// The run was asked to stop (`StopFlag`) and ended early.
    pub stopped: bool,
    /// The cache was in use by an earlier run's blocking task; this run did
    /// nothing (REQ-BDC-0703).
    pub busy: bool,
    /// The cache's mutex had been poisoned by a panic; the cache it held was
    /// dropped and this run used a recomposed one (S55).
    pub poisoned: bool,
}

impl SyncRun {
    /// A run that has done nothing yet.
    #[must_use]
    pub(crate) fn started() -> Self {
        Self {
            drain: None,
            scopes: Vec::new(),
            outage: None,
            stopped: false,
            busy: false,
            poisoned: false,
        }
    }

    /// A run skipped because an earlier run still holds the cache.
    #[must_use]
    pub fn busy() -> Self {
        let mut run = Self::started();
        run.busy = true;
        run
    }

    /// The scopes whose sync failed with an error, in order.
    pub fn failures(&self) -> impl Iterator<Item = &ScopeResult> {
        self.scopes.iter().filter(|s| s.error.is_some())
    }
}
