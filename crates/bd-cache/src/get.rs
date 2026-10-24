//! `get` and `entry` (REQ-BDC-0001, 0202, 0908).
//!
//! Owned by sprint a-6, which replaces the stub below; the signature is
//! the contract `lib.rs` delegates to (sprint a-1, Public contract).

use std::time::Duration;

use bd_cache_model::{CacheError, GetReply, JournalEntry, Seq};
use sc_beads::BeadId;

use crate::Cache;

use bd_cache_model::{CacheStore, DrainLock};
use sc_beads::CacheBackend;

/// Stub until sprint a-6.
#[expect(
    clippy::unimplemented,
    reason = "skeleton: sprint a-6 fills this function"
)]
pub(crate) fn get<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    id: &BeadId,
    miss_timeout: Option<Duration>,
) -> Result<GetReply, CacheError> {
    let _ = (cache, id, miss_timeout);
    unimplemented!("get::get: sprint a-6")
}

/// Stub until sprint a-6.
#[expect(
    clippy::unimplemented,
    reason = "skeleton: sprint a-6 fills this function"
)]
pub(crate) fn entry<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    seq: Seq,
) -> Result<JournalEntry, CacheError> {
    let _ = (cache, seq);
    unimplemented!("get::entry: sprint a-6")
}
