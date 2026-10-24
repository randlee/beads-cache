//! `drain`: apply journaled writes in seq order under the drain lock (REQ-BDC-0401..0406).
//!
//! Owned by sprint a-5, which replaces the stub below; the signature is
//! the contract `lib.rs` delegates to (sprint a-1, Public contract).

use bd_cache_model::{CacheError, DrainBudget, DrainReport};

use crate::Cache;

use bd_cache_model::{CacheStore, DrainLock};
use sc_beads::CacheBackend;

/// Stub until sprint a-5.
#[expect(
    clippy::unimplemented,
    reason = "skeleton: sprint a-5 fills this function"
)]
pub(crate) fn drain<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    budget: &DrainBudget,
) -> Result<DrainReport, CacheError> {
    let _ = (cache, budget);
    unimplemented!("drain::drain: sprint a-5")
}
