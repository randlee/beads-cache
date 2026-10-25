//! `write`: validate, dedupe, check the precondition, journal (REQ-BDC-0002, 0004, 0302).
//!
//! The steps run in this order (`docs/bd-cache/write-path.md`, REQ-CORE-0101):
//!
//! 1. [`check_write_size`], then [`Op::parse_new`]: `BAD_ARGS` for args or a key
//!    over its size limit ([`MAX_WRITE_ARGS_BYTES`](bd_cache_model::MAX_WRITE_ARGS_BYTES),
//!    [`MAX_IDEM_KEY_BYTES`](bd_cache_model::MAX_IDEM_KEY_BYTES)), then
//!    `BAD_OP` / `BAD_ARGS`, before anything else, so an invalid write never
//!    starts a bd call and is never journaled.
//! 2. Dedupe on the idempotency key ([`CacheStore::entry_by_key`], any
//!    state): the same `(bead, op)` returns the existing entry with
//!    `duplicate: true`; anything else under that key is `IDEM_CONFLICT`.
//!    Dedupe runs before the precondition, so a retry of a write whose first
//!    attempt already changed the view is still a duplicate.
//! 3. The current view (`view::current_view`), or on a miss one bounded
//!    fill (`miss::fill` under `miss_fill_timeout`); a bead in neither
//!    cache nor bd is `NOT_FOUND`. This is the only bd call `write` makes,
//!    so a write to a cached bead is journaled while the backend is
//!    unreachable, and one to an uncached bead is `BD_UNAVAILABLE` within
//!    the miss-fill timeout (REQ-BDC-0903).
//! 4. The precondition against that view ([`unmet`]): `PRECONDITION` with
//!    `unmet {key: {expected, actual}}`, nothing journaled.
//! 5. [`CacheStore::insert_entry`] with the scope from the view's labels
//!    ([`ScopeId::from_labels`]). If another process took the key after
//!    step 2, the store answers `IDEM_CONFLICT` and the key is deduped again
//!    against the entry that won.

use std::collections::BTreeMap;

use bd_cache_model::{
    CacheError, CacheStore, DrainLock, GuardKey, IdemKey, JournalEntry, NewEntry, Op,
    PreconditionReason, ScopeId, Unmet, WriteReply, check_write_size, replay, unmet,
};
use sc_beads::{Actor, Bead, BeadId, CacheBackend, Guards};

use crate::Cache;

/// Journals one write; the module docs give the order of the checks.
pub(crate) fn write<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    id: &BeadId,
    op_name: &str,
    args_json: &str,
    pre: Guards,
    key: IdemKey,
    actor: Actor,
) -> Result<WriteReply, CacheError> {
    check_write_size(args_json, &key)?;
    let op = Op::parse_new(op_name, args_json)?;
    if let Some(held) = cache.store.entry_by_key(&key)? {
        return dedupe(&held, id, &op);
    }
    let view = current_view_or_fill(cache, id)?;
    let unmet = unmet(&view, &pre);
    if !unmet.is_empty() {
        return Err(CacheError::Precondition {
            detail: format!(
                "precondition unmet for {id} at write time: {}; nothing journaled",
                describe_unmet(&unmet)
            ),
            unmet,
            reason: PreconditionReason::PreconditionFailed,
            source: None,
        });
    }
    let entry = NewEntry {
        idem_key: key,
        scope: ScopeId::from_labels(&view.labels),
        bead: id.clone(),
        op: op.clone(),
        precondition: pre,
        actor,
    };
    let at = cache.clock.now_utc();
    match cache.store.insert_entry(entry, &at) {
        Ok(seq) => Ok(WriteReply {
            seq,
            duplicate: false,
        }),
        // Another process journaled this key after our lookup: dedupe against it.
        Err(CacheError::IdemConflict { seq, detail }) => match cache.store.entry_by_seq(seq)? {
            Some(held) => dedupe(&held, id, &op),
            None => Err(CacheError::IdemConflict { detail, seq }),
        },
        Err(e) => Err(e),
    }
}

/// The reply for a key that already journals `held` (REQ-BDC-0004).
///
/// A write's identity is `(bead, op)`, where the [`Op`] carries its args;
/// the precondition and the actor are not part of it.
fn dedupe(held: &JournalEntry, id: &BeadId, op: &Op) -> Result<WriteReply, CacheError> {
    if held.bead == *id && held.op == *op {
        return Ok(WriteReply {
            seq: held.seq,
            duplicate: true,
        });
    }
    Err(CacheError::IdemConflict {
        detail: format!(
            "idem_key {} already journals entry {} ({} {} on {}); this write is {} {} on {id}",
            held.idem_key,
            held.seq,
            held.op.name(),
            held.op.to_args_json(),
            held.bead,
            op.name(),
            op.to_args_json(),
        ),
        seq: held.seq,
    })
}

/// The bead as reads see it, filling a miss with one bounded `bd list --id`.
fn current_view_or_fill<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    id: &BeadId,
) -> Result<Bead, CacheError> {
    if let Some((view, _)) = crate::view::current_view(cache, id)? {
        return Ok(view);
    }
    let Some(row) = crate::miss::fill(cache, id, None)? else {
        return Err(CacheError::NotFound {
            detail: format!("bead {id} is in neither the cache nor bd"),
        });
    };
    // A rebuild drops rows but keeps the journal, so an uncached bead can
    // still have pending entries: replay them as `current_view` would.
    let pending = cache.store.pending_for(id)?;
    Ok(replay(&row, &pending))
}

/// Renders unmet keys as `status: expected "open", actual "closed"; ...`.
pub(crate) fn describe_unmet(unmet: &BTreeMap<GuardKey, Unmet>) -> String {
    unmet
        .iter()
        .map(|(k, u)| format!("{k}: expected {:?}, actual {:?}", u.expected, u.actual))
        .collect::<Vec<_>>()
        .join("; ")
}
