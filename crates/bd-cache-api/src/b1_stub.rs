//! B1-STUB: an interim adapter for sprint b-1's core additions.
//!
//! Sprint b-1 (`docs/plans/phase-b/sprint-b-1-core-store.md`, Public
//! contract) adds `Cache::write_returning`, `Cache::entry_by_key`,
//! `Cache::journal_counts`, `WriteOutcome`, `StateCounts`, `JournalCounts`,
//! `GetReply.scope` and the `JournalEntry` fields `last_error_code`, `note`
//! and `changed`. It is built in parallel, so until it merges this module
//! provides the same signatures over today's facade, reaching the store
//! through `bd_cache::testing`. **Delete this module once b-1 lands**, and
//! point every caller at the b-1 items:
//!
//! | Here | b-1 |
//! |---|---|
//! | [`WriteOutcome`], [`StateCounts`], [`JournalCounts`] | `bd_cache_model::{WriteOutcome, StateCounts, JournalCounts}` |
//! | [`write_returning`] | `Cache::write_returning` |
//! | [`entry_by_key`] | `Cache::entry_by_key` |
//! | [`journal_counts`] | `Cache::journal_counts` |
//! | [`get_scope`] | `GetReply.scope` |
//! | [`extras`] | `JournalEntry.{last_error_code, note, changed}` |
//!
//! Two items are **not** in b-1's Public contract, so the lead must place
//! them (in b-1 or a later sprint) before this module can go:
//! [`pending_entries`] (`cache journal list --state pending` needs a facade
//! read of the pending entries) and [`backend_check`] (`cache health
//! --probe` needs a public backend check).
//!
//! What the stub cannot do as b-1 will: it does not run in one store
//! transaction, and it does not store `changed`. On a duplicate it
//! recomputes the original `changed` from the stored row and the entries
//! journaled before the original one, which equals the stored value while
//! those entries are still pending (every test here).

use std::collections::BTreeMap;

use bd_cache::Cache;
use bd_cache_model::{
    CLOSED_STATUS, CacheError, CacheStore, DrainLock, GetReply, IdemKey, JournalEntry,
    JournalState, Op, RowView, ScopeId, check_write_size, replay,
};
use sc_beads::{Actor, Bead, BeadId, CacheBackend, Guards};

/// b-1's `WriteOutcome`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WriteOutcome {
    /// The entry as stored, after this call.
    pub(crate) entry: JournalEntry,
    /// The view before the write.
    pub(crate) before: RowView,
    /// The view after the write.
    pub(crate) after: RowView,
    /// The key already journaled this write.
    pub(crate) duplicate: bool,
    /// Stored with the entry; on a duplicate, the stored value.
    pub(crate) changed: bool,
}

/// b-1's `StateCounts`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct StateCounts {
    pub(crate) pending: u64,
    pub(crate) applied: u64,
    pub(crate) escalated: u64,
    pub(crate) resolved: u64,
}

impl StateCounts {
    fn add(&mut self, state: JournalState, n: u64) {
        let slot = match state {
            JournalState::Pending => &mut self.pending,
            JournalState::Applied => &mut self.applied,
            JournalState::Escalated => &mut self.escalated,
            JournalState::Resolved => &mut self.resolved,
        };
        *slot += n;
    }
}

/// b-1's `JournalCounts`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct JournalCounts {
    pub(crate) total: StateCounts,
    pub(crate) by_scope: BTreeMap<ScopeId, StateCounts>,
    pub(crate) unscoped: StateCounts,
}

/// b-1's `JournalEntry` additions, derived from `last_error` the way b-1
/// reads a Python-written entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryExtras {
    /// The error code, from the `CODE: detail` prefix.
    pub(crate) last_error_code: Option<String>,
    /// `last_error` without a trailing resolve note.
    pub(crate) last_error: Option<String>,
    /// The resolve note, split out of `" | resolved by <actor>: <note>"`.
    pub(crate) note: Option<String>,
}

/// The separator `resolve` appends before the actor and note.
const RESOLVED_BY: &str = " | resolved by ";

/// b-1's `JournalEntry.{last_error_code, note}`.
pub(crate) fn extras(e: &JournalEntry) -> EntryExtras {
    let Some(text) = e.last_error.as_deref() else {
        return EntryExtras {
            last_error_code: None,
            last_error: None,
            note: None,
        };
    };
    let (error, note) = match text.rfind(RESOLVED_BY) {
        Some(at) if e.state == JournalState::Resolved => {
            let (head, tail) = text.split_at(at);
            let tail = tail.get(RESOLVED_BY.len()..).unwrap_or("");
            let note = tail.split_once(": ").map_or(tail, |(_, n)| n);
            (head, Some(note.to_owned()))
        }
        _ => (text, None),
    };
    let error = (!error.is_empty()).then(|| error.to_owned());
    let last_error_code = error
        .as_deref()
        .and_then(|t| t.split_once(": "))
        .map(|(code, _)| code)
        .filter(|code| code.starts_with(crate::CODE_PREFIX))
        .map(str::to_owned);
    EntryExtras {
        last_error_code,
        last_error: error,
        note,
    }
}

/// b-1's `GetReply.scope`: the bead's scope, `None` when it has none.
pub(crate) fn get_scope<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    reply: &GetReply,
) -> Result<Option<ScopeId>, CacheError> {
    let Some(s) = ScopeId::from_labels(&reply.row.bead.labels) else {
        return Ok(None);
    };
    let known = cache.config().scopes.contains(&s) || cache.scope_synced(&s)?;
    Ok(known.then_some(s))
}

/// b-1's `Cache::entry_by_key`: `NOT_FOUND` when absent.
pub(crate) fn entry_by_key<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    key: &IdemKey,
) -> Result<JournalEntry, CacheError> {
    bd_cache::testing::store(cache)
        .entry_by_key(key)?
        .ok_or_else(|| CacheError::NotFound {
            detail: format!("no journal entry holds idempotency key {key}"),
        })
}

/// The pending entries, by seq. Not in b-1's Public contract (module docs).
pub(crate) fn pending_entries<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
) -> Result<Vec<JournalEntry>, CacheError> {
    bd_cache::testing::store(cache).pending_entries()
}

/// b-1's `Cache::journal_counts`, from the status' `<phase>:<state>` counts.
pub(crate) fn journal_counts<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
) -> Result<JournalCounts, CacheError> {
    let mut counts = JournalCounts::default();
    for (key, n) in cache.status()?.store.journal {
        let Some((phase, state)) = key.rsplit_once(':') else {
            continue;
        };
        let Some(state) = JournalState::parse(state) else {
            continue;
        };
        counts.total.add(state, n);
        match ScopeId::phase(phase).ok().filter(|_| phase != "None") {
            Some(s) => counts.by_scope.entry(s).or_default().add(state, n),
            None => counts.unscoped.add(state, n),
        }
    }
    Ok(counts)
}

/// b-1's `Cache::write_returning`, over `Cache::write` (module docs).
pub(crate) fn write_returning<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    id: &BeadId,
    op_name: &str,
    args_json: &str,
    pre: Guards,
    key: IdemKey,
    actor: Actor,
) -> Result<WriteOutcome, CacheError> {
    // The write's own first checks, so an invalid write is refused before
    // the stub reads the view (as `Cache::write` orders them).
    check_write_size(args_json, &key)?;
    let op = Op::parse_new(op_name, args_json)?;
    let held = bd_cache::testing::store(cache).entry_by_key(&key)?;
    let before = match (&held, cache.cached(id)?) {
        (_, Some(r)) => Some(r.row),
        // A miss: `get` fills it as `write` would, with the same errors.
        (None, None) => Some(cache.get(id, None)?.row),
        (Some(_), None) => None,
    };
    let reply = cache.write(id, op_name, args_json, pre, key, actor)?;
    let entry = cache.entry(reply.seq)?;
    let after = match cache.cached(id)? {
        Some(r) => r.row,
        None => cache.get(id, None)?.row,
    };
    let before = before.unwrap_or_else(|| after.clone());
    let changed = if reply.duplicate {
        original_changed(cache, &entry)?
    } else {
        changed_by(&op, &before.bead)
    };
    Ok(WriteOutcome {
        entry,
        before,
        after,
        duplicate: reply.duplicate,
        changed,
    })
}

/// What b-1 stores as `changed` for `op` written over the view `before`:
/// a close stores `!already_closed`, any other op whether the overlay
/// changes the view.
fn changed_by(op: &Op, before: &Bead) -> bool {
    match op {
        Op::Close { .. } => before.status.as_deref() != Some(CLOSED_STATUS),
        Op::Update(_) | Op::DepAdd { .. } => op.overlay(before) != *before,
    }
}

/// The `changed` the original entry was journaled with: the stored row with
/// the entries journaled before it replayed is the view it was written over.
fn original_changed<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    held: &JournalEntry,
) -> Result<bool, CacheError> {
    let store = bd_cache::testing::store(cache);
    let Some((row, _)) = store.get_row(&held.bead)? else {
        return Ok(false);
    };
    let earlier: Vec<JournalEntry> = store
        .pending_for(&held.bead)?
        .into_iter()
        .filter(|e| e.seq < held.seq)
        .collect();
    Ok(changed_by(&held.op, &replay(&row, &earlier)))
}

/// The backend check for `cache health --probe`: `(outcome, model_current)`.
/// Not in b-1's Public contract (module docs): the facade has no public
/// backend check, so the lead must add one before this module can go.
pub(crate) fn backend_check<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    hard: sc_beads::Deadline,
) -> Result<(bd_cache_model::FingerprintOutcome, bool), CacheError> {
    bd_cache::testing::backend_check(cache, hard)
        .map_err(|f| f.into_error("bd version / bd schema: no answer in time"))
}
