//! `write`: validate, dedupe, check the precondition, journal (REQ-BDC-0002, 0004, 0302).
//!
//! Owned by sprint a-5, which replaces the stub below; the signature is
//! the contract `lib.rs` delegates to (sprint a-1, Public contract).

use bd_cache_model::{CacheError, IdemKey, WriteReply};
use sc_beads::{Actor, BeadId, Guards};

use crate::Cache;

use bd_cache_model::{CacheStore, DrainLock};
use sc_beads::CacheBackend;

/// Stub until sprint a-5.
#[expect(
    clippy::unimplemented,
    reason = "skeleton: sprint a-5 fills this function"
)]
pub(crate) fn write<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    id: &BeadId,
    op_name: &str,
    args_json: &str,
    pre: Guards,
    key: IdemKey,
    actor: Actor,
) -> Result<WriteReply, CacheError> {
    let _ = (cache, id, op_name, args_json, pre, key, actor);
    unimplemented!("write::write: sprint a-5")
}
