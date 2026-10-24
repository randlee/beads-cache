//! `prune` of a scope (REQ-BDC-0602).
//!
//! Owned by sprint a-6, which replaces the stub below; the signature is
//! the contract `lib.rs` delegates to (sprint a-1, Public contract).

use bd_cache_model::{CacheError, PruneReport, ScopeId};

use crate::Cache;

use bd_cache_model::{CacheStore, DrainLock};
use sc_beads::CacheBackend;

/// Stub until sprint a-6.
#[expect(
    clippy::unimplemented,
    reason = "skeleton: sprint a-6 fills this function"
)]
pub(crate) fn prune<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    s: &ScopeId,
) -> Result<PruneReport, CacheError> {
    let _ = (cache, s);
    unimplemented!("prune::prune: sprint a-6")
}
