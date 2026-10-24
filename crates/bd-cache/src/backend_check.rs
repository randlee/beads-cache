//! The backend check and rebuild rule (REQ-BDC-0105).
//!
//! Runs at the start of every sync and every non-empty drain: one
//! `fingerprint` call (`bd version` + sha256 of `bd schema`), then the store's
//! rebuild rule. The first check records the fingerprint and drops nothing; a
//! recorded sha that differs drops every cached row, edge, membership, sync
//! cursor and ready base and keeps the journal. `model_current` says whether
//! the live schema is the one the strict rows were generated from.

use bd_cache_model::{CacheStore, CallKind, CallOutcome, DrainLock, FingerprintOutcome};
use sc_beads::{BD_SCHEMA_SHA256, CacheBackend, Deadline};

use crate::Cache;

/// Fingerprints the backend under `hard` and applies the rebuild rule.
///
/// Returns what the store did and `model_current`. A failed check is the
/// classified [`CallOutcome`] (an unreachable backend is an outage; the check
/// never clears one).
pub(crate) fn run<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    hard: Deadline,
) -> Result<(FingerprintOutcome, bool), CallOutcome> {
    let f = crate::call(cache, CallKind::Check, hard, B::fingerprint)?;
    let outcome = cache
        .store
        .record_fingerprint(&f)
        .map_err(CallOutcome::Error)?;
    Ok((outcome, f.schema_sha256 == BD_SCHEMA_SHA256))
}
