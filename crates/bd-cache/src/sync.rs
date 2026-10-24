//! `sync` of one scope, including its ready base (REQ-BDC-0501..0504; ADR-BDC-0002).
//!
//! Owned by sprint a-6, which replaces the stub below; the signature is
//! the contract `lib.rs` delegates to (sprint a-1, Public contract).

use bd_cache_model::{CacheError, ScopeId, SyncReport};
use sc_beads::Deadline;

use crate::Cache;

use bd_cache_model::{CacheStore, DrainLock};
use sc_beads::CacheBackend;

/// Stub until sprint a-6.
#[expect(
    clippy::unimplemented,
    reason = "skeleton: sprint a-6 fills this function"
)]
pub(crate) fn sync<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    s: &ScopeId,
    d: Deadline,
) -> Result<SyncReport, CacheError> {
    let _ = (cache, s, d);
    unimplemented!("sync::sync: sprint a-6")
}
