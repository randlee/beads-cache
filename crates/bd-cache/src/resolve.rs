//! `resolve` of an escalated entry (REQ-BDC-0601).
//!
//! Owned by sprint a-6, which replaces the stub below; the signature is
//! the contract `lib.rs` delegates to (sprint a-1, Public contract).

use bd_cache_model::{CacheError, Seq};
use sc_beads::Actor;

use crate::Cache;

use bd_cache_model::{CacheStore, DrainLock};
use sc_beads::CacheBackend;

/// Stub until sprint a-6.
#[expect(
    clippy::unimplemented,
    reason = "skeleton: sprint a-6 fills this function"
)]
pub(crate) fn resolve<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    seq: Seq,
    note: &str,
    actor: Actor,
) -> Result<(), CacheError> {
    let _ = (cache, seq, note, actor);
    unimplemented!("resolve::resolve: sprint a-6")
}
