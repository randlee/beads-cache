//! `ready`: the stored base adjusted for our journaled writes, or a bounded fresh answer (REQ-BDC-0911, 0912).
//!
//! Owned by sprint a-6, which replaces the stub below; the signature is
//! the contract `lib.rs` delegates to (sprint a-1, Public contract).

use std::time::Duration;

use bd_cache_model::{CacheError, ReadyReply, ScopeId};

use crate::Cache;

use bd_cache_model::{CacheStore, DrainLock};
use sc_beads::CacheBackend;

/// Stub until sprint a-6.
#[expect(
    clippy::unimplemented,
    reason = "skeleton: sprint a-6 fills this function"
)]
pub(crate) fn ready<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    s: &ScopeId,
    fresh_within: Option<Duration>,
) -> Result<ReadyReply, CacheError> {
    let _ = (cache, s, fresh_within);
    unimplemented!("ready::ready: sprint a-6")
}
