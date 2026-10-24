//! The `call` helper and the outage record (ADR-BDC-0005).
//!
//! Every backend call goes through [`call`]: it hands the backend the earlier
//! of the caller's hard deadline and the per-call cap, and classifies a
//! failure ([`classify`]):
//!
//! | Failure | Outcome |
//! |---|---|
//! | timed out and the hard deadline has expired | `Cut`: no outage, no attempt |
//! | unreachable (timeout, refused, cannot start) on `Check`, `PreRead`, `Sync`, `Refresh`, `MissFill`, `Ready` | `Outage`, recorded |
//! | unreachable on `Write` or `ReadBack` | `Error(BD_UNAVAILABLE)`: a write counts an attempt |
//! | anything else | `Error`, mapped by [`CacheError::from_backend`] |
//!
//! An answer from a list call (`PreRead`, `ReadBack`, `Sync`, `Refresh`,
//! `MissFill`, `Ready`) clears the outage; the backend check alone never does,
//! because `bd version` can pass while lists time out on a loaded host.

use bd_cache_model::{CacheError, CacheStore, CallKind, CallOutcome, DrainLock, Outage};
use sc_beads::{BeadsCacheError, CacheBackend, Deadline, ProjectRef};

use crate::Cache;

/// Classifies a failed backend call.
pub(crate) fn classify(kind: CallKind, e: &BeadsCacheError, hard_expired: bool) -> CallOutcome {
    if e.is_timeout() && hard_expired {
        return CallOutcome::Cut;
    }
    let mapped = CacheError::from_backend(e, kind);
    if e.is_unavailable() && kind.unavailable_is_outage() {
        CallOutcome::Outage(mapped.to_string())
    } else {
        CallOutcome::Error(mapped)
    }
}

/// Runs one backend call under `hard` and the per-call cap, and classifies it.
///
/// `f` receives the backend, the project and the deadline to stop at. An
/// outage is recorded before returning; a list that answers clears one.
pub(crate) fn call<S: CacheStore, L: DrainLock, B: CacheBackend, T>(
    cache: &mut Cache<S, L, B>,
    kind: CallKind,
    hard: Deadline,
    f: impl FnOnce(&B, &ProjectRef, Deadline) -> Result<T, BeadsCacheError>,
) -> Result<T, CallOutcome> {
    let cap = Deadline::after(&*cache.clock, cache.config.call_timeout);
    let d = hard.earliest(cap);
    match f(&cache.backend, &cache.config.project, d) {
        Ok(value) => {
            if kind.is_list() {
                note_ok(cache).map_err(CallOutcome::Error)?;
            }
            Ok(value)
        }
        Err(e) => {
            let outcome = classify(kind, &e, hard.expired(&*cache.clock));
            if let CallOutcome::Outage(text) = &outcome {
                note_unavailable(cache, text).map_err(CallOutcome::Error)?;
            }
            Err(outcome)
        }
    }
}

/// Records an unreachable backend: keeps the outage's start, stores the latest error.
pub(crate) fn note_unavailable<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    err: &str,
) -> Result<(), CacheError> {
    let since = match cache.store.outage()? {
        Some(o) => o.since,
        None => cache.clock.now_utc(),
    };
    cache.store.set_outage(Some(&Outage {
        since,
        last_error: err.to_owned(),
    }))
}

/// Clears the outage after a list answered.
pub(crate) fn note_ok<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
) -> Result<(), CacheError> {
    if cache.store.outage()?.is_some() {
        cache.store.set_outage(None)?;
    }
    Ok(())
}
