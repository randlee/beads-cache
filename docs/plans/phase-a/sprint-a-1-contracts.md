---
id: a-1
phase: a
status: planned
closure_type: contract
target_boundary: sc-beads, bd-cache-model, bd-cache (skeleton)
vertical_rationale: "Contract sprint for three boundaries every wave-2 sprint consumes: the shared-trait crate, the pure model crate whose behaviour is the contract, and the bd-cache facade skeleton that gives a-5 and a-6 disjoint modules."
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
  constrained_by: [REQ-BDC-0108, REQ-BDC-0303, REQ-BDC-0604, REQ-BDC-0901]
adrs: [ADR-BDC-0001, ADR-BDC-0002, ADR-BDC-0003, ADR-BDC-0004, ADR-BDC-0005, ADR-BDC-0007]
owned_paths:
  - "Cargo.toml"
  - "rust-toolchain.toml"
  - ".gitignore"
  - "crates/sc-beads/**"
  - "crates/bd-cache-model/**"
  - "crates/bd-cache/Cargo.toml"
  - "crates/bd-cache/src/lib.rs"
  - "docs/architecture.md"
---

# Sprint a-1 — contracts

## Outcome and closure

Freeze every interface phase A shares, with test doubles and contract tests,
so the five wave-2 sprints run in parallel. Closes when the workspace builds,
`FakeBackend` and `FakeStore` pass their contract tests, and the signatures
below are committed.

`sc-beads` is the backend contract crate only, ported from btit-beads and
btit-types; implementations live in `sc-bd-cli` (a-2) and later
`sc-bd-http`. `bd-cache-model` is pure (no I/O); its behaviour (overlay,
achieved, argument validation) is contract content, tested here.

## Decision prerequisites

- P-3 (shared API). Fallback in the phase plan: btit's traits unchanged plus a
  `CacheBackend: BeadsBackend` extension trait.
- Strict `Bead` generation from the vendored bd-1.3.1 schema fixture; record
  the generator command and pinned sha in the module header.

## Public contract

Normative. Layer sprints reference these and do not restate them.

```rust
// ---- sc-beads ----
pub struct ProjectRef { /* btit: beads root, optional BEADS_DIR */ }
pub struct BeadId(String);
pub struct Actor(String);
pub struct Deadline(std::time::Instant);          // monotonic; every call is capped by it
pub enum DepType { /* bd's dependency types, from bd schema */ }
pub struct Edge { pub issue_id: BeadId, pub depends_on_id: BeadId, pub kind: DepType }

/// Strict row: generated from `bd schema` (deny_unknown_fields) plus the four --json extras.
pub struct Bead { /* id, status, assignee, labels, metadata, notes, is_blocked, updated_at, dependencies: Vec<Edge>, ... */ }
/// Tolerant row (btit's BdRawIssue), unchanged.
pub struct BdRawIssue { /* btit */ }

pub struct Fingerprint { pub bd_version: String, pub schema_sha256: String, pub schema_version: String }
pub enum ScopeFilter { Label(String) /* phase-<x>; more kinds later (ADR-BDC-0001) */ }
pub struct SyncCursor(String);                    // opaque: RFC 3339 today, commit hash or seq later
pub struct Guards { pub status: Option<String>, pub assignee: Option<String> } // "" = unassigned
pub struct Patch {                                 // /v0-aligned (ADR-BDC-0003)
    pub status: Option<String>, pub assignee: Option<String>,
    pub add_labels: Vec<String>, pub remove_labels: Vec<String>,
    pub set_metadata: std::collections::BTreeMap<String, MetaValue>, pub unset_metadata: Vec<String>,
    pub append_notes: Option<String>,
}
pub enum MetaValue { Int(i64), Bool(bool), Str(String) }

pub enum Refusal { Precondition, NotClosable, AlreadyClaimed, Other(String) }
pub enum BeadsError {
    Unavailable { reason: String },               // timeout, cannot start, connection refused (ADR-BDC-0005)
    Refused(Refusal),                             // bd exit 13 -> Precondition
    Contract { bead: Option<BeadId>, field: String, detail: String },
    Failed { code: i32, stderr: String },
    // btit's existing variants kept
}

pub trait BeadsBackend: Send + Sync {
    // btit's methods, signatures unchanged: list, ready, status, show, create, update(UpdatePayload),
    // close, search, label_add, label_remove, delete, comment_add, dep_add, dep_remove, relation_types,
    // sync, cli(), dolt()
    // cache extensions (on `CacheBackend: BeadsBackend` under the P-3 fallback):
    fn fingerprint(&self, p: &ProjectRef, d: Deadline) -> Result<Fingerprint, BeadsError>;
    fn list_by_ids(&self, p: &ProjectRef, ids: &[BeadId], d: Deadline) -> Result<Vec<Bead>, BeadsError>;
    fn list_scope(&self, p: &ProjectRef, f: &ScopeFilter, since: Option<&SyncCursor>, d: Deadline)
        -> Result<Vec<Bead>, BeadsError>;
    fn list_changed(&self, p: &ProjectRef, ids: &[BeadId], since: &SyncCursor, d: Deadline)
        -> Result<Vec<Bead>, BeadsError>;     // outside blockers (ADR-BDC-0002)
    fn close_guarded(&self, p: &ProjectRef, id: &BeadId, reason: Option<&str>, g: &Guards, a: &Actor, d: Deadline)
        -> Result<(), BeadsError>;
    fn update_guarded(&self, p: &ProjectRef, id: &BeadId, patch: &Patch, g: &Guards, a: &Actor, d: Deadline)
        -> Result<(), BeadsError>;
    fn dep_add_typed(&self, p: &ProjectRef, id: &BeadId, on: &BeadId, kind: DepType, a: &Actor, d: Deadline)
        -> Result<(), BeadsError>;
}
pub mod testing { pub struct FakeBackend; /* validated rows, recorded command plan, injected failures, timeouts, exit-13 */ }

// ---- bd-cache-model ----
pub enum Op { Close { reason: Option<String> }, Update(Patch), DepAdd { depends_on: BeadId, kind: DepType } }
impl Op {
    pub fn validate(&self) -> Result<(), CacheError>;            // BAD_OP / BAD_ARGS incl. metadata coercion rule
    pub fn overlay(&self, row: &Bead) -> Bead;
    pub fn achieved(&self, row: &Bead, edges: &[Edge]) -> bool;
}
pub struct Seq(i64);
pub struct IdemKey(String);
pub enum JournalState { Pending, Applied, Escalated, Resolved }
pub struct JournalEntry { pub seq: Seq, pub idem_key: IdemKey, pub scope: Option<String>, pub bead: BeadId,
    pub op: Op, pub precondition: Guards, pub actor: Actor, pub state: JournalState, pub attempts: u32,
    pub last_error: Option<String>, pub announced_at: Option<String> }
pub struct NewEntry { pub idem_key: IdemKey, pub scope: Option<String>, pub bead: BeadId, pub op: Op,
    pub precondition: Guards, pub actor: Actor }
pub enum InsertOutcome { Inserted(Seq), Duplicate(Seq), IdemConflict }
pub enum ScopeId { Phase(String) }
pub struct SyncState { pub scope: ScopeId, pub cursor: Option<SyncCursor>, pub last_started_at: Option<String>,
    pub last_ok_at: Option<String>, pub last_error: Option<String> }
pub enum Freshness { Fresh, Stale }
pub enum FingerprintOutcome { Recorded, Unchanged, Rebuilt }
pub struct OpenWork { pub pending: u64, pub escalated_unannounced: u64 }
pub struct StoreStatus { /* meta, journal counts per scope:state, per-scope sync state and bead counts, escalated entries */ }

/// The store port; a-4 implements it over SQLite. No DDL here.
pub trait CacheStore {
    fn get_row(&self, id: &BeadId) -> Result<Option<(Bead, Freshness)>, CacheError>;
    fn rows_in_scope(&self, s: &ScopeId) -> Result<Vec<Bead>, CacheError>;
    fn upsert_rows(&mut self, s: Option<&ScopeId>, rows: &[Bead], force: bool) -> Result<(), CacheError>; // newer-row guard unless force
    fn mark_stale(&mut self, id: &BeadId) -> Result<(), CacheError>;
    fn stale_rows(&self) -> Result<Vec<BeadId>, CacheError>;
    fn dependents_of(&self, id: &BeadId) -> Result<Vec<BeadId>, CacheError>;   // edges index by target
    fn outside_blockers(&self) -> Result<Vec<BeadId>, CacheError>;
    fn mark_blocker_changed(&mut self, id: &BeadId) -> Result<(), CacheError>;
    fn take_changed_blockers(&mut self) -> Result<Vec<BeadId>, CacheError>;
    fn insert_entry(&mut self, e: NewEntry) -> Result<InsertOutcome, CacheError>;
    fn pending_entries(&self) -> Result<Vec<JournalEntry>, CacheError>;
    fn pending_for(&self, id: &BeadId) -> Result<Vec<JournalEntry>, CacheError>;
    fn apply_with_readback(&mut self, seq: Seq, row: Option<&Bead>, err: Option<&str>) -> Result<(), CacheError>; // one transaction
    fn record_attempt(&mut self, seq: Seq, err: &str) -> Result<u32, CacheError>;
    fn escalate(&mut self, seq: Seq, reason: &str) -> Result<(), CacheError>;
    fn resolve(&mut self, seq: Seq, note: &str, actor: &Actor) -> Result<(), CacheError>;
    fn unannounced_escalations(&self) -> Result<Vec<JournalEntry>, CacheError>;
    fn mark_announced(&mut self, seq: Seq, at: &str) -> Result<(), CacheError>;
    fn record_fingerprint(&mut self, f: &Fingerprint) -> Result<FingerprintOutcome, CacheError>; // rebuild rule
    fn sync_state(&self, s: &ScopeId) -> Result<SyncState, CacheError>;
    fn record_sync(&mut self, st: &SyncState) -> Result<(), CacheError>;
    fn sync_alert(&self, s: &ScopeId) -> Result<Option<String>, CacheError>;
    fn set_sync_alert(&mut self, s: &ScopeId, cause: Option<&str>) -> Result<(), CacheError>;
    fn prune_scope(&mut self, s: &ScopeId) -> Result<(), CacheError>;          // PRUNE_BLOCKED; keeps beads other scopes hold
    fn open_work(&self) -> Result<OpenWork, CacheError>;
    fn status(&self) -> Result<StoreStatus, CacheError>;
}
pub mod testing { pub struct FakeStore; /* in-memory CacheStore */ }

/// Escalation delivery port; a-7 provides the atm-send implementation.
pub trait EscalationSender {
    fn send(&self, lead: &str, team: &str, body: &str) -> Result<(), SendError>;
}

pub enum CacheError { /* one variant per BD_CACHE.* code in the baseline CLI table */ }
impl CacheError {
    pub fn code(&self) -> &'static str;           // "BD_CACHE.PRECONDITION", ...
    pub fn fix(&self) -> &'static str;
    pub fn reason(&self) -> Option<&'static str>; // /v0 name: precondition_failed, not_closable, already_claimed
    pub fn to_json(&self) -> serde_json::Value;   // {ok: false, code, detail, fix, ...}
}

// ---- bd-cache (skeleton only) ----
pub struct Cache<S: CacheStore, B: BeadsBackend> { /* store, backend, project, clock */ }
// lib.rs declares: mod write; mod drain; mod outage;                                   (bodies: a-5)
//                  mod get; mod list; mod sync; mod refresh; mod resolve; mod prune; mod status; (bodies: a-6)
```

## Deliverables

1. Workspace `Cargo.toml` with `members = ["crates/*"]` and every phase-A
   third-party dependency in `[workspace.dependencies]`;
   `rust-toolchain.toml`; `.gitignore` with `/Cargo.lock` (removed by a-7).
2. `sc-beads`: traits, rows, errors and `FakeBackend`, keeping btit's traits
   and cargo features (REQ-BDC-0906, REQ-BDC-0007); vendored bd-1.3.1
   fixtures (`bd-schema.json`, `list-rows.json`) with provenance.
3. `bd-cache-model`: ops with validate, overlay and achieved (REQ-BDC-0301,
   0304, 0305), the `CacheStore` port and `FakeStore`, the
   `EscalationSender` port, `CacheError` with codes, fix text, `/v0` reason
   and JSON form (REQ-BDC-0802, 0803).
4. `bd-cache` crate skeleton: `Cargo.toml`, `lib.rs` with the `Cache` struct
   and empty module files declared for a-5 and a-6.
5. Contract tests: one suite every `BeadsBackend` implementation must pass
   (run here against `FakeBackend`), one every `CacheStore` implementation
   must pass (run here against `FakeStore`) (NFR-BDC-0005).
6. `docs/architecture.md`: ADR-BDC-0007 records the P-3 outcome (shared
   trait or extension trait).

## Acceptance criteria

| Criterion | Root |
|---|---|
| `test_pinned_model_matches_recorded_schema`, `test_recorded_bd_rows_validate` pass (REQ-BDC-0007) | boundary:sc-beads |
| An unknown key, missing key or wrong type is `BeadsError::Contract` naming bead and field (REQ-BDC-0007) | boundary:sc-beads |
| `test_bad_ops_are_typed` passes; the metadata coercion validator rejects every value class the baseline lists (REQ-BDC-0301, 0304) | boundary:bd-cache-model |
| `overlay` and `achieved` match the op table for close, each update field and dep_add (REQ-BDC-0301, 0305) | boundary:bd-cache-model |
| Every baseline error code has a `CacheError` variant with fix text, and `to_json` gives `{ok: false, code, detail, fix}` (REQ-BDC-0802, 0803) | boundary:bd-cache-model |
| The backend and store contract suites pass against `FakeBackend` and `FakeStore` (NFR-BDC-0005) | ADR-BDC-0004 |
| btit's traits and cargo features are present with unchanged signatures (REQ-BDC-0906) | ADR-BDC-0007 |
| `cargo publish --dry-run -p sc-beads` passes | ADR-BDC-0007 |
| Planted bugs: no metadata coercion guard, skip validation, no dep-type check each fail a test | boundary:bd-cache-model |

## This sprint does not close

Running `bd` (a-2), SQLite (a-4), write and drain (a-5), read and sync
(a-6), scheduling (a-7).

## Required validation

`cargo fmt --all --check`; `cargo clippy --all-targets --all-features -- -D
warnings`; `cargo test --workspace --all-features`.
