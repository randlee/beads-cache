---
phase: c
status: complete
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
| ANC-003, replacement and rollback | sync/job.rs, sync/module_hook.rs; existing sync/tests/schedule_switch.rs, cron_rows.rs and py/tests/test_sync.py |
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
