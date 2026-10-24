//! `ready` and the adjusted ready base (REQ-BDC-0904, 0911, 0912; ADR-BDC-0002).
//!
//! bd's answer is the base: each sync of a scope ends with `bd blocked -l`
//! and `bd ready -l -n 0`, stored with the instant taken before the first of
//! the two calls (`fetched_at`). Reads apply our own journal to that base
//! without a bd call ([`adjusted`]): every `Pending` entry plus every entry
//! applied at or after `fetched_at`, in seq order; escalated and resolved
//! entries are skipped. The rules (ADR-CORE-0201 in
//! `docs/bd-cache/read-path.md`, one test per row in `tests/read_ready.rs`):
//!
//! | Entry on bead X | Effect |
//! |---|---|
//! | `Close`, or `Update` to `closed` | X leaves `ready` and the blocked set and is removed from every `blocked_by`; a bead whose list empties is unblocked |
//! | `Update` to another status | X leaves `ready`, unless that was already its status |
//! | the same, when X was `closed` or its prior status is unknown (a reopen) | X joins `blocked_by` of every bead with a blocking, non-`parent-child` edge onto X; X's own blockers come back from its cached edges |
//! | `DepAdd` X → T of a blocking type other than `parent-child` | T joins X's `blocked_by` unless T's view is `closed` (an uncached T blocks) |
//! | `DepAdd` X → T of type `parent-child` | T joins X's `blocked_by` when T is blocked |
//! | a bead becomes blocked (parent block) | it joins `blocked_by` of each `parent-child` child |
//! | a bead becomes unblocked (parent unblock) | it leaves `blocked_by` of each child listing it only as parent; a bead with an empty list joins `ready` when its view is `open`, in the scope and not deferred |
//!
//! Incoming edges are the store's edge index plus the edges of pending
//! `dep_add` entries, as the views see them.
//!
//! Every rule errs towards "not ready": a bead the rules cannot settle stays
//! out of `ready` until the next sync (REQ-BDC-0904).

use std::collections::BTreeMap;
use std::time::Duration;

use bd_cache_model::{
    CacheError, CacheStore, CallFailure, CallKind, DrainLock, Freshness, JournalEntry,
    JournalState, Op, ReadyBase, ReadyBasis, ReadyReply, RowView, ScopeId, replay,
};
use sc_beads::{
    Bead, BeadId, BlockedRow, CacheBackend, Deadline, DepType, Edge, Timestamp, rfc3339_to_unix,
};

use crate::Cache;

const CLOSED: &str = "closed";
const OPEN: &str = "open";

/// A scope's ready base with our journal applied (ADR-BDC-0002).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Adjusted {
    scope: ScopeId,
    /// The base's fetch instant.
    pub(crate) fetched_at: Timestamp,
    /// The base's stale mark.
    pub(crate) stale: bool,
    blocked: BTreeMap<BeadId, Vec<BeadId>>,
    /// Ready ids: bd's order, then the beads our journal made ready.
    pub(crate) ready: Vec<BeadId>,
    /// True when the journal changed the base.
    pub(crate) changed: bool,
}

impl Adjusted {
    /// `RowView.blocked` of a view: `None` when the scope does not hold the
    /// bead (it is not in this base), else its blockers (`Some([])` = not blocked).
    pub(crate) fn blocked_of(&self, view: &Bead) -> Option<Vec<BeadId>> {
        self.scope
            .holds(view)
            .then(|| self.blocked.get(&view.id).cloned().unwrap_or_default())
    }
}

/// A bead as a read returns it: its view, freshness and blocked state.
pub(crate) fn row_view(view: Bead, freshness: Freshness, adjusted: Option<&Adjusted>) -> RowView {
    let blocked = adjusted.and_then(|a| a.blocked_of(&view));
    RowView {
        bead: view,
        stale: freshness == Freshness::Stale,
        blocked,
    }
}

/// A bead as a read returns it, with its blocked state already settled.
pub(crate) fn row_view_with(
    view: Bead,
    freshness: Freshness,
    blocked: Option<Vec<BeadId>>,
) -> RowView {
    RowView {
        bead: view,
        stale: freshness == Freshness::Stale,
        blocked,
    }
}

/// `RowView.blocked` of one view in scope `s`, at a cost that does not grow with the scope.
///
/// It equals `adjusted(cache, s)` followed by [`Adjusted::blocked_of`], but
/// reads only the bead's own entry of the stored base
/// ([`CacheStore::base_entry`]) when no journal entry can have changed the
/// base: no pending entry and none applied at or after the base's fetch
/// instant. Otherwise (a write is in flight, for the few seconds until the
/// next sync) a blocker's close can unblock any bead, so the whole base is
/// adjusted. Rows outside the scope have no blocked state.
pub(crate) fn blocked_state<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    s: &ScopeId,
    view: &Bead,
) -> Result<Option<Vec<BeadId>>, CacheError> {
    let Some(entry) = cache.store.base_entry(s, &view.id)? else {
        return Ok(None);
    };
    if !s.holds(view) {
        return Ok(None);
    }
    let untouched = cache.store.pending_entries()?.is_empty()
        && cache.store.applied_since(&entry.fetched_at)?.is_empty();
    if untouched {
        return Ok(Some(entry.blocked_by));
    }
    Ok(adjusted(cache, s)?.and_then(|a| a.blocked_of(view)))
}

/// The registered scope a view belongs to: its v3 phase
/// ([`ScopeId::from_labels`]) when that scope is known, else the first known
/// scope that holds the view; `None` for a bead outside every scope.
pub(crate) fn scope_for<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    view: &Bead,
) -> Result<Option<ScopeId>, CacheError> {
    let known = cache.store.scopes()?;
    Ok(ScopeId::from_labels(&view.labels)
        .filter(|s| known.contains(s))
        .or_else(|| known.into_iter().find(|s| s.holds(view))))
}

/// `SCOPE_UNKNOWN`, naming the known scopes, unless `s` was configured or
/// synced (REQ-BDC-0902, 0909).
pub(crate) fn require_known<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    s: &ScopeId,
) -> Result<(), CacheError> {
    let known = cache.store.scopes()?;
    if known.contains(s) {
        Ok(())
    } else {
        Err(CacheError::ScopeUnknown {
            detail: format!("phase {s} was never configured or synced"),
            known,
        })
    }
}

/// The scope's stored base with our journal applied; `None` when the scope
/// has no base yet. Starts no bd call.
pub(crate) fn adjusted<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    s: &ScopeId,
) -> Result<Option<Adjusted>, CacheError> {
    let Some(base) = cache.store.ready_base(s)? else {
        return Ok(None);
    };
    let pending = cache.store.pending_entries()?;
    let mut entries = cache.store.applied_since(&base.fetched_at)?;
    entries.extend(pending.iter().cloned());
    entries.sort_by_key(|e| e.seq);
    entries.dedup_by_key(|e| e.seq);
    let now_unix = cache.clock.now_utc().unix();
    let mut engine = Engine::new(&cache.store, s, &base, &pending, now_unix);
    for e in &entries {
        engine.apply(e)?;
    }
    let (blocked, ready) = (engine.blocked, engine.ready);
    let changed = blocked != base_blocked(&base) || ready != base_ready(&base);
    Ok(Some(Adjusted {
        scope: s.clone(),
        fetched_at: base.fetched_at,
        stale: base.stale,
        blocked,
        ready,
        changed,
    }))
}

/// bd's blocked and ready sets for a scope, with the instant taken before the first call.
#[derive(Debug)]
pub(crate) struct FreshBase {
    blocked: Vec<BlockedRow>,
    ready: Vec<BeadId>,
    at: Timestamp,
}

impl FreshBase {
    /// Number of blocked rows.
    pub(crate) fn blocked_len(&self) -> u32 {
        u32::try_from(self.blocked.len()).unwrap_or(u32::MAX)
    }
}

/// `list_blocked` then `list_ready` for the scope, both through `call`
/// (`CallKind::Ready`) under the one deadline `hard`; `at` is taken before
/// the first call.
pub(crate) fn fetch_base<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    s: &ScopeId,
    hard: Deadline,
) -> Result<FreshBase, CallFailure> {
    let at = cache.clock.now_utc();
    let filter = s.filter();
    let blocked = crate::call(cache, CallKind::Ready, hard, |b, p, d| {
        b.list_blocked(p, &filter, d)
    })?;
    let ready = crate::call(cache, CallKind::Ready, hard, |b, p, d| {
        b.list_ready(p, &filter, d)
    })?;
    Ok(FreshBase { blocked, ready, at })
}

/// Replaces the scope's stored base with a fetched one (clears its stale mark).
pub(crate) fn store_base<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    s: &ScopeId,
    base: &FreshBase,
) -> Result<(), CacheError> {
    cache
        .store
        .replace_ready_base(s, &base.blocked, &base.ready, &base.at)
}

/// `ready(scope, fresh_within)`: the module docs, REQ-BDC-0911 and 0912.
pub(crate) fn ready<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    s: &ScopeId,
    fresh_within: Option<Duration>,
) -> Result<ReadyReply, CacheError> {
    require_known(cache, s)?;
    // The fetch instant of bd's answer, when one came in time.
    let mut fresh: Option<Timestamp> = None;
    let mut read_fallback = None;
    if let Some(limit) = fresh_within {
        // One deadline for both calls: the caller's limit expiring is a cut,
        // never an outage, and the reply never waits past it.
        let hard = Deadline::after(&*cache.clock, limit);
        match fetch_base(cache, s, hard) {
            Ok(base) => {
                store_base(cache, s, &base)?;
                fresh = Some(base.at);
            }
            Err(failure) => read_fallback = Some(fallback_reason(failure, limit)),
        }
    }
    let last_ok_at = cache.store.sync_state(s)?.last_ok_at;
    let Some(adj) = adjusted(cache, s)? else {
        return Ok(ReadyReply {
            scope: s.clone(),
            last_ok_at,
            basis: ReadyBasis::Unknown,
            locally_adjusted: false,
            ready: Vec::new(),
            read_fallback,
            missing_rows: Vec::new(),
        });
    };
    let mut ready = Vec::with_capacity(adj.ready.len());
    let mut missing_rows = Vec::new();
    for id in &adj.ready {
        match crate::view::current_view(cache, id)? {
            Some((view, freshness)) => ready.push(row_view(view, freshness, Some(&adj))),
            None => missing_rows.push(id.clone()),
        }
    }
    let basis = match fresh {
        Some(fetched_at) => ReadyBasis::Fresh { fetched_at },
        None => ReadyBasis::Cached {
            fetched_at: adj.fetched_at.clone(),
            stale: adj.stale,
        },
    };
    Ok(ReadyReply {
        scope: s.clone(),
        last_ok_at,
        basis,
        locally_adjusted: adj.changed,
        ready,
        read_fallback,
        missing_rows,
    })
}

/// Hint appended to every `read_fallback`.
const FALLBACK_HINT: &str = "retry, or run sync";

/// Why a requested fresh answer fell back to the cached one: the stable
/// code first (`BD_CACHE.BD_UNAVAILABLE` for a cut), then the detail and a
/// next step.
fn fallback_reason(failure: CallFailure, limit: Duration) -> String {
    match failure {
        CallFailure::Cut => format!(
            "BD_CACHE.BD_UNAVAILABLE: bd blocked / bd ready: no answer within {}ms; {FALLBACK_HINT}",
            limit.as_millis()
        ),
        CallFailure::Outage(e) | CallFailure::Error(e) => {
            format!("{}: {}; {FALLBACK_HINT}", e.code(), e.detail())
        }
    }
}

/// The base's blocked set as a map, without empty lists or repeated blockers.
fn base_blocked(base: &ReadyBase) -> BTreeMap<BeadId, Vec<BeadId>> {
    let mut blocked: BTreeMap<BeadId, Vec<BeadId>> = BTreeMap::new();
    for row in &base.blocked {
        let list = blocked.entry(row.id.clone()).or_default();
        for b in &row.blocked_by {
            if !list.contains(b) {
                list.push(b.clone());
            }
        }
    }
    blocked.retain(|_, l| !l.is_empty());
    blocked
}

/// The base's ready ids in bd's order, without repeats.
fn base_ready(base: &ReadyBase) -> Vec<BeadId> {
    let mut seen = std::collections::BTreeSet::new();
    let mut ready = Vec::with_capacity(base.ready.len());
    for id in &base.ready {
        if seen.insert(id) {
            ready.push(id.clone());
        }
    }
    ready
}

/// What the adjustment knows of a bead's status just before an entry.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Prior {
    /// The exact status.
    Known(String),
    /// bd listed it in the base, so it was not closed.
    NotClosed,
    /// Nothing is known; treated as possibly closed.
    Unknown,
}

impl Prior {
    fn is(&self, status: &str) -> bool {
        matches!(self, Self::Known(s) if s == status)
    }
}

/// The parts of a bead's current view the rules read.
#[derive(Debug, Clone)]
struct Facts {
    status: Option<String>,
    in_scope: bool,
    deferred: bool,
    edges: Vec<Edge>,
}

impl Facts {
    fn closed(&self) -> bool {
        self.status.as_deref() == Some(CLOSED)
    }
}

/// Applies journal entries to one scope's base.
struct Engine<'a, S: CacheStore> {
    store: &'a S,
    scope: &'a ScopeId,
    base: &'a ReadyBase,
    pending: &'a [JournalEntry],
    now_unix: i64,
    blocked: BTreeMap<BeadId, Vec<BeadId>>,
    ready: Vec<BeadId>,
    priors: BTreeMap<BeadId, Prior>,
    facts: BTreeMap<BeadId, Option<Facts>>,
    dependents: BTreeMap<BeadId, Vec<Edge>>,
}

impl<'a, S: CacheStore> Engine<'a, S> {
    fn new(
        store: &'a S,
        scope: &'a ScopeId,
        base: &'a ReadyBase,
        pending: &'a [JournalEntry],
        now_unix: i64,
    ) -> Self {
        Self {
            store,
            scope,
            base,
            pending,
            now_unix,
            blocked: base_blocked(base),
            ready: base_ready(base),
            priors: BTreeMap::new(),
            facts: BTreeMap::new(),
            dependents: BTreeMap::new(),
        }
    }

    /// The bead's current view (stored row with its pending entries
    /// replayed), memoized; `None` for an uncached bead.
    fn facts(&mut self, id: &BeadId) -> Result<Option<Facts>, CacheError> {
        if let Some(f) = self.facts.get(id) {
            return Ok(f.clone());
        }
        let facts = self.store.get_row(id)?.map(|(row, _)| {
            let mine: Vec<JournalEntry> = self
                .pending
                .iter()
                .filter(|e| e.bead == *id)
                .cloned()
                .collect();
            let view = replay(&row, &mine);
            Facts {
                in_scope: self.scope.holds(&view),
                deferred: view
                    .defer_until
                    .as_deref()
                    .and_then(rfc3339_to_unix)
                    .is_some_and(|t| t > self.now_unix),
                status: view.status,
                edges: view.dependencies,
            }
        });
        self.facts.insert(id.clone(), facts.clone());
        Ok(facts)
    }

    /// Incoming edges of a bead as the views see them, memoized: the store's
    /// edge index plus the edges of pending `dep_add` entries onto the bead,
    /// which replay adds to the views but the index does not hold yet.
    fn dependents(&mut self, id: &BeadId) -> Result<Vec<Edge>, CacheError> {
        if let Some(d) = self.dependents.get(id) {
            return Ok(d.clone());
        }
        let mut d = self.store.dependents_of(id)?;
        for e in self.pending {
            if let Op::DepAdd { depends_on, kind } = &e.op
                && depends_on == id
            {
                let edge = Edge::new(e.bead.clone(), depends_on.clone(), *kind);
                if !d.contains(&edge) {
                    d.push(edge);
                }
            }
        }
        self.dependents.insert(id.clone(), d.clone());
        Ok(d)
    }

    /// The bead's status just before `e`, its first entry in this pass.
    fn seed(&self, e: &JournalEntry) -> Result<Prior, CacheError> {
        if e.state == JournalState::Pending {
            // The bead's earlier entries are applied, so in the stored row.
            return Ok(match self.store.get_row(&e.bead)? {
                Some((row, _)) => row.status.map_or(Prior::Unknown, Prior::Known),
                None => Prior::Unknown,
            });
        }
        // Applied after the base was fetched: the stored row already shows
        // this write, so the prior state is what bd said in the base.
        let id = &e.bead;
        Ok(if self.base.ready.contains(id) {
            Prior::Known(OPEN.to_owned())
        } else if self
            .base
            .blocked
            .iter()
            .any(|r| r.id == *id || r.blocked_by.contains(id))
        {
            Prior::NotClosed
        } else {
            Prior::Unknown
        })
    }

    fn apply(&mut self, e: &JournalEntry) -> Result<(), CacheError> {
        let prior = match self.priors.get(&e.bead) {
            Some(p) => p.clone(),
            None => self.seed(e)?,
        };
        let x = &e.bead;
        let next = match &e.op {
            Op::Close { .. } => {
                self.close(x)?;
                Prior::Known(CLOSED.to_owned())
            }
            Op::Update(patch) => match patch.status.as_deref() {
                Some(CLOSED) => {
                    self.close(x)?;
                    Prior::Known(CLOSED.to_owned())
                }
                Some(status) => {
                    if !prior.is(status) {
                        self.leave_ready(x);
                    }
                    if prior.is(CLOSED) || prior == Prior::Unknown {
                        self.reopen(x)?;
                    }
                    Prior::Known(status.to_owned())
                }
                None => prior,
            },
            Op::DepAdd { depends_on, kind } => {
                self.dep_add(x, depends_on, *kind)?;
                prior
            }
        };
        self.priors.insert(x.clone(), next);
        Ok(())
    }

    fn leave_ready(&mut self, x: &BeadId) {
        self.ready.retain(|r| r != x);
    }

    /// X closed: it is neither ready nor blocked, and blocks nothing.
    fn close(&mut self, x: &BeadId) -> Result<(), CacheError> {
        self.leave_ready(x);
        self.blocked.remove(x);
        let listing: Vec<BeadId> = self
            .blocked
            .iter()
            .filter(|(_, l)| l.contains(x))
            .map(|(id, _)| id.clone())
            .collect();
        for y in listing {
            self.drop_blocker(&y, x)?;
        }
        Ok(())
    }

    /// X reopened: it blocks its dependents again, and its own blockers come back.
    fn reopen(&mut self, x: &BeadId) -> Result<(), CacheError> {
        for e in self.dependents(x)? {
            if e.kind.is_blocking() && e.kind != DepType::ParentChild {
                self.add_blocker(&e.issue_id, x)?;
            }
        }
        let Some(facts) = self.facts(x)? else {
            return Ok(());
        };
        for e in facts.edges {
            self.dep_add(x, &e.depends_on_id, e.kind)?;
        }
        Ok(())
    }

    /// X gained (or regained) an edge onto T of the given kind.
    fn dep_add(&mut self, x: &BeadId, t: &BeadId, kind: DepType) -> Result<(), CacheError> {
        let blocks = match kind {
            DepType::ParentChild => self.blocked.contains_key(t),
            k if k.is_blocking() => !self.facts(t)?.is_some_and(|f| f.closed()),
            _ => false,
        };
        if blocks {
            self.add_blocker(x, t)?;
        }
        Ok(())
    }

    /// Adds B to Y's blockers; a newly blocked Y blocks its children (parent block).
    ///
    /// Terminates: each recursive step turns one more bead from unblocked
    /// into blocked, and nothing is unblocked meanwhile.
    fn add_blocker(&mut self, y: &BeadId, b: &BeadId) -> Result<(), CacheError> {
        if self.facts(y)?.is_some_and(|f| f.closed()) {
            return Ok(());
        }
        let list = self.blocked.entry(y.clone()).or_default();
        if list.contains(b) {
            return Ok(());
        }
        let newly = list.is_empty();
        list.push(b.clone());
        self.leave_ready(y);
        if newly {
            for e in self.dependents(y)? {
                if e.kind == DepType::ParentChild {
                    self.add_blocker(&e.issue_id, y)?;
                }
            }
        }
        Ok(())
    }

    /// Removes B from Y's blockers; an emptied list unblocks Y.
    fn drop_blocker(&mut self, y: &BeadId, b: &BeadId) -> Result<(), CacheError> {
        let Some(list) = self.blocked.get_mut(y) else {
            return Ok(());
        };
        list.retain(|i| i != b);
        if list.is_empty() {
            self.blocked.remove(y);
            self.unblocked(y)?;
        }
        Ok(())
    }

    /// Y has no blocker left: it may be ready, and it stops blocking its
    /// children as their parent (parent unblock).
    ///
    /// Terminates: each recursive step turns one more bead from blocked into
    /// unblocked, and nothing is blocked meanwhile.
    fn unblocked(&mut self, y: &BeadId) -> Result<(), CacheError> {
        if let Some(f) = self.facts(y)?
            && f.in_scope
            && f.status.as_deref() == Some(OPEN)
            && !f.deferred
            && !self.ready.contains(y)
        {
            self.ready.push(y.clone());
        }
        let incoming = self.dependents(y)?;
        for e in incoming.iter().filter(|e| e.kind == DepType::ParentChild) {
            let child = &e.issue_id;
            let blocks_otherwise = incoming.iter().any(|o| {
                o.issue_id == *child && o.kind.is_blocking() && o.kind != DepType::ParentChild
            });
            if !blocks_otherwise {
                self.drop_blocker(child, y)?;
            }
        }
        Ok(())
    }
}
