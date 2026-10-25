//! `write` and `write_returning`: validate, dedupe, check the precondition,
//! journal (REQ-BDC-0002, 0004, 0302, 0809).
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
//!    attempt already changed the view is still a duplicate. Guards and
//!    actor are not compared (contract §5.1).
//! 3. The current view (`view::current_view`), or on a miss one bounded
//!    fill (`miss::fill` under `miss_fill_timeout`); a bead in neither
//!    cache nor bd is `NOT_FOUND`. This is the only bd call `write` makes,
//!    so a write to a cached bead is journaled while the backend is
//!    unreachable, and one to an uncached bead is `BD_UNAVAILABLE` within
//!    the miss-fill timeout (REQ-BDC-0903).
//! 4. The precondition against that view ([`unmet`]): `PRECONDITION` with
//!    `unmet {key: {expected, actual}}`, nothing journaled.
//! 5. `changed` against that view ([`Op::changes`]: for a close
//!    `!already_closed`, else "the overlay changes the view").
//! 6. [`CacheStore::insert_entry`] with the scope from the view's labels
//!    ([`ScopeId::from_labels`]) and `changed`: one store transaction, so no
//!    reader sees the entry without its `changed`. If another process took
//!    the key after step 2, the store answers `IDEM_CONFLICT` and the key is
//!    deduped again against the entry that won.
//!
//! `write_returning` also answers the views (REQ-BDC-0809): `before` is the
//! row step 3's view gives, read before step 6; `after` is the row a `get`
//! answers right after the write. A duplicate returns the stored entry and
//! its stored `changed` (never recomputed: the entry's overlay is already in
//! the view), with `before` and `after` both the current view. `write` is the
//! same journaling without the views, so a duplicate still needs no row.
use std::collections::BTreeMap;

use bd_cache_model::{
    CacheError, CacheStore, DrainLock, Freshness, GuardKey, IdemKey, JournalEntry, NewEntry, Op,
    PreconditionReason, RowView, ScopeId, Seq, Unmet, WriteOutcome, WriteReply, check_write_size,
    replay, unmet,
};
use sc_beads::{Actor, Bead, BeadId, CacheBackend, Guards};

use crate::Cache;
use crate::get::row_in_scope;

/// Whether a journaling reads the bead's views for a reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Views {
    /// `write_returning`: read `before` ahead of the insert.
    Read,
    /// `write`: no views; a duplicate needs no row.
    Skip,
}

/// What journaling one write did.
struct Journaled {
    /// The entry: new, or the one the key already journals.
    seq: Seq,
    /// True when the key already journaled this write.
    duplicate: bool,
    /// With [`Views::Read`], for a new entry: the row before the write.
    before: Option<RowView>,
}

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
    let j = journal(cache, id, op_name, args_json, pre, key, actor, Views::Skip)?;
    Ok(WriteReply {
        seq: j.seq,
        duplicate: j.duplicate,
    })
}

/// Journals one write and answers the entry with the views around it (REQ-BDC-0809).
pub(crate) fn write_returning<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    id: &BeadId,
    op_name: &str,
    args_json: &str,
    pre: Guards,
    key: IdemKey,
    actor: Actor,
) -> Result<WriteOutcome, CacheError> {
    let j = journal(cache, id, op_name, args_json, pre, key, actor, Views::Read)?;
    let entry = cache
        .store
        .entry_by_seq(j.seq)?
        .ok_or_else(|| CacheError::NotFound {
            detail: format!("journal entry {} is absent after the write", j.seq),
        })?;
    let (view, freshness) = current_view_or_fill(cache, id)?;
    let (after, _) = row_in_scope(cache, view, freshness)?;
    let changed = match entry.changed {
        Some(stored) => stored,
        None => predicted_changed(cache, &entry, &after.bead)?,
    };
    let before = j.before.unwrap_or_else(|| after.clone());
    Ok(WriteOutcome {
        entry,
        before,
        after,
        duplicate: j.duplicate,
        changed,
    })
}

/// Steps 1-6 of the module docs.
#[expect(
    clippy::too_many_arguments,
    reason = "the write's own arguments, as Cache::write takes them, plus the views flag"
)]
fn journal<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    id: &BeadId,
    op_name: &str,
    args_json: &str,
    pre: Guards,
    key: IdemKey,
    actor: Actor,
    views: Views,
) -> Result<Journaled, CacheError> {
    check_write_size(args_json, &key)?;
    let op = Op::parse_new(op_name, args_json)?;
    if let Some(held) = cache.store.entry_by_key(&key)? {
        return dedupe(&held, id, &op);
    }
    let (view, freshness) = current_view_or_fill(cache, id)?;
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
    let changed = op.changes(&view);
    let scope = ScopeId::from_labels(&view.labels);
    let before = match views {
        Views::Read => Some(row_in_scope(cache, view, freshness)?.0),
        Views::Skip => None,
    };
    let entry = NewEntry {
        idem_key: key,
        scope,
        bead: id.clone(),
        op: op.clone(),
        precondition: pre,
        actor,
        changed: Some(changed),
    };
    let at = cache.clock.now_utc();
    match cache.store.insert_entry(entry, &at) {
        Ok(seq) => Ok(Journaled {
            seq,
            duplicate: false,
            before,
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
fn dedupe(held: &JournalEntry, id: &BeadId, op: &Op) -> Result<Journaled, CacheError> {
    if held.bead == *id && held.op == *op {
        return Ok(Journaled {
            seq: held.seq,
            duplicate: true,
            before: None,
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

/// `changed` of an entry journaled without one (by `bd_cache.py`): predicted
/// against the stored row with only the pending entries before it replayed,
/// the view that write saw. `current` stands in when the row is gone.
fn predicted_changed<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    entry: &JournalEntry,
    current: &Bead,
) -> Result<bool, CacheError> {
    let Some((row, _)) = cache.store.get_row(&entry.bead)? else {
        return Ok(entry.op.changes(current));
    };
    let earlier: Vec<JournalEntry> = cache
        .store
        .pending_for(&entry.bead)?
        .into_iter()
        .filter(|e| e.seq < entry.seq)
        .collect();
    Ok(entry.op.changes(&replay(&row, &earlier)))
}

/// The bead as reads see it, filling a miss with one bounded `bd list --id`.
fn current_view_or_fill<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    id: &BeadId,
) -> Result<(Bead, Freshness), CacheError> {
    if let Some(found) = crate::view::current_view(cache, id)? {
        return Ok(found);
    }
    let Some(row) = crate::miss::fill(cache, id, None)? else {
        return Err(CacheError::NotFound {
            detail: format!("bead {id} is in neither the cache nor bd"),
        });
    };
    // A rebuild drops rows but keeps the journal, so an uncached bead can
    // still have pending entries: replay them as `current_view` would.
    let pending = cache.store.pending_for(id)?;
    Ok((replay(&row, &pending), Freshness::Fresh))
}

/// Renders unmet keys as `status: expected "open", actual "closed"; ...`.
pub(crate) fn describe_unmet(unmet: &BTreeMap<GuardKey, Unmet>) -> String {
    unmet
        .iter()
        .map(|(k, u)| format!("{k}: expected {:?}, actual {:?}", u.expected, u.actual))
        .collect::<Vec<_>>()
        .join("; ")
}
