---
phase: c
status: planned
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
adrs:
- ADR-BDC-0005
- ADR-BDC-0006
- ADR-BDC-0010
- ADR-BDC-0011
- ADR-BDC-0014
- ADR-BDC-0015
- ADR-BDC-0016 (proposed)
- ADR-STORE-0001
- ADR-STORE-0003
- ADR-STORE-0006
- ADR-SYNC-0008
- ADR-CLI-0001
- ADR-CLI-0005
- ADR-PY-0006
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
- docs/sc-cron/architecture.md
- docs/bd-cache-api/requirements.md
---

# Sprint c-1 — Published runtime scheduler in the existing cache composition

## Outcome

The existing bdc and Python surfaces use upstream sc-cron 0.1.1 over the same
cache file. No local scheduler remains to drift. This sprint closes scheduler
integration independently of the later generated application.

## Deliverables

1. REQ-BDC-1002: land the already-tested rusqlite 0.39 alignment; move sc-command
   and sc-cron to the published 0.1.1 baseline with default features off. Resolve
   one libsqlite3-sys. Retire the local sc-cron workspace member and source/tests;
   retain historical design docs with an upstream ownership note. Remove local
   sc-cron from existing CI publish dry-runs. Do not update unrelated dependencies.
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
   gate changes. Preserve public execution signatures shown below.

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
| req:REQ-API-0008 | contract-only sc-command path is permitted, clap/server/runtime leakage remains rejected in isolated consumer builds | enable sc-command/server in the isolated Python consumer; graph test fails |

## Required validation

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
