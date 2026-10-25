//! [`FakeBackend`]: a deterministic bd model for unit tests (NFR-BDC-0005).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{lock, ManualClock};
use crate::backend::BeadsBackend;
use crate::cache::{
    Actor, Assignee, BeadId, BeadsCacheError, BlockedRow, CacheBackend, CheckStep, Edge,
    Fingerprint, Guards, MetaValue, Patch, Refusal, ScopeFilter, SyncCursor,
};
use crate::clock::{rfc3339_to_unix, Clock, Deadline};
use crate::error::BeadsError;
use crate::row::{schema_sha256, Bead, DepType, BD_VERSION};
use crate::types::backend::{ProjectRef, RelationType};
use crate::types::cli::CliClient;
use crate::types::issue::BdRawIssue;
use crate::types::payload::{CreatePayload, ListQuery, UpdatePayload};

/// The pinned bd 1.3.1 `bd schema` output, the fake's default schema.
const PINNED_SCHEMA: &str = include_str!("../../tests/fixtures/bd-1.3.1/bd-schema.json");

/// Which [`CacheBackend`] method a call was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FakeOp {
    /// [`CacheBackend::fingerprint`]
    Fingerprint,
    /// [`CacheBackend::list_by_ids`]
    ListByIds,
    /// [`CacheBackend::list_scope`]
    ListScope,
    /// [`CacheBackend::list_blocked`]
    ListBlocked,
    /// [`CacheBackend::list_ready`]
    ListReady,
    /// [`CacheBackend::close_guarded`]
    Close,
    /// [`CacheBackend::update_guarded`]
    Update,
    /// [`CacheBackend::dep_add_typed`]
    DepAdd,
}

impl FakeOp {
    /// True for the three write calls.
    #[must_use]
    pub fn is_write(self) -> bool {
        matches!(self, Self::Close | Self::Update | Self::DepAdd)
    }

    /// True for the four list calls (not the fingerprint).
    #[must_use]
    pub fn is_list(self) -> bool {
        matches!(
            self,
            Self::ListByIds | Self::ListScope | Self::ListBlocked | Self::ListReady
        )
    }
}

/// One recorded call: the fake's command plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FakeCall {
    /// `bd version` + `bd schema`.
    Fingerprint,
    /// `bd list --id <ids> --status <all seven>`.
    ListByIds(Vec<BeadId>),
    /// `bd list <filter> [--updated-after <since>]`.
    ListScope {
        /// The scope filter.
        filter: ScopeFilter,
        /// The cursor, when given.
        since: Option<String>,
    },
    /// `bd blocked <filter>`.
    ListBlocked {
        /// The scope filter.
        filter: ScopeFilter,
    },
    /// `bd ready <filter> -n 0`.
    ListReady {
        /// The scope filter.
        filter: ScopeFilter,
    },
    /// `bd close`.
    Close {
        /// The bead.
        id: BeadId,
        /// `--reason`.
        reason: Option<String>,
        /// `--if-*` guards.
        guards: Guards,
        /// `--actor`.
        actor: Actor,
    },
    /// `bd update`.
    Update {
        /// The bead.
        id: BeadId,
        /// The patch.
        patch: Patch,
        /// `--if-*` guards.
        guards: Guards,
        /// `--actor`.
        actor: Actor,
    },
    /// `bd dep add`.
    DepAdd {
        /// The depending bead.
        id: BeadId,
        /// The bead depended on.
        on: BeadId,
        /// `--type`.
        kind: DepType,
        /// `--actor`.
        actor: Actor,
    },
}

impl FakeCall {
    /// The method this call was.
    #[must_use]
    pub fn op(&self) -> FakeOp {
        match self {
            Self::Fingerprint => FakeOp::Fingerprint,
            Self::ListByIds(_) => FakeOp::ListByIds,
            Self::ListScope { .. } => FakeOp::ListScope,
            Self::ListBlocked { .. } => FakeOp::ListBlocked,
            Self::ListReady { .. } => FakeOp::ListReady,
            Self::Close { .. } => FakeOp::Close,
            Self::Update { .. } => FakeOp::Update,
            Self::DepAdd { .. } => FakeOp::DepAdd,
        }
    }

    /// The bead a write call targets.
    #[must_use]
    pub fn write_target(&self) -> Option<&BeadId> {
        match self {
            Self::Close { id, .. } | Self::Update { id, .. } | Self::DepAdd { id, .. } => Some(id),
            _ => None,
        }
    }
}

/// A scripted outcome for one call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Injection {
    /// The backend cannot answer: [`BeadsCacheError::Unavailable`].
    Unavailable {
        /// Reported as a timeout.
        timed_out: bool,
    },
    /// The backend refuses the write: [`BeadsCacheError::Refused`] (bd exit 13 for
    /// [`Refusal::Precondition`]).
    Refused(Refusal),
    /// The command fails: [`BeadsCacheError::Failed`] with this exit code.
    Failed(i32),
    /// The backend check fails at this step (`fingerprint` only):
    /// [`BeadsCacheError::CheckFailed`]. `Failed` on a fingerprint fails the
    /// `version` step.
    CheckFailed {
        /// The failing step.
        step: CheckStep,
        /// Exit code.
        code: i32,
    },
    /// The call takes this long. If that reaches the deadline, the clock moves
    /// to the deadline and the call is `Unavailable { timed_out: true }`;
    /// otherwise the call then runs normally.
    Delay(Duration),
}

type Matcher = Box<dyn Fn(&FakeCall) -> bool + Send + Sync>;

struct State {
    rows: BTreeMap<BeadId, Bead>,
    schema: String,
    bd_version: String,
    calls: Vec<FakeCall>,
    injections: Vec<(Matcher, Injection)>,
    cost: Duration,
    op_costs: BTreeMap<FakeOp, Duration>,
}

/// Deterministic in-memory bd implementing [`CacheBackend`].
///
/// Behaviour, modelled on bd 1.3.x and the P-6 probe
/// (`docs/research/bd-blocked-state-probe.md`, recorded in
/// `tests/fixtures/p6-blocked/`):
///
/// - Rows are strict [`Bead`]s and never carry `is_blocked`.
/// - `list_by_ids` returns closed beads too, as an implementation passing
///   bd's full status list does (bd omits them without it); absent ids are
///   missing from the result.
/// - `list_scope` returns the beads the filter selects
///   ([`ScopeFilter::selects`]: labels, type, assignee) in every status and,
///   with a cursor, `updated_at > since` compared as instants (a value that
///   does not parse is listed).
/// - `list_blocked` computes the blocked set as bd does: a non-closed bead is
///   blocked by each non-closed target of a `blocks`, `conditional-blocks` or
///   `waits-for` edge, and a child (`parent-child` edge) of a blocked parent
///   is blocked by the parent; `related` and the other types do not block.
///   Blockers outside the filter are included; only selected beads are rows.
/// - `list_ready` returns selected beads that are `open`, not blocked and
///   not deferred into the future (`bd ready <filter> -n 0`).
/// - Writes check the guards (`""` = unassigned) and refuse with
///   [`Refusal::Precondition`] (bd exit 13); an unknown bead is
///   `Failed(1)`. Close and reopen bump only the bead's own `updated_at`
///   (`BlockedStateInvariant`); reopen clears `closed_at` and
///   `close_reason`; `dep add` bumps nothing, as bd 1.3.0 does.
/// - Every call is recorded ([`FakeBackend::calls`]), then advances the
///   [`ManualClock`] by its cost; a cost that reaches the deadline moves the
///   clock to the deadline and returns `Unavailable { timed_out: true }`.
///   An already expired deadline always times out.
/// - Injections are consumed in order, by the first call they match.
/// - Clones share one bd: keep a clone to drive and inspect a backend that
///   was moved into a cache.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use std::time::Duration;
/// use sc_beads::{CacheBackend, Deadline, ProjectRef, ScopeFilter};
/// use sc_beads::testing::{FakeBackend, FakeOp, Injection, ManualClock, bead};
///
/// let clock = Arc::new(ManualClock::default());
/// let fake = FakeBackend::new(Arc::clone(&clock));
/// let mut b = bead("b1");
/// b.labels = vec!["phase-x".into()];
/// fake.insert(b);
/// let p = ProjectRef::local(None);
/// let d = Deadline::after(&*clock, Duration::from_secs(30));
/// let scope = ScopeFilter::Label("phase-x".into());
/// assert_eq!(fake.list_scope(&p, &scope, None, d).unwrap().len(), 1);
///
/// fake.inject(FakeOp::ListScope, Injection::Unavailable { timed_out: false });
/// assert!(fake.list_scope(&p, &scope, None, d).unwrap_err().is_unavailable());
/// assert_eq!(fake.calls().len(), 2);
/// ```
#[derive(Clone)]
pub struct FakeBackend {
    clock: Arc<ManualClock>,
    state: Arc<Mutex<State>>,
}

impl std::fmt::Debug for FakeBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = lock(&self.state);
        f.debug_struct("FakeBackend")
            .field("rows", &s.rows.len())
            .field("calls", &s.calls.len())
            .field("injections", &s.injections.len())
            .finish_non_exhaustive()
    }
}

impl FakeBackend {
    /// An empty bd on the pinned bd 1.3.1 schema, sharing `clock`.
    #[must_use]
    pub fn new(clock: Arc<ManualClock>) -> Self {
        Self {
            clock,
            state: Arc::new(Mutex::new(State {
                rows: BTreeMap::new(),
                schema: PINNED_SCHEMA.to_owned(),
                bd_version: BD_VERSION.to_owned(),
                calls: Vec::new(),
                injections: Vec::new(),
                cost: Duration::ZERO,
                op_costs: BTreeMap::new(),
            })),
        }
    }

    /// The shared clock.
    #[must_use]
    pub fn clock(&self) -> &Arc<ManualClock> {
        &self.clock
    }

    /// Stores (or replaces) a row as given.
    pub fn insert(&self, bead: Bead) {
        lock(&self.state).rows.insert(bead.id.clone(), bead);
    }

    /// Stores every row as given.
    pub fn insert_all(&self, beads: impl IntoIterator<Item = Bead>) {
        let mut s = lock(&self.state);
        for b in beads {
            s.rows.insert(b.id.clone(), b);
        }
    }

    /// Removes a row (the bead disappears from bd).
    #[expect(clippy::must_use_candidate, reason = "tests usually drop the result")]
    pub fn remove(&self, id: &str) -> Option<Bead> {
        let id = BeadId::new(id).ok()?;
        lock(&self.state).rows.remove(&id)
    }

    /// The current row of a bead.
    #[must_use]
    pub fn row(&self, id: &str) -> Option<Bead> {
        let id = BeadId::new(id).ok()?;
        lock(&self.state).rows.get(&id).cloned()
    }

    /// Every row, by id.
    #[must_use]
    pub fn rows(&self) -> Vec<Bead> {
        lock(&self.state).rows.values().cloned().collect()
    }

    /// Edits a row outside the cache (an agent's own `bd` call) and bumps its
    /// `updated_at` to the clock's UTC time. Returns false for an unknown bead.
    pub fn edit(&self, id: &str, f: impl FnOnce(&mut Bead)) -> bool {
        let now = self.clock.now_utc();
        let Ok(id) = BeadId::new(id) else {
            return false;
        };
        let mut s = lock(&self.state);
        match s.rows.get_mut(&id) {
            Some(row) => {
                f(row);
                row.updated_at = now.into();
                true
            }
            None => false,
        }
    }

    /// Adds an edge `id -> on` without bumping anything. Returns false for an unknown bead.
    #[expect(clippy::must_use_candidate, reason = "tests usually drop the result")]
    pub fn add_edge(&self, id: &str, on: &str, kind: DepType) -> bool {
        let (Ok(id), Ok(on)) = (BeadId::new(id), BeadId::new(on)) else {
            return false;
        };
        let mut s = lock(&self.state);
        match s.rows.get_mut(&id) {
            Some(row) => {
                let edge = Edge::new(id, on, kind);
                if !row.dependencies.contains(&edge) {
                    row.dependencies.push(edge);
                }
                true
            }
            None => false,
        }
    }

    /// Replaces the `bd schema` output (simulates a bd upgrade).
    pub fn set_schema(&self, schema: impl Into<String>) {
        lock(&self.state).schema = schema.into();
    }

    /// Replaces the `bd version` answer.
    pub fn set_bd_version(&self, version: impl Into<String>) {
        lock(&self.state).bd_version = version.into();
    }

    /// Cost of every call (default zero).
    pub fn set_cost(&self, cost: Duration) {
        lock(&self.state).cost = cost;
    }

    /// Cost of every call of one method; overrides [`FakeBackend::set_cost`].
    pub fn set_cost_for(&self, op: FakeOp, cost: Duration) {
        lock(&self.state).op_costs.insert(op, cost);
    }

    /// Scripts the next call of `op` to produce `outcome`.
    pub fn inject(&self, op: FakeOp, outcome: Injection) {
        self.inject_where(move |c| c.op() == op, outcome);
    }

    /// Scripts the next call matching `matcher` to produce `outcome`.
    pub fn inject_where(
        &self,
        matcher: impl Fn(&FakeCall) -> bool + Send + Sync + 'static,
        outcome: Injection,
    ) {
        lock(&self.state)
            .injections
            .push((Box::new(matcher), outcome));
    }

    /// Injections not yet consumed.
    #[must_use]
    pub fn pending_injections(&self) -> usize {
        lock(&self.state).injections.len()
    }

    /// Every call so far, in order.
    #[must_use]
    pub fn calls(&self) -> Vec<FakeCall> {
        lock(&self.state).calls.clone()
    }

    /// The write calls so far.
    #[must_use]
    pub fn writes(&self) -> Vec<FakeCall> {
        self.calls()
            .into_iter()
            .filter(|c| c.op().is_write())
            .collect()
    }

    /// The list calls so far.
    #[must_use]
    pub fn reads(&self) -> Vec<FakeCall> {
        self.calls()
            .into_iter()
            .filter(|c| c.op().is_list())
            .collect()
    }

    /// Forgets the recorded calls.
    pub fn clear_calls(&self) {
        lock(&self.state).calls.clear();
    }

    /// Records the call, charges its cost and applies any injection.
    fn begin(&self, call: FakeCall, d: Deadline) -> Result<(), BeadsCacheError> {
        let is_check = call.op() == FakeOp::Fingerprint;
        let (cost, injected) = {
            let mut s = lock(&self.state);
            let op = call.op();
            let injected = s
                .injections
                .iter()
                .position(|(m, _)| m(&call))
                .map(|i| s.injections.remove(i).1);
            s.calls.push(call);
            let base = s.op_costs.get(&op).copied().unwrap_or(s.cost);
            match injected {
                Some(Injection::Delay(delay)) => (delay, None),
                other => (base, other),
            }
        };
        let remaining = d.remaining(&*self.clock);
        if cost >= remaining {
            self.clock.advance(remaining);
            return Err(BeadsCacheError::Unavailable {
                reason: "deadline reached".into(),
                timed_out: true,
            });
        }
        self.clock.advance(cost);
        match injected {
            None | Some(Injection::Delay(_)) => Ok(()),
            Some(Injection::Unavailable { timed_out }) => Err(BeadsCacheError::Unavailable {
                reason: if timed_out {
                    "timed out (injected)"
                } else {
                    "connection refused (injected)"
                }
                .into(),
                timed_out,
            }),
            Some(Injection::Refused(r)) => Err(BeadsCacheError::Refused(r)),
            Some(Injection::Failed(code)) if is_check => Err(BeadsCacheError::CheckFailed {
                step: CheckStep::Version,
                code,
                stderr: "fake failure".into(),
            }),
            Some(Injection::CheckFailed { step, code }) => Err(BeadsCacheError::CheckFailed {
                step,
                code,
                stderr: "fake failure".into(),
            }),
            Some(Injection::Failed(code)) => Err(BeadsCacheError::Failed {
                code,
                stderr: "fake failure".into(),
            }),
        }
    }

    fn write<R>(
        &self,
        id: &BeadId,
        g: &Guards,
        f: impl FnOnce(&mut Bead, &str) -> R,
    ) -> Result<R, BeadsCacheError> {
        let now = self.clock.now_utc();
        let mut s = lock(&self.state);
        let Some(row) = s.rows.get_mut(id) else {
            return Err(BeadsCacheError::Failed {
                code: 1,
                stderr: format!("Error: no issue found matching {id}"),
            });
        };
        let status_ok = g
            .status
            .as_deref()
            .is_none_or(|want| row.status.as_deref().unwrap_or("") == want);
        let assignee_ok = g
            .assignee
            .as_ref()
            .is_none_or(|want| want.matches(row.assignee.as_deref()));
        if !(status_ok && assignee_ok) {
            return Err(BeadsCacheError::Refused(Refusal::Precondition));
        }
        Ok(f(row, now.as_str()))
    }
}

/// Ids of beads that are blocked, with their blockers, computed over every row.
fn blocked_map(rows: &BTreeMap<BeadId, Bead>) -> BTreeMap<BeadId, Vec<BeadId>> {
    fn blockers(
        id: &BeadId,
        rows: &BTreeMap<BeadId, Bead>,
        memo: &mut BTreeMap<BeadId, Vec<BeadId>>,
        visiting: &mut BTreeSet<BeadId>,
    ) -> Vec<BeadId> {
        if let Some(done) = memo.get(id) {
            return done.clone();
        }
        let Some(row) = rows.get(id) else {
            return Vec::new();
        };
        if row.status.as_deref() == Some("closed") || !visiting.insert(id.clone()) {
            return Vec::new();
        }
        let open = |target: &BeadId| {
            rows.get(target)
                .is_some_and(|t| t.status.as_deref() != Some("closed"))
        };
        let mut out = Vec::new();
        for e in &row.dependencies {
            let hit = match e.kind {
                DepType::Blocks | DepType::ConditionalBlocks | DepType::WaitsFor => {
                    open(&e.depends_on_id)
                }
                DepType::ParentChild => {
                    open(&e.depends_on_id)
                        && !blockers(&e.depends_on_id, rows, memo, visiting).is_empty()
                }
                _ => false,
            };
            if hit && !out.contains(&e.depends_on_id) {
                out.push(e.depends_on_id.clone());
            }
        }
        visiting.remove(id);
        memo.insert(id.clone(), out.clone());
        out
    }
    let mut memo = BTreeMap::new();
    let mut visiting = BTreeSet::new();
    for id in rows.keys() {
        blockers(id, rows, &mut memo, &mut visiting);
    }
    memo.retain(|_, v| !v.is_empty());
    memo
}

fn apply_patch(row: &mut Bead, patch: &Patch, now: &str) {
    if let Some(status) = &patch.status {
        set_status(row, status, now);
    }
    if let Some(assignee) = &patch.assignee {
        // bd omits an empty assignee from its rows.
        row.assignee = match assignee {
            Assignee::Unassigned => None,
            Assignee::Named(n) => Some(n.clone()),
        };
    }
    for l in &patch.add_labels {
        if !row.labels.contains(l) {
            row.labels.push(l.clone());
        }
    }
    row.labels.retain(|l| !patch.remove_labels.contains(l));
    for (k, v) in &patch.set_metadata {
        row.metadata.insert(k.clone(), MetaValue::to_json(v));
    }
    for k in &patch.unset_metadata {
        row.metadata.remove(k);
    }
    if let Some(text) = &patch.append_notes {
        row.notes = Some(match row.notes.take().filter(|n| !n.is_empty()) {
            Some(n) => format!("{n}\n{text}"),
            None => text.clone(),
        });
    }
}

fn set_status(row: &mut Bead, status: &str, now: &str) {
    let was_closed = row.status.as_deref() == Some("closed");
    row.status = Some(status.to_owned());
    if status == "closed" && !was_closed {
        row.closed_at = Some(now.to_owned());
    } else if status != "closed" && was_closed {
        row.closed_at = None;
        row.close_reason = None;
    }
}

impl CacheBackend for FakeBackend {
    fn fingerprint(&self, _p: &ProjectRef, d: Deadline) -> Result<Fingerprint, BeadsCacheError> {
        self.begin(FakeCall::Fingerprint, d)?;
        let s = lock(&self.state);
        let schema_version = serde_json::from_str::<serde_json::Value>(&s.schema)
            .ok()
            .and_then(|v| v.get("schema_version").map(ToString::to_string))
            .unwrap_or_default();
        Ok(Fingerprint {
            bd_version: s.bd_version.clone(),
            schema_sha256: schema_sha256(&s.schema),
            schema_version,
        })
    }

    fn list_by_ids(
        &self,
        _p: &ProjectRef,
        ids: &[BeadId],
        d: Deadline,
    ) -> Result<Vec<Bead>, BeadsCacheError> {
        self.begin(FakeCall::ListByIds(ids.to_vec()), d)?;
        let s = lock(&self.state);
        let mut seen = BTreeSet::new();
        Ok(ids
            .iter()
            .filter(|id| seen.insert((*id).clone()))
            .filter_map(|id| s.rows.get(id).cloned())
            .collect())
    }

    fn list_scope(
        &self,
        _p: &ProjectRef,
        f: &ScopeFilter,
        since: Option<&SyncCursor>,
        d: Deadline,
    ) -> Result<Vec<Bead>, BeadsCacheError> {
        self.begin(
            FakeCall::ListScope {
                filter: f.clone(),
                since: since.map(|c| c.as_str().to_owned()),
            },
            d,
        )?;
        let s = lock(&self.state);
        Ok(s.rows
            .values()
            .filter(|r| f.selects(r))
            .filter(|r| {
                since.is_none_or(|c| {
                    match (rfc3339_to_unix(&r.updated_at), rfc3339_to_unix(c.as_str())) {
                        (Some(row), Some(cursor)) => row > cursor,
                        // Unparseable: never text-ordered; the row is listed, so a
                        // sync stores it rather than silently missing it.
                        _ => true,
                    }
                })
            })
            .cloned()
            .collect())
    }

    fn list_blocked(
        &self,
        _p: &ProjectRef,
        f: &ScopeFilter,
        d: Deadline,
    ) -> Result<Vec<BlockedRow>, BeadsCacheError> {
        self.begin(FakeCall::ListBlocked { filter: f.clone() }, d)?;
        let s = lock(&self.state);
        Ok(blocked_map(&s.rows)
            .into_iter()
            .filter(|(id, _)| s.rows.get(id).is_some_and(|r| f.selects(r)))
            .map(|(id, blocked_by)| BlockedRow { id, blocked_by })
            .collect())
    }

    fn list_ready(
        &self,
        _p: &ProjectRef,
        f: &ScopeFilter,
        d: Deadline,
    ) -> Result<Vec<BeadId>, BeadsCacheError> {
        self.begin(FakeCall::ListReady { filter: f.clone() }, d)?;
        let now = self.clock.now_utc().unix();
        let s = lock(&self.state);
        let blocked = blocked_map(&s.rows);
        Ok(s.rows
            .values()
            .filter(|r| f.selects(r) && r.status.as_deref() == Some("open"))
            .filter(|r| !blocked.contains_key(&r.id))
            .filter(|r| {
                r.defer_until
                    .as_deref()
                    .and_then(rfc3339_to_unix)
                    .is_none_or(|t| t <= now)
            })
            .map(|r| r.id.clone())
            .collect())
    }

    fn close_guarded(
        &self,
        _p: &ProjectRef,
        id: &BeadId,
        reason: Option<&str>,
        g: &Guards,
        a: &Actor,
        d: Deadline,
    ) -> Result<(), BeadsCacheError> {
        self.begin(
            FakeCall::Close {
                id: id.clone(),
                reason: reason.map(str::to_owned),
                guards: g.clone(),
                actor: a.clone(),
            },
            d,
        )?;
        self.write(id, g, |row, now| {
            set_status(row, "closed", now);
            if let Some(r) = reason {
                row.close_reason = Some(r.to_owned());
            }
            now.clone_into(&mut row.updated_at);
        })
    }

    fn update_guarded(
        &self,
        _p: &ProjectRef,
        id: &BeadId,
        patch: &Patch,
        g: &Guards,
        a: &Actor,
        d: Deadline,
    ) -> Result<(), BeadsCacheError> {
        self.begin(
            FakeCall::Update {
                id: id.clone(),
                patch: patch.clone(),
                guards: g.clone(),
                actor: a.clone(),
            },
            d,
        )?;
        self.write(id, g, |row, now| {
            apply_patch(row, patch, now);
            now.clone_into(&mut row.updated_at);
        })
    }

    fn dep_add_typed(
        &self,
        _p: &ProjectRef,
        id: &BeadId,
        on: &BeadId,
        kind: DepType,
        a: &Actor,
        d: Deadline,
    ) -> Result<(), BeadsCacheError> {
        self.begin(
            FakeCall::DepAdd {
                id: id.clone(),
                on: on.clone(),
                kind,
                actor: a.clone(),
            },
            d,
        )?;
        self.write(id, &Guards::default(), |row, _| {
            let edge = Edge::new(id.clone(), on.clone(), kind);
            if !row.dependencies.contains(&edge) {
                row.dependencies.push(edge);
            }
        })
    }
}

fn unsupported(operation: &'static str) -> BeadsError {
    BeadsError::Unsupported {
        operation,
        client: CliClient::Unknown,
    }
}

/// btit's surface is not modelled: every call is `Unsupported` except the
/// pure ones (`project_uses_dolt` is false, `relation_types` lists bd's
/// dependency types).
impl BeadsBackend for FakeBackend {
    fn project_uses_dolt(&self, _project: &ProjectRef) -> bool {
        false
    }
    fn list(
        &self,
        _project: &ProjectRef,
        _query: &ListQuery,
    ) -> Result<Vec<BdRawIssue>, BeadsError> {
        Err(unsupported("list"))
    }
    fn ready(&self, _project: &ProjectRef) -> Result<Vec<BdRawIssue>, BeadsError> {
        Err(unsupported("ready"))
    }
    fn status(&self, _project: &ProjectRef) -> Result<serde_json::Value, BeadsError> {
        Err(unsupported("status"))
    }
    fn show(&self, _project: &ProjectRef, _id: &str) -> Result<Option<BdRawIssue>, BeadsError> {
        Err(unsupported("show"))
    }
    fn create(
        &self,
        _project: &ProjectRef,
        _payload: &CreatePayload,
    ) -> Result<BdRawIssue, BeadsError> {
        Err(unsupported("create"))
    }
    fn update(
        &self,
        _project: &ProjectRef,
        _id: &str,
        _updates: &UpdatePayload,
    ) -> Result<Option<BdRawIssue>, BeadsError> {
        Err(unsupported("update"))
    }
    fn close(&self, _project: &ProjectRef, _id: &str) -> Result<serde_json::Value, BeadsError> {
        Err(unsupported("close"))
    }
    fn search(&self, _project: &ProjectRef, _query: &str) -> Result<Vec<BdRawIssue>, BeadsError> {
        Err(unsupported("search"))
    }
    fn label_add(&self, _project: &ProjectRef, _id: &str, _label: &str) -> Result<(), BeadsError> {
        Err(unsupported("label add"))
    }
    fn label_remove(
        &self,
        _project: &ProjectRef,
        _id: &str,
        _label: &str,
    ) -> Result<(), BeadsError> {
        Err(unsupported("label remove"))
    }
    fn delete(&self, _project: &ProjectRef, _id: &str) -> Result<(), BeadsError> {
        Err(unsupported("delete"))
    }
    fn comment_add(
        &self,
        _project: &ProjectRef,
        _id: &str,
        _content: &str,
    ) -> Result<(), BeadsError> {
        Err(unsupported("comment add"))
    }
    fn dep_add(
        &self,
        _project: &ProjectRef,
        _issue_id: &str,
        _depends_on_id: &str,
        _relation_type: Option<&str>,
    ) -> Result<(), BeadsError> {
        Err(unsupported("dep add"))
    }
    fn dep_remove(
        &self,
        _project: &ProjectRef,
        _issue_id: &str,
        _depends_on_id: &str,
    ) -> Result<(), BeadsError> {
        Err(unsupported("dep remove"))
    }
    fn relation_types(&self) -> Vec<RelationType> {
        DepType::ALL
            .iter()
            .map(|t| RelationType {
                value: t.as_str(),
                label: t.as_str(),
            })
            .collect()
    }
    fn sync(&self, _project: &ProjectRef) -> Result<(), BeadsError> {
        Err(unsupported("sync"))
    }
}
