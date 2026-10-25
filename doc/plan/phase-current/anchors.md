# Anchors - phase-current

Constraints captured during planning. Each anchor is a testable statement written by the orchestrator and approved by the user; the user's verbatim words are kept beneath it as provenance. Every anchor is judged at the planning gate and again at the development gate. Edit `anchors.json`, not this file; this file is rendered.

Intent: `doc/plan/phase-current/intent.md`

| ID | Status | Summary | Stated |
| --- | --- | --- | --- |
| ANC-001 | locked | No ATM strings in product files | 2026-10-25 |
| ANC-002 | locked | bd sync runs from the SQLite cron table | 2026-10-25 |
| ANC-003 | locked | Consumer Python sync can replace the built-in sync | 2026-10-25 |
| ANC-004 | locked | Optional post-sync hook gets results and errors | 2026-10-25 |
| ANC-005 | locked | sc-cron stays a general job scheduler | 2026-10-25 |
| ANC-006 | locked | sc-cron rows have an enabled flag | 2026-10-25 |
| ANC-007 | locked | Stop hook returns continue, disable or delete | 2026-10-25 |
| ANC-008 | locked | At most one built-in sync per repository | 2026-10-25 |
| ANC-009 | locked | Sync scopes are bd filters, not phases | 2026-10-25 |
| ANC-010 | locked | team-protocol stays exempt from the lint | 2026-10-25 |
| ADR-BDC-0009 | locked | bdc has a beads surface and a cache surface | 2026-10-25 |
| ADR-STORE-0001 | locked | Store file stays v3 plus additive extensions | 2026-10-25 |
| ANC-011 | locked | Only code justified by a REQ or ADR | 2026-10-25 |
| REQ-BDC-0001 | locked | Reads never start bd | 2026-10-25 |
| REQ-BDC-0006 | locked | Backend access only through bd | 2026-10-25 |
| REQ-BDC-0007 | locked | Strict row contract on every ingested row | 2026-10-25 |
| REQ-BDC-0008 | locked | A bad row stores nothing | 2026-10-25 |
| REQ-BDC-0502 | locked | Newer-row guard on every upsert | 2026-10-25 |
| REQ-BDC-0703 | locked | A sync run stays within its time budget | 2026-10-25 |
| REQ-BDC-0812 | locked | Python 0.2 additions leave 0.1 unchanged | 2026-10-25 |
| NFR-BDC-0004 | locked | Every bd call and hook run is bounded | 2026-10-25 |
| NFR-BDC-0005 | locked | Unit tests run without bd | 2026-10-25 |
| ADR-BDC-0005 | locked | A slow or unreachable backend is an outage, not a failed read | 2026-10-25 |

## ANC-001

Anchor: beads-cache source and test files must not contain ATM strings such as cron@tool, where ATM is the agent-team messaging tool.

> blocking 'cron@tool' and other atm strings from source or tests

Said by Rand, phase-b planning session, 2026-10-25 (Rand); wording by bc-lead under Rand's instruction to use judgement, 2026-10-25. Status: locked. Version 3.

Summary: No ATM strings in product files

Scope: any bead that adds or edits source, test, script or doc files in beads-cache

Deterministic check to pair with the development gate: python3 scripts/lint_no_atm.py exits 0 on the tree (CI step 'Strict consumer boundary (ADR-BDC-0010)'); per sprint: python3 scripts/lint_no_atm.py --paths <sprint owned paths> exits 0

Beads: anchor `bdc-nrp`, plan questions `bdc-nrp.1`, dev questions `bdc-nrp.2`

Fixtures:
- pass: This sprint deletes crates/bd-cache-sync/src/atm.rs and its tests, and python3 scripts/lint_no_atm.py exits 0 on the tree.
- fail: This sprint keeps the cron@tool sender in crates/bd-cache-sync/src/atm.rs and adds an atm send retry test.

## ANC-002

Anchor: beads-cache must run its sync of bd, the beads issue-tracker CLI, at the times set in a SQLite cron table.

> beads-cache needs to handle the synchronization of bd on a cron tick. [...] long term sync would be done by beads-cache at a time scheduled in the cron scheduler in sqlite cron table.

Said by Rand, phase-b planning session, 2026-10-25 (Rand); wording by bc-lead under Rand's instruction to use judgement, 2026-10-25. Status: locked. Version 4.

Summary: bd sync runs from the SQLite cron table

Scope: any bead that schedules, runs or configures the bd sync

Deterministic check to pair with the development gate: a test runs the built-in sync from an sc-cron schedule row in the cache file

Excluded from (not judged; the cell passes as `excluded`):
- `b-1`: b-1 extends the store/journal contract and compose_with_config; it does not schedule, run or configure the bd sync.
- `b-2`: b-2 defines the bd-cache-api request vocabulary; sync scheduling is not in it.
- `b-3a`: b-3a builds the CLI skeleton and fake-bd harness; it does not schedule, run or configure the bd sync.
- `b-6`: sc-cron only gains the Python hook target, enabled flag and stop-hook action; the bd sync schedule row is planned in b-7.
- `b-8`: b-8 adds only a refresh deadline flag and a health read; the cron-table schedule that runs the sync is b-7's.

Beads: anchor `bdc-g4s`, plan questions `bdc-g4s.1`, dev questions `bdc-g4s.2`

Fixtures:
- pass: The built-in bd sync is an sc-cron schedule row in the cache file's SQLite cron table and runs at that row's times; test sync_runs_from_schedule_row in crates/bd-cache-sync/tests/schedule.rs passes in CI (run 1234).
- fail: The bd sync runs only when an external scheduler invokes bdc cache tick; the cache keeps no schedule of its own.

## ANC-003

Anchor: beads-cache must let a consumer-registered Python sync method replace its hard-coded bd sync.

> In the event of the 'hard-coded sync' not working, the hard-coded sync could be disabled and replaced w/ a python method registered by consumer (atm-bd).

Said by Rand, phase-b planning session, 2026-10-25 (Rand); wording by bc-lead under Rand's instruction to use judgement, 2026-10-25. Status: locked. Version 4.

Summary: Consumer Python sync can replace the built-in sync

Scope: any bead that touches the sync job, sync scheduling or ingest of synced rows

Deterministic check to pair with the development gate: an end-to-end test disables the built-in sync row and a registered Python sync ingests a scope

Excluded from (not judged; the cell passes as `excluded`):
- `b-2`: b-2 does not touch the sync job or its replacement.
- `b-3`: b-3 only invokes the sync from the CLI; it neither registers nor replaces a sync method.
- `b-4`: b-4 adds the cache.beads and cache.manage Python namespaces; it does not touch registering a sync method.
- `b-6`: sc-cron stays generic; the consumer-registered Python sync that replaces the built-in bd sync is planned in b-7.
- `b-8`: The consumer-registered Python sync that replaces the built-in bd sync is planned in b-7; b-8 only cleans the boundary and adds the lint gate.

Beads: anchor `bdc-iid`, plan questions `bdc-iid.1`, dev questions `bdc-iid.2`

Fixtures:
- pass: A consumer disables the built-in sync row and its Python sync row ingests a scope through ingest_scope; the end-to-end test test_python_sync_replaces_builtin in crates/bd-cache-py/tests/test_sync.py passes in CI (run 1234).
- fail: The bd sync is always the built-in bd list sync; a consumer cannot replace it with its own method.

## ANC-004

Anchor: beads-cache must support an optional consumer-registered post-sync hook that receives the bd sync results and errors.

> there can also be hooks necessary to report/escelate based on atm-bd requirements [...] We would also likely want a 'post-sync' hook that could contain sync results and errors and allow consumer (atm-bd) to do custom things at that point

Said by Rand, phase-b planning session, 2026-10-25 (Rand); wording by bc-lead under Rand's instruction to use judgement, 2026-10-25. Status: locked. Version 5.

Summary: Optional post-sync hook gets results and errors

Scope: any bead that builds or changes the bd sync job, the reporting or escalation of bd sync failures, or what runs after a bd sync; a bead that only invokes the sync (for example a CLI command calling it) or handles write-journal escalation is outside this scope

Deterministic check to pair with the development gate: an end-to-end test asserts the post-sync hook request carries each scope's result and error

Excluded from (not judged; the cell passes as `excluded`):
- `b-1`: b-1 does not build or change the sync job or post-sync behaviour.
- `b-2`: b-2 does not touch the sync job or post-sync behaviour.
- `b-3`: b-3 only invokes the sync from the CLI (cache tick); it does not build the sync job or a post-sync hook.
- `b-4`: b-4 does not touch the sync job or a post-sync hook.
- `b-8`: b-8 adds health and a refresh timeout flag only; it builds no sync job, hook or sync-failure reporting.
- `b-6`: b-6 only builds the generic sc-cron hook runner; the bd sync job and the consumer post-sync hook are planned in b-7.

Beads: anchor `bdc-3df`, plan questions `bdc-3df.1`, dev questions `bdc-3df.2`

Fixtures:
- pass: After each built-in sync the post-sync hook receives every scope's result and error; test post_sync_hook_gets_results_and_errors in crates/bd-cache-sync/tests/hook.rs passes in CI (run 1234).
- fail: Sync results and errors are only written to the tick reply; no hook is called after the sync.

## ANC-005

Anchor: sc-cron, the SQLite cron job scheduler crate, must remain a general-purpose job scheduler that the sc-runtime repository can copy.

> I expect 'sc-cron' to be a general tool that can be used for scheduling jobs. [...] should retain that general purpose shape for porting (copying) to sc-runtime.

Said by Rand, phase-b planning session, 2026-10-25 (Rand); wording by bc-lead under Rand's instruction to use judgement, 2026-10-25. Status: locked. Version 4.

Summary: sc-cron stays a general job scheduler

Scope: any bead that edits crates/sc-cron

Deterministic check to pair with the development gate: cargo tree -p sc-cron shows no bd-cache-* or sc-beads dependency; grep finds no beads or cache types in crates/sc-cron/src

Excluded from (not judged; the cell passes as `excluded`):
- `b-7`: b-7 uses sc-cron through its public Job and Python-runner interfaces; crates/sc-cron is b-6's, not in b-7's owned_paths.
- `b-8`: b-8 edits nothing under crates/sc-cron; its lint hits there are b-6's.

Beads: anchor `bdc-3ku`, plan questions `bdc-3ku.1`, dev questions `bdc-3ku.2`

Fixtures:
- pass: crates/sc-cron gained a Python hook target and an enabled flag; cargo tree -p sc-cron shows no bd-cache or sc-beads dependency and grep -rn 'bead' crates/sc-cron/src finds nothing (output attached).
- fail: crates/sc-cron adds a BeadsSyncJob type and a dependency on bd-cache-sync so it can run the bd sync directly.

## ANC-006

Anchor: sc-cron's cron table must have an enabled boolean column that enables or disables a schedule without rewriting the row.

> cron table should have an 'enable' bool flag so scheduled tasks can be enabled/disabled w/out needing to rewrite the entire row.

Said by Rand, phase-b planning session, 2026-10-25 (Rand); wording by bc-lead under Rand's instruction to use judgement, 2026-10-25. Status: locked. Version 4.

Summary: sc-cron rows have an enabled flag

Scope: any bead that edits crates/sc-cron's schedule table or scheduler API

Deterministic check to pair with the development gate: a test toggles enabled and asserts the row's generation, expression and payload are unchanged

Excluded from (not judged; the cell passes as `excluded`):
- `b-8`: b-8 edits nothing in sc-cron's schedule table or scheduler API.

Beads: anchor `bdc-mvv`, plan questions `bdc-mvv.1`, dev questions `bdc-mvv.2`

Fixtures:
- pass: sc-cron adds an enabled column and set_enabled updates only it; test set_enabled_keeps_expression_payload_generation in crates/sc-cron/tests/enabled.rs passes in CI (run 1234).
- fail: To disable a schedule, sc-cron deletes the row and re-inserts it later with the same expression and payload.

## ANC-007

Anchor: sc-cron, the SQLite cron job scheduler crate, must let a schedule define a stop hook that returns the string value continue, disable or delete.

> sc-cron should include the ability to define a `stop hook` [...] allow stop hook to return a stringenum with values 'disable', 'delete', 'continue' (or similar)

Said by Rand, phase-b planning session, 2026-10-25 (Rand); wording by bc-lead under Rand's instruction to use judgement, 2026-10-25. Status: locked. Version 4.

Summary: Stop hook returns continue, disable or delete

Scope: any bead that edits sc-cron's hooks, Job trait or run loop

Deterministic check to pair with the development gate: tests cover each of continue, disable and delete for a Rust and a Python stop hook

Excluded from (not judged; the cell passes as `excluded`):
- `b-7`: b-7 uses sc-cron's stop hook through its interface; the stop hook itself is built in b-6, and crates/sc-cron is not in b-7's owned_paths.
- `b-8`: b-8 edits nothing in sc-cron's hooks, Job trait or run loop.

Beads: anchor `bdc-7of`, plan questions `bdc-7of.1`, dev questions `bdc-7of.2`

Fixtures:
- pass: The stop hook returns continue, disable or delete and sc-cron acts on each; tests stop_hook_continue, stop_hook_disable and stop_hook_delete in crates/sc-cron/tests/stop_hook.rs pass in CI (run 1234).
- fail: The stop hook returns a boolean; true always deletes the schedule and false runs the tick.

## ANC-008

Anchor: Each repository must have at most one built-in beads-cache bd sync schedule in sc-cron, the SQLite cron job scheduler.

> I expect 'sc-cron' to be a general tool that can be used for scheduling jobs.  limiting 'builtin' sync to one per repo makes sense.

Said by Rand, phase-b planning session, 2026-10-25 (Rand); wording by bc-lead under Rand's instruction to use judgement, 2026-10-25. Status: locked. Version 5.

Summary: At most one built-in sync per repository

Scope: any bead that creates or manages sync schedule rows

Deterministic check to pair with the development gate: a test shows a second built-in sync row is refused while other rows are allowed

Excluded from (not judged; the cell passes as `excluded`):
- `b-1`: b-1 creates and manages no sync schedule rows.
- `b-8`: b-8 creates and manages no sync schedule rows.

Beads: anchor `bdc-i61`, plan questions `bdc-i61.1`, dev questions `bdc-i61.2`

Fixtures:
- pass: schedule set refuses a second built-in sync row for one repository while other rows are allowed; test second_builtin_sync_row_is_refused in crates/bd-cache-cli/tests/schedule.rs passes in CI (run 1234).
- fail: Each scope gets its own built-in sync schedule row, so a repository with five scopes has five built-in sync rows.

## ANC-009

Anchor: beads-cache sync scopes must be defined as bd filters, such as labels or --all, instead of as phases.

> 0702 Active phases - would NEED to be defined w/ filters.  'phase' is an atm/atm-bd concept.  other applications would have different scopes including --all.

Said by Rand, phase-b planning session, 2026-10-25 (Rand); wording by bc-lead under Rand's instruction to use judgement, 2026-10-25. Status: locked. Version 4.

Summary: Sync scopes are bd filters, not phases

Scope: any bead that defines, parses or syncs scopes

Deterministic check to pair with the development gate: tests sync a label-filter scope and an --all scope; grep finds no phase-file reading in crates/

Excluded from (not judged; the cell passes as `excluded`):
- `b-8`: b-8 adds only a refresh deadline flag; how scopes are defined is b-7's.

Beads: anchor `bdc-6e7`, plan questions `bdc-6e7.1`, dev questions `bdc-6e7.2`

Fixtures:
- pass: A scope is a name plus a bd filter and the built-in sync runs bd list with it; tests sync_label_filter_scope and sync_all_scope in crates/bd-cache-sync/tests/scopes.rs pass in CI (run 1234).
- fail: Scopes are read from .atm-bd/phase-*.toml files, and a scope is active while its phase root bead is open.

## ANC-010

Anchor: The no-ATM lint (script scripts/lint_no_atm.py; ATM is the agent-team messaging tool) must not report docs/team-protocol.md as a violation.

> team-protocol stays as it is and is exempting from lint.

Said by Rand, phase-b planning session, 2026-10-25 (Rand); wording by bc-lead under Rand's instruction to use judgement, 2026-10-25. Status: locked. Version 4.

Summary: team-protocol stays exempt from the lint

Scope: any bead that edits scripts/lint_no_atm.py or docs/team-protocol.md

Deterministic check to pair with the development gate: grep -n 'docs/team-protocol.md' scripts/lint_no_atm.py shows it in EXEMPT_FILES; git diff <phase base>..HEAD -- docs/team-protocol.md is empty

Beads: anchor `bdc-7qk`, plan questions `bdc-7qk.1`, dev questions `bdc-7qk.2`

Fixtures:
- pass: The lint adds a scope-prefix rule and keeps docs/team-protocol.md in EXEMPT_FILES; grep -n team-protocol scripts/lint_no_atm.py shows the EXEMPT_FILES entry and git diff integrate/phase-b -- docs/team-protocol.md is empty (output attached).
- fail: The sprint moves docs/team-protocol.md out of docs/ and removes it from the lint's exemptions.

## ADR-BDC-0009

Anchor: The bdc CLI must offer two command surfaces: a beads surface shaped like the bd /v0 interface, and a separate cache-management surface.

> The bdc CLI must offer two command surfaces: a beads surface shaped like the bd /v0 interface, and a separate cache-management surface.

Said by randlee, planning session, 2026-10-25. Status: locked. Version 4.

Summary: bdc has a beads surface and a cache surface

Scope: any bead that adds, renames or changes a bdc command or the bd-cache-api request vocabulary

Deterministic check to pair with the development gate: contract.md section 1 lists both groups; bdc --help shows the beads and cache command groups

Excluded from (not judged; the cell passes as `excluded`):
- `b-5`: b-5 changes release packaging and channels only; it adds or renames no bdc command or bd-cache-api request.

Beads: anchor `bdc-hvp`, plan questions `bdc-hvp.1`, dev questions `bdc-hvp.2`

Fixtures:
- pass: bdc keeps its beads surface shaped like bd /v0 and its separate cache surface; test help_lists_beads_and_cache_groups in crates/bd-cache-cli/tests/help.rs passes in CI (run 1234).
- fail: bdc merges the beads and cache commands into one flat command list with new names unrelated to bd /v0.

## ADR-STORE-0001

Anchor: The beads-cache store file must keep the v3 tables and indexes unchanged, adding only nullable columns and new tables.

> The beads-cache store file must keep the v3 tables and indexes unchanged, adding only nullable columns and new tables.

Said by randlee, planning session, 2026-10-25. Status: locked. Version 4.

Summary: Store file stays v3 plus additive extensions

Scope: any bead that edits crates/bd-cache-store/src/schema.rs or adds a store column or table

Deterministic check to pair with the development gate: the format golden test matches the exact v3 DDL; PRAGMA user_version stays 3

Excluded from (not judged; the cell passes as `excluded`):
- `b-3`: b-3 is the bdc CLI; it edits neither crates/bd-cache-store/src/schema.rs nor any store column or table.
- `b-8`: b-8 asserts the v3 format through a golden test but edits no schema.rs, store table, column or index.

Beads: anchor `bdc-2rd`, plan questions `bdc-2rd.1`, dev questions `bdc-2rd.2`

Fixtures:
- pass: This sprint adds nullable sync_state.failing_since and notified_since columns and leaves the v3 tables unchanged; test format_golden_matches_v3_ddl in crates/bd-cache-store/tests/format.rs passes in CI (run 1234).
- fail: This sprint renames the journal table's phase column to scope and bumps PRAGMA user_version to 4.

## ANC-011

Anchor: Every code deliverable in a phase-B sprint plan must cite a REQ or ADR identifier that justifies it.

> Add an anchor for only allowing code that is justified by req/adr to keep project tight.

Said by Rand, phase-b planning session, 2026-10-25 (Rand), 2026-10-25. Status: locked. Version 5.

Summary: Only code justified by a REQ or ADR

Scope: any sprint bead that adds, changes or removes code

Deterministic check to pair with the development gate: every numbered deliverable under '## Deliverables' in docs/plans/phase-b/sprint-*.md cites at least one REQ-* or ADR-* id, and each cited id exists in docs/requirements.md, docs/*/requirements.md or docs/**/architecture.md

Beads: anchor `bdc-9hq`, plan questions `bdc-9hq.1`, dev questions `bdc-9hq.2`

Fixtures:
- pass: Deliverable 3 adds the enabled column (REQ-CRON-0014) and deliverable 4 adds ingest_scope (REQ-BDC-0007); the citation check script lists every deliverable with an existing REQ or ADR id and exits 0 (output attached).
- fail: Deliverable 3 adds a metrics dashboard and a retry daemon to bd-cache-sync; no requirement or ADR is cited for either.

## REQ-BDC-0001

Anchor: A read of a cached bead must be answered from the local SQLite file without starting a bd process.

> A read of a cached bead must be answered from the local SQLite file without starting a bd process.

Said by randlee, planning session, 2026-10-25. Status: locked. Version 3.

Summary: Reads never start bd

Scope: any bead that changes the read path, the sync job, a post-sync hook or a Python sync method

Deterministic check to pair with the development gate: test_miss_fetches_once_then_serves_from_cache passes; a read during a running sync or hook test starts no bd process

Excluded from (not judged; the cell passes as `excluded`):
- `b-8`: b-8 does not change the read path, the sync job, a post-sync hook or a Python sync method.
- `b-6`: sc-cron has no bd dependency and no cache read path.

Beads: anchor `bdc-vq2`, plan questions `bdc-vq2.1`, dev questions `bdc-vq2.2`

Fixtures:
- pass: Reads stay on the SQLite file while the sync and post-sync hook run in their own process; test reads_never_start_bd_during_sync in crates/bd-cache/tests/read.rs passes in CI (run 1234).
- fail: Each get first runs the post-sync hook to refresh its scope, then answers from the cache.

## REQ-BDC-0006

Anchor: beads-cache must reach the beads backend only through bd and must never read Dolt tables directly.

> beads-cache must reach the beads backend only through bd and must never read Dolt tables directly.

Said by randlee, planning session, 2026-10-25. Status: locked. Version 2.

Summary: Backend access only through bd

Scope: any bead that adds or changes how beads-cache fetches or writes backend data

Deterministic check to pair with the development gate: grep finds no dolt or mysql client dependency or SQL against Dolt tables in crates/

Beads: anchor `bdc-8d2`, plan questions `bdc-8d2.1`, dev questions `bdc-8d2.2`

Fixtures:
- pass: The built-in sync still fetches only through bd list and bd blocked; cargo tree shows no Dolt or MySQL client in any beads-cache crate (output attached).
- fail: The built-in sync queries the Dolt issues table directly over a MySQL connection for speed.

## REQ-BDC-0007

Anchor: Every bead row stored by beads-cache, including rows ingested from a consumer sync, must pass the strict row decoder generated from bd schema.

> Every bead row stored by beads-cache, including rows ingested from a consumer sync, must pass the strict row decoder generated from bd schema.

Said by randlee, planning session, 2026-10-25. Status: locked. Version 2.

Summary: Strict row contract on every ingested row

Scope: any bead that adds or changes a path that stores bead rows, such as sync, fetch or ingest_scope

Deterministic check to pair with the development gate: test_recorded_bd_rows_validate passes; an ingest_scope test with an unknown key returns BD_CONTRACT naming the bead and field

Excluded from (not judged; the cell passes as `excluded`):
- `b-8`: b-8 adds or changes no path that stores bead rows.

Beads: anchor `bdc-svf`, plan questions `bdc-svf.1`, dev questions `bdc-svf.2`

Fixtures:
- pass: ingest_scope decodes every row with the same strict sc-beads decoder as the built-in sync; test ingest_rejects_unknown_key in crates/bd-cache/tests/ingest.rs passes in CI (run 1234).
- fail: ingest_scope stores rows as raw JSON so a consumer sync can add its own fields without validation.

## REQ-BDC-0008

Anchor: A sync, fetch or ingest whose result contains one row that fails validation must store no row from that result.

> A sync, fetch or ingest whose result contains one row that fails validation must store no row from that result.

Said by randlee, planning session, 2026-10-25. Status: locked. Version 2.

Summary: A bad row stores nothing

Scope: any bead that adds or changes a path that stores bead rows, such as sync, fetch or ingest_scope

Deterministic check to pair with the development gate: test_contract_violation_is_typed_and_sync_stores_nothing passes; an ingest_scope test with one bad row leaves the stored scope unchanged

Excluded from (not judged; the cell passes as `excluded`):
- `b-6`: sc-cron stores no bead rows; b-6 adds or changes no sync, fetch or ingest path.
- `b-8`: b-8 adds or changes no path that stores bead rows.

Beads: anchor `bdc-w37`, plan questions `bdc-w37.1`, dev questions `bdc-w37.2`

Fixtures:
- pass: ingest_scope writes one transaction per scope and stores nothing if any row fails; test one_bad_row_ingests_nothing in crates/bd-cache/tests/ingest.rs passes in CI (run 1234).
- fail: ingest_scope stores every valid row and skips the rows that fail validation, logging each skipped bead.

## REQ-BDC-0502

Anchor: A sync or ingest must not replace a stored bead row whose updated_at is newer than the incoming row.

> A sync or ingest must not replace a stored bead row whose updated_at is newer than the incoming row.

Said by randlee, planning session, 2026-10-25. Status: locked. Version 2.

Summary: Newer-row guard on every upsert

Scope: any bead that adds or changes a path that upserts bead rows, such as sync or ingest_scope

Deterministic check to pair with the development gate: test_sync_is_incremental_and_never_replaces_a_newer_row passes; an ingest_scope test with an older row keeps the newer stored row

Excluded from (not judged; the cell passes as `excluded`):
- `b-8`: b-8 adds or changes no path that upserts bead rows.
- `b-6`: sc-cron upserts no bead rows; b-6 adds or changes no sync or ingest path.

Beads: anchor `bdc-a40`, plan questions `bdc-a40.1`, dev questions `bdc-a40.2`

Fixtures:
- pass: ingest_scope applies the same updated_at guard as the built-in sync; test ingest_keeps_newer_row in crates/bd-cache/tests/ingest.rs passes in CI (run 1234).
- fail: ingest_scope overwrites stored rows unconditionally, since the consumer sync is authoritative.

## REQ-BDC-0703

Anchor: A beads-cache sync run must finish within its schedule's time budget, draining first with an earlier deadline and deferring any scope not started before the end.

> A beads-cache sync run must finish within its schedule's time budget, draining first with an earlier deadline and deferring any scope not started before the end.

Said by randlee, planning session, 2026-10-25. Status: locked. Version 2.

Summary: A sync run stays within its time budget

Scope: any bead that adds or changes the sync job, its schedule timeout, a post-sync hook or a Python sync run

Deterministic check to pair with the development gate: test_tick_budget_defers_drain_entries passes; a test with a slow hook shows the run ends within the row's timeout

Excluded from (not judged; the cell passes as `excluded`):
- `b-6`: sc-cron is a general scheduler with no bd dependency; the sync run's budget, early drain deadline and deferral belong to the sync job in b-7.
- `b-8`: b-8 does not change the sync job, its schedule timeout, a post-sync hook or a Python sync run.

Beads: anchor `bdc-4jy`, plan questions `bdc-4jy.1`, dev questions `bdc-4jy.2`

Fixtures:
- pass: The sync job takes its budget from the schedule row's timeout, drains with a deadline before the end and defers late scopes; test sync_run_stays_within_budget in crates/bd-cache-sync/tests/budget.rs passes in CI (run 1234).
- fail: The sync job runs every scope and the post-sync hook to completion, however long they take.

## REQ-BDC-0812

Anchor: New Python binding methods must leave every 0.1 verb, argument and returned dict unchanged and must not issue a DeprecationWarning.

> New Python binding methods must leave every 0.1 verb, argument and returned dict unchanged and must not issue a DeprecationWarning.

Said by randlee, planning session, 2026-10-25. Status: locked. Version 2.

Summary: Python 0.2 additions leave 0.1 unchanged

Scope: any bead that adds or changes a method, argument or return value of the bd-cache-rs Python binding

Deterministic check to pair with the development gate: pytest -W error::DeprecationWarning passes and the 0.1 call corpus test is unchanged

Excluded from (not judged; the cell passes as `excluded`):
- `b-6`: b-6 changes only sc-cron and its own hook protocol; it adds or changes no bd-cache-rs Python binding method.

Beads: anchor `bdc-hqn`, plan questions `bdc-hqn.1`, dev questions `bdc-hqn.2`

Fixtures:
- pass: ingest_scope and the hook registration are new methods; the 0.1 call corpus passes unchanged under pytest -W error::DeprecationWarning in CI (run 1234).
- fail: The 0.1 sync(phase) verb is renamed to sync(scope) and now returns a list instead of a dict.

## NFR-BDC-0004

Anchor: Each bd call, Python sync method and post-sync hook started by beads-cache must be killed at its timeout, capped by the time left before the run's deadline.

> Each bd call, Python sync method and post-sync hook started by beads-cache must be killed at its timeout, capped by the time left before the run's deadline.

Said by randlee, planning session, 2026-10-25. Status: locked. Version 2.

Summary: Every bd call and hook run is bounded

Scope: any bead that adds or changes a bd call, a Python sync method run or a post-sync hook run

Deterministic check to pair with the development gate: a test with a hanging fake bd and a hanging hook shows each is killed at its timeout and the run ends by its deadline

Beads: anchor `bdc-vkg`, plan questions `bdc-vkg.1`, dev questions `bdc-vkg.2`

Fixtures:
- pass: The Python sync and post-sync hook run in their own process group and are killed at min(timeout, time left); test hanging_hook_is_killed in crates/sc-cron/tests/python_hook.rs passes in CI (run 1234).
- fail: The post-sync hook runs with no timeout so a slow consumer can finish its escalation.

## NFR-BDC-0005

Anchor: Unit tests for beads-cache must run with no bd and no Dolt, using the deterministic fake bd.

> Unit tests for beads-cache must run with no bd and no Dolt, using the deterministic fake bd.

Said by randlee, planning session, 2026-10-25. Status: locked. Version 2.

Summary: Unit tests run without bd

Scope: any bead that adds or changes tests for the sync job, hooks, ingest or the CLI

Deterministic check to pair with the development gate: CI unit jobs run with no bd on PATH; new tests use fake-bd

Beads: anchor `bdc-vof`, plan questions `bdc-vof.1`, dev questions `bdc-vof.2`

Fixtures:
- pass: The sync job, ingest and post-sync hook tests run against fake-bd with no bd installed; CI job unit (run 1234) has no bd on PATH.
- fail: The post-sync hook tests call the real bd against a live Dolt server.

## ADR-BDC-0005

Anchor: When bd cannot be reached, beads-cache must keep answering reads from the cache and record an outage for the affected scopes.

> When bd cannot be reached, beads-cache must keep answering reads from the cache and record an outage for the affected scopes.

Said by randlee, planning session, 2026-10-25. Status: locked. Version 2.

Summary: A slow or unreachable backend is an outage, not a failed read

Scope: any bead that adds or changes sync failure handling, outage state, or what a post-sync hook is told about failures

Deterministic check to pair with the development gate: the outage tests show reads still answered and the scope's outage recorded on spawn failure, timeout and transport errors

Excluded from (not judged; the cell passes as `excluded`):
- `b-8`: b-8 adds only a refresh deadline flag and a health read; sync failure and outage handling are b-7's.

Beads: anchor `bdc-a5u`, plan questions `bdc-a5u.1`, dev questions `bdc-a5u.2`

Fixtures:
- pass: On a bd timeout the sync records an outage for the scope and reads keep coming from the cache; test unreachable_bd_records_outage in crates/bd-cache-sync/tests/outage.rs passes in CI (run 1234).
- fail: When bd times out, every read for that scope returns BD_UNAVAILABLE until the next successful sync.
