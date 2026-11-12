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

The existing bdc and Python surfaces use the approved upstream sc-cron source over the same
cache file. No local scheduler remains to drift. This sprint closes scheduler
integration independently of the later generated application.


## Deliverables

1. REQ-BDC-1002: land the already-tested rusqlite 0.39 alignment; move sc-command
   and sc-cron to the published exact registry 0.2.0 baseline under ADR-BDC-0016,
   with default features off. Resolve
   one libsqlite3-sys. Retire the local sc-cron workspace member and source/tests;
   retain historical design docs with an upstream ownership note. Remove local
   sc-cron from existing CI publish dry-runs. Do not update unrelated dependencies.
   After explicit approval, apply the narrowly scoped REQ-CLI-0001 amendment in
   docs/bd-cache-cli/requirements.md from the old git tag to the published exact
   registry 0.2.0 baseline. Keep its three-OS build, version envelope and usage/exit guarantees.
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
approved immutable dependency source; do not restate it in a local trait copy.

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

## Adapter implementation notes

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


## QA1 correction delivery and remaining public API blockers


| Finding | Correction / remaining disposition |
|---|---|
| REQ-QA-001 | TrustedInterpreter enforces the inherited 256-byte module bound for both sync modules and post-sync hooks; schedule_switch.rs proves 256 accepted, 257 refused with both prior rows unchanged. |
| REQ-QA-005 | Existing perf.rs now qualifies real migrated scheduled incremental sync over the existing 1000-bead fake-Bd composition, asserts an incremental cursor and changed cached row. Release qualification measured 6.184458 ms, below 1 s. No live bd ran. |
| RBQA-F002 | Existing isolated graph gate now traverses root-reachable normal/build edges and permits sc-command introducers only from published sc-cron 0.1.1. Explicit axum/rmcp exclusions added. Focused planted negatives cover direct sync introducer, local source, server feature, HTTP/MCP and ignored dev-only edges. Existing candidate leak is detected, not suppressed. |
| RBQA-F003 | ModuleHook is exported only under testing; invoke is crate-private and no normal consumer surface is added. |
| C1-RBP-F001 | Local unrunnable constructor returns Arc<dyn Job> directly; only genuinely fallible resolver branches retain Result. |
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


### QA2 narrow correction receipt (2026-11-02)


| Finding | QA2 correction / remaining disposition |
|---|---|
| FTQ-001 | Correction present, specialist verification pending. health.rs runs the existing causal mutex fixture in an owned child test process. The parent enforces a five-second lifetime, kills and reaps on expiry, and its owner Drop also kills/reaps during unwind. File-backed diagnostics avoid pipe-drain threads; parent-owned scratch storage removes a killed child's cache. The original holder/read-start handshakes, 50 ms read budget, 200 ms answer deadline, <150 ms assertion and post-release store.status check remain. A deliberately stalled reader reaches the contended-read failure path and proves timeout kill/reap, rather than allowing a detached-thread success or health retry. |


### Consumer restart correction: bounded cache scheduler shutdown


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


### QA4 test-lifetime correction (FTQ-009 / FTQ-010)


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
families; the lifecycle one also verifies real Python reaping.


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



### QA6 owned inventory and Linux identity correction (FTQ-010 / REQ-QA-004)

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


### QA8 inventory budget correction (FTQ-011)

QA8's exact report is
https://github.com/randlee/beads-cache/pull/108#issuecomment-6622741614.
The retained developer lifecycle run failed 14/15 and the independent Rust QA8
run failed 13/15 with owned inventory timeouts. Both original logs were committed
under `.cursor/qa-evidence/phase-c/c-1/qa-8/logs/`, which PR #118 removed from the tree;
they remain readable at `d6022fe55`. The focused later pass does not erase
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





## QA9 private phase protocol and planted classification control


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



## QA10 inventory stage diagnostics


The session helper now reports each cleanup stage, call index, original absolute
deadline offset, remaining time and actual inventory result. Inventory subprocess
PID, owner thread, elapsed time and actual exit or timeout kill/wait are recorded.
The thread tag links each command with its synchronous stage callback; it is not
a new execution controller. Root wait, final complete census/cleanup receipt and
independent post-cleanup audit outcomes are separate observations. The existing
transient first scan and third-scan stopped-command80ms injection retain their
exact order and behavior; diagnostics record actual stop acknowledgement and
remaining time before and after that original delay.



## QA10 recovery fixture composition correction

The retained one-shot macOS observation at literal C2 source
`7fb224623280dae9cb056915484cb698ddf6bdf0`, run41796715401/job126462431901,
failed the recovery test18/19. Its ec68 helper receipts localize that occurrence
to quiescence call3: the deliberately STOPped inventory was delayed80ms before
CONT inside the same absolute250ms window requiring two complete stable censuses.
The command timed out and was actually killed/waited; root exit101 and a complete
final empty-session drain were retained, while cleanup correctly stayed failed.
The independent audit was not reached. Lower-level signal/ps/observer timing and
all earlier uninstrumented failure stages remain unproven. The independent owner
supported separating these test inputs; this delivery does not claim a production
cancellation or kernel fix.

| Required proof | Earlier composition | Corrected independent coverage |
|---|---|---|
| Transient census recovery | First scan error plus extra stopped-command delay on scan3 | Same first error, followed by real full inventory, two fresh stable quiet censuses, actual root wait and final empty-session census; at least5 scans required |
| Original failure preserved | Deliberately failed fixture must remain failed | Same failed status and child panic output assertions; cleanup success cannot convert its result to pass |
| Independent final verification | Separate250ms audit and root absence | Same complete audit and root absence assertions; any audit error still fails the test |
| Owned delayed inventory exceeds former50ms cap | Real STOP/80ms/CONT in recovery and separate existing preflight test | Existing independent250ms preflight regression retains actual STOP acknowledgement, requested80ms, verifies measured stopped-delay>=80ms, successful CONT, complete parsed inventory, actual wait/reap and PID absence; no redundant delay in recovery |
| Incomplete quiescence fails closed | Held real command consumes original quiescence deadline | Unchanged stage control: false cleanup receipt, original root failure, command/root reap, complete independent session audit, refused second attempt |
| Independent audit timeout stays an error | Held real command consumes independent250ms audit | Unchanged stage control: audit fails while prior cleanup receipt stays true; command/root reap and complete independent later verification cannot waive the failed audit |
| Noncooperation, one attempt and unwind | Complete owned cleanup under original budgets | Unchanged incomplete-inventory and stopped-inventory timeout/unwind tests, plus pre-registration and fixture/Python assertion-unwind lifecycle proofs |

Only the two private test bodies change. SessionFixture, inventory execution,
quiescence, final-drain, observer protocol and cleanup/Drop implementations remain
unchanged. All numerical budgets remain: preflight250ms, initial drain100ms,
absolute quiescence250ms, final cleanup1s, independent audit250ms and fixture5s.
No error becomes successful recovery, no census/audit is removed, and no retry,
minimum success-margin reservation, fresh budget or host/thread setting is added.
Any required real inventory exceeding its deadline still fails honestly.



## QA11 native fixture qualification correction

QA11 remains FAIL29/34 with six open findings. Its actual Windows trace puts
busy-fixture setup near4.785s and worker entry near4.975s against the complete
five-second owner; cancellation/required return qualification then ran out of
that lifetime. The disposable test store now uses a cfg(test)-only constructor
with synchronous=OFF before the SAME complete initialization/identity transaction,
canonical file claim, busy policy and WAL configuration. It still exercises real
public schedule operations, external writer contention and actual running SQL;
no schema substitute or production durability change is introduced. The shared
management/error/unwind/health test uses this fixture too and remains required.
The observed trace does not isolate fsync as the exact historical cost or promise
a host scheduling margin. All setup remains within the original pre-spawn5s
owner, with30ms cancellation,500ms observation and all required negative markers.

QA11's distinct macOS delayed-inventory positive observed command exit0/wait after
its250ms output deadline. That isolated regression now owns spawn and actual STOP
readiness in the existing SessionFixture5s exact-child supervisor, then creates
ONE250ms caller deadline before process_ids_using. The real>=80ms hold,CONT,
complete real inventory parsing/self-presence and wait/reap/PID-absence all remain
inside that caller qualification. This explicitly changes isolated test setup
scope; no supplied deadline is renewed. Ordinary preflight/cleanup/audit callers
still include their command spawn and complete parsing under the same original
absolute deadlines. All timeout, assertion-unwind, incomplete-receipt/no-retry
and root/census controls remain; arbitrary timeout cannot pass the positive.
The historical50ms cap placed BEFORE injection must still fail this case.

RQA10 and CI-C2-RUNNER-001 retain QA11's independently resolved exact-C1 scope;
prior FTQ/RSH credits and all original failures remain. RQA9/RQA11 corrections
require independent QA12/owner adjudication. ARCH-001, QA2-001, RBQA-F001 and
REQ-QA-004 retain their genuine public receiver/graph/current-acceptance blockers.
No SDK/pin/guard/publisher workaround or whole-sprint acceptance is claimed.
Ordinary developer source/test evidence is in QA11's native-qualification result;
no separate coverage-ledger artifact or standing process gate is required.


## Public logging facade


The real receiver now implements the public
`sc_cron::observability::CanonicalLog` and uses its shared/event/service/error
exports. The direct sync-to-sc-command dependency is removed. Canonical event
projection, ordering, redaction, error/outcome fields and best-effort emission
remain unchanged. The fixed validated service identity remains bd-cache; no
no-op adapter, type copy, dependency relocation or altered public facade is used.
All prior fixture/startup/runner corrections remain in source.



## Historical qualified C5 prerelease pin and provenance correction

The user's conditional authorization was: “ok, sc-runtime team has a ci failure
on pr115. once this is resolved, you can pin that release to use before the
0.2.0 release is finalized.” The parent relayed this verbatim and interpreted
the producer-qualified PR118 merge candidate as fulfilling that condition.
ADR-BDC-0016's qualified prerelease source amendment records this interpretation,
including the distinction between green PR checks and literal merge-head CI.

Before registry publication, C1 pinned sc-cron/sc-command package version 0.2.0 to exactly
`c3ce3d1b271b54fbb47efaef57ee1563a390cd9e` at the approved upstream URL. Only those
two lockfile records change; observability, rusqlite and the single SQLite
provider remain unchanged. No production Rust edit is needed: the genuine public
cron receiver, event projection/redaction, service identity, public facades and
all previously reviewed startup/shutdown/fixture behavior are retained.

The temporary isolated-consumer gate permitted precisely published crates.io
0.1.1 or that URL/revision/resolved-commit/version tuple. It rejects wrong rev,
resolved commit, URL, moving source, local path, version, mixed nominal identities,
forbidden introducers and SDK features/assembly. The existing independent API and
Python consumer commands must actually pass; a fixture-only pass is insufficient.

This temporary narrow source amendment superseded the earlier published-only
source predicates retained in Git and earlier reports. Publication now retires
that exception; current deliverables and ADR/REQ clauses use registry 0.2.0.
Prior failure logs and development-pin provenance remain historical evidence.
Parent anchor review must adjudicate the exact
interpretation: published-only source becomes published baseline OR this single
qualified prerelease Git candidate; domain semantics and all locked ANC bounds
remain unchanged. No anchor is silently waived or rewritten.

QA14's actual owner/enforcer reviews at `60f89200fe337d57ede24ecaa55e36171a1c4305`
resolve ARCH13-001 and RBQA-F001, preserve earlier scoped credits and record
15/15 CI checks. REQ-QA-004 remains OPEN for independent QA15, applicable
anchors and new-head CI. The existing lifecycle suite now verifies the public
cached-read route while an acknowledged scheduled Python sync remains active,
with no additional BdCli argv request and a real cold-read counterexample.
The parent-maintained REQ-BDC-0007 version-3 deterministic-check metadata is
included unchanged; no anchor statement, scope or question changes. Ordinary
handoff evidence does not claim sprint/anchor/CI acceptance or merge approval.


The first qualified-pin store run retained a 36/37 failure: the management-health
regression inspected nested Debug for “read-only”, while the candidate SDK now
redacts StoreError context in Debug. The minimal SDK error-API adoption changes
only that assertion to traverse the real Error source chain and require the
exact typed BD_CACHE.STORE_FAILED read-only refusal. It retains every operation,
restoration/unwind assertion and deadline. This does not weaken health policy or
alter production source. The original raw failed run remains in the handoff.


## Current published registry baseline

C1 consumes sc-cron/sc-command at exact crates.io `=0.2.0`, default features off.
The temporary qualified C5 Git source allowance is retired. The real public
`sc_cron::observability` receiver and all store/startup/shutdown/fixture corrections
remain; SDK 0.2.0 does not change bd-cache API 0.1 compatibility. sc-observability
1.5, rusqlite 0.39 and one SQLite provider remain the dependency policy. The
isolated graph gate permits only this registry version/source pair and retains
all introducer, feature and assembly restrictions. Current source qualification
and provenance receipts live in `/tmp/phase-c-execution/c-1/registry020-adoption`;
original failures and pin evidence remain historical. Independent QA15, full-input
version-3 development-anchor review and exact-head CI remain required; publication
alone does not establish upstream full consumer acceptance or sprint completion.
