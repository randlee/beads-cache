//! `list` of a known scope (REQ-BDC-0203, 0902, 0909).
//!
//! Owned by sprint a-6, which replaces the stub below; the signature is
//! the contract `lib.rs` delegates to (sprint a-1, Public contract).

use bd_cache_model::{CacheError, ListReply, ScopeId};

use crate::Cache;

use bd_cache_model::{CacheStore, DrainLock};
use sc_beads::CacheBackend;

/// Stub until sprint a-6.
#[expect(
    clippy::unimplemented,
    reason = "skeleton: sprint a-6 fills this function"
)]
pub(crate) fn list<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    s: &ScopeId,
    status: Option<&str>,
    label: Option<&str>,
) -> Result<ListReply, CacheError> {
    let _ = (cache, s, status, label);
    unimplemented!("list::list: sprint a-6")
}
