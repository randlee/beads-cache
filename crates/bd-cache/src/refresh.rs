//! `refresh`: a fresh read stored with force, overlay applied (REQ-BDC-0907).
//!
//! Owned by sprint a-6, which replaces the stub below; the signature is
//! the contract `lib.rs` delegates to (sprint a-1, Public contract).

use std::time::Duration;

use bd_cache_model::{CacheError, RowView};
use sc_beads::BeadId;

use crate::Cache;

use bd_cache_model::{CacheStore, DrainLock};
use sc_beads::CacheBackend;

/// Stub until sprint a-6.
#[expect(
    clippy::unimplemented,
    reason = "skeleton: sprint a-6 fills this function"
)]
pub(crate) fn refresh<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    ids: &[BeadId],
    timeout: Duration,
) -> Result<Vec<RowView>, CacheError> {
    let _ = (cache, ids, timeout);
    unimplemented!("refresh::refresh: sprint a-6")
}
