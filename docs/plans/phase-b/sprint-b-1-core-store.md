---
id: b-1
phase: b
status: ready
closure_type: boundary
target_boundary: bd-cache, bd-cache-model, bd-cache-store
track: core
wave: 1
branch: sprint/b-1-core-store
worktree: sprint/b-1-core-store
recommended_agent: rust-developer
recommended_model: opus
must_follow: []
parallel_safe: [b-2, b-3a]
requirements:
  owns: [REQ-BDC-0809]
  constrained_by: [REQ-BDC-0004, REQ-BDC-0103, REQ-BDC-0107, REQ-BDC-0804, REQ-BDC-0908, REQ-BDC-0910, NFR-BDC-0001, NFR-BDC-0002]
adrs: [ADR-BDC-0003, ADR-BDC-0004, ADR-BDC-0009, ADR-MODEL-0002, ADR-STORE-0001, ADR-CORE-0206]
owned_paths:
  - "crates/bd-cache/**"
  - "crates/bd-cache-model/**"
  - "crates/bd-cache-store/**"
  - "docs/bd-cache/**"
  - "docs/bd-cache-model/**"
  - "docs/bd-cache-store/**"
  - "crates/bd-cache-sync/src/compose.rs"   # compose_with_config only
  - "crates/bd-cache-sync/src/**/tests*"    # construction sites of the extended types only
  - "crates/bd-cache-py/tests/**"           # 0.1 dict key-set assertions only
---

# Sprint b-1 — core and store for the contract

## Outcome

The core and store provide what contract §9 "Core changes" lists, so that
`bd-cache-api` can build every reply without a second transaction or a store
scan. The 0.1 Python dicts stay byte-for-byte the same.

## Public contract

These signatures are normative. b-2 codes against them from day one.

```rust
// bd-cache-model
pub struct WriteOutcome {
    pub entry: JournalEntry,   // as stored, after this call
    pub before: RowView,       // the view before the write
    pub after: RowView,        // the view after the write (contract §5.3 "issue")
    pub duplicate: bool,
    pub changed: Option<bool>, // S68: Some(stored); Some(predicted) only for a Pending pre-b-1 entry with a stored row; None (unknown) otherwise
}
pub struct StateCounts { pub pending: u64, pub applied: u64, pub escalated: u64, pub resolved: u64 }
pub struct JournalCounts {
    pub total: StateCounts,
    pub by_scope: BTreeMap<ScopeId, StateCounts>,
    pub unscoped: StateCounts,
}
// added fields
GetReply     { /* … */ pub scope: Option<ScopeId> }
JournalEntry { /* … */ pub last_error_code: Option<String>, pub note: Option<String>, pub changed: Option<bool> }

// bd-cache::Cache
pub fn write_returning(&mut self, id: &BeadId, op_name: &str, args_json: &str,
    pre: Guards, key: IdemKey, actor: Actor) -> Result<WriteOutcome, CacheError>;
pub fn entry_by_key(&self, key: &IdemKey) -> Result<JournalEntry, CacheError>; // NOT_FOUND when absent
pub fn journal_counts(&self) -> Result<JournalCounts, CacheError>;

// bd-cache-sync
pub fn compose_with_config(repo_root: &Path, config: RepoConfig, bd_binary: &str,
    clock: Arc<dyn Clock>, beads_dir_env: Option<&OsStr>) -> Result<Composed, ComposeError>;
```

`changed` for a close stores `!already_closed`, where `already_closed`
describes the view before the write. `Cache::write` keeps its signature and
reply, and becomes a thin wrapper over `write_returning`.

## Deliverables

1. `write_returning` (REQ-BDC-0809). One store transaction does three
   things: journals the entry, computes `before` and `after`, and stores
   `changed`. A duplicate returns the stored entry and its stored `changed`;
   guards and actor are not compared (contract §5.1).
   - Files: `crates/bd-cache/src/write.rs`, `crates/bd-cache-model/src/reply.rs`.
2. `entry_by_key` on the facade, over the existing
   `CacheStore::entry_by_key`. File: `crates/bd-cache/src/get.rs`.
3. `journal_counts` (contract §6, `cache status` `journal`). It is one
   grouped query, not a journal scan in Rust.
   - Files: `CacheStore::journal_counts` in `crates/bd-cache-model/src/store.rs`,
     with `FakeStore` and `SqliteStore` implementations, and the facade in
     `crates/bd-cache/src/status.rs`.
4. `GetReply.scope`: the bead's scope, `None` when it has none.
   - Files: `crates/bd-cache/src/get.rs`, `crates/bd-cache-model/src/reply.rs`.
5. The store columns `last_error_code`, `note` and `changed` (the third one
   accepted by lead ruling S62, bc-lead, 2026-10-25, and named in contract
   §9): additive and nullable, with `user_version` still 3 (REQ-BDC-0804). `resolve` writes
   `note` and stops appending to `last_error`.
   - Reading a Python-written entry with `note` null splits a trailing
     `" | resolved by <actor>: <note>"` out of `last_error` into `note`.
   - Reading a Python-written entry with `last_error_code` null takes the
     code from the `CODE: detail` prefix.
   - Files: `crates/bd-cache-store/src/{schema,store,codec}.rs`, ADR-STORE-0001
     amendment in `docs/bd-cache-store/architecture.md`.
6. The new model fields are not serialized into the 0.1 Python replies.
   bd-cache-api has its own data model, so the 0.1 dicts keep their key sets.
7. `compose_with_config`: `compose` with an already-merged `RepoConfig`, for
   bdc's flag and env overrides (contract §2). `compose` becomes a wrapper.
   File: `crates/bd-cache-sync/src/compose.rs`.

## Acceptance criteria

| # | Root | Positive observable | Planted negative |
|---|---|---|---|
| 1 | REQ-BDC-0809 | A test reads the store while the write transaction holds the drain lock and sees either no entry, or the entry together with `changed`, never one without the other. `after` equals a subsequent `get`'s row. | Commit the entry in a separate transaction from `changed` → the test fails. |
| 2 | REQ-BDC-0004 | A retry with the same key and a different actor and guard returns `duplicate: true`, the original `seq` and the original `changed`. | Recompute `changed` on a duplicate → the test fails, because the overlay already applied makes it `false`. |
| 3 | REQ-BDC-0908 | `entry_by_key` returns the entry in any state; an unknown key returns `NOT_FOUND`. | Return `Ok` with a default entry for an unknown key → the test fails. |
| 4 | contract §6 `journal` | Counts over a seeded journal (four states, two scopes, unscoped entries) equal a hand count, and the store issues one query (the store's query counter is 1). | Drop the `unscoped` bucket → totals ≠ the sum of the buckets, and the test fails. |
| 5 | contract §5.3 `issues get` | `GetReply.scope` is set for a scoped bead and `None` for an out-of-scope row. | Always return the first configured scope → the out-of-scope case fails. |
| 6 | REQ-BDC-0804 | The vendored Python `bd_cache.py` opens a file the Rust store wrote with the new columns, and resolves an entry. The Rust store reads that entry back with `note` and `last_error_code` split out. | Put `note` back into `last_error` only → the Rust split test fails; or bump `user_version` → the Python open fails. |
| 7 | REQ-BDC-0910 | bd-cache-py's 0.1 tests pass unchanged. A new key-set assertion pins the 0.1 `get` and `entry` dicts. | Serialize `GetReply.scope` → the key-set assertion fails. |
| 8 | NFR-BDC-0001 | The existing in-process `get` latency test stays green, with the scope lookup for `GetReply.scope` included. | Resolve the scope by scanning the scope's members → the 1 ms bound fails on the 1,000-row fixture. |

## Dependencies

None inbound. b-2, b-3 and b-4 consume the Public contract.
