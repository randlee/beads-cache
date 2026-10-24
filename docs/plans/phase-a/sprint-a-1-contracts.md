---
id: a-1
phase: a
status: planned
closure_type: contract
target_boundary: sc-beads, bd-cache-model, bd-cache (skeleton and shared helpers)
vertical_rationale: "Contract sprint for three boundaries every wave-2 sprint consumes: the shared-trait crate, the pure model crate whose behaviour is the contract, and the bd-cache skeleton with the three helpers (view, backend_check, miss) that both a-5 and a-6 call, so neither depends on the other."
track: contracts
wave: 1
branch: sprint/a-1-contracts
worktree: sprint/a-1-contracts
recommended_agent: rust-developer
recommended_model: opus
must_follow: []
parallel_safe: [a-3]
requirements:
  owns: [REQ-BDC-0007, REQ-BDC-0301, REQ-BDC-0304, REQ-BDC-0305, REQ-BDC-0802, REQ-BDC-0803, REQ-BDC-0906, NFR-BDC-0005]
  constrained_by: [REQ-BDC-0004, REQ-BDC-0108, REQ-BDC-0202, REQ-BDC-0303, REQ-BDC-0402, REQ-BDC-0604, REQ-BDC-0804, REQ-BDC-0901, REQ-BDC-0902]
adrs: [ADR-BDC-0001, ADR-BDC-0002, ADR-BDC-0003, ADR-BDC-0004, ADR-BDC-0005, ADR-BDC-0007]
owned_paths:
  - "Cargo.toml"
  - "rust-toolchain.toml"
  - ".gitignore"
  - "crates/sc-beads/**"
  - "crates/bd-cache-model/**"
  - "crates/bd-cache/Cargo.toml"
  - "crates/bd-cache/src/lib.rs"
  - "crates/bd-cache/src/view.rs"
  - "crates/bd-cache/src/backend_check.rs"
  - "crates/bd-cache/src/miss.rs"
  - "crates/bd-cache/tests/common/**"
  - "docs/sc-beads/**"
  - "docs/bd-cache-model/**"
  - "docs/bd-cache/**"
  - "docs/architecture.md"
---

# Sprint a-1 — contracts

## Outcome and closure

Freeze every interface phase A shares, with test doubles and contract tests,
so the five wave-2 sprints run in parallel without amending a-1. Closes when
the workspace builds, `FakeBackend` and `FakeStore` pass their contract
suites, the JSON shapes have golden tests, and the signatures below are
committed.

`sc-beads` is the backend contract crate only, ported from btit-beads and
btit-types; implementations live in `sc-bd-cli` (a-2) and later
`sc-bd-http`. `bd-cache-model` is pure (no I/O). The `bd-cache` skeleton
carries the `Cache` facade and the three helpers both halves of the core call.

## Decision prerequisites

- P-3: btit's traits stay unchanged either way; the cache methods are on the
  `CacheBackend: BeadsBackend` extension trait. P-3 only confirms the shared
  error type and the tolerant/strict row split.
- P-6: the `--id` ∩ `--updated-after` probe result decides `list_changed`'s
  implementation note (one filtered call, or `list_by_ids` filtered on
  `updated_at`); the signature is the same either way.
- Strict `Bead` generation from the bd-1.3.1 schema fixture; the generator
  command and pinned sha go in the module header.

## Public contract

Normative. Layer sprints reference these and do not restate them. Every type
is `Send + Sync` unless noted.

```rust
// ================= sc-beads =================
pub struct ProjectRef { /* btit: beads root, optional BEADS_DIR */ }
pub struct BeadId(String);
pub struct Actor(String);
pub enum DepType { /* bd's dependency types; DepType::parse(&str) -> Option<Self> */ }
pub struct Edge { pub issue_id: BeadId, pub depends_on_id: BeadId, pub kind: DepType }
/// Strict row, generated from `bd schema` (deny_unknown_fields) plus the four --json extras.
pub struct Bead { /* id, status, assignee, labels, metadata, notes, is_blocked, updated_at, dependencies: Vec<Edge>, ... */ }
pub struct BdRawIssue { /* btit's tolerant row, unchanged */ }
pub struct Fingerprint { pub bd_version: String, pub schema_sha256: String, pub schema_version: String }
pub enum ScopeFilter { Label(String) }
pub struct SyncCursor(String); // RFC 3339 minted by the cache; the backend treats it as opaque
pub struct Guards { pub status: Option<String>, pub assignee: Option<String> } // "" = unassigned
pub enum MetaValue { Int(i64), Bool(bool), Str(String) }
pub struct Patch { pub status: Option<String>, pub assignee: Option<String>,
    pub add_labels: Vec<String>, pub remove_labels: Vec<String>,
    pub set_metadata: std::collections::BTreeMap<String, MetaValue>, pub unset_metadata: Vec<String>,
    pub append_notes: Option<String> }                       // /v0-aligned (ADR-BDC-0003)
pub struct Deadline { /* monotonic instant from a Clock */ }
impl Deadline { pub fn after(clock: &dyn Clock, d: Duration) -> Self; pub fn remaining(&self, clock: &dyn Clock) -> Duration; pub fn expired(&self, clock: &dyn Clock) -> bool; }
pub trait Clock: Send + Sync { fn now_utc(&self) -> String /* RFC 3339, seconds */; fn now_mono(&self) -> std::time::Instant; }
pub struct SystemClock;
pub mod testing { pub struct ManualClock; /* advance(Duration), set_utc(&str) */ }

/// Refusals bd can report. The CLI only distinguishes exit 13 (Precondition);
/// NotClosable / AlreadyClaimed come from the HTTP implementation's 409 codes.
pub enum Refusal { Precondition, NotClosable, AlreadyClaimed, Other(String) }
pub enum BeadsError {
    Unavailable { reason: String },  // timeout, cannot start, connection refused
    DeadlineCut,                     // cut short by the caller's hard deadline (not an outage)
    Refused(Refusal),
    Contract { bead: Option<BeadId>, field: String, detail: String },
    Failed { code: i32, stderr: String },
    // btit's existing variants kept
}

// btit's traits, unchanged: BeadsBackend, CliBackend, DoltOperations.
/// The cache's backend surface. Implemented by sc-bd-cli (a-2) and FakeBackend.
pub trait CacheBackend: BeadsBackend + Send + Sync {
    fn fingerprint(&self, p: &ProjectRef, d: &Deadline) -> Result<Fingerprint, BeadsError>;
    fn list_by_ids(&self, p: &ProjectRef, ids: &[BeadId], d: &Deadline) -> Result<Vec<Bead>, BeadsError>;
    fn list_scope(&self, p: &ProjectRef, f: &ScopeFilter, since: Option<&SyncCursor>, d: &Deadline) -> Result<Vec<Bead>, BeadsError>;
    fn list_changed(&self, p: &ProjectRef, ids: &[BeadId], since: &SyncCursor, d: &Deadline) -> Result<Vec<Bead>, BeadsError>;
    fn close_guarded(&self, p: &ProjectRef, id: &BeadId, reason: Option<&str>, g: &Guards, a: &Actor, d: &Deadline) -> Result<(), BeadsError>;
    fn update_guarded(&self, p: &ProjectRef, id: &BeadId, patch: &Patch, g: &Guards, a: &Actor, d: &Deadline) -> Result<(), BeadsError>;
    fn dep_add_typed(&self, p: &ProjectRef, id: &BeadId, on: &BeadId, kind: DepType, a: &Actor, d: &Deadline) -> Result<(), BeadsError>;
}
pub mod testing {
    /// Deterministic bd model: validated rows; settles `is_blocked` on dependers and parent-child
    /// descendants on close/reopen WITHOUT bumping their `updated_at`; `--id` ∩ `--updated-after`
    /// semantics for list_changed; records the command plan; injects Unavailable, DeadlineCut,
    /// exit-13 and Failed per call; advances a ManualClock by a configured cost per call.
    pub struct FakeBackend;
}

// ================= bd-cache-model =================
pub enum Op { Close { reason: Option<String> }, Update(Patch), DepAdd { depends_on: BeadId, kind: DepType } }
impl Op {
    pub fn parse(op: &str, args_json: &str) -> Result<Op, CacheError>;   // BAD_OP, BAD_ARGS (incl. dep type, metadata coercion)
    pub fn to_args_json(&self) -> String;                                 // canonical, Python-compatible
    pub fn overlay(&self, row: &Bead) -> Bead;
    pub fn achieved(&self, row: &Bead) -> bool;                           // dep_add: edge in row.dependencies
}
pub fn replay(row: &Bead, pending: &[JournalEntry]) -> Bead;             // seq order; escalated skipped
pub struct Seq(i64); pub struct IdemKey(String);
pub enum JournalState { Pending, Applied, Escalated, Resolved }
pub struct JournalEntry { pub seq: Seq, pub idem_key: IdemKey, pub scope: Option<ScopeId>, pub bead: BeadId,
    pub op: Op, pub precondition: Guards, pub actor: Actor, pub state: JournalState, pub attempts: u32,
    pub last_error: Option<String>, pub announced_at: Option<String> }
pub struct NewEntry { pub idem_key: IdemKey, pub scope: Option<ScopeId>, pub bead: BeadId, pub op: Op, pub precondition: Guards, pub actor: Actor }
pub enum InsertOutcome { Inserted(Seq), Duplicate(Seq), IdemConflict } // same key: equal (bead, op name, canonical args) => Duplicate
pub enum ScopeId { Phase(String) }
impl ScopeId { pub fn filter(&self) -> ScopeFilter; /* Phase(x) -> Label("phase-x") */ }
pub enum Freshness { Fresh, Stale }
pub struct SyncState { pub scope: ScopeId, pub cursor: Option<SyncCursor>, pub last_started_at: Option<String>, pub last_ok_at: Option<String>, pub last_error: Option<String> }
pub struct Outage { pub since: String, pub last_error: String, pub announced: bool }
pub enum FingerprintOutcome { Recorded, Unchanged, Rebuilt }
pub struct UpsertReport { pub stored: Vec<BeadId>, pub skipped_newer: Vec<BeadId>, pub status_changed: Vec<BeadId> }
pub struct OpenWork { pub pending: u64, pub escalated_unannounced: u64 }
pub struct StoreStatus { /* meta, journal counts per scope:state, per-scope sync state and bead counts, escalated entries, outage */ }

/// Store port; a-4 implements it over SQLite. No DDL here. Rules every implementation obeys
/// (and the contract suite checks): a successful store of a bead clears `stale`;
/// `upsert_rows(Some(s), ..)` sets that bead's membership of `s` from its labels;
/// rows stored with `None` have no membership (out-of-scope rows, REQ-BDC-0902).
pub trait CacheStore: Send {
    fn ensure_scope(&mut self, s: &ScopeId) -> Result<(), CacheError>;
    fn get_row(&self, id: &BeadId) -> Result<Option<(Bead, Freshness)>, CacheError>;
    fn rows_in_scope(&self, s: &ScopeId) -> Result<Vec<(Bead, Freshness)>, CacheError>;
    fn upsert_rows(&mut self, s: Option<&ScopeId>, rows: &[Bead], force: bool) -> Result<UpsertReport, CacheError>;
    fn mark_stale(&mut self, id: &BeadId) -> Result<(), CacheError>;
    fn stale_rows(&self) -> Result<Vec<BeadId>, CacheError>;
    fn dependents_of(&self, id: &BeadId) -> Result<Vec<BeadId>, CacheError>;
    fn outside_blockers(&self) -> Result<Vec<BeadId>, CacheError>;
    fn mark_blocker_changed(&mut self, id: &BeadId) -> Result<(), CacheError>;
    fn take_changed_blockers(&mut self) -> Result<Vec<BeadId>, CacheError>;
    fn insert_entry(&mut self, e: NewEntry, at: &str) -> Result<InsertOutcome, CacheError>;
    fn pending_entries(&self) -> Result<Vec<JournalEntry>, CacheError>;
    fn pending_for(&self, id: &BeadId) -> Result<Vec<JournalEntry>, CacheError>;
    fn apply_with_readback(&mut self, seq: Seq, row: Option<&Bead>, err: Option<&str>, at: &str) -> Result<(), CacheError>;
    fn record_attempt(&mut self, seq: Seq, err: &str, at: &str) -> Result<u32, CacheError>;
    fn escalate(&mut self, seq: Seq, reason: &str, at: &str) -> Result<(), CacheError>;
    fn resolve(&mut self, seq: Seq, note: &str, actor: &Actor, at: &str) -> Result<(), CacheError>; // NOT_ESCALATED
    fn unannounced_escalations(&self) -> Result<Vec<JournalEntry>, CacheError>;
    fn mark_announced(&mut self, seq: Seq, at: &str) -> Result<(), CacheError>;
    fn record_fingerprint(&mut self, f: &Fingerprint) -> Result<FingerprintOutcome, CacheError>;
    fn sync_state(&self, s: &ScopeId) -> Result<SyncState, CacheError>;
    fn record_sync(&mut self, st: &SyncState) -> Result<(), CacheError>;
    fn sync_alert(&self, s: &ScopeId) -> Result<Option<String>, CacheError>;
    fn set_sync_alert(&mut self, s: &ScopeId, cause: Option<&str>) -> Result<(), CacheError>;
    fn outage(&self) -> Result<Option<Outage>, CacheError>;
    fn set_outage(&mut self, o: Option<&Outage>) -> Result<(), CacheError>;
    fn prune_scope(&mut self, s: &ScopeId) -> Result<PruneReport, CacheError>;  // PRUNE_BLOCKED; keeps beads other scopes hold; sweeps out-of-scope rows with no open entry
    fn open_work(&self) -> Result<OpenWork, CacheError>;
    fn status(&self) -> Result<StoreStatus, CacheError>;
}
/// Cross-process single drain writer. a-4: flock on .atm-bd/<db>.bd-cache.drain.lock.
pub trait DrainLock: Send + Sync { fn try_acquire(&self) -> Result<Option<Box<dyn DrainGuard>>, CacheError>; }
pub trait DrainGuard: Send {}
pub mod testing { pub struct FakeStore; pub struct FakeDrainLock; }

pub struct SendError { pub detail: String }
/// Escalation delivery port; a-7 implements it with `atm send`.
pub trait EscalationSender: Send + Sync { fn send(&self, lead: &str, team: &str, body: &str) -> Result<(), SendError>; }

pub struct CacheConfig {
    pub project: ProjectRef, pub scopes: Vec<ScopeId>,
    pub drain_start_gate: Duration, pub drain_hard: Duration, // REQ-BDC-0402
    pub call_timeout: Duration,                               // 30 s cap per bd call
    pub miss_fill_timeout: Duration,                          // default 2 s (ADR-BDC-0005)
    pub max_attempts: u32,                                    // 5
}

// Replies (serde, JSON shapes golden-tested; the later bdc binary prints them):
pub struct GetReply { pub bead: Bead, pub stale: bool, pub scope_last_ok_at: Option<String>, pub pending: u32 }
pub struct ListReply { pub scope: ScopeId, pub last_ok_at: Option<String>, pub rows: Vec<(Bead, bool /*stale*/)> }
pub struct WriteReply { pub seq: Seq, pub duplicate: bool }
pub struct DrainReport { pub busy: bool, pub applied: Vec<Seq>, pub retrying: Vec<Seq>, pub escalated: Vec<EscalatedItem>,
    pub deferred: Vec<Seq>, pub deferred_in_flight: Vec<Seq>, pub outage: Option<Outage> }
pub struct EscalatedItem { pub seq: Seq, pub bead: BeadId, pub op: String, pub actor: Actor, pub phase: Option<String>, pub reason: String }
pub struct SyncReport { pub scope: ScopeId, pub rebuilt: bool, pub model_current: bool, pub stored: u32, pub refreshed_dependers: u32, pub outage: Option<Outage> }
pub struct PruneReport { pub scope: ScopeId, pub beads_deleted: u32, pub out_of_scope_swept: u32 }
pub struct StatusReply { /* StoreStatus plus outage */ }

/// One variant per baseline code, carrying its payload:
/// BD_UNAVAILABLE, NO_DATABASE, WRONG_DATABASE, NOT_FOUND, BD_VERSION_FAILED, BD_SCHEMA_FAILED,
/// BD_CONTRACT{bead, field}, BD_WRITE_FAILED, BD_LIST_FAILED, SYNC_FAILED, PRECONDITION{unmet: map key -> {expected, actual}},
/// IDEM_CONFLICT, BAD_OP, BAD_ARGS, BAD_PRECONDITION, SCHEMA_VERSION, PRUNE_BLOCKED, NOT_ESCALATED,
/// plus SCOPE_UNKNOWN{configured} (additive, REQ-BDC-0902). NO_TOML is retired (Python-only); never emitted.
pub enum CacheError { /* ... */ }
impl CacheError {
    pub fn code(&self) -> &'static str; pub fn fix(&self) -> &'static str;
    pub fn reason(&self) -> Option<&'static str>; // /v0: precondition_failed, not_closable, already_claimed
    pub fn exit_code(&self) -> i32;               // 3 for every typed error (0 ok, 2 usage are the binary's)
    pub fn to_json(&self) -> serde_json::Value;   // {ok: false, code, detail, fix, reason?, ...payload}
}

// ================= bd-cache (skeleton + shared helpers) =================
/// Not Sync (the SQLite connection is not); a-7 wraps it in Arc<Mutex<_>> and calls it under spawn_blocking.
pub struct Cache<S: CacheStore, L: DrainLock, B: CacheBackend> { /* config, store, lock, backend, clock: Arc<dyn Clock> */ }
impl<S: CacheStore, L: DrainLock, B: CacheBackend> Cache<S, L, B> {
    pub fn new(config: CacheConfig, store: S, lock: L, backend: B, clock: Arc<dyn Clock>) -> Result<Self, CacheError>; // ensure_scope for each
    pub fn get(&mut self, id: &BeadId) -> Result<GetReply, CacheError>;                                       // a-6
    pub fn list(&self, s: &ScopeId, status: Option<&str>, label: Option<&str>) -> Result<ListReply, CacheError>; // a-6
    pub fn write(&mut self, id: &BeadId, op: Op, pre: Guards, key: IdemKey, actor: Actor) -> Result<WriteReply, CacheError>; // a-5
    pub fn drain(&mut self) -> Result<DrainReport, CacheError>;                                              // a-5
    pub fn sync(&mut self, s: &ScopeId, d: &Deadline) -> Result<SyncReport, CacheError>;                      // a-6 (incl. refresh)
    pub fn resolve(&mut self, seq: Seq, note: &str, actor: Actor) -> Result<(), CacheError>;                  // a-6
    pub fn prune(&mut self, s: &ScopeId) -> Result<PruneReport, CacheError>;                                  // a-6
    pub fn status(&self) -> Result<StatusReply, CacheError>;                                                  // a-6
}
// a-1 implements these helpers (pub(crate)); a-5 and a-6 call them:
//   view::current_view(&self, id) -> Result<Option<(Bead, Freshness)>, CacheError>  // stored row + replay of pending
//   backend_check::run(&mut self, d) -> Result<(FingerprintOutcome, bool /*model_current*/), CacheError>
//   miss::fill(&mut self, id) -> Result<Option<Bead>, CacheError>  // list_by_ids under miss_fill_timeout; stored with no scope
// Module files a-5 fills: write.rs, drain.rs, outage.rs. a-6 fills: get.rs, list.rs, sync.rs, refresh.rs, resolve.rs, prune.rs, status.rs.
```

## Deliverables

1. Workspace `Cargo.toml` (`members = ["crates/*"]`) with every phase-A
   third-party dependency in `[workspace.dependencies]`, including rusqlite
   (bundled), fs4 (flock), criterion, tokio, async-trait, chrono, cron;
   `rust-toolchain.toml` pinning the toolchain; `.gitignore` with
   `/Cargo.lock` (removed by a-7).
2. `sc-beads`: btit's traits unchanged, `CacheBackend`, rows, errors,
   `Clock`/`Deadline`, `FakeBackend` with the semantics above (REQ-BDC-0906,
   0007); bd-1.3.1 fixtures (`bd-schema.json`, `list-rows.json`) copied
   with provenance.
3. `bd-cache-model`: `Op` parse, canonical args JSON, overlay, achieved,
   `replay` (REQ-BDC-0301, 0304, 0305); ports and fakes; `CacheConfig`;
   replies and reports; `CacheError` with payloads, fix text, `/v0` reason,
   exit code and JSON form (REQ-BDC-0802, 0803).
4. `bd-cache`: `Cargo.toml` (with the `[[bench]]` entry a-7 uses),
   `lib.rs` with `Cache` and the method stubs above, the `view`,
   `backend_check` and `miss` helpers implemented and tested, empty module
   files for a-5 and a-6, `tests/common/` fixtures shared by both.
5. Contract suites: one every `CacheBackend` must pass (run against
   `FakeBackend`), one every `CacheStore` + `DrainLock` must pass (run
   against `FakeStore` and `FakeDrainLock`), including the stale-clearing,
   membership and out-of-scope rules (NFR-BDC-0005).
6. Per-crate docs: `docs/sc-beads/`, `docs/bd-cache-model/`, `docs/bd-cache/`
   (`requirements.md` refining the owned `REQ-BDC-*`, `architecture.md` with
   crate ADRs). `docs/architecture.md`: ADR-BDC-0007 records the P-3 outcome.

## Acceptance criteria

| Criterion | Root |
|---|---|
| `test_pinned_model_matches_recorded_schema`, `test_recorded_bd_rows_validate` pass (REQ-BDC-0007) | boundary:sc-beads |
| A row with an unknown key, missing key or wrong type is `BeadsError::Contract` naming bead and field (REQ-BDC-0007) | boundary:sc-beads |
| `FakeBackend` settles dependers' `is_blocked` on close and reopen without bumping their `updated_at`, and `list_changed` returns only ids changed after the cursor (contract test) | ADR-BDC-0002 |
| `test_bad_ops_are_typed`: bad op name is `BAD_OP`; unknown dep type and every rejected metadata value class are `BAD_ARGS` from `Op::parse` (REQ-BDC-0301, 0304) | boundary:bd-cache-model |
| `Op::to_args_json` round-trips Python-written journal args fixtures, and equal ops compare equal for idempotency (REQ-BDC-0301) | boundary:bd-cache-model |
| `overlay`, `achieved` and `replay` match the op table and skip escalated entries (REQ-BDC-0305) | boundary:bd-cache-model |
| Golden JSON for every `CacheError` code, including `PRECONDITION` with `unmet` and the `reason` field; `exit_code()` is 3 (REQ-BDC-0802, 0803) | boundary:bd-cache-model |
| Golden JSON for `GetReply`, `ListReply`, `WriteReply`, `DrainReport`, `SyncReport`, `PruneReport`, `StatusReply` | boundary:bd-cache-model |
| `view::current_view` replays pending entries; `miss::fill` stops at `miss_fill_timeout` with `BD_UNAVAILABLE`; `backend_check::run` reports `Recorded`/`Unchanged`/`Rebuilt` | boundary:bd-cache |
| Both contract suites pass against the fakes (NFR-BDC-0005) | ADR-BDC-0004 |
| btit's traits and cargo features are present with unchanged signatures (REQ-BDC-0906) | ADR-BDC-0007 |
| `cargo publish --dry-run -p sc-beads` passes | ADR-BDC-0007 |
| Planted bugs: no overlay, no metadata coercion guard, skip validation, no dep-type check each fail a test | boundary:bd-cache-model |

## This sprint does not close

Running `bd` (a-2), SQLite and flock (a-4), write and drain (a-5), read,
sync and refresh (a-6), scheduling (a-7).

## Required validation

`cargo fmt --all --check`; `cargo clippy --all-targets --all-features -- -D
warnings`; `cargo test --workspace --all-features`.
