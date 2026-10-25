---
id: b-7
phase: b
status: planned
closure_type: integration
target_boundary: bd-cache-sync
track: core
wave: 5
branch: sprint/b-7-sync-job
worktree: sprint/b-7-sync-job
recommended_agent: rust-developer
recommended_model: opus
must_follow: [b-6]
parallel_safe: [b-8]
requirements:
  owns: ["REQ-BDC-0701 (amended)", "REQ-BDC-0702 (amended)", REQ-BDC-0703, "REQ-BDC-0704..0706 (amended)", "REQ-BDC-0707 (removed)"]
  constrained_by: [REQ-BDC-0001, REQ-BDC-0006, REQ-BDC-0007, REQ-BDC-0008, REQ-BDC-0502, REQ-BDC-0812, NFR-BDC-0004, NFR-BDC-0005]
adrs: [ADR-BDC-0001, ADR-BDC-0005, ADR-BDC-0010, ADR-STORE-0001, "ADR-SYNC-0008 (new; supersedes ADR-SYNC-0007)"]
owned_paths:
  - "crates/bd-cache-sync/**"
  - "crates/bd-cache/**"
  - "crates/bd-cache-store/**"
  - "crates/bd-cache-model/src/**"
  - "crates/bd-cache-cli/src/commands/tick.rs"
  - "crates/bd-cache-cli/src/commands/schedule.rs"
  - "crates/bd-cache-cli/src/args.rs"
  - "crates/bd-cache-py/src/**"
  - "docs/bd-cache-sync/**"
---

# Sprint b-7 — the bd sync job on sc-cron, scopes as filters, Python sync and post-sync hook

## Outcome

beads-cache syncs bd itself, at the times set in sc-cron's SQLite cron table in the cache file
(REQ-BDC-0701 amended). A consumer can disable the built-in sync row and enable its own Python
sync row, which feeds the cache through `ingest_scope`. A registered post-sync hook receives the
sync results and errors. beads-cache sends no messages (ADR-BDC-0010).

## Deliverables

1. **Built-in sync job** (REQ-BDC-0701 amended, REQ-BDC-0703).
   - The bd sync is a Rust `Job` on sc-cron, scheduled by at most one built-in row per
     repository. Its budget is the row's timeout.
   - It drains with a deadline before the end, then syncs each scope, deferring any scope not
     started before the end.
   - Every bd call stays capped by the time left (NFR-BDC-0004).
2. **Scopes are bd filters** (REQ-BDC-0702 amended, ADR-BDC-0001).
   - A scope is a name plus a bd filter: labels (all or any), status, type, assignee, or
     `--all`. The built-in sync runs `bd list` with that filter.
   - Phase files, the root-bead rule and the hard-coded `phase-` scope prefix in `ScopeId` are
     removed.
   - Scope ids in bd-cache-api and the Python binding resolve to these filters.
3. **Python sync replacement** (REQ-BDC-0007, REQ-BDC-0008, REQ-BDC-0502).
   - A consumer disables the built-in row and enables its own sc-cron Python row (b-6).
   - The consumer's method feeds the cache through `ingest_scope(scope, rows, blocked,
     revision=None)`, which uses the same strict row decoder, all-or-nothing transaction and
     newer-row guard as the built-in sync.
   - `ingest_scope` is available in Rust, the CLI (`bdc cache scopes ingest`) and PyO3, as an
     addition to the binding (REQ-BDC-0812).
4. **Post-sync hook** (REQ-BDC-0704..0706 amended).
   - After each built-in sync the job calls the optional post-sync hook named in its row's
     payload, through sc-cron's Python runner and bounded by its timeout (NFR-BDC-0004).
   - The hook gets each scope's result and error, `failing_since` and notified state, the
     escalated entries not yet notified, and the pending count.
   - The hook marks what it handled through `journal announce` and a new compare-and-set
     `scopes announce --since`. Anything it doesn't mark is offered again on the next run.
   - Reads never wait on the hook (REQ-BDC-0001).
5. **Outage classification** (ADR-BDC-0005).
   - Errors that can come from bd get `details.outage: bool`, true only when bd could not be
     reached. A `BD_LIST_FAILED` from a bd that answered has `outage: false`.
   - Reads keep coming from the cache during an outage.
6. **Removals** (ADR-BDC-0010, REQ-BDC-0707 removed).
   - `atm.rs`, `phases.rs`, `active.rs`, the stop rule and their tests;
   - `bd-cache-model/src/sender.rs`;
   - the `[tick] team`, `fallback_lead`, `phases_dir` and `phases` keys;
   - the `refused` / `pass-through` replies;
   - the `.atm-bd/` refusal.

   The sync still reaches the backend only through bd (REQ-BDC-0006).
7. **Store** (ADR-STORE-0001). Only nullable `sync_state.failing_since` and
   `sync_state.notified_since` are added. The v3 tables and indexes stay unchanged.
8. **CLI.**
   - `bdc cache schedule set|show|enable|disable|clear` manages the built-in row and refuses a
     second one. `enable` and `disable` flip only sc-cron's `enabled` column on the existing row
     (b-6), without rewriting it. A consumer disables the built-in row this way before enabling
     its own Python row.
   - `bdc cache tick` runs the job once, on demand. The schedule stays in the cron table.

## Tests

All tests run against fake-bd with no bd installed (NFR-BDC-0005):
- One built-in row runs from the cache file's cron table, and a second is refused.
- A label scope and an `--all` scope sync.
- A Python sync row ingests one scope and fails another; one bad row rejects that ingest
  whole; an older row never replaces a newer one.
- The post-sync hook receives every scope's result and error. It marks one escalation and the
  failed scope; the next run does not offer what was marked, and does offer the rest again. A
  hanging hook is killed at its timeout.
- The outage table has one test per row: spawn failure, timeout, lock, transport, and an
  answered `BD_LIST_FAILED`.
- An end-to-end test with a real Python interpreter through `bdc cache tick`, also against
  fake-bd with no bd and no Dolt; the interpreter is its only real dependency.

## Does not close

The no-ATM lint CI step and the remaining neutral-docs work (b-8).
