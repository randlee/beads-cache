---
id: a-1
phase: a
status: complete
closure_type: contract
target_boundary: sc-beads, bd-cache-model, bd-cache (skeleton and shared helpers)
vertical_rationale: "Contract sprint for three boundaries every wave-2 sprint consumes: the shared-trait crate, the pure model crate whose behaviour is the contract, and the bd-cache skeleton with the helpers (view, backend_check, miss, outage) that both a-5 and a-6 call, so neither depends on the other."
track: contracts
wave: 1
branch: sprint/a-1-contracts
worktree: sprint/a-1-contracts
recommended_agent: rust-developer
recommended_model: opus
must_follow: []
parallel_safe: [a-3]
requirements:
  owns: [REQ-BDC-0007, REQ-BDC-0105, REQ-BDC-0301, REQ-BDC-0304, REQ-BDC-0305, REQ-BDC-0802, REQ-BDC-0803, REQ-BDC-0906, NFR-BDC-0005]
  constrained_by: [REQ-BDC-0004, REQ-BDC-0108, REQ-BDC-0202, REQ-BDC-0303, REQ-BDC-0402, REQ-BDC-0604, REQ-BDC-0804, REQ-BDC-0806, REQ-BDC-0901, REQ-BDC-0902, REQ-BDC-0903, REQ-BDC-0904, REQ-BDC-0907, REQ-BDC-0908, REQ-BDC-0909, REQ-BDC-0911, REQ-BDC-0912]
adrs: [ADR-BDC-0001, ADR-BDC-0002, ADR-BDC-0003, ADR-BDC-0004, ADR-BDC-0005, ADR-BDC-0007]
owned_paths:
  - "Cargo.toml"
  - "rust-toolchain.toml"
  - ".gitignore"
  - ".github/workflows/ci.yml"
  - "crates/sc-beads/**"
  - "crates/bd-cache-model/**"
  - "crates/bd-cache/Cargo.toml"
  - "crates/bd-cache/src/lib.rs"
  - "crates/bd-cache/src/view.rs"
  - "crates/bd-cache/src/backend_check.rs"
  - "crates/bd-cache/src/miss.rs"
  - "crates/bd-cache/src/outage.rs"
  - "crates/bd-cache/tests/common/**"
  - "crates/bd-cache/tests/helpers_*.rs"
  - "docs/sc-beads/**"
  - "docs/bd-cache-model/**"
  - "docs/bd-cache/requirements.md"
  - "docs/bd-cache/architecture.md"
  - "docs/architecture.md"
---

# Sprint a-1 — contracts

## Outcome and closure

Freeze every interface phase A shares, with test doubles and contract tests,
so the five wave-2 sprints run in parallel. Closes when the workspace builds
in CI, `FakeBackend` and `FakeStore` pass their contract suites, the helpers
pass their tests, and the signatures below are committed.

`sc-beads` is the backend contract crate, ported from beads-task-issue-tracker
at `8d8e82ae9f85` (btit-beads, btit-types); implementations live in
`sc-bd-cli` (a-2) and later `sc-bd-http`. `bd-cache-model` is pure (no I/O).
The `bd-cache` skeleton carries the `Cache` facade and the four helpers both
halves of the core call.

**Amendment rule:** a defect found in a wave-2 sprint in an a-1 signature or
helper is an amendment to a-1, merged into `integrate/phase-a` before the
wave-2 fix that needs it.

## Decision prerequisites

- P-3: confirms `BeadsError` as the shared error type and the tolerant/strict
  row split. btit's traits stay unchanged regardless.
- P-6: done (`docs/research/bd-blocked-state-probe.md`); it set
  ADR-BDC-0002's design.
- Strict `Bead` generation from the bd-1.3.1 schema fixture; generator
  command and pinned sha in the module header.

## Public contract

Normative. Layer sprints reference these and do not restate them.

```rust
// ================= sc-beads =================
// From btit (8d8e82ae9f85), unchanged: ProjectRef, BdRawIssue, ListQuery, CreatePayload,
// UpdatePayload, RelationType, BeadsError's existing variants, and the traits
// BeadsBackend, CliBackend, DoltOperations (snapshot fixture: tests/fixtures/btit-api.txt).
// New in sc-beads (btit passes ids as &str): BeadId, Actor, DepType.
pub struct BeadId(String);
pub struct Actor(String);
pub enum DepType { /* bd's dependency types from bd schema; DepType::parse(&str) -> Option<Self>; is_blocking() */ }
pub struct Edge { pub issue_id: BeadId, pub depends_on_id: BeadId, pub kind: DepType }
/// Strict row: generated from `bd schema` (deny_unknown_fields) plus the four --json extras.
pub struct Bead { /* id, status, assignee, labels, metadata, notes, updated_at, dependencies: Vec<Edge>, ...; is_blocked is optional and bd never emits it (ADR-BDC-0002) */ }
/// One row of `bd blocked --json`, strict.
pub struct BlockedRow { pub id: BeadId, pub blocked_by: Vec<BeadId> }
pub struct Fingerprint { pub bd_version: String, pub schema_sha256: String, pub schema_version: String }
pub enum ScopeFilter { Label(String) }
pub struct SyncCursor(String); // RFC 3339 derived by the cache from last_ok_at; opaque to the backend
pub struct Guards { pub status: Option<String>, pub assignee: Option<String> } // "" = unassigned
pub enum MetaValue { Int(i64), Bool(bool), Str(String) }
pub struct Patch { pub status: Option<String>, pub assignee: Option<String>,
    pub add_labels: Vec<String>, pub remove_labels: Vec<String>,
    pub set_metadata: std::collections::BTreeMap<String, MetaValue>, pub unset_metadata: Vec<String>,
    pub append_notes: Option<String> } // /v0-aligned (ADR-BDC-0003)

pub trait Clock: Send + Sync { fn now_utc(&self) -> String /* RFC 3339, s */; fn now_mono(&self) -> std::time::Instant; }
pub struct SystemClock;
#[derive(Clone, Copy)]
pub struct Deadline(std::time::Instant);
impl Deadline {
    pub fn at(i: std::time::Instant) -> Self;
    pub fn after(clock: &dyn Clock, d: Duration) -> Self;
    pub fn earliest(self, other: Deadline) -> Deadline;
    pub fn remaining(&self, clock: &dyn Clock) -> Duration;
    pub fn expired(&self, clock: &dyn Clock) -> bool;
}

/// Refusals: the CLI only produces Precondition (exit 13); NotClosable and
/// AlreadyClaimed come from the HTTP implementation's 409 codes.
pub enum Refusal { Precondition, NotClosable, AlreadyClaimed, Other(String) }
// btit's BeadsError is unchanged (owner amendment, ADR-BDC-0007): cache errors are a separate type.
pub enum BeadsCacheError {
    Unavailable { reason: String, timed_out: bool }, // timeout, cannot start, connection refused
    Refused(Refusal),
    Contract { bead: Option<BeadId>, field: String, detail: String },
    Failed { code: i32, stderr: String },
    Btit(BeadsError),                                // any btit error
}
impl From<BeadsError> for BeadsCacheError;

/// The cache's backend surface; implemented by sc-bd-cli (a-2) and FakeBackend.
/// Implementations are constructed with the same Arc<dyn Clock> as the Cache and
/// stop each call at the Deadline they are handed; they do not apply their own cap.
/// Every method returns BeadsCacheError; btit's traits keep returning BeadsError.
pub trait CacheBackend: BeadsBackend + Send + Sync {
    fn fingerprint(&self, p: &ProjectRef, d: Deadline) -> Result<Fingerprint, BeadsCacheError>;
    fn list_by_ids(&self, p: &ProjectRef, ids: &[BeadId], d: Deadline) -> Result<Vec<Bead>, BeadsCacheError>;
    fn list_scope(&self, p: &ProjectRef, f: &ScopeFilter, since: Option<&SyncCursor>, d: Deadline) -> Result<Vec<Bead>, BeadsCacheError>;
            fn list_blocked(&self, p: &ProjectRef, f: &ScopeFilter, d: Deadline) -> Result<Vec<BlockedRow>, BeadsCacheError>; // bd blocked -l
    fn list_ready(&self, p: &ProjectRef, f: &ScopeFilter, d: Deadline) -> Result<Vec<BeadId>, BeadsCacheError>;        // bd ready -l -n 0
    fn close_guarded(&self, p: &ProjectRef, id: &BeadId, reason: Option<&str>, g: &Guards, a: &Actor, d: Deadline) -> Result<(), BeadsCacheError>;
    fn update_guarded(&self, p: &ProjectRef, id: &BeadId, patch: &Patch, g: &Guards, a: &Actor, d: Deadline) -> Result<(), BeadsCacheError>;
    fn dep_add_typed(&self, p: &ProjectRef, id: &BeadId, on: &BeadId, kind: DepType, a: &Actor, d: Deadline) -> Result<(), BeadsCacheError>;
}

/// Behind the `testing` feature.
pub mod testing {
    pub struct ManualClock; // advance(Duration), set_utc(&str)
        /// Deterministic bd model: validated rows without is_blocked; close/reopen bump only the bead's
    /// own updated_at; list_blocked computes the blocked set as bd does (blocks edges, parent-child
    /// inheritance with the child's blocked_by listing its parent, related non-blocking) including
    /// blockers outside the label; list_ready returns open, unblocked, labelled ids (bd ready -l -n 0); list_by_ids needs
    /// the status list for closed beads; records the command plan; injects Unavailable (timed_out or not), exit 13
    /// and Failed per call; advances the ManualClock by a configured cost per call.
    pub struct FakeBackend;
}

// ================= bd-cache-model =================
pub enum Op { Close { reason: Option<String> }, Update(Patch), DepAdd { depends_on: BeadId, kind: DepType } }
impl Op {
    pub fn parse(op: &str, args_json: &str) -> Result<Op, CacheError>; // BAD_OP; BAD_ARGS incl. dep type, metadata coercion
    pub fn to_args_json(&self) -> String;                                // canonical, Python-compatible
    pub fn overlay(&self, row: &Bead) -> Bead;
    pub fn achieved(&self, row: &Bead) -> bool;                          // dep_add: edge in row.dependencies
    
}
pub fn replay(row: &Bead, pending: &[JournalEntry]) -> Bead;            // seq order; escalated skipped
pub struct Seq(i64); pub struct IdemKey(String);
pub enum JournalState { Pending, Applied, Escalated, Resolved }
pub struct JournalEntry { pub seq: Seq, pub idem_key: IdemKey, pub scope: Option<ScopeId>, pub bead: BeadId,
    pub op: Op, pub precondition: Guards, pub actor: Actor, pub state: JournalState, pub attempts: u32,
        pub last_error: Option<String>, pub announced_at: Option<String>,
    pub created_at: String, pub applied_at: Option<String> }   // applied_at set by apply_with_readback
pub struct NewEntry { pub idem_key: IdemKey, pub scope: Option<ScopeId>, pub bead: BeadId, pub op: Op, pub precondition: Guards, pub actor: Actor }
pub enum ScopeId { Phase(String) }
impl ScopeId {
    pub fn filter(&self) -> ScopeFilter;                       // Phase(x) -> Label("phase-x")
    pub fn from_labels(labels: &[String]) -> Option<ScopeId>;  // first phase-<x> label (the v3 `phase` column)
}
pub enum Freshness { Fresh, Stale }
pub struct SyncState { pub scope: ScopeId, pub last_started_at: Option<String>, pub last_ok_at: Option<String>, pub last_error: Option<String> }
pub struct Outage { pub since: String, pub last_error: String }
pub enum FingerprintOutcome { Recorded, Unchanged, Rebuilt }
pub struct UpsertReport { pub stored: Vec<BeadId>, pub skipped_newer: Vec<BeadId>, pub status_changed: Vec<BeadId> }
pub struct ReadyBase { pub blocked: Vec<BlockedRow>, pub ready: Vec<BeadId>, pub fetched_at: String, pub stale: bool }
pub struct OpenWork { pub pending: u64, pub escalated_unannounced: u64 }
pub struct StoreStatus { /* meta, journal counts per scope:state, per-scope sync state and bead counts, escalated entries, outage */ }

/// Store port; a-4 implements it over SQLite. Rules (checked by the contract suite):
/// - get_row(upsert(b)) == normalize(b): `dependencies` rebuilt from the edges table, counts absent.
/// - upsert writes the bead's outgoing edges from `Bead.dependencies`, replacing the old ones.
/// - any successful store of a bead clears `stale`.
/// - upsert_rows(Some(s), ..) sets membership of s from the row's labels; upsert_rows(None, ..)
///   never changes existing membership (a new row stored with None has none: out-of-scope row).
/// - status_changed lists existing rows whose status text changed, forced or not; new rows excluded.
/// - apply_with_readback(seq, None, Some(err)) marks the entry applied with err and sets stale on the bead.
/// - record_fingerprint Rebuilt deletes beads, edges, memberships, sync_state rows and every ready base;
///   keeps the journal.
/// - ensure_scope persists; scopes() returns configured and on-demand scopes across restarts.
/// - replace_ready_base replaces the whole base for the scope and clears its stale mark; its `at` is the
///   instant taken before the sync's first list_blocked/list_ready call. ready_base is None only when the
///   scope never stored one; mark_ready_base_stale on a missing base is a no-op; prune_scope deletes it.
/// - applied_since(at) returns entries applied at or after `at`, in seq order.
pub trait CacheStore: Send {
        fn ensure_scope(&mut self, s: &ScopeId) -> Result<(), CacheError>;
    fn scopes(&self) -> Result<Vec<ScopeId>, CacheError>;
    fn get_row(&self, id: &BeadId) -> Result<Option<(Bead, Freshness)>, CacheError>;
    fn rows_in_scope(&self, s: &ScopeId) -> Result<Vec<(Bead, Freshness)>, CacheError>;
    fn upsert_rows(&mut self, s: Option<&ScopeId>, rows: &[Bead], force: bool) -> Result<UpsertReport, CacheError>;
    fn stale_rows(&self) -> Result<Vec<BeadId>, CacheError>;
    fn dependents_of(&self, id: &BeadId) -> Result<Vec<Edge>, CacheError>;     // incoming edges, with kind
            fn replace_ready_base(&mut self, s: &ScopeId, blocked: &[BlockedRow], ready: &[BeadId], at: &str) -> Result<(), CacheError>; // whole base for the scope
    fn ready_base(&self, s: &ScopeId) -> Result<Option<ReadyBase>, CacheError>; // None = never fetched
    fn mark_ready_base_stale(&mut self, s: &ScopeId) -> Result<(), CacheError>;

        fn entry_by_key(&self, k: &IdemKey) -> Result<Option<JournalEntry>, CacheError>; // any state
    fn entry_by_seq(&self, seq: Seq) -> Result<Option<JournalEntry>, CacheError>;
    fn insert_entry(&mut self, e: NewEntry, at: &str) -> Result<Seq, CacheError>;
    fn pending_entries(&self) -> Result<Vec<JournalEntry>, CacheError>;
        fn pending_for(&self, id: &BeadId) -> Result<Vec<JournalEntry>, CacheError>;
    fn applied_since(&self, at: &str) -> Result<Vec<JournalEntry>, CacheError>;
    fn apply_with_readback(&mut self, seq: Seq, row: Option<&Bead>, err: Option<&str>, at: &str) -> Result<(), CacheError>; // one transaction
    fn record_attempt(&mut self, seq: Seq, err: &str, at: &str) -> Result<u32, CacheError>;
    fn escalate(&mut self, seq: Seq, reason: &str, at: &str) -> Result<(), CacheError>;
    fn resolve(&mut self, seq: Seq, note: &str, actor: &Actor, at: &str) -> Result<(), CacheError>; // NOT_ESCALATED
    fn unannounced_escalations(&self) -> Result<Vec<JournalEntry>, CacheError>;
    fn mark_announced(&mut self, seq: Seq, at: &str) -> Result<(), CacheError>;
    fn record_fingerprint(&mut self, f: &Fingerprint) -> Result<FingerprintOutcome, CacheError>;
    fn sync_state(&self, s: &ScopeId) -> Result<SyncState, CacheError>;
    fn record_sync(&mut self, st: &SyncState) -> Result<(), CacheError>;
    fn sync_alert(&self, s: &ScopeId) -> Result<Option<String>, CacheError>;   // stable cause key
    fn set_sync_alert(&mut self, s: &ScopeId, cause: Option<&str>) -> Result<(), CacheError>;
    fn outage(&self) -> Result<Option<Outage>, CacheError>;
    fn set_outage(&mut self, o: Option<&Outage>) -> Result<(), CacheError>;
    fn prune_scope(&mut self, s: &ScopeId) -> Result<PruneReport, CacheError>; // PRUNE_BLOCKED; keeps beads other scopes hold; sweeps out-of-scope rows with no open entry
    fn open_work(&self) -> Result<OpenWork, CacheError>;
    fn status(&self) -> Result<StoreStatus, CacheError>;
}
/// Cross-process single drain writer (a-4: flock). Dropping the guard releases the lock.
pub trait DrainLock: Send + Sync { fn try_acquire(&self) -> Result<Option<Box<dyn DrainGuard>>, CacheError>; }
pub trait DrainGuard: Send {}
/// Behind the `testing` feature. FakeDrainLock clones share one lock, so two Caches contend.
pub mod testing { pub struct FakeStore; pub struct FakeDrainLock; }

pub struct SendError { pub detail: String }
/// Escalation delivery port; a-7 implements it with `atm send`.
pub trait EscalationSender: Send + Sync { fn send(&self, lead: &str, team: &str, body: &str) -> Result<(), SendError>; }

/// The `.beads/bd-cache.toml` schema; a-4 owns the one loader (bd_cache_store::load_repo_config).
pub struct RepoConfig { pub cache_path: Option<PathBuf>, pub scopes: Vec<String>, pub call_timeout_s: Option<u64>,
    pub miss_fill_timeout_ms: Option<u64>, pub max_attempts: Option<u32>,
        pub schema_packages: Vec<toml::Value> /* opaque; ADR-BDC-0008, ignored in phase a */,
    pub tick: Option<toml::Table> /* bd-cache-sync's keys: team, fallback_lead, phases dir */ }
impl RepoConfig { pub fn cache_config(&self, project: ProjectRef) -> CacheConfig; }
pub struct CacheConfig {
    pub project: ProjectRef, pub scopes: Vec<ScopeId>, // configured; sync of another phase registers it on demand (REQ-BDC-0909)
    pub call_timeout: Duration,      // single owner of the 30 s per-call cap
    pub miss_fill_timeout: Duration, // default 2 s (ADR-BDC-0005)
    pub max_attempts: u32,           // 5
}
/// REQ-BDC-0402. a-7 builds it from the tick end T: start_gate = T - 5 s, hard = T - 2 s.
pub struct DrainBudget { pub start_gate: Deadline, pub hard: Deadline }

/// Which call failed, for mapping backend errors.
pub enum CallKind { Check, PreRead, Write, ReadBack, Sync, Refresh, MissFill, Ready }
/// Outcome of one backend call after the Cache's classification:
/// Unavailable{timed_out: true} when the caller's hard deadline has expired is a cut, not an outage.
pub enum CallOutcome { Ok, Cut, Outage(String), Error(CacheError) }

// Replies are provisional: their exact field set is owned by the later `bdc` sprint
// (REQ-BDC-0806, deferred). Rows are objects, never tuples. Goldens here pin the
// Rust shapes so wave-2 sprints agree; they are not claimed equal to Python.
pub struct RowView { pub bead: Bead, pub stale: bool, pub blocked: Option<Vec<BeadId>> /* None = unknown, Some([]) = not blocked */ }
pub struct GetReply { pub row: RowView, pub scope_last_ok_at: Option<String>, pub pending: u32, pub outage: Option<Outage> }
pub struct ListReply { pub scope: ScopeId, pub last_ok_at: Option<String>, pub rows: Vec<RowView>, pub outage: Option<Outage> }
pub enum ReadyBasis { Fresh, Cached { fetched_at: String, stale: bool }, Unknown }
pub struct ReadyReply { pub scope: ScopeId, pub basis: ReadyBasis, pub locally_adjusted: bool, pub ready: Vec<RowView>,
        pub read_fallback: Option<String> /* why a requested fresh answer fell back */,
    pub missing_rows: Vec<BeadId> /* ready per bd but no cached row yet; omitted from `ready` */ }
pub struct WriteReply { pub seq: Seq, pub duplicate: bool }
pub struct DrainReport { pub busy: bool, pub applied: Vec<Seq>, pub retrying: Vec<Seq>, pub escalated: Vec<EscalatedItem>,
    pub deferred: Vec<Seq>, pub deferred_in_flight: Vec<Seq>, pub outage: Option<Outage> }
pub struct EscalatedItem { pub seq: Seq, pub bead: BeadId, pub op: String, pub actor: Actor, pub phase: Option<String>, pub reason: String }
pub struct SyncReport { pub scope: ScopeId, pub rebuilt: bool, pub model_current: bool, pub stored: u32,
        pub blocked: u32, pub deferred: bool, pub outage: Option<Outage> }
pub struct PruneReport { pub scope: ScopeId, pub beads_deleted: u32, pub out_of_scope_swept: u32 }
pub struct StatusReply { /* StoreStatus plus outage */ }

/// One variant per baseline code with its payload: BD_UNAVAILABLE, NO_DATABASE, WRONG_DATABASE,
/// NOT_FOUND, BD_VERSION_FAILED, BD_SCHEMA_FAILED, BD_CONTRACT{bead, field}, BD_WRITE_FAILED,
/// BD_LIST_FAILED, SYNC_FAILED, PRECONDITION{unmet: key -> {expected, actual}}, IDEM_CONFLICT,
/// BAD_OP, BAD_ARGS, BAD_PRECONDITION, SCHEMA_VERSION, PRUNE_BLOCKED, NOT_ESCALATED;
/// plus SCOPE_UNKNOWN{known} (additive). NO_TOML is never emitted (Python-only).
pub enum CacheError { /* ... */ }
impl CacheError {
    pub fn code(&self) -> &'static str; pub fn fix(&self) -> &'static str;
    pub fn reason(&self) -> Option<&'static str>; // /v0: precondition_failed, not_closable, already_claimed
    pub fn exit_code(&self) -> i32;               // 3 for every typed error
    pub fn to_json(&self) -> serde_json::Value;   // {ok: false, code, detail, fix, reason?, ...payload}
    /// Mapping table (contract-tested): Contract -> BD_CONTRACT; Refused(Precondition) -> PRECONDITION;
    /// Failed -> BD_WRITE_FAILED (Write), BD_LIST_FAILED (PreRead, ReadBack, Refresh, MissFill),
    /// SYNC_FAILED (Sync), BD_VERSION_FAILED / BD_SCHEMA_FAILED (Check); Unavailable -> BD_UNAVAILABLE;
    /// Btit(e) by the btit variant (Spawn -> BD_UNAVAILABLE, parse errors -> BD_CONTRACT, else as Failed).
    pub fn from_backend(e: &BeadsCacheError, kind: CallKind) -> CacheError;
}

// ================= bd-cache (skeleton + shared helpers) =================
/// Not Sync (one SQLite connection). Threading model: ADR-CORE-0001 in docs/bd-cache/architecture.md.
pub struct Cache<S: CacheStore, L: DrainLock, B: CacheBackend> { /* config, store, lock, backend, clock: Arc<dyn Clock> */ }
impl<S: CacheStore, L: DrainLock, B: CacheBackend> Cache<S, L, B> {
    pub fn new(config: CacheConfig, store: S, lock: L, backend: B, clock: Arc<dyn Clock>) -> Result<Self, CacheError>; // ensure_scope each
        pub fn get(&mut self, id: &BeadId, miss_timeout: Option<Duration>) -> Result<GetReply, CacheError>;          // a-6
    pub fn refresh(&mut self, ids: &[BeadId], timeout: Duration) -> Result<Vec<RowView>, CacheError>;            // a-6: fresh read, stored with force, overlay applied
    pub fn entry(&self, seq: Seq) -> Result<JournalEntry, CacheError>;                                          // a-6
        pub fn list(&self, s: &ScopeId, status: Option<&str>, label: Option<&str>) -> Result<ListReply, CacheError>;  // a-6
        pub fn ready(&mut self, s: &ScopeId, fresh_within: Option<Duration>) -> Result<ReadyReply, CacheError>;      // a-6; never exceeds fresh_within
    pub fn write(&mut self, id: &BeadId, op_name: &str, args_json: &str, pre: Guards, key: IdemKey, actor: Actor) -> Result<WriteReply, CacheError>; // a-5
    pub fn drain(&mut self, budget: &DrainBudget) -> Result<DrainReport, CacheError>;                            // a-5
        pub fn sync(&mut self, s: &ScopeId, d: Deadline) -> Result<SyncReport, CacheError>;                          // a-6 (incl. blocked set)
    pub fn resolve(&mut self, seq: Seq, note: &str, actor: Actor) -> Result<(), CacheError>;                     // a-6
    pub fn prune(&mut self, s: &ScopeId) -> Result<PruneReport, CacheError>;                                     // a-6
        pub fn status(&self) -> Result<StatusReply, CacheError>;                                                     // a-6
    // Announcement state, so an external scheduler (atm-bd's bd_cache_cron.py) can announce
    // through its own sender (a-6; passthrough to the store):
    pub fn unannounced_escalations(&self) -> Result<Vec<JournalEntry>, CacheError>;
    pub fn mark_announced(&mut self, seq: Seq) -> Result<(), CacheError>;
    pub fn sync_alert(&self, s: &ScopeId) -> Result<Option<String>, CacheError>;
    pub fn set_sync_alert(&mut self, s: &ScopeId, cause: Option<&str>) -> Result<(), CacheError>;
    pub fn open_work(&self) -> Result<OpenWork, CacheError>;
}
// Helpers implemented and tested by a-1 (pub(crate)); a-5 and a-6 call them:
//   call(kind, hard: Deadline, f) -> CallOutcome
//       hands f `hard.earliest(after(call_timeout))`; classifies the result: Cut iff timed_out and
//       hard expired; for Check, PreRead, Sync, Refresh, MissFill and Ready an Unavailable is an Outage;
//       Ready uses one hard = Deadline::after(fresh_within) for both of its calls, so the caller's
//       limit expiring is a Cut, never an outage;
//       for Write an Unavailable is an Error that counts an attempt (v3 rule); note_ok only after a
//       list call (PreRead, ReadBack, Sync, Refresh, MissFill, Ready) answers, never after Check alone.
//   outage::{note_unavailable(&mut self, err), note_ok(&mut self)}   // record on Unavailable, clear on next success
//   view::current_view(&self, id) -> Result<Option<(Bead, Freshness)>, CacheError>   // stored row + replay
//   backend_check::run(&mut self, hard) -> Result<(FingerprintOutcome, bool /*model_current*/), CallOutcome>
//   miss::fill(&mut self, id, timeout: Option<Duration>) -> Result<Option<Bead>, CacheError>
//       // under timeout, else config miss_fill_timeout; stored with None
// Module files a-5 fills: write.rs, drain.rs. a-6 fills: get.rs (incl. entry), refresh.rs, list.rs, sync.rs, ready.rs, resolve.rs, prune.rs, status.rs.
```

## Deliverables

1. Workspace `Cargo.toml` (`members = ["crates/*"]`) with every phase-A
   third-party dependency in `[workspace.dependencies]` (rusqlite bundled,
      fs4, serde, serde_json, sha2, toml, tracing, tokio, async-trait, chrono, cron,
   criterion, pyo3); `rust-toolchain.toml`; `.gitignore` with `/Cargo.lock` (removed at phase close-out).
2. `.github/workflows/ci.yml`: `cargo fmt --all --check`, `cargo clippy
   --all-targets --all-features -- -D warnings`, `cargo test --workspace
   --all-features`, `cargo publish --dry-run -p sc-beads`, uv installed for
      a-4's compat test, and a `cargo metadata` check that fails on an edge the
   Boundaries section of `docs/architecture.md` forbids. The workspace gates
   run with `--exclude bd-cache-py` (a PyO3 extension module does not link
   under `cargo test`); `python.yml` (a-8) is that crate's gate. No live
   probes.
3. `sc-beads`: btit's types and traits unchanged with the API snapshot
   fixture, `CacheBackend`, rows, errors, `Clock`/`Deadline`, `testing`
      (REQ-BDC-0906, 0007); bd-1.3.1 fixtures (`bd-schema.json`, `list-rows.json`) with provenance, plus a
   recorded `bd blocked --json` fixture from the P-6 probe workspace.
4. `bd-cache-model`: `Op`, `replay`, ports and fakes, `CacheConfig`,
   `DrainBudget`, `CallKind`, replies, `CacheError` with the mapping table
   (REQ-BDC-0301, 0304, 0305, 0802, 0803).
5. `bd-cache`: `Cargo.toml`, `lib.rs` with `Cache` and method stubs, the
   `call`, `outage`, `view`, `backend_check` and `miss` helpers implemented
   and tested (REQ-BDC-0105), empty module files for a-5 and a-6,
   `tests/common/` fixtures.
6. Contract suites: one every `CacheBackend` must pass (against
   `FakeBackend`), one every `CacheStore` + `DrainLock` must pass (against
   `FakeStore` + `FakeDrainLock`), covering every rule in the `CacheStore`
   doc comment (NFR-BDC-0005).
7. Docs: `docs/sc-beads/`, `docs/bd-cache-model/` (requirements and
   architecture); `docs/bd-cache/requirements.md` and `architecture.md`
   with ADR-CORE-0001 (threading model) and an index linking
   `write-path.md` (a-5) and `read-path.md` (a-6), which carry the REQ-CORE
   and ADR-CORE entries for each path. `docs/architecture.md`: ADR-BDC-0007
   records the P-3 outcome.

## Acceptance criteria

| Criterion | Root |
|---|---|
| `test_pinned_model_matches_recorded_schema`, `test_recorded_bd_rows_validate` (REQ-BDC-0007) | boundary:sc-beads |
| A row with an unknown key, missing key or wrong type is `BeadsError::Contract` naming bead and field (REQ-BDC-0007) | boundary:sc-beads |
| btit's traits and types match the API snapshot at `8d8e82ae9f85`; cargo features unchanged (REQ-BDC-0906) | ADR-BDC-0007 |
| `FakeBackend` rows carry no `is_blocked`; `list_blocked` matches the probe's recorded cases (blocks, parent-child inheritance, related, outside blocker, close and reopen) | ADR-BDC-0002 |
| `test_bad_ops_are_typed`: bad op name `BAD_OP`; unknown dep type and each rejected metadata class `BAD_ARGS` from `Op::parse` (REQ-BDC-0301, 0304) | boundary:bd-cache-model |
| `Op::to_args_json` round-trips Python-written journal args fixtures; equal ops compare equal (REQ-BDC-0301) | boundary:bd-cache-model |
| `overlay`, `achieved` and `replay` match the op table; replay skips escalated entries (REQ-BDC-0305) | boundary:bd-cache-model |
| Golden JSON for every `CacheError` code incl. `PRECONDITION` with `unmet` and `reason`; `exit_code()` is 3; `from_backend` matches the table for every `CallKind` (REQ-BDC-0802, 0803) | boundary:bd-cache-model |
| `test_schema_change_rebuilds_rows_and_keeps_the_journal`, `test_model_current_when_live_schema_matches_pin`, `test_first_backend_check_records_schema_without_dropping_cached_rows` through `backend_check::run` (REQ-BDC-0105) | boundary:bd-cache |
| `call` hands `min(call_timeout, hard)`; a timeout after `hard` expired is `Cut` and records no outage; `Unavailable` on a check or list records the outage, on a write counts an attempt; only an answering list clears it | ADR-BDC-0005 |
| `test_outage_keeps_its_start_time_and_latest_error` | ADR-BDC-0005 |
| `miss::fill` stops at `miss_fill_timeout` with `BD_UNAVAILABLE`; `view::current_view` replays pending entries | boundary:bd-cache |
| Both contract suites pass against the fakes, covering every listed store rule, including the ready-base, scope and `applied_since` rules (NFR-BDC-0005) | ADR-BDC-0004 |
| `RepoConfig` parses a sample `.beads/bd-cache.toml` and derives `CacheConfig` with defaults | boundary:bd-cache-model |
| CI runs green on the sprint PR, including the boundary `cargo metadata` check | boundary:bd-cache |
| `cargo publish --dry-run -p sc-beads` passes | ADR-BDC-0007 |
| Planted bugs: no overlay, no metadata coercion guard, skip validation, no dep-type check, skip rebuild, rebuild on first check each fail a test | boundary:bd-cache-model |

## This sprint does not close

Running `bd` (a-2), SQLite and flock (a-4), write and drain (a-5), read,
sync and refresh (a-6), scheduling (a-7).

## Required validation

CI (`.github/workflows/ci.yml`) green.
