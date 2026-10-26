---
id: b-7
phase: b
status: complete
closure_type: boundary
target_boundary: bd-cache-store, bd-cache, bd-cache-model, the scope filters (sc-beads, sc-bd-cli), sc-cron's store trait
track: core
wave: 5
branch: sprint/b-7-store-contract
worktree: sprint/b-7-store-contract
recommended_agent: rust-developer
recommended_model: opus
must_follow: [b-6]
parallel_safe: []
requirements:
  owns: [REQ-BDC-0721, REQ-BDC-0726, REQ-BDC-0728, REQ-BDC-0729, "REQ-BDC-0501 (amended)", "REQ-BDC-0602 (amended)", "REQ-BDC-0603 (amended)", "REQ-BDC-0804 (amended)", "REQ-BDC-0812 (amended: the 0.1 bare-name rule)", "REQ-BDC-0901 (amended)", "REQ-BDC-0909 (amended)", "NFR-BDC-0007 (amended)", "REQ-STORE-0003 (amended)", "REQ-STORE-0007 (amended)", REQ-CRON-0017]
  not_binding: "REQ-CRON-0002..0005, 0016, 0018: next-run, jitter, missed-tick, stop-check, resolver and supervisor rules of the worker and hooks, which b-7's store methods do not touch (b-8)"
  constrained_by: [REQ-BDC-0001, REQ-BDC-0006, REQ-BDC-0007, REQ-BDC-0008, REQ-BDC-0103, REQ-BDC-0107, REQ-BDC-0502, REQ-BDC-0504, REQ-BDC-0604, REQ-BDC-0722, REQ-BDC-0727, REQ-BDC-0803, REQ-BDC-0910, NFR-BDC-0001, NFR-BDC-0005, "REQ-STORE-0001, 0002, 0004..0006, 0008", "REQ-CORE-0001..0004, REQ-CORE-0101..0104", "REQ-MODEL-0001..0005", "REQ-BEADS-0001..0005", "REQ-BDCLI-0001..0009", "REQ-CRON-0001, 0006..0015", "REQ-API-0001..0008, REQ-CLI-0001..0012, REQ-PY-0001..0014, REQ-SYNC-0001..0013 (the mechanical edits only)"]
adrs: [ADR-BDC-0001, ADR-BDC-0002, ADR-BDC-0003, ADR-BDC-0004, ADR-BDC-0005, ADR-BDC-0006, ADR-BDC-0007, ADR-BDC-0009, ADR-BDC-0010, ADR-BDC-0011, ADR-BDC-0012, ADR-BDC-0013, ADR-BDC-0014, ADR-BDC-0015, "ADR-STORE-0001..0006", "ADR-CORE-0001, 0002, 0101..0103, 0201..0206", "ADR-MODEL-0001..0004", "ADR-BEADS-0001..0004", "ADR-BDCLI-0001..0006", "ADR-CRON-0001..0009 (the store methods)", "ADR-API-0001..0003, ADR-CLI-0001..0005, ADR-PY-0001..0006, ADR-SYNC-0001..0008 (the mechanical edits only)"]
owned_paths:
  - "crates/bd-cache-store/** (except tests/compat*)"
  - "crates/bd-cache/**"
  - "crates/bd-cache-model/**"
  - "crates/sc-beads/src/cache.rs, crates/sc-beads/src/testing/**, crates/sc-beads/tests/**"
  - "crates/sc-bd-cli/src/argv.rs, crates/sc-bd-cli/src/testing/**, crates/sc-bd-cli/tests/**"
  - "crates/sc-cron/src/{store.rs,sqlite.rs,scheduler.rs}, crates/sc-cron/tests/{sqlite_store.rs,enabled.rs} and the RecordingStore impls in crates/sc-cron/tests/worker.rs and crates/sc-cron/tests/common/** (the two store methods only)"
  - "crates/bd-cache-api/**, crates/bd-cache-cli/** (with src/bin/fake-bd.rs), crates/bd-cache-py/**, crates/bd-cache-sync/** (the mechanical edits the ScopeId, ScopeFilter and store changes force, and the code rows of crates/bd-cache-api/src/error.rs)"
  - "crates/bd-cache-api/tests/error_table.rs (the TICK_ row match only), the new crates/bd-cache-api/tests/scope_names.rs, and crates/bd-cache-py/tests/test_reads.py (the 0.1 bare-name cases)"
  - "docs/bd-cache-cli/contract.md (the §4 error-table rows of the four new codes only)"
  - "docs/architecture.md (the bd-cache-model, bd-cache-store, sc-beads and sc-bd-cli rows of the Crate Layout and Boundaries tables)"
  - "docs/bd-cache-store/**, docs/bd-cache/**, docs/bd-cache-model/** (implementation notes)"
---

# Sprint b-7 — store, scopes and the schedule store (work package WP-6a)

## Outcome

The store and the core library hold everything the sync rows, the consumer's Python sync and
the post-sync hook need, as additive v3 extensions (ADR-STORE-0006); scopes are named bd
filters end to end (ADR-BDC-0013); and the schedule rows live in the cache file through
sc-cron's store trait over the cache's one connection (ADR-BDC-0014). The workspace builds and
tests green on the three OSes after this sprint alone: every caller of a changed type is edited
here mechanically, and the semantic removals stay with b-8.

## Deliverables

1. **Store extensions** (ADR-STORE-0006, REQ-STORE-0003 amended, REQ-BDC-0804 amended). Exactly
   ADR-STORE-0006's list: the columns `scope.revision`, `scope.filter`,
   `sync_state.failing_since`, `sync_state.notified_since`, `journal.first_offered_at`; the
   tables `tick_lease`, `hook_state` and `sc_cron_schedule`; the `meta` row
   `sc_cron_schedule_version`; through the existing mechanism (`PRAGMA table_info` / `CREATE
   TABLE IF NOT EXISTS` under `BEGIN IMMEDIATE`); `user_version` stays 3. The format golden
   (`crates/bd-cache-store/tests/format_golden.rs` and its DDL fixture) pins the exact v3 DDL
   plus every extension; it is this sprint's alone, and b-9 only verifies it.
2. **Scope filters** (ADR-BDC-0013; REQ-BDC-0721, 0501, 0602, 0603, 0901, 0909 amended).
   - `ScopeName` (`PhaseName`'s rule, so every stored name stays valid) and `ScopeFilter`
     (`All` | `Criteria`) in `bd-cache-model`, validated once, with the one conversion to
     `sc_beads::ScopeFilter`; `ScopeId(ScopeName)` replaces `ScopeId::Phase`, and `Scope { id,
     filter }` is the registry's row. Stored names stay bare; a NULL `scope.filter` is the
     phase-a scope whose label is `LEGACY_SCOPE_PREFIX` plus the name (ADR-BDC-0013). The registry: `scopes add|set|remove|list` operations in the store and the
     core; the `.beads/bd-cache.toml` `[[scope]]` tables and `scopes` list, created or updated on
     open, the file winning (`CONFIG_INVALID` on `set`/`remove` of a file scope); on-demand
     registration as a label scope. These operations surface only as `bdc cache` cache-management
     verbs (b-8, ADR-BDC-0009); the bd /v0-shaped beads commands keep their names, flags and
     replies, and a filter changes only the argv sent to bd. Removing a scope deletes its membership rows, not bead rows
     another scope still holds.
   - `sc-beads`' `ScopeFilter` gains `All` and `Criteria`; `FakeBackend` answers them.
     `sc-bd-cli`'s argv for `bd list`, `bd blocked` and `bd ready` per variant, as ADR-BDC-0013's
     table; `FakeBd` and the `fake-bd` binary answer each variant; bd 1.3.1 fixtures for each
     variant beside the existing ones.
   - The ready and blocked base of a filter scope is bd's answer intersected with the scope's
     members (ADR-BDC-0002 as amended); the status criterion is applied to membership by the
     store.
   - The 0.1 compatibility rule (REQ-BDC-0812 amended, ADR-BDC-0012): the constant
     `LEGACY_SCOPE_PREFIX` in `bd-cache-model`'s compatibility module, used by the `scopes`
     string list and by the 0.1 Python verbs; the 0.1 verbs and their dicts stay unchanged.
3. **`ingest_scope`** (REQ-BDC-0726, REQ-BDC-0007, REQ-BDC-0008, REQ-BDC-0502). The Rust API in
   the store and the core: strict decoder, declared membership, the built-in `blocked` rule,
   empty-snapshot refusal, the 50 000-row / 64 MiB limits, `INGEST_INVALID` with the stored scope
   untouched, one transaction, other memberships kept, the newer-row guard, and opaque
   `revision`. One invalid row among valid ones stores no row from that call (REQ-BDC-0008).
   A consumer's Python sync, which runs as the sync while the built-in row is disabled (b-8
   deliverable 4), stores its snapshots only through `ingest_scope`, so it replaces the built-in
   bd sync with no bd process of beads-cache's (ANC-003).
   `ingest_scope` stores a snapshot the caller fetched through bd; the store and core open no
   Dolt or MySQL connection and read no Dolt tables (REQ-BDC-0006).
4. **Failure state and marking** (REQ-BDC-0728). The `failing_since` lifecycle (with the
   `FailingSince` token type in `bd-cache-model`), `notified_since`, `scope_announce(scope,
   since)` compare-and-set, and `first_offered_at` set once. `journal_announce` is unchanged.
   After each bd sync, the optional post-sync hook a consumer names in the built-in row receives
   each scope's result, error and `failing_since` and the escalated entries with
   `first_offered_at`, and marks them through these calls; b-8 deliverable 5 calls it (ANC-004).
5. **Lease and hook state, store side** (REQ-BDC-0722 and REQ-BDC-0727, store calls only; the
   guard and the backoff are b-8's). Store calls for `tick_lease` (take under `BEGIN IMMEDIATE`,
   refusal of a live lease, takeover of an expired one, release) and for `hook_state` (count,
   reset, next attempt); the `TickBudget` type in `bd-cache-model` with its one constructor.
6. **Codes and outage** (REQ-BDC-0729, REQ-BDC-0803, ADR-BDC-0005). `INGEST_INVALID`,
   `SCHEDULE_INVALID`, `TICK_RUN_FAILED` and `TICK_IN_FLIGHT` in the model's error table, as rows
   of `bd-cache-api`'s `ERROR_TABLE` and as the matching rows of contract §4's error table, one
   row per code, with REQ-BDC-0729's kinds, exits and details keys. `tests/error_table.rs`
   matches the `TICK_<REFUSAL>` row by that literal code, not by the `TICK_` prefix, so the two
   new `TICK_` codes parse as rows; that row and its expectation stay for b-8. And
   `details.outage` on errors that can come from bd. An outage is recorded per affected scope
   in `sync_state` (its result, error and `failing_since`); reads keep answering from the cache
   with no bd process (REQ-BDC-0001).
7. **The schedule store** (ADR-BDC-0014, NFR-BDC-0007 amended, REQ-CRON-0017).
   `ScheduleStore::set_enabled_many` and `ScheduleStore::replace_and_set_enabled` in sc-cron
   (`store.rs`), required methods with no default body, implemented by `SqliteScheduleStore`
   (`sqlite.rs`) in one `BEGIN IMMEDIATE` each, by both `RecordingStore` test doubles (delegating),
   and wrapped by `Scheduler` methods and their async forms (`scheduler.rs`). `set_enabled_many`
   flips only each row's `enabled` boolean column in place, never rewriting the row, on
   `SqliteScheduleStore` and on `CacheScheduleStore`'s `sc_cron_schedule` (ANC-006,
   REQ-CRON-0014); `replace_and_set_enabled` writes one row with its flag and other rows' flags in
   one transaction, as REQ-CRON-0017 says. `sqlite.rs`'s rustdoc cites REQ-CRON-0009, not
   NFR-BDC-0007. No other sc-cron change.
   Both are general store-trait methods with no beads-cache name; sc-cron gains no
   dependency and stays copyable to sc-runtime unchanged, and the cache-specific schedule logic is
   `CacheScheduleStore`'s (ANC-005). `CacheScheduleStore` in `bd-cache-store` implements `sc_cron::ScheduleStore` over the
   cache's one connection (the connection and the per-process claim in one object behind a
   mutex shared with `SqliteStore`, `SqliteStore::schedule_store()`, so the claim is released only
   when the last holder drops), keeping `sc_cron_schedule` in the cache file and checking
   the `sc_cron_schedule_version` row; a mismatch is refused before any schedule is served.
   `bd-cache-store` gains the `sc-cron` dependency (its own `Cargo.toml`; the workspace entry
   exists) and the Boundaries / Crate Layout rows say so.
8. **Mechanical caller edits** (ADR-BDC-0013). The `ScopeId`, `ScopeFilter` and store changes
   are followed in `bd-cache`, `bd-cache-api`, `bd-cache-cli` (the `fake-bd` binary included),
   `bd-cache-py` and `bd-cache-sync`: type and call-site changes only, with no behaviour change
   beyond the scope filters. The sender port, the `[tick]` keys and every other semantic removal
   are b-8's.
9. **Wording** (ADR-BDC-0010). The consumer wording in the comments, test names and fixtures of
   this sprint's own crates (`bd-cache-store`, `bd-cache`, `bd-cache-model`), so AC-0 passes on
   them.

## Acceptance

Each item has a positive test and a planted negative (AC-N). Every test runs with no bd and no
Dolt, against the store and the fakes (NFR-BDC-0005).

1. The format golden matches the exact v3 DDL plus every extension. Opening a v3-only file adds
   them; a file with them, opened by a reader that does not know them, still reads; `PRAGMA
   user_version` is 3 afterwards. Planted negative: drop `scope.filter` from the extension list,
   and the golden fails.
2. Store API: each Ingest rule; the built-in snapshot regression test (a real built-in snapshot,
   rows and blocked, through ingest); the scope registry, with a file scope refused by `scopes
   set` and `scopes remove`; `scope_announce` including a stale `since` refused; one test per
   `failing_since` transition; `first_offered_at` set once; lease take, refusal, expiry takeover
   and release; `hook_state` count and reset; an `ingest_scope` snapshot is stored with no
   built-in sync run and no bd process started (the fakes record no call).
3. Scope filters: `sync_label_filter_scope` and `sync_all_scope` sync a label-criteria scope and
   an `All` scope end to end against `FakeBackend`, and against `fake-bd` through the CLI; each
   variant's argv equals ADR-BDC-0013's table; a status criterion drops a bead whose status left
   the set on the next sync. `ScopeFilter::Criteria` with every field empty, and `all` with a
   criterion, are refused at construction. Planted negative: pass `--status` to bd for a status
   criterion, and the argv test fails.
4. The 0.1 bare-name rule (REQ-BDC-0812, ADR-BDC-0013): on a cache holding the phase-a scope
   stored as `x` (label `<prefix>x`), a 0.1 Python verb given `x` or `<prefix>x` reads it; a new
   scope argument (`issues list --scope`, the 0.2 namespaces) given `x` reads it, and given
   `<prefix>x` is `SCOPE_UNKNOWN`. A read of an unknown name is `SCOPE_UNKNOWN` and registers
   nothing; `cache refresh scopes` and the 0.1 `sync` register it. The new-argument cases run in
   `crates/bd-cache-api/tests/scope_names.rs`, the 0.1 cases in the existing
   `crates/bd-cache-py/tests/test_reads.py`. Planted negative: drop the prefix rule from the 0.1
   path, and the 0.1 test fails; apply it to a new argument, and the new-argument test fails.
   The existing 0.1 Python tests pass unchanged.
5. The outage truth table has one test per row: spawn failure, timeout, lock, transport, and an
   answered `BD_LIST_FAILED` with `outage: false`. With an outage recorded for a scope, a cached
   read of one of its beads still answers from the store and starts no bd process; one invalid
   row among valid ones in an ingest stores none of them.
6. A second process opens the same file and ingests while a first holds a handle with no
   transaction open; the first then reads the result.
7. The schedule store: a v3 cache file with scopes gains `sc_cron_schedule` and stays `PRAGMA
   user_version` 3; sc-cron's store contract tests pass against `CacheScheduleStore`; while the
   store and its schedule view are open, the process holds one connection to the cache file (a
   second `SqliteStore` open in the process is still `STORE_IN_USE`, and the schedule view
   shares the store's connection, which the test checks by a write through one being visible
   through the other inside one transaction); after the `SqliteStore` is dropped with the view
   alive, a second open is still `STORE_IN_USE`; a cache file whose `sc_cron_schedule_version`
   row names another version is refused before any schedule is served. `set_enabled_many` flips
   two rows in one transaction, seen whole by a second connection; `replace_and_set_enabled`
   creating a disabled row while enabling another, polled by a second connection, never shows
   both enabled; a replace of an active row keeps its id, raises its generation and keeps the
   given flag; a call naming a missing row changes nothing; each on both stores. Planted
   negatives: open a second connection for the schedule view (the one-connection test fails);
   keep the claim in `SqliteStore` (the drop test fails); write the row enabled and then flip it
   (the polling test fails).
8. The new codes are in `ERROR_TABLE` and in contract §4, one row each, with REQ-BDC-0729's
   kinds and exits; the model and api tables agree, and `error_table.rs` passes with the
   `TICK_<REFUSAL>` row still present. Planted negative: drop one new row from the contract, and
   the test fails.
9. The whole workspace builds and its tests pass with this sprint's changes alone. AC-0:
   `python3 scripts/lint_no_atm.py --paths crates/bd-cache-store/ crates/bd-cache/
   crates/bd-cache-model/ docs/bd-cache-store/ docs/bd-cache/ docs/bd-cache-model/` exits 0,
   except for `crates/bd-cache-model/src/sender.rs` and the `[tick]` parts of
   `crates/bd-cache-model/src/config.rs`, which b-8 removes; the mechanically edited crates keep
   their hits for b-8 and b-9. AC-Q: S57 QA PASS and green CI on the three OSes.

## Reference

PR #93 (REFERENCE ONLY) holds the stopped earlier b-7 attempt; reuse what matches this doc.

## Does not close

The sync rows, the switch, the runner, the CLI and Python verbs and the semantic removals (b-8);
the contract text, the neutral tests and the lint gate (b-9).
