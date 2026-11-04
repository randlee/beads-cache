---
phase: c
status: in_progress
recommended_agent: rust-developer
recommended_model: gpt-6.1-sol
id: c-1
closure_type: integration
target_boundary: existing scheduler composition
track: scheduler
wave: 1
branch: sprint/c-1-runtime-migration
worktree: sprint/c-1-runtime-migration
vertical_rationale: One compatibility migration must change the persisted schedule writer, reader and worker together
  while preserving their existing public facade; splitting requires temporary dual scheduler types rather than independent
  durable work.
must_follow: []
parallel_safe:
- c-3
requirements:
  owns:
  - REQ-BDC-1002
  constrained_by:
  - REQ-BDC-0804
  - REQ-BDC-0903
  - REQ-API-0001
  - REQ-API-0002
  - REQ-API-0003
  - REQ-API-0004
  - REQ-API-0005
  - REQ-API-0006
  - REQ-API-0007
  - REQ-BDC-0001
  - REQ-BDC-0006
  - REQ-BDC-0103
  - REQ-BDC-0107
  - REQ-BDC-0720
  - REQ-BDC-0722
  - REQ-BDC-0723
  - REQ-BDC-0724
  - REQ-BDC-0725
  - REQ-BDC-0727
  - REQ-BDC-0729
  - REQ-BDC-0730
  - REQ-BDC-0731
  - REQ-BDC-0732
  - REQ-BDC-0802
  - REQ-BDC-0803
  - REQ-BDC-0812
  - REQ-BDC-0911
  - NFR-BDC-0002
  - NFR-BDC-0004
  - NFR-BDC-0005
  - NFR-BDC-0006
  - NFR-BDC-0007
  - REQ-STORE-0002
  - REQ-STORE-0003
  - REQ-API-0008
  - REQ-CLI-0001
  - REQ-CLI-0003
  - REQ-CLI-0007
  - REQ-PY-0002
  - REQ-PY-0006
  - REQ-BDC-0910
  - REQ-BDC-0721
  - REQ-BDC-0007
  - REQ-BDC-0008
  - REQ-BDC-0502
  - REQ-CLI-0002
  - REQ-CLI-0004
  - REQ-CLI-0005
  - REQ-CLI-0006
  - REQ-CLI-0008
  - REQ-CLI-0009
  - REQ-CLI-0010
  - REQ-CLI-0011
  - REQ-CLI-0012
  - REQ-PY-0001
  - REQ-PY-0003
  - REQ-PY-0004
  - REQ-PY-0005
  - REQ-PY-0007
  - REQ-PY-0008
  - REQ-PY-0010
  - REQ-PY-0011
  - REQ-PY-0012
  - REQ-PY-0013
  - REQ-PY-0014
  - REQ-STORE-0001
  - REQ-STORE-0004
  - REQ-STORE-0005
  - REQ-STORE-0006
  - REQ-STORE-0007
  - REQ-STORE-0008
  - REQ-SYNC-0001
  - REQ-SYNC-0004
  - REQ-SYNC-0005
  - REQ-SYNC-0010
  - REQ-SYNC-0012
  - REQ-SYNC-0013
  - REQ-CRON-0001
  - REQ-CRON-0002
  - REQ-CRON-0003
  - REQ-CRON-0004
  - REQ-CRON-0005
  - REQ-CRON-0006
  - REQ-CRON-0007
  - REQ-CRON-0008
  - REQ-CRON-0009
  - REQ-CRON-0010
  - REQ-CRON-0011
  - REQ-CRON-0012
  - REQ-CRON-0013
  - REQ-CRON-0016
  - REQ-CRON-0017
  - REQ-CRON-0018
  - REQ-CRON-0014
  - REQ-CRON-0015
  - NFR-BDC-0001
  - NFR-BDC-0003
  - REQ-BDC-0703
  - REQ-BDC-0726
  - REQ-BDC-0728
adrs:
- ADR-BDC-0005
- ADR-BDC-0006
- ADR-BDC-0010
- ADR-BDC-0011
- ADR-BDC-0014
- ADR-BDC-0015
- ADR-STORE-0001
- ADR-STORE-0003
- ADR-STORE-0006
- ADR-SYNC-0008
- ADR-CLI-0001
- ADR-CLI-0005
- ADR-PY-0006
- ADR-CRON-0001
- ADR-CRON-0002
- ADR-CRON-0003
- ADR-CRON-0004
- ADR-CRON-0005
- ADR-CRON-0006
- ADR-CRON-0007
- ADR-CRON-0008
- ADR-CRON-0009
- ADR-STORE-0002
- ADR-STORE-0004
- ADR-STORE-0005
- ADR-SYNC-0001
- ADR-SYNC-0002
- ADR-SYNC-0006
- ADR-CLI-0002
- ADR-CLI-0003
- ADR-CLI-0004
- ADR-PY-0001
- ADR-PY-0002
- ADR-PY-0003
- ADR-PY-0004
- ADR-PY-0005
- ADR-BDC-0009
- ADR-BDC-0012
- ADR-BDC-0013
- ADR-CORE-0001
- ADR-BDC-0016
owned_paths:
- Cargo.toml
- Cargo.lock
- crates/sc-cron/**
- crates/bd-cache-store/**
- crates/bd-cache-sync/**
- crates/bd-cache-cli/**
- crates/bd-cache-py/**
- .github/workflows/ci.yml
- .github/workflows/python.yml
- docs/architecture.md
- docs/project-plan.md
- docs/bd-cache-store/architecture.md
- docs/bd-cache-sync/architecture.md
- docs/bd-cache-cli/architecture.md
- docs/bd-cache-cli/requirements.md
- docs/bd-cache-py/architecture.md
- docs/sc-cron/architecture.md
- docs/sc-cron/requirements.md
- docs/bd-cache-api/requirements.md
inherited_bounds:
- ANC-001
- ANC-002
- ANC-003
- ANC-004
- ANC-008
- ANC-009
- ADR-BDC-0009
- ADR-STORE-0001
- REQ-BDC-0001
- REQ-BDC-0006
- REQ-BDC-0007
- REQ-BDC-0008
- REQ-BDC-0502
- REQ-BDC-0812
- NFR-BDC-0004
- NFR-BDC-0005
- ADR-BDC-0005
---

# Sprint c-1 — Published runtime scheduler in the existing cache composition

## Outcome

The existing bdc and Python surfaces use upstream sc-cron 0.1.1 over the same
cache file. No local scheduler remains to drift. This sprint closes scheduler
integration independently of the later generated application.

## Recorded owner approval

The project owner's explicit 2026-11-02 instruction to execute all approved
Phase C sprints satisfies this sprint's dispatch prerequisite. See the phase
plan's [recorded approval](phase-c-plan.md#implementation-dispatch-gate) and
ADR-BDC-0016 for provenance and the unchanged bounds. ADR-BDC-0016 is now in the
binding `adrs` list. c-1 applies the approved CLI/API/CRON amendments in its owned
files; no other existing rule is waived. Independent implementation QA, applicable
anchor gates and green CI remain required before merge.

## Deliverables

1. REQ-BDC-1002: land the already-tested rusqlite 0.39 alignment; move sc-command
   and sc-cron to the published 0.1.1 baseline with default features off. Resolve
   one libsqlite3-sys. Retire the local sc-cron workspace member and source/tests;
   retain historical design docs with an upstream ownership note. Remove local
   sc-cron from existing CI publish dry-runs. Do not update unrelated dependencies.
   After explicit approval, apply the narrowly scoped REQ-CLI-0001 amendment in
   docs/bd-cache-cli/requirements.md from the old git tag to the published 0.1.1
   baseline. Keep its three-OS build, version envelope and usage/exit guarantees.
   Until approval, the current Approved requirement remains unchanged.
2. REQ-BDC-1002, REQ-STORE-0003: implement upstream ScheduleStore with existing
   schedule persistence and explicit limits. Preserve v3 user_version, old rows,
   next times, generations, enabled flags, outcomes, leases and hook state.
   Upstream target metadata must round-trip or be derived by a documented adapter;
   never silently reset schedules or open upstream's standalone database.
3. REQ-BDC-0720, 0724, 0725, 0730: replace TargetResolver/spawn_worker_with/run_due_with
   usage with upstream target APIs. Preserve built-in/Python switching, rollback,
   trust checks, mode lock, run identity, outages and bounded graceful/hurried stop.
   Resolve incompatibility with an upstream fix if necessary; do not fork sc-cron.
4. REQ-BDC-1002, REQ-API-0008: implement ADR-BDC-0016's narrow contract/logging
   dependency exception in existing CI graph checks. Keep clap and server/runtime
   assembly out of independently resolved API/Python consumer builds. Workspace
   feature unification with the daemon is not evidence of isolated wheel leakage;
   prove isolation using a temporary standalone consumer Cargo root and the actual
   wheel build. The all-workspace build may unify sc-command/server legitimately. Record the exception in governed docs before the
   gate changes. Amend docs/bd-cache-py/architecture.md in this sprint to record
   that same narrow ADR-BDC-0016 contract/logging exception; retain its no-clap
   and no-server restriction for isolated Python builds. Preserve public execution
   signatures shown below.

## Prototype contract disposition

The listed REQ-CRON-0001..0018 and ADR-CRON-0001..0009 constrain the prototype's
retirement as currently amended; they are not silently discarded. Under the
proposed ownership transition, c-1 preserves their parse/time/jitter/missed-tick,
stop/deadline, single-run/generation, store-atomicity/enabled, hook-target and
supervised-process guarantees through the published scheduler and cache adapter.
REQ-CRON-0009's standalone store remains upstream's default; this application
continues the existing approved cache-owned ScheduleStore exception. Prototype
API signatures, local implementation paths and local publication ownership become
historical only after the explicit transition approval, with that disposition
noted in docs/sc-cron/requirements.md and architecture.md; no obsolete API is
re-created locally. A semantic mismatch blocks the migration pending an upstream
fix or a separately authorized narrow amendment. Superseded sync/Python decisions
remain history; they are not revived by the metadata additions.

## Inherited preservation bounds

ADR-STORE-0001: the store migration includes schema.rs's schedule-extension DDL
and its format tests. Keep all v3 tables and indexes unchanged; any required
schedule-extension schema change may add only nullable columns or new tables.
ANC-002: bd sync runs at the times stored in the cache file's SQLite cron table.
ANC-003: a consumer-registered Python sync method can replace the built-in bd sync.
ANC-004: after each built-in sync completes, the optional consumer-registered
post-sync hook receives both the sync results and sync errors, including each
scope's result/error after migration to upstream target APIs. REQ-BDC-0007/0008/0502: every stored bead row, including consumer-sync
rows, passes the strict bd-schema-generated decoder; one invalid row stores no
rows from that result; sync/ingest never overwrites a newer stored updated_at.
ADR-BDC-0005: a failed bd spawn, timeout or transport failure leaves cached reads
available and records an outage for the affected scope. REQ-BDC-0006: all beads
backend reads and writes go through bd; never query Dolt tables directly or use a
generated SQL store for backend access. ANC-009: a sync scope remains a bd filter, such as labels or --all; it is never
a phase definition. NFR-BDC-0004: kill every bd call, Python sync method and post-sync hook at its
per-call timeout capped by the remaining run deadline. These bounds preserve
existing behavior.

## Bounded store-health seam for c-2

REQ-BDC-1002 / NFR-BDC-0004: add a store-owned budget handle over the already-open
connection and expose it before Composed.cache moves. c-2 consumes this additive
seam; existing CacheStore methods and compose/execute signatures stay available.
The new handle shares the connection claim, never opens another connection and
does not depend on API/transport types:

```rust
// bd-cache-store; Clone + Send + Sync, tied to one shared connection identity.
pub struct HealthReadControl { /* private store state */ }
impl SqliteStore {
    pub fn health_read_control(&self) -> HealthReadControl;
}
impl HealthReadControl {
    pub fn within<T, E>(
        &self, deadline: std::time::Instant,
        read: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E>
    where E: From<bd_cache_model::CacheError>;
}
// Composed gains the additive public health_reads: HealthReadControl field.
```

`within` is a synchronous, read-only, calling-thread scope for health evaluation
on that connection and its schedule view. It must not hold the connection mutex
around the closure, which calls normal store methods. Instead, affected sessions
acquire that mutex with deadline-aware try-lock/recheck (no unbounded wait), cap
each SQLite busy wait by remaining time, and use SQLite progress/deadline checks
through statement execution and row decoding. Restore per-session busy/progress
settings on success, error and unwind while the connection guard is held; other
threads' sync/write sessions must not inherit the health budget or be interrupted
by a late watchdog. Reject mutation in this read-only scope. No detached watchdog
or cancellation-by-dropping-a-future is an implementation of this contract.
Use rusqlite's safe hook API (enable its hooks feature only if needed), not new
unsafe code or a backend client. Expiry/lock refusal is the existing
BD_CACHE.STORE_FAILED with a bounded health-read cause, preserving source details;
the original unconstrained health path retains its current behavior.

Extend existing store/session and health tests to cover mutex contention, a long
SQLite statement and row iteration, restoration after error/unwind, and concurrent
normal scheduler writes. A 1-second health budget must complete or return its typed
error within that budget plus documented scheduling/interrupt-check tolerance.
The finite scope must finish before the blocking task is joined; an async timeout
alone is not a passing test. c-2's production readiness test consumes this handle.

## Interface preserved for consumers

```rust
pub fn compose_with_config(
    repo_root: &Path, config: RepoConfig, bd_binary: &str,
    clock: Arc<dyn Clock>, beads_dir_env: Option<&OsStr>,
) -> Result<Composed, ComposeError>;

// On SystemSchedules, as on the current Schedules implementation:
pub async fn cron_run(
    &self, stop: StopFlag, shutdown_timeout: Duration,
    on_expiry: impl FnOnce() + Send,
) -> CronStopped;
```

`Composed::into_schedules`, `execute_api`, `Schedules::cron_run_hurried` and the
existing front-end API remain available. c-2 consumes their migrated production
implementation, not a second scheduler. The exact upstream trait is fixed by the
0.1.1 dependency source; do not restate it in a local trait copy.

## Acceptance criteria

| Root | Positive observable | Planted negative |
|---|---|---|
| req:REQ-BDC-1002 | lock/metadata show upstream sc-cron, rusqlite 0.39 and one SQLite link provider; existing CLI and wheel still build | restore local sc-cron/path patch; dependency assertion detects it |
| req:REQ-BDC-0103 | open a pre-change v3 cache with schedules and pending journal entries, run migrated sync, reopen it; state and identity survive | reset schedule rows or bump user_version; compatibility test fails |
| req:REQ-BDC-0724 | real composition switches built-in → Python → built-in, including failed-switch rollback and same-process health | leave both enabled or reset prior mode on failed run; scenario fails |
| req:REQ-BDC-0730 | CLI and Python use migrated worker; timeout, second stop, hook child-tree kill and lease release obey current bounds | release lease/instance state before child reap or omit kill; process test fails |
| req:REQ-BDC-0001 | current outage and cached-read suites pass; cache hits invoke no bd and retain latency budget | insert a backend read on a hit; call-log/latency test fails |
| req:REQ-BDC-1002 / NFR-BDC-0004 | the store-health scope enforces its deadline through lock wait, SQLite work and decoding, restores settings and leaves concurrent scheduler work intact | replace the scope with timeout(spawn_blocking) or leave a stale progress hook; bounded/restoration tests fail |
| req:REQ-API-0008 | contract-only sc-command path is permitted, clap/server/runtime leakage remains rejected in isolated consumer builds | enable sc-command/server in the isolated Python consumer; graph test fails |
| req:REQ-BDC-0721 / ANC-009 | a configured label-filter scope and an --all scope both sync through the migrated worker as bd filters | derive a scope from phase files instead; scope-sync scenario fails |

## Current CI prerequisite

PR #105 exposed a pre-existing runner compatibility failure: Ubuntu 26.04 under
ubuntu-latest does not offer Python 3.9 in actions/setup-python's build manifest.
Preserve Python 3.9 support by pinning both Linux wheel-build and wheel-test jobs
to a compatible runner together (24.04 is available), then qualify that matrix.
A test-only runner pin risks testing a newer-glibc wheel on an older image. This
repair serves REQ-BDC-0812/REQ-BDC-0910, adds no product operation and must not be
mistaken for a failed cache test or a reason to drop Python 3.9 support.

## Required validation

NFR-BDC-0005: unit tests run with no bd and no Dolt, using the deterministic fake
bd for backend behavior. Live checks stay on the designated scratch testbed.

- `cargo fmt --all --check`.
- Both existing workspace Clippy gates, with `--locked`, excluding bd-cache-py.
- `cargo test --locked --workspace --exclude bd-cache-py --all-features` and docs.
- Existing Python Rust/wheel/pytest matrix, including the neutral 0.1 corpus.
- Existing process/shutdown, v3 golden and scheduler-switch tests; extend these
  suites for the upstream migration instead of adding a parallel harness.
- Existing ci.yml boundary, no-backend, advisory and publish-dry-run gates.
- Green Linux/macOS/Windows CI; report local evidence separately from CI.

## Dependencies and limits

c-3 is parallel-safe: its template pin/answers/helper do not overlap this fence.
c-2 follows the actual upstream-backed SystemSchedules and migrated store.
This sprint does not close generated daemon behavior or template upkeep (c-2),
or artifact installation (c-4). sc-lint adoption is deferred pending upstream 0.2.0
and a later plan decision, outside the current sprint set.

## Implementation delivery (2026-11-02)

Development is delivered on the declared sprint branch; independent QA, the
sprint anchor gate and hosted three-OS CI still govern integration closeout.
The approved owner record was merged from integrate/phase-c commit 8850fc8 before
implementation. The primary checkout's Cargo files were not touched.

- Cargo.toml/Cargo.lock use published sc-cron/sc-command exactly 0.1.1, default
  features off, rusqlite 0.39 and one libsqlite3-sys. The local scheduler source,
  tests and publish dry-run entry are retired; historical CRON docs retain their
  semantic obligations and upstream ownership note.
- store/schedule.rs implements upstream's exact ScheduleStore with a 1024 active
  row ceiling. Cache meta format 2 and user_version 3 stay unchanged. Rust hook
  metadata and empty environment are derived; consumer module policy already
  round-trips in existing payloads. Unsupported native upstream hook metadata
  is refused before writes instead of being discarded.
- sync/launch.rs adds scheduler-only row/generation envelopes; sync/job.rs uses
  upstream's shared Job port and creates each launch's own job and run id.
  sync/module_hook.rs preserves -I -m module invocation using upstream HookCall,
  HookOutput and process supervision. cron_log.rs bridges real upstream canonical
  events into the existing tracing/log-redaction surface; no scheduler or
  supervisor was copied or patched.
- store/health.rs and store/store.rs expose HealthReadControl over the same
  connection claim, with deadline-aware locking, dynamic busy retries, safe
  SQLite progress hooks, per-row decode checks, mutation refusal and RAII
  restoration. Composed.health_reads is captured before Cache owns the store.
- Governed CLI/API/Python/CRON docs record ADR-BDC-0016 before CI boundary changes.
  The existing transitive CI boundary now resolves temporary standalone API and
  Python consumers via tests/check_consumer_graph.py. Published contracts pass;
  a real sc-command/server dependency is the planted failing graph. The existing
  actual wheel build remains required. Linux wheel-build and wheel-test already
  both use ubuntu-24.04, preserving Python 3.9 qualification.

Concrete compatibility/anchor proof paths:

| Bound | Implementation and proof |
|---|---|
| ANC-001/002, cache-time scheduling | sync/launch.rs, sync/cron.rs; sync/tests/e2e.rs literal pre-migration v3 scheduled-drain/reopen test preserves generation, id, payload, journal, hook state and user_version |
| ANC-003, replacement and rollback | sync/job.rs, sync/module_hook.rs; existing sync/tests/schedule_switch.rs, cron_rows.rs and py/tests/test_schedule.py |
| ANC-004, per-scope hook results/errors | existing sync/hook.rs and run.rs use published supervisor; sync/tests/hook_runner.rs and builtin_run.rs |
| ANC-008, row identity | launch.rs derives row identity independently of payload; existing sync/tests/cron_rows.rs rejects another row carrying a sync payload |
| ANC-009, scopes are bd filters | the v3 e2e test schedules both label-filter and All scopes; existing builtin_run.rs and CLI tests/scope_filters.rs assert filter-specific behavior |
| NFR-BDC-0004, bounded health | health.rs tests exercise actual public row reads, mutex refusal, one-second SQLite work, row iteration, restoration after error/unwind and concurrent normal schedule writes |
| REQ-BDC-0730, reaping and lease release | unchanged CLI tests/sync_process.rs and tree_kill.rs run against upstream-backed SystemSchedules; both tree-kill cases pass with process inspection allowed |
| REQ-BDC-0001, outage/cache hit | existing CLI tests/outage_scenario.rs, sync/tests/e2e.rs and perf.rs; release latency remains an explicit validation command |

Pre-delivery evidence: workspace check and all-target/all-feature Clippy pass;
new v3 scheduled-drain/reopen test and canonical-log/redaction test pass; four
initial bounded health tests pass; both process-tree timeout tests pass outside
the sandbox. The sandbox denies ps, so its child-alive assertions are not valid
process evidence. Full post-push validation and actual wheel/pytest results are
reported separately; hosted Linux/macOS/Windows results remain CI evidence.

### Post-push local validation (2026-11-02)

All local required checks PASS at source head 750ed9b (the following evidence
update changes documentation only). Workspace tests ran with bd/dolt absent
from PATH and local process inspection permitted; no live backend was used.

| Check | Result |
|---|---|
| cargo fmt --all --check; git diff --check | PASS |
| Both locked workspace Clippy gates, excluding bd-cache-py | PASS, all-target/all-feature and default-feature library modes |
| cargo test --locked --workspace --exclude bd-cache-py --all-features | PASS: 1,145 passed, 0 failed, 20 intentionally ignored across 110 test/doctest groups |
| RUSTDOCFLAGS=-D warnings cargo doc --locked --workspace --exclude bd-cache-py --no-deps | PASS |
| Python Rust Clippy and cargo test, Python 3.11 | PASS: 23 Rust tests |
| maturin build --release --locked, actual wheel | PASS: macOS arm64 cp39-abi3, Python >=3.9 |
| Installed wheel, Python 3.11 pytest with deprecations as errors | PASS: 226 tests, including neutral 0.1 corpus, switches, hooks, CLI end-to-end and threading |
| Release new-process CLI cache-hit latency | PASS: median 6.6 ms below 50 ms, p95 7.2 ms; pending-write median 7.5 ms/p95 8.0 ms reported |
| Installed-wheel cached get | PASS: median 20.5 us, p99 26.9 us over 2,000 calls |
| Existing direct boundary workflow body; independent API/Python graphs | PASS; published 0.1.1 contracts permitted; actual standalone sc-command/server planted negative rejected |
| Existing no-backend PATH/graph and strict consumer lint | PASS |
| Existing Python scripts suite | PASS: 71 tests |
| cargo deny --offline check advisories | PASS against the installed local advisory database; hosted CI performs its normal refresh |
| cargo publish --dry-run -p sc-beads -p sc-bd-cli | PASS: both packaged and verified; uploads aborted by dry-run |

Logs are available to the parent in /tmp/c1-full-validation.log,
/tmp/c1-health-final.log, /tmp/c1-v3.log, /tmp/c1-py-rust.log,
/tmp/c1-pytest.log, /tmp/c1-wheel.log, /tmp/c1-latency.log,
/tmp/c1-graph-final.log, /tmp/c1-graph-negative.log, /tmp/c1-boundary-direct.log,
/tmp/c1-scripts.log, /tmp/c1-advisories.log and /tmp/c1-publish.log.
The actual installed package came from /tmp/c1-wheel-venv/lib/python3.11/site-packages.

Hosted Linux/macOS/Windows CI and Python 3.9/3.13 qualification remain required
before integration; local macOS/Python 3.11 evidence is not a substitute. The
existing Linux wheel and test jobs are both pinned to ubuntu-24.04. The bead
remains in_progress until the parent's independent QA/anchor/CI closeout.


## QA1 correction delivery and remaining public API blockers

QA1 reviewed 4d27523d2919b8e0c0e6d0f9287c5e4fc9736e56 on PR #108 and
published FAIL with 22 IDs (8 Blocking, 12 Important, 2 Minor). Source of truth:
/tmp/phase-c-execution/c-1/qa-1/manager-result.md; ceremony triage:
/tmp/phase-c-execution/c-1/qa-1/screen-result.md. No ID is waived or collapsed.
The sprint is in_progress. The following implementation dispositions request
independent verification, rather than declaring the QA findings closed.

| Finding | Correction / remaining disposition |
|---|---|
| REQ-QA-001 | TrustedInterpreter enforces the inherited 256-byte module bound for both sync modules and post-sync hooks; schedule_switch.rs proves 256 accepted, 257 refused with both prior rows unchanged. |
| REQ-QA-002 | Existing constrained_by list now includes REQ-BDC-0804, REQ-BDC-0903 and REQ-API-0001..0007. |
| REQ-QA-003 | Sprint status is in_progress; narrowly authorized project-plan lifecycle receipt records owner approval and C1 QA1 corrections, preserving all other sprint rows. |
| REQ-QA-004 | Earlier reviewed head eventually completed 15/15 hosted checks; manager receipt records ci run 41563460661 and python run 41563460677. Req reviewer must confirm that receipt. Changed production head needs fresh checks; the strengthened boundary gate intentionally remains red for the unresolved logging edge. |
| REQ-QA-005 | Existing perf.rs now qualifies real migrated scheduled incremental sync over the existing 1000-bead fake-Bd composition, asserts an incremental cursor and changed cached row. Release qualification measured 6.184458 ms, below 1 s. No live bd ran. |
| ARCH-001 | OPEN/BLOCKED: current direct receiver edge is unauthorized. Removed implementation-document text that appeared to bless it; public 0.1.1 cannot construct this receiver through sc-cron alone. Exact minimal upstream need is below. |
| RBQA-F001 | OPEN/BLOCKED, correlated with ARCH-001 but retained independently. No new direct framework edge, baseline change, fork or missing-event substitution is introduced. |
| RBQA-F002 | Existing isolated graph gate now traverses root-reachable normal/build edges and permits sc-command introducers only from published sc-cron 0.1.1. Explicit axum/rmcp exclusions added. Focused planted negatives cover direct sync introducer, local source, server feature, HTTP/MCP and ignored dev-only edges. Existing candidate leak is detected, not suppressed. |
| RBQA-F003 | ModuleHook is exported only under testing; invoke is crate-private and no normal consumer surface is added. |
| C1-RBP-F001 | Local unrunnable constructor returns Arc<dyn Job> directly; only genuinely fallible resolver branches retain Result. |
| RSH-001 | OPEN/BLOCKED: published WorkerHandle shutdown owns an unbounded join of a blocking store call after its handle deadline. No timeout/drop or fake abort is substituted. Exact minimal upstream need is below. |
| RSH-002 | Scheduled built-in job propagates its upstream absolute deadline through bounded start/end record cache claims, preparation, sync/drain, hook offer/launch and final cache claim. Budget derives from original scheduler launch instant (deadline minus row timeout), before record or mutex delay, and is capped by the caller. Public on-demand run_builtin remains. job_threading.rs proves held cache claim expires and no detached preparation issues later bd calls. |
| RSH-003 | Scope/filter decoding and every public store row iteration/status assembly boundary now use the common calling-thread health deadline check. Added deterministic public scopes/status expiration coverage; normal unscoped reads/restoration remain covered. |
| RSH-004 | Continuous worker context retains no detailed failure collection; finite cron_run_once retains exact reply details. Persistent run counters/logs remain. Unit proof records 10000 continuous failures with no retained history and verifies finite details drain exactly once. |
| FTQ-001 | QA1 removed the release-sleep race but QA2 found its failure-path reader join still unbounded. The QA2 correction below confines the whole scenario to an owned child with an enforced lifetime; this replaces the incomplete QA1 cleanup claim. |
| FTQ-002 | Public row-decode test injects expiration after decoding begins using a cfg(test) calling-thread seam; it no longer assumes finite fixture decoding takes 20 ms. Separate real one-second SQLite integration bound remains. |
| FTQ-003 | Health session first acquires the shared connection, then a normal writer acknowledges an actual failed acquisition via a cfg(test) contention probe. SQL expires while the writer contends; restored scope precedes the successful normal write. |
| FTQ-004 | Store observer and both sync observers acknowledge their first successful snapshot before mutations. Owned stop guards release/reap observers on success/unwind with finite completion waits. |
| FTQ-005 | Mode-lock test waits for an actual contended acquisition attempt through the existing testing-only facade, then checks unchanged flags/incomplete task. Release and final task join are bounded; the scheduling sleep is removed. |
| FTQ-006 | Manual-clock hook/run tests use a real bounded owned-task join helper; expiry aborts the owned task and waits bounded cleanup before failing. Both repeated hook_runner occurrences are corrected. |
| FTQ-007 | Cargo metadata runs with a hard subprocess deadline and captured diagnostics. subprocess.run kills/reaps its owned child on timeout; a real sleeping-child test proves the deadline path. |
| FTQ-008 | Lease-abort holder acquisition/readiness/release and task/thread completion are bounded; release sender ownership closes the wait during unwind. |

### Minimal unresolved upstream public API requirements (no local workaround)

Published immutable sc-cron 0.1.1 is the only approved baseline. No publication
request, version relaxation, source patch or copied scheduler is authorized by
this correction record.

1. Logging (ARCH-001 / RBQA-F001, ADR-BDC-0016): scheduler.rs:288-292 accepts
   sc_command::observability::SharedCanonicalLog, but lib.rs:100-178 exports no
   public receiver/type adapter. Scheduler::new at scheduler.rs:279-280 uses
   private noop_logger (lib.rs:181-183). The minimum upstream public seam is
   re-exporting the receiver and all required signature types through sc-cron,
   or an equivalent upstream callback-to-canonical-receiver constructor that
   can be used without any direct application -> sc-command dependency. Then
   remove the direct sync dependency and bridge through that seam. Current
   source still contains that rejected edge, and the strengthened graph fails
   on it. Dropping canonical events would violate REQ-BDC-0730, not resolve it.
2. Shutdown (RSH-001, REQ-BDC-0730 / REQ-CRON-0012): WorkerHandle's task and
   shutdown sender are private (scheduler.rs:850-853). shutdown at :904-954
   unconditionally awaits the task after timeout_at; worker.rs:539-552 awaits
   uncancelled spawn_blocking storage. The minimum behavioral/public contract
   must let stop deadlines/cancellation reach outstanding store operations and
   retain their ownership through a bounded join, including T=0, a contended
   reload/persistence call, and second-stop shortening. Merely exposing an
   abort handle or dropping a timeout future leaves blocking work unowned and
   is insufficient. The existing health read-only scope cannot cancel those
   normal writes. No local cancellation framework is added; this remains a
   real upstream compatibility blocker.

The owner-directed paste/sc-lint 0.2.0 deferral does not waive any finding above.
Existing gates remain enabled. Final correction validation and exact push SHA
are reported separately; this record does not claim QA PASS or merge readiness.


### QA1 correction validation receipt

Exact validated source head: 0885ce639a5cd53d1458baf3ea6acb48b74479a7.
Independent corrections are commit 0134c15; merge 0140cf3 consumed approved C3
integration including 22f9d6b9ddc67274d95a23c16a2a8345c0785bc0, and 0885ce6
removed the duplicate shared prerequisite installation created by that merge.
This receipt changes documentation only.

- PASS: fmt/diff checks, both locked workspace Clippy modes and warning-free
  docs. Logs: /tmp/c1-fix-prevalidate-clippy.log,
  /tmp/c1-fix-clippy-default.log, /tmp/c1-fix-doc.log.
- PASS: full workspace tests/doctests, 1149 passed / 0 failed / 21 intentionally
  ignored, with bd/dolt absent from PATH and process inspection permitted.
  /tmp/c1-fix-workspace.log. Focused store/sync: 282 passed, 8 intentional
  ignores (/tmp/c1-fix-focused.log).
- PASS: explicit release-profile incremental qualification, 6.184458 ms for a
  migrated scheduled 1000-bead scope, with cursor and changed-row assertions.
  /tmp/c1-fix-incremental.log. Its normal-suite ignore is not the evidence:
  the exact timing command ran explicitly with --release --ignored --nocapture.
- PASS: fresh actual cp39-abi3 macOS arm64 wheel built from the changed source;
  installed-wheel Python 3.11 pytest 226 passed, including neutral corpus,
  switching, hooks and real CLI fixtures. Py Rust tests: 23 passed; Py Clippy
  passed. /tmp/c1-fix-wheel.log, /tmp/c1-fix-pytest.log,
  /tmp/c1-fix-py-rust.log, /tmp/c1-fix-py-clippy.log.
- PASS: merged script suite in the clean declared dependency environment:
  97 run, 96 passed, 1 explicit opt-in real-generator smoke skip.
  /tmp/c1-fix-scripts.log. No script tests were suppressed for C1.
- PASS: four graph checker/provenance/forbidden-package/owned-child deadline
  proofs (/tmp/c1-fix-graph-unit.log). Actual API isolated graph passes.
  EXPECTED FAIL remains for actual isolated Python normal/build graph:
  bd-cache-sync -> sc-command is unauthorized (/tmp/c1-fix-graph-final.log).
  The existing CI gate executes that rejection; no allowlist masks it.
- PASS: no-backend PATH/graph and strict consumer lint after the source changes.

Overall correction disposition remains FAIL / blocked, not QA PASS: ARCH-001,
RBQA-F001 and RSH-001 need the documented upstream public APIs. REQ-QA-004
requires the owning reviewer's confirmation and appropriate exact-head hosted
qualification; current expected boundary failure cannot be declared green.
The other 18 implementation/evidence corrections request independent QA2
verification. No acceptance criterion, baseline pin or advisory gate was waived.
The bead and sprint remain in_progress, and no merge is authorized by this receipt.

### QA2 narrow correction receipt (2026-11-02)

Manager QA2 reviewed 73ced71 and published FAIL (4 Blocking, 2 Important) at
https://github.com/randlee/beads-cache/pull/108#issuecomment-6594983314.
The byte-identical manager record and full six-ID assignment are
/tmp/phase-c-execution/c-1/qa-2/manager-result.md and fix.xml; ceremony triage is
/tmp/phase-c-execution/c-1/qa-2/screen-result.md. No finding is waived.

| Finding | QA2 correction / remaining disposition |
|---|---|
| FTQ-001 | Correction present, specialist verification pending. health.rs runs the existing causal mutex fixture in an owned child test process. The parent enforces a five-second lifetime, kills and reaps on expiry, and its owner Drop also kills/reaps during unwind. File-backed diagnostics avoid pipe-drain threads; parent-owned scratch storage removes a killed child's cache. The original holder/read-start handshakes, 50 ms read budget, 200 ms answer deadline, <150 ms assertion and post-release store.status check remain. A deliberately stalled reader reaches the contended-read failure path and proves timeout kill/reap, rather than allowing a detached-thread success or health retry. |
| REQ-QA-004 | OPEN. Actual authorized dependency composition and all required hosted checks must pass on the reviewed changed head. Prior-head green and local successful suites do not close it. |
| ARCH-001 | OPEN/BLOCKED. Unauthorized direct sync -> sc-command receiver edge remains; the minimal upstream public seam recorded above is still required. |
| RBQA-F001 | OPEN/BLOCKED. Same unresolved production edge as ARCH-001, separately retained. No new exception is authorized. |
| RSH-001 | OPEN/BLOCKED. Published 0.1.1 bounded-shutdown incompatibility remains; no local cancellation, abort or timeout/drop workaround is added. |
| QA2-001 | OPEN. The actual isolated Python graph rejects the unauthorized introducer, correlated with ARCH-001/RBQA-F001 but independently retained. The strict gate stays enabled and unchanged. |

This round changes only health.rs test code and this receipt, after updating from
the integration branch. Production code, dependency pins, acceptance criteria and
baseline/advisory gates remain unchanged. Targeted validation results and exact
push SHA are reported separately. Overall FAIL / blocked and in_progress remain.


### Consumer restart correction: bounded cache scheduler shutdown

The fresh restart assessment inspected actual hosted job 126038712470 at source
90baf073. Its failure is the actual isolated Python graph's direct
`bd-cache-sync -> sc-command` edge, after direct boundary and graph unit checks
pass. It is unrelated to the deferred paste/sc-lint work. The sprint consumed
current integrate/phase-c at ea53f022 before this correction; the merge changes
only orchestration metadata.

RSH-001 now has a consumer-owned implementation pending independent QA. This
supersedes the earlier claim that outstanding cache-store cancellation necessarily
requires an upstream change. `CacheScheduleStore::worker_view` shares the existing
connection and file claim; its separate `ScheduleShutdownControl` exists before
worker calls begin and can shorten a mutex wait, SQLite busy/progress work and
row decoding already in progress. Scoped session settings restore before the
connection is unlocked. Other views/connections, ordinary cache sessions and
HealthReadControl retain their own policies; no health deadline is repurposed.

One upstream Scheduler remains the execution owner for continuous and run-due
work, with its original coordination and canonical receiver. Public management
operations invoke that same Scheduler's documented synchronous methods through
`share()` in the existing owned blocking helper. A calling-thread RAII management
scope names both the exact worker-view control and physical connection. It
preserves normal management wait/error behavior and never exempts health
read-only/deadline rules, worker calls, run-due work or job callbacks. The same
Core performs validation/state changes and immediate reload notification; a
second mutation Scheduler and a caller-supplied executor/factory are not used.

The cron facade selects expiry independently of the producer's shutdown future,
requests child/bd kill at expiry or a second signal, and still awaits the actual
owned worker/store join. Its cleanup deadline is fixed from the original grace
boundary, preserving the existing `2 × T` approved by ADR-SYNC-0008's shutdown
amendment (docs/bd-cache-sync/architecture.md:735–742) and ADR-BDC-0016. A second
signal begins the existing cleanup stage early. The store control resets only
after the worker really joins, so reuse needs no caller reset and a rejected
second start cannot reset the first worker's deadline. This does not assert that
an arbitrary third-party ScheduleStore can be preempted; the production cache
composition uses the new bounded store constructor and its existing signatures
remain available.

Focused development evidence: store/sync suites passed 292 tests with 8 explicit
ignores, including real process/tree/lease paths. Final dedicated lifecycle tests
cover ordinary, hurried and zero-timeout store joins before external writer
release; ordinary total completion is checked inside the existing `2 × T`;
public management remains owned and completes normally after writer release.
They also cover same-object immediate Python cancellation before the 30-second
poll with no new outcome, repeated worker lifetimes, and upstream's exact
concurrent-start panic without changing the first deadline. Store tests cover a
real held mutex, busy persistence, executing SQL, per-view/connection isolation,
health policy and restoration after error/unwind. Planted mutants disabling
store cancellation fail all three bounded-store tests; disabling management
isolation fails its scope test. Sources were restored and the store suite passed.
The normal and all-target/all-feature locked Clippy gates pass. Detailed logs
are under /tmp/phase-c-execution/c-1/qa-4/restart-*.log; final post-push validation
is reported separately.

All five IDs remain **open**: RSH-001 is implemented_pending_independent_QA;
ARCH-001, RBQA-F001 and QA2-001 still require a consumable public canonical
receiver path; REQ-QA-004 still requires all actual checks green on the reviewed
head. The published 0.1.1 Scheduler accepts a nominal sc-command receiver trait
that sc-cron does not re-export or construct from a callback. Moving the bridge
into C2's optional daemon cannot deliver sc-cron's own events for existing
CLI/Python callers below that boundary; moving it into another lower crate only
moves the forbidden introducer. Minimum producer need is a public sc-cron
receiver/type re-export or equivalent constructor delivering real canonical
events without a direct consumer sc-command dependency. No producer change, new
PR, baseline relaxation, architecture waiver or missing-event substitute is
introduced. Independent QA, anchors and green CI remain prerequisites to merge.


### QA4 test-lifetime correction (FTQ-009 / FTQ-010)

QA4's actual manager report is
https://github.com/randlee/beads-cache/pull/108#issuecomment-6620322598,
reviewing 95def5f3c1c2ffe7a0e9ebfe3630ac835217c680. The owning service reviewer
accepted RSH-001 for the production composition and the parent recorded that
closure with owner/enforcer evidence. That scoped acceptance is preserved;
production shutdown, session, control, adapter and dependency code is unchanged.
The new Important FTQ-009/010 findings concern failure cleanup in the new tests.

The complete three synchronous store fixtures and the new lifecycle fixtures
now execute in exactly filtered owned child processes. A small shared test-only
helper follows the existing health fixture pattern, with file-backed output,
parent-owned scratch storage, a five-second lifetime measured from spawn, and
kill/reap cleanup on timeout and assertion unwind. No tested thread or async
shutdown future is detached. The existing real task/thread joins stay inside
that enforceable process lifetime. Original causal probes, 30 ms cancellation,
500 ms store observation, production elapsed/callback checks and two-second
second-start observation are unchanged; process containment grants no additional
product grace or cleanup budget.

Lifecycle fixtures also register each actual managed Python child in the
parent-owned scratch root, which survives a child assertion's local TempDir
cleanup. The supervisor kills those separately owned process groups while the
real runtime/parent remains alive to reap them, kills/reaps the test process,
and confirms the registered Python PIDs have disappeared. The timeout and Drop
paths share this cleanup. Its OS cleanup confirmations have finite 250/750 ms
observations; a missing reap is an error, never a success claim.

Meaningful negatives cover all three store joins after an actual mutex, busy
write or running-SQL operation, plus lifecycle worker join, management join and
incorrectly accepted second-start cleanup stalls. They reach the original
bounded failure paths before the whole fixture is killed and reaped. The
second-start negative owns a real running Python child and verifies its PID is
gone after cleanup. Separate assertion-unwind negatives verify both fixture
families; the lifecycle one also verifies real Python reaping. Pre-push focused
runs passed 5 store tests and 8 lifecycle tests; affected locked Clippy modes and
fmt passed without suppressions. Final pushed-source logs are reported separately
under /tmp/phase-c-execution/c-1/qa-4/fix-*.log.

| Finding | Current correction/disposition |
|---|---|
| FTQ-009 | Implemented pending independent QA: complete store fixtures have enforced owned lifetimes; all three real post-call stalled join paths and parent unwind cleanup are tested. |
| FTQ-010 | Implemented pending independent QA: complete lifecycle fixtures contain worker/management/second-start cleanup stalls, retain real ownership, and prove timeout/unwind Python kill and reap. |
| ARCH-001 | OPEN: the genuine published canonical receiver interface gap remains; the prior exact-source assessment and minimum public receiver contract still apply. |
| RBQA-F001 | OPEN: the same unauthorized introducer remains independently tracked; no architectural exception is widened. |
| QA2-001 | OPEN: the actual isolated Python graph still rejects that unchanged direct sc-command edge; no graph guard is bypassed. |
| REQ-QA-004 | OPEN: actual changed-head hosted checks remain required, with the existing boundary blocker unwaived. |

RSH-001 remains **closed only under its actual QA4 owner/enforcer evidence**.
FTQ-009/010 remain open pending fresh independent review. No new upstream work,
version change, publisher dependency, full workspace rerun or sprint/anchor/CI
PASS is introduced by this test-only correction. Merge remains gated.


### QA5 pre-registration process ownership correction (FTQ-010)

QA5 manager https://github.com/randlee/beads-cache/pull/108#issuecomment-6620709101
accepted FTQ-009 and retained the actual production RSH-001 closure, but its
owning flaky reviewer reproduced a residual FTQ-010 orphan before the Python
stub's PID registration. The source-free PATH `cat` delay left the real Python
shell/cat group alive after the old fixture timeout. The prior registered-child
proof was insufficient for complete launch ownership.

Python lifecycle fixtures now establish a new kernel session with the existing
safe rustix API before creating their runtime, stores, stub or any producer job.
Published sc-cron's separate process groups inherit that session. A test-only
SessionFixture composes the existing owned fixture child and uses kernel session
membership as its cleanup authority, not child-written PID files. A process
inventory capability check occurs before launching a fixture. No SDK, dependency,
production execution, logging, graph, or baseline change is made.

Within the existing first 250 ms cleanup observation, the supervisor first lets
actual runtime parents reap killed job groups, then stops the fixture root and
acknowledges a stable census of stopped/zombie session members before termination.
This prevents those members from launching more work between census and kill.
The supervisor kills and actually waits for the direct fixture process, then
re-enumerates and drains the inherited session during the original final 750 ms
observation. Session ownership survives separate groups and reparenting; the
post-root-join census covers in-flight launches. Cleanup succeeds only when all
session members have disappeared. Timeout and assertion unwind use the same
path. No future or synchronous work is detached.

A native negative stalls the actual production PythonJob/PyStub before input
consumption and before any PID registry entry, with its real shell and external
command in the producer's distinct process group. Both the five-second fixture
timeout and parent assertion unwind confirm kernel session emptiness plus actual
PID and process-group disappearance/reap. The test keeps the original causal
and strict production timing checks. The existing post-registration, management,
worker-join, same-Scheduler cancellation and rejected-start cases remain.

The exact QA5 source-free PATH-cat counterexample also ran against the rebuilt
binary: it still fails finitely (exit 101), while `post_test_exit_survivors` is
empty **before** the probe's explicit cleanup. Final post-push lifecycle and
unchanged-probe evidence, including SID/PID/PGID audit, is reported under
/tmp/phase-c-execution/c-1/qa-5/fix-*. The old failing probe is retained as the
counterexample; no failure was converted into a false successful fixture.

Accepted production files and the complete store test module are byte-identical
to 2111df6f. The accepted FTQ-009 shared supervisor's logic is also byte-identical;
only four test-internal field visibilities allow the lifecycle session owner to
compose it. This proof is /tmp/phase-c-execution/c-1/qa-5/fix-source-equivalence.json.
The normal store-only fixture path is unchanged. RSH-001 and FTQ-009 remain closed
only under their actual owning reviewer/enforcer evidence.

FTQ-010 remains open, implementation pending independent QA6. REQ-QA-004,
ARCH-001, RBQA-F001 and QA2-001 remain open for the unchanged genuine published
receiver/isolated graph and actual-head CI blockers. The previous bounded public
interface assessment is retained without another workaround loop. No publisher
work, SDK factory/executor, version change, waiver or whole-sprint/anchor/CI/merge
PASS is introduced.


### QA6 owned inventory and Linux identity correction (FTQ-010 / REQ-QA-004)

The test-only session inventory now owns each `/bin/ps` child, redirects its
output to an owned file, polls completion against the caller's absolute deadline,
and kills and actually reaps a stalled child before returning its failure. Drop
also kills/reaps that same child; no pipe reader or wait task is detached. A scan
gets at most 50ms inside the existing cleanup observation, with no new scan
started in a phase's final partial slice. The 250ms cooperative/quiescence and
750ms final observations now share one absolute one-second cleanup budget. The
final drain remains after the actual root join and includes unregistered and
in-flight Python groups. An inventory error is retained as failure even if a
subsequent inventory removes the tree; it never becomes a cleanup-success receipt.

The session owner performs cleanup once. The composed shared FixtureChild has an
explicit Drop ownership handoff, defaulting to its original store cleanup. Only
SessionFixture takes that ownership; its failed cleanup cannot restart a fresh
registry budget during field destruction. The accepted store test bodies and
production shutdown implementation remain unchanged. Focused default store
cleanup tests cover this necessary shared-helper adjustment.

On Linux, the all-host PID census reads kernel `/proc/<pid>/stat` metadata as raw
identifiers and excludes nonpositive session/group IDs before constructing a
positive typed PID. Parsing uses the final comm delimiter, including names with
spaces or parentheses. Every kill/stop still rechecks current session AND group
identity; the new inventory retains kernel-session ownership established before
real production PythonJob launch. Darwin retains its native kernel identity path.
The old hosted getsid assertion is established by the actual stack; a particular
foreign kernel PID was not logged, so its identity remains an inference. The
regression covers zero/negative raw IDs, a normal identity and malformed metadata.
Local Darwin passes do not establish Linux or Windows qualification.

Focused evidence is recorded in /tmp/phase-c-execution/c-1/qa-6/fix-*. Logs include
actual stopped-inventory timeout/unwind, complete lifecycle and store cleanup
families, unchanged QA5 PATH-cat and QA6 SIGSTOP counterexamples, and metadata-only
SID/PID/PGID/inventory-child audits before probe fallback cleanup. The original
negative fixtures must still fail finitely; no failing test is reported as a
successful fixture. Required exact-new-source hosted Linux CI is tracked separately.

FTQ-010 remains OPEN pending independent QA7. REQ-QA-004 remains OPEN for actual
new-head platform checks AND the unchanged strict logging dependency gate.
ARCH-001, RBQA-F001 and QA2-001 retain their genuine public logging receiver and
isolated dependency graph blocker. RSH-001 and FTQ-009 closures remain owned by
their actual reviewer/enforcer evidence. No SDK change, dependency change,
publisher work, timing relaxation, guard waiver or sprint/anchor/merge PASS is
introduced.


### QA7 supported startup stop correction (RSH-001)

Independent C2 QA5 reopened RSH-001 for a new startup occurrence. A supported
public caller may retain Composed's ordinary table view and start management
before Application starts. The old cron prelude awaited warn_at_startup/rows on
that normal view before observing stop. The actual Application probe remained
pending at 902ms with T=250ms and returned only after writer rollback. This does
not establish a stock-daemon HTTP reproduction: stock composition does not
itself initiate that independent pre-start management writer. The previously
qualified post-start fix remains credited. No exclusive-startup precondition is
added to the public contract.

Cron's internal startup read now uses the existing bounded raw worker view of
the same physical connection. Public rows, standalone warn_at_startup and
independent management keep their normal policies. Cron selects stop/hurry while
retaining the actual blocking read future. The first observed stop starts the
original T/2T budget; expiry/hurry calls the real callback and cooperatively ends
only the owned startup read. Cron really joins it, then resets its control. It
never spawns the SDK worker after a startup stop, never renews the stop budget
for a later phase, and never reports a canceled read as drained/ready. Normal
startup still emits the existing warnings before spawning the one SDK worker.
The private lifecycle claim covers startup through real worker/store/cache
joins, retaining the documented concurrent-worker panic and preventing a rejected
start from modifying another lifetime's deadline. No caller factory or new SDK
port is introduced. Cache health read-only and deadline policies are unchanged.

The new C1 public-core test uses SystemSchedules and its retained ordinary table
under a real external BEGIN IMMEDIATE writer. A separately bounded view proves
the actual management operation owns the shared mutex, then is canceled and
joined. The old production implementation fails the new 500ms stop observation
(500.76ms) and completes only after explicit rescue rollback; management and cron
are both actually joined before the failure assertion. This finite negative is
recorded in /tmp/phase-c-execution/c-1/qa-7/startup-old-negative.log. The fixed
regressions cover ordinary pre-stopped startup, zero timeout, a second signal
after startup-read contention, rejected concurrent startup, read completion
within grace, and reuse of the SAME object after joined startup cancellation.
Every complete case uses the qualified owned fixture supervisor and retains the
500ms/100ms timing assertions and real management/worker joins. Existing
post-start and FTQ-009/010 regressions remain acceptance targets.

Focused final-source validation and receipts live under
/tmp/phase-c-execution/c-1/qa-7/startup-*. This C1 task qualifies the public core;
the exact public Application probe must be rerun by C2 after consuming the
correction. No candidate Application/HTTP result is claimed from old C2 source.
FTQ-010's actual QA7 owning-reviewer/enforcer closure and FTQ-009's closure are
preserved. RSH-001 remains OPEN, implemented pending independent QA8 and C2
consumption. REQ-QA-004, ARCH-001, RBQA-F001 and QA2-001 remain OPEN for the genuine
published canonical receiver/isolated graph and required actual-head CI gates.
No dependency upgrade, publisher work, graph waiver, timing widening, detached
blocking work, sprint/anchor/merge PASS or self-closure is introduced.


### QA8 inventory budget correction (FTQ-011)

QA8's exact report is
https://github.com/randlee/beads-cache/pull/108#issuecomment-6622741614.
The retained developer lifecycle run failed 14/15 and the independent Rust QA8
run failed 13/15 with owned inventory timeouts. Both original logs remain under
`.cursor/qa-evidence/phase-c/c-1/qa-8/logs/`; the focused later pass does not erase
either failure. They establish the actual helper timing dependency, without
establishing a particular slow invocation, PID or host-load cause.

The test-only inventory now consumes its caller's remaining absolute budget;
there is no separate 50ms cap or minimum 50ms slice. Preflight retains its 250ms
allowance. Diagnostic/readiness callers supply their existing absolute observation
or fixture deadline, capped by the five-second fixture lifetime. Cleanup retains
the same initial 100ms drain, absolute 250ms quiescence boundary and absolute
one-second final boundary. No phase or fixture timeout is renewed after error.
Every started inventory owns its child and output file and completes or is
killed and actually reaped, including output/parse errors and assertion unwind.
An empty or incomplete inventory is never evidence of session emptiness.

A transient scan error may recover only with fresh complete evidence before the
same phase deadline. Failed scans reset the stable-census observation. Cleanup
requires two complete stable stopped-session censuses, the actual root wait,
and a fresh complete empty-session census after that join, including the root.
Exhausting any required proof remains an error even if a later diagnostic sees
an empty session. The preliminary drain may recover through those required
proofs; recovery is logged while the original fixture failure/timeout is retained.
The one-attempt handoff to the shared supervisor remains: error, Drop and a
second cleanup call cannot start a fresh registry or session cleanup budget.
Pre-registration ownership, in-flight launch closure, Linux raw nonpositive SID
filtering and identity rechecks are preserved.

Deterministic added regressions self-stop and acknowledge an actual owned
inventory command, then resume it after 80ms under the existing preflight and
cleanup budgets. Another injects a transient census error while retaining a real
failed fixture exit. A genuine noncooperative command consumes the existing
cleanup phases, is killed/reaped each time, and leaves the failed receipt false
after root reaping, a repeated cleanup call, Drop and a separate diagnostic audit.
The existing true timeout/unwind inventory proof now acknowledges the command's
kernel stop before testing it. These are test inputs, not host tuning or new
process-control promises.

| Finding | Current developer disposition |
|---|---|
| FTQ-011 | OPEN: correction validated locally; final pushed-source proof and independent QA9 required. Original failures retained. |
| RSH-001 | OPEN: C1 startup production remains exactly adbcc88d3dc134ae94403bc2af7a8db76ba921a5; C2 actual public Application candidate qualification is a separate parent-owned gate. |
| REQ-QA-004 | OPEN: exact candidate CI, including each supported platform and the actual dependency gate, remains required. |
| ARCH-001 | OPEN: genuine published public receiver composition remains blocked; no new interface research or workaround is included. |
| RBQA-F001 | OPEN: direct sync-to-sc-command dependency remains independently tracked and unwaived. |
| QA2-001 | OPEN: strict isolated dependency graph remains unchanged and failing until compatible composition exists. |

The C1 branch merged current integration metadata before editing. Production
cron.rs and both sync/store schedule.rs files remain byte-identical to adbcc88d;
SDK dependencies, logging, consumer graph checks, shared store supervisor and
startup/store cancellation mechanisms are unchanged. The initial phase used formatting and source inspection only while C2 owned the
sole Cargo slot. After explicit release/grant, native inventory regressions pass
5/5, the complete affected lifecycle suite passes 18/18 (the prior 15 plus three
new cases), default store cleanup passes 5/5, both scoped strict Clippy modes
pass, and formatting passes. Restoring the old 50ms cap makes the deterministic
80ms inventory regression fail with TimedOut; canonical source was restored.
The initial sandbox inventory execution's explicit EPERM failures remain
recorded separately from authorized native qualification. The mutant assessment
wrapper initially checked the wrong timeout wording; its preserved actual exit
101/log establishes the intended negative without a repeated run.

Concrete logs are `/tmp/phase-c-execution/c-1/qa-8/helper-*.log` and source freeze
is recorded in `helper-edit-progress.json`. Original source-free counterexamples,
SID audits and final pushed-source evidence follow the push. Per-platform CI
remains separate from these local Darwin checks, following memory rule
b-mr7cp6x0-ipdnhs. No QA, sprint, anchor, CI or merge acceptance is claimed by this
developer receipt.


## QA9 Windows shared-store diagnostic RQA9-001

QA9 published FAIL (3 Blocking, 2 Important) for C1 `a753d279` and paired C2
`b7de8e98`; the actual C2 Windows job 126397662727 failed the shared-store
busy-persistence negative fixture without the post-call/stall receipt. Both
schedule source `d30f281` and fixture helper `d312b56` match across those heads.
The retained raw Windows log remains under the QA9 evidence archive. Its absent
receipt does not identify which child phase exhausted the original five seconds
or prove that the operation never began or returned.

This diagnostic adds elapsed receipts for setup/open, the actual worker call and
wait result, cancellation installation, completion timeout, rollback and actual
join. The owned supervisor emits its captured child diagnostics on success as
well as failure; the missing-receipt failure message states the observed limit.
All original real-operation and timeout assertions, 30 ms cancellation, 500 ms
observation, five-second absolute fixture lifetime and kill/reap ownership remain.
No production source, dependency, guard or startup behavior changes. Initial
strict affected lint rejected the expanded test's line count; the same existing
session-settings assertions were extracted and strict lint then passed. Native
Darwin affected store tests pass 5/5; this does not establish the Windows cause.

RQA9-001 remains OPEN: this is a diagnostic push, without a supported cause or
correction claim. REQ-QA-004, ARCH-001, RBQA-F001 and QA2-001 remain OPEN for the
unchanged published logging seam, strict graph and actual CI gates. Exact-source
hosted Windows observations and independent QA10 remain required. Candidate-
scoped QA9 RSH-001/FTQ-011 evidence and all earlier triage history were preserved
while merging exact integration `63ebc1fa`. No sprint acceptance or merge is claimed.


## QA9 private phase protocol and planted classification control

Exact integration `b731c09e` is consumed by ordinary feature merge, preserving
both RQA9 diagnostic/owner histories and prior candidate-scoped closures. The
independent causal owner keeps the original Windows failure OPEN: neither the
selected nor full-store successful observation identifies its historical cause.

The private store-test protocol now writes small structured checkpoints to
separate main/worker files under the existing owned scratch directory, independent
of captured stderr and without a shared logging mutex. It records setup, actual
wait result, installed cancellation deadline, real call result/SQLite code before
any result/stall text, original marker-write completion, completion timeout and
actual join attempt. All child phases use the fixture-body monotonic origin.
Parent receipt timestamps use the existing owner Instant captured before spawn;
spawn-return and parent setup offsets are reported. Receipt times indicate
observation, not exact child execution. Separately labeled wall-clock correlation
may move and is never used for timing decisions. A final partial/malformed record
is a failure; silence never proves operation non-entry.

The actual busy-persistence planted control parks only after the real call and
its independent return checkpoint complete, before the required stderr marker.
The unchanged normal negative qualification fails that occurrence. The outer
regression passes only after catching that failed qualification, verifying the
real DatabaseBusy return, missing marker, original timeout and completed owned
root reap. An explicit direct negative command can retain libtest exit101.
The original normal three-family qualification and normal/restoration/unwind
coverage remain. There is no early-stop or scenario-selection change.

The shared test helper's observer is optional and inline; existing callers use a
no-op observer. Original absolute five-second lifetime, 30ms cancellation, 500ms
observation and seven-second supervisor assertion remain. Observer error/unwind
still drops the owned root through actual kill/wait. Cleanup can be attempted
once: an incomplete cleanup receipt remains false and Drop cannot create a retry
or fresh cleanup budget. Existing session-fixture ownership handoff is retained.
New owned controls check observer error/panic and invalid receipt/root-reap paths.

Only the schedule test module and private test helpers change. The production
prefix of schedule.rs is checked separately; it is inaccurate to call that whole
Rust file unchanged. Production store/cron/sync policy, accepted startup and
FTQ009/010/011 mechanisms, public interfaces, dependencies and strict guards stay
as reviewed. This is diagnostic/classification work, not an original-cause fix.
All five canonical findings remain OPEN; QA10 and actual platform/current CI,
sprint/anchor and merge gates are not waived. Final exact-source validation and
push receipts follow in the QA9 phase-protocol result artifacts.


## QA10 inventory stage diagnostics

QA10's actual macOS cron_hurry17/18 failure remains a valid unlocalized inventory
timeout. The recovered initial-drain notice does not identify whether the later
failure was in quiescence, final drain or independent audit. Exact integration
`c0b251da` is consumed by ordinary merge, preserving all owning histories and
previous candidate-scoped dispositions. All six current findings stay OPEN.

The session helper now reports each cleanup stage, call index, original absolute
deadline offset, remaining time and actual inventory result. Inventory subprocess
PID, owner thread, elapsed time and actual exit or timeout kill/wait are recorded.
The thread tag links each command with its synchronous stage callback; it is not
a new execution controller. Root wait, final complete census/cleanup receipt and
independent post-cleanup audit outcomes are separate observations. The existing
transient first scan and third-scan stopped-command80ms injection retain their
exact order and behavior; diagnostics record actual stop acknowledgement and
remaining time before and after that original delay.

An owned controlled counterexample establishes the old generic timeout's ambiguity:
a held quiescence command fails cleanup, with root actually reaped and receipt
false; a held post-audit command fails only after completed root cleanup, with its
separate cleanup receipt still true. In both cases the held inventory is killed
and waited, the original fixture remains failed, and complete independent session
verification cannot rewrite the intentionally failed observation. Failed cleanup
cannot acquire a second attempt or fresh deadline. These are stage-classification
controls, not reproduction or correction of the natural macOS failure.

Authorized local inventory controls pass6/6. The original EPERM sandbox attempt
is retained separately: real process inventory was denied, not evidence about the
hosted timeout. A normal local recovery observed scan3 in quiescence with214.069ms
left after stop acknowledgement,81.886ms actual delay and132.183ms left before
CONT; that observation does not establish the original failure stage or cause.

Initial100ms drain, absolute250ms quiescence,1s cleanup, independent250ms audit,
whole-child5s and one-attempt ownership remain unchanged. No fault or audit was
removed or moved, no new per-scan cap, retry-to-green, production correction,
public artifact, SDK/pin/guard change or acceptance waiver is introduced. This is
diagnostic-only delivery pending exact-source hosted stage evidence and owner
assessment. Store phase protocol and reviewed production sources remain unchanged.
Final source, targeted post-push proof and macOS handle are recorded in QA10's
inventory-stage result artifacts; no overall CI, sprint, anchor or merge claim.
