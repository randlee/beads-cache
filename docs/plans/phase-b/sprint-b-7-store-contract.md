---
id: b-7
phase: b
status: planned
closure_type: boundary
target_boundary: bd-cache-store, bd-cache, bd-cache-model
track: core
wave: 5
branch: sprint/b-7-store-contract
worktree: sprint/b-7-store-contract
recommended_agent: rust-developer
recommended_model: opus
must_follow: [b-6]
parallel_safe: [b-9]
requirements:
  owns: [REQ-BDC-0721, REQ-BDC-0722, REQ-BDC-0724, REQ-BDC-0726, REQ-BDC-0728, REQ-BDC-0729, "REQ-BDC-0804 (amended)"]
  constrained_by: [REQ-BDC-0007, REQ-BDC-0008, REQ-BDC-0502, REQ-BDC-0812, NFR-BDC-0005]
adrs: [ADR-STORE-0006, ADR-STORE-0001, ADR-BDC-0005, ADR-BDC-0010]
owned_paths:
  - "crates/bd-cache-store/** (except tests/compat*)"
  - "crates/bd-cache/**"
  - "crates/bd-cache-model/**"
  - "docs/bd-cache-store/**"
  - "docs/bd-cache/**"
  - "docs/bd-cache-model/**"
---

# Sprint b-7 — store and contract (work package WP-6a)

## Outcome

The store and the core library hold everything the tick, the Python methods and their hooks
need, as additive v3 extensions (ADR-STORE-0006, ADR-STORE-0001). The design is
[phase-b-boundary.md](phase-b-boundary.md) §Scopes, §Single flight, §Failure state and marking,
§Ingest, §Store extensions, §New codes and §Registration; this sprint does not restate it.

## Deliverables

1. **Store extensions** (ADR-STORE-0006, REQ-BDC-0804 amended). Every row of §Store
   extensions, through the existing mechanism (`PRAGMA table_info` / `CREATE TABLE IF NOT
   EXISTS` under `BEGIN IMMEDIATE`); `user_version` stays 3. The `sc-cron` schedule tables are
   created on the first `schedule set`, by b-8.
2. **Scope registry** (REQ-BDC-0721). `scope.method` (`builtin` by default) and
   `scope.revision`; a scope is a name plus a bd filter (Amendment A5). Removing a scope deletes
   its membership rows, not bead rows another scope still holds. `scope.method` and the
   `registration` table are how a consumer's registered Python `sync` method replaces the
   built-in sync for a scope (ANC-003); b-7 stores them, and b-8's tick dispatches on them.
3. **`ingest_scope`** (REQ-BDC-0726, REQ-BDC-0007, REQ-BDC-0008, REQ-BDC-0502). Every rule of
   §Ingest: strict decoder, declared membership, the built-in `blocked` rule, empty-snapshot
   refusal, the 50 000-row / 64 MiB limits, `INGEST_INVALID` with the stored scope untouched,
   one transaction, other memberships kept, the newer-row guard, opaque `revision`, and the
   `tick_id` stamp. One invalid row among valid ones stores no row from that call
   (REQ-BDC-0008).
   `ingest_scope` stores a snapshot the caller fetched through bd (`bd list`, `bd blocked`);
   the store and core open no Dolt or MySQL connection and read no Dolt tables (REQ-BDC-0006).
4. **Failure state and marking** (REQ-BDC-0728). The `failing_since` lifecycle,
   `notified_since`, `scope_announce(scope, since)` compare-and-set, and `first_offered_at` set
   once. `journal_announce` is unchanged. These are what the optional post-sync hook receives
   and marks (ANC-004). The hook is optional: a consumer registers it as the `post_sync` kind in
   the `registration` table (deliverable 6), and b-8's tick calls it after each sync
   with each scope's result, error, `failing_since` and notified state, and the escalated
   entries not yet notified with `first_offered_at`.
5. **Tick lease** (REQ-BDC-0722). `tick_lease` take, refusal of a live lease, and takeover of an
   expired one.
6. **Registration** (REQ-BDC-0724). Read and write of the `registration` table.
7. **Codes and outage** (REQ-BDC-0729, ADR-BDC-0005). The four new codes and `details.outage`
   in the model's error table. An outage is recorded per affected scope in `sync_state` (its
   result, error and `failing_since`); reads keep answering from the cache with no bd process
   (REQ-BDC-0001).
8. **Removals** (ADR-BDC-0010). `bd-cache-model/src/sender.rs`; the `[tick]` table's message-routing and
   phase-file settings; the consumer wording in store comments.

## Acceptance

Each item has a positive test and a planted negative (AC-N). Every test runs with no bd and no
Dolt, against the store and the fakes (NFR-BDC-0005).

1. The golden has the exact v3 DDL plus every extension. Opening a v3-only file adds them; a
   file with them, opened by a reader that does not know them, still reads.
2. Store API: each Ingest rule; the built-in snapshot regression test (a real built-in
   snapshot, rows and blocked, through ingest); the scope registry; `scope_announce` including
   a stale `since` refused; one test per `failing_since` transition; `first_offered_at` set
   once; lease take, refusal and expiry takeover; registration read and write.
3. The outage truth table has one test per row: spawn failure, timeout, lock, transport, and an
   answered `BD_LIST_FAILED` with `outage: false`.
4. With an outage recorded for a scope, a cached read of one of its beads still answers from
   the store and starts no bd process; one invalid row among valid ones in an ingest stores
   none of them.
5. A second process opens the same file and ingests while a first holds a handle with no
   transaction open; the first then reads the result.
6. AC-0: `python3 scripts/lint_no_atm.py --paths <this fence>` exits 0. AC-Q: S57 QA PASS and
   green CI on the three OSes.

## Reference

PR #93 (REFERENCE ONLY) holds the stopped earlier b-7 attempt; reuse what matches this doc.

## Does not close

The tick, the schedule, the runner and the CLI verbs (b-8); the contract text and the lint gate
(b-9).
