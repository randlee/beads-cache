//! `resolve` of an escalated entry (REQ-BDC-0601).
//!
//! The lead set the bead by hand after an escalation; the entry becomes
//! `resolved` (never replayed, no longer blocking `prune`) and the note is
//! stored in its `note` as `resolved by <actor>: <note>`, `last_error` left as
//! it was (sprint b-1; the 0.1 `entry` dict still shows the note appended to
//! `last_error`, as the v3 Python writer writes it). The
//! state rule is the store's (`CacheStore::resolve`): any entry that is not
//! escalated, or an absent seq, is `NOT_ESCALATED`. A note over
//! [`MAX_NOTE_BYTES`](bd_cache_model::MAX_NOTE_BYTES) is `BAD_ARGS` before the
//! store is touched (PE-RSH-009).

use bd_cache_model::{CacheError, CacheStore, DrainLock, Seq, check_note_size};
use sc_beads::{Actor, CacheBackend};

use crate::Cache;

/// `resolve(seq, note, actor)` at the clock's UTC time: see the module docs.
#[expect(
    clippy::needless_pass_by_value,
    reason = "signature fixed by the a-1 public contract (Cache::resolve takes Actor)"
)]
pub(crate) fn resolve<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    seq: Seq,
    note: &str,
    actor: Actor,
) -> Result<(), CacheError> {
    check_note_size(note)?;
    let at = cache.clock.now_utc();
    cache.store.resolve(seq, note, &actor, &at)
}
