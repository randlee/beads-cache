---
id: b-8
phase: b
status: planned
closure_type: integration
target_boundary: bd-cache-sync, bd-cache-cli, bd-cache-py, bd-cache-api (the new verbs), sc-cron (general features)
track: core
wave: 6
branch: sprint/b-8-tick-python
worktree: sprint/b-8-tick-python
recommended_agent: rust-developer
recommended_model: opus
must_follow: [b-7]
parallel_safe: []
requirements:
  owns:
    - REQ-BDC-0720
    - REQ-BDC-0722
    - REQ-BDC-0723
    - REQ-BDC-0724
    - REQ-BDC-0725
    - REQ-BDC-0727
    - REQ-BDC-0730
    - REQ-BDC-0731
    - "REQ-BDC-0703 (amended)"
    - "REQ-BDC-0803 (amended)"
    - "REQ-BDC-0810 (amended)"
    - "REQ-BDC-0701, 0702, 0704..0707 (superseded; their code removed)"
    - "REQ-SYNC-0001, 0004, 0010, 0012 (amended)"
    - "REQ-SYNC-0002, 0003, 0006..0009, 0011 (superseded; their code removed)"
    - "REQ-CRON-0005 (amended)"
    - REQ-CRON-0016
    - REQ-CRON-0018
    - "REQ-MODEL-0005 (amended)"
    - "REQ-CLI-0006, REQ-CLI-0008 (amended)"
    - "REQ-PY-0002, 0005, 0008 (amended)"
    - "REQ-API-0001, REQ-API-0005 (amended)"
  constrained_by:
    - REQ-BDC-0001
    - REQ-BDC-0006
    - REQ-BDC-0007
    - REQ-BDC-0008
    - REQ-BDC-0401
    - REQ-BDC-0402
    - REQ-BDC-0502
    - REQ-BDC-0721
    - REQ-BDC-0726
    - REQ-BDC-0728
    - REQ-BDC-0729
    - REQ-BDC-0801
    - REQ-BDC-0802
    - REQ-BDC-0807
    - REQ-BDC-0812
    - REQ-BDC-0910
    - NFR-BDC-0002
    - NFR-BDC-0003
    - NFR-BDC-0004
    - NFR-BDC-0005
    - NFR-BDC-0007
    - "REQ-SYNC-0005, 0013"
    - "REQ-CRON-0001..0004, 0006..0015, 0017"
    - "REQ-API-0002..0004, 0006..0008"
    - "REQ-CLI-0001..0005, 0007, 0009..0012"
    - "REQ-PY-0001, 0003, 0004, 0006, 0007, 0010..0014"
    - "REQ-MODEL-0001..0004"
adrs:
  - ADR-BDC-0004
  - ADR-BDC-0005
  - ADR-BDC-0006
  - ADR-BDC-0009
  - ADR-BDC-0010
  - ADR-BDC-0011
  - ADR-BDC-0012
  - ADR-BDC-0013
  - ADR-BDC-0014
  - ADR-BDC-0015
  - "ADR-SYNC-0001..0008"
  - "ADR-CRON-0001..0009"
  - "ADR-PY-0001..0006"
  - "ADR-CLI-0001..0005"
  - "ADR-API-0001..0003"
  - "ADR-STORE-0001, 0003, 0006"
  - "ADR-MODEL-0004"
owned_paths:
  - "crates/bd-cache-sync/**"
  - "crates/bd-cache-model/src/{sender.rs,config.rs,lib.rs} and crates/bd-cache-model/tests/config.rs (the removals)"
  - "crates/sc-cron/src/{scheduler.rs,worker.rs,hook.rs,process.rs,types.rs,lib.rs}, crates/sc-cron/tests/{worker.rs,python_hook.rs,stop_hook.rs,two_process.rs,reload_poll.rs,common/**} and new sc-cron test files"
  - "crates/bd-cache-api/src/{request/**,data/**,error.rs,lib.rs}, the new crates/bd-cache-api/src/ops/scopes.rs, the scopes reply member in crates/bd-cache-api/src/ops/manage.rs, and crates/bd-cache-api/tests/** for the new verbs"
  - "crates/bd-cache-cli/src/{lib.rs,args.rs,config.rs,request.rs,scenario.rs,log.rs,help.rs,render/**,bin/**}, crates/bd-cache-cli/src/commands/{mod.rs,tick.rs,signal.rs} and the new commands/{schedule,cron,sync,scopes}.rs"
  - "crates/bd-cache-cli/tests/{cli.rs,process.rs,scenarios.rs,fixtures.rs,harness.rs,fixtures/**,support/**} and new test files for the new verbs"
  - "crates/bd-cache-py/src/**, crates/bd-cache-py/{README.md,pyproject.toml,Cargo.toml}, the new crates/bd-cache-py/python/**"
  - "the new pytest files crates/bd-cache-py/tests/{test_tick.py,test_schedule.py,test_switch.py,test_run.py,test_sync_harness.py,test_ingest.py,test_scopes.py}"
  - "docs/architecture.md (the bd-cache-sync, bd-cache-py, bd-cache-cli, sc-cron and bd-cache-model rows of the Crate Layout and Boundaries tables, and the ADR implementation table)"
  - "docs/bd-cache-sync/**, docs/sc-cron/**, docs/bd-cache-api/**, docs/bd-cache-cli/{architecture.md,requirements.md} (implementation notes)"
---

# Sprint b-8 — the sync rows, the switch, the Python surface and end to end (WP-6b, WP-9)

## Outcome

beads-cache syncs bd itself, on a row of the sc-cron table in the cache file. A consumer can
switch to its own Python sync row with one command, which runs the script at once and rolls back
on failure, and the built-in sync gives a post-sync hook every result and error; the cache sends
nothing (ADR-BDC-0010). One sync runs at a time, `cron run` stops cleanly on a signal, and sc-cron
gains only general features (ADR-CRON-0009). The design is ADR-SYNC-0008, ADR-BDC-0015,
ADR-PY-0006 and ADR-CRON-0009, with REQ-BDC-0720..0731.

## Deliverables

1. **Schedule** (REQ-BDC-0720, REQ-BDC-0731, ADR-SYNC-0008). `bdc cache schedule set <cron-expr>
   [--budget-ms N] [--hook-reserve-ms N] [--post-sync <module:function> --python <absolute
   interpreter> [--env NAME]...]`, `show`, `enable`, `disable` and `clear` manage the built-in
   row in `CacheScheduleStore` (b-7), with REQ-BDC-0720's defaults and ranges and the row's
   sc-cron timeout `budget_ms + 10000`. At most one built-in row per repository (ANC-008);
   `enable` and `disable` flip only its `enabled` flag (REQ-CRON-0014); `enable` while the
   Python row is enabled is `SCHEDULE_INVALID`. `schedule set` validates everything before it
   writes: the interpreter (`TrustedInterpreter`: absolute, a regular executable, then `-I -c
   'import bd_cache_rs'` within 5 s), the `module:function`, and `hook_reserve_ms` of at least
   1000 when a hook is named. `schedule show` reports everything REQ-BDC-0731 lists.
2. **Entry points and shutdown** (REQ-BDC-0720, REQ-BDC-0730, S90). `bdc cache cron run` (the
   sc-cron worker with bd-cache-sync's resolver, `MissedTickPolicy::Skip`, on a multi-thread
   tokio runtime; `bd-cache-cli` gains `tokio` with `rt-multi-thread` and `signal` in its own
   `Cargo.toml`), `cron run --once`, `bdc cache tick` and PyO3 `Cache.tick()` run the built-in
   row's code; `Cache.tick()` releases the GIL and owns its runtime as ADR-PY-0006 says. Scheduled
   times come only from the rows (ANC-002). `cron run` handles `SIGINT` / `SIGTERM` (Ctrl-C and
   Ctrl-Break on Windows) with `--shutdown-timeout-ms` (default 10000): no new run, runs in flight
   awaited to the timeout, those still going killed with their process trees, the lease released
   on every exit path, and the exit codes of REQ-BDC-0730; a `kill -9` is covered only by lease
   expiry, as documented. `cron run --once` waits for the runs it started and exits 0, or 3 with
   `TICK_RUN_FAILED`. `cron run` installs its stderr log as the process-wide subscriber and logs
   one event per run step with the row, `run_id` and outcome. Every new command (`schedule`,
   `cron`, `tick`, `sync`, `scopes`) is on the `bdc cache` cache-management surface; none is
   added to the bd /v0-shaped beads surface (ADR-BDC-0009, ADR-BDC-0015).
3. **The built-in run** (REQ-BDC-0703 amended, REQ-BDC-0722, REQ-BDC-0727). `TickLease` (the RAII
   guard over b-7's lease calls) and `Budget` (the sync and hook deadlines) as ADR-SYNC-0008 says;
   lease, drain, sync of every configured scope by its filter, post-sync hook and reply; the hook
   keeps `hook_reserve_ms`; no write transaction and no drain lock while the hook runs.
   - Rows are stored only through b-7's strict row decoder (REQ-BDC-0007), all or nothing per
     scope result: one invalid row stores no row from that result and fails the scope
     (REQ-BDC-0008); a stored row whose `updated_at` is newer is never replaced (REQ-BDC-0502).
   - When bd cannot be reached, a scope's result is `outage` (`details.outage: true`), its rows
     stay as they were, reads keep answering from the cache, and the hook is told (ADR-BDC-0005).
   - While the Python row is enabled, `tick` and `Cache.tick()` are `PRECONDITION` and start no
     bd process.
4. **The Python sync row and the switch** (REQ-BDC-0724, REQ-BDC-0725, REQ-BDC-0722, ANC-003).
   - `bdc cache sync use-python --python <absolute interpreter> --module <module> [--cron
     <expr>] [--timeout-ms N] [--env NAME]...` and `bdc cache sync use-builtin`, with the input
     checks, lease, one-transaction mode switch (sc-cron's `set_enabled_many`), immediate run,
     rollback, failure reasons, stderr cap and redaction, and the recovery rule of REQ-BDC-0724
     and ADR-SYNC-0008. A second Python sync row is refused.
   - `PythonSyncJob`: the Python row's target, resolved per launch with the trust check, running
     the script with sc-cron's protocol (`cron.stop`, then `cron.tick`) under the lease, with the
     payload of REQ-BDC-0725 (`cache_path`, `beads_dir`, `run_id`) and the environment allowlist.
   - With the Python row enabled, no `bd list` runs for the configured scopes; the script fills
     the cache through `ingest_scope`, with the same rules as the built-in sync (REQ-BDC-0007,
     0008, 0502), and its run is judged by its exit status. beads-cache itself reaches the backend
     only through bd on both paths and reads no Dolt table (REQ-BDC-0006).
   - `.beads/bd-cache.toml` cannot name code to run (`CONFIG_INVALID`).
5. **Post-sync hook** (REQ-BDC-0723, REQ-BDC-0727, REQ-BDC-0728). Run through sc-cron's exported
   supervisor (REQ-CRON-0018) as ADR-SYNC-0008 says: the command, the cleared environment with
   the allowlist and `--env` names, the private temp cwd, the 1 MiB caps and the 8 KiB stderr in
   the reply, the kill at its deadline (NFR-BDC-0004), the request of ADR-SYNC-0008 with the
   500-entry and 4 MiB caps, at-least-once delivery, the consecutive-failure count and backoff in
   `hook_state`, and the trust check before every launch (`TrustedCacheFile`; the Windows ACL
   check is a documented gap). The run itself marks nothing. bd-cache-sync adds no dependency
   (ADR-SYNC-0008 amends ADR-SYNC-0006) and drops `toml` and `libc`.
6. **sc-cron's general features** (REQ-CRON-0016, REQ-CRON-0018, REQ-CRON-0005 amended,
   ADR-CRON-0009). `TargetResolver` and `Scheduler::spawn_worker_with`, called once per launch,
   an error recorded `Failed` with the row left active; `spawn_worker_for` and `spawn_worker` kept.
   `sc_cron::process` public (`ChildSpec`, `ChildEnv`, `TempCwd`, `Captured`, `supervise`),
   `PythonHook::with_env`, `HookOutput::stop_decision` / `run_outcome`. `StopDecision::Stop`
   removed with its match arms. sc-cron's rustdoc cites only its own requirements; test fixtures
   use neutral names. sc-cron gets no beads-cache name, path, type, protocol or option.
7. **Python packaging and entry points** (ADR-PY-0006, REQ-BDC-0723, REQ-BDC-0725). The mixed
   layout: native module `bd_cache_rs._native` (`module-name` in `pyproject.toml`, `#[pymodule]
   fn _native`), `python/bd_cache_rs/__init__.py` re-exporting every name with the same
   `__all__`, `__version__` and `CONTRACT_VERSION`; `python -m bd_cache_rs.run post_sync
   <module:function>` (opens its own `Cache` on `BD_CACHE_PATH`, calls `func(cache, request)`,
   prints the result envelope); `bd_cache_rs.sync.main(func)` (the sync-script harness). `flush`,
   `drain`, `tick` and `schedule_*` raise `PRECONDITION` inside a run. The pyproject description
   loses its consumer wording.
8. **PyO3 surface** (ADR-PY-0006, REQ-PY-0002, 0005, 0008 amended, REQ-BDC-0726). `ingest_scope`,
   `scope_announce`, `scopes_*`, `schedule_*`, `sync_use_python` / `sync_use_builtin` and `tick()`
   on the handle's own connection (ADR-PY-0003), and the new codes in the exports. New scope
   arguments take scope names as written; the 0.1 verbs keep the bare-name rule (b-7). The 0.1
   verbs, arguments and dicts are unchanged, and `dir(Cache)` is still the 0.1 set. `bd-cache-py`
   gains the `bd-cache-sync` dependency, and its Boundaries and Crate Layout rows say so.
9. **The new verbs in `bd-cache-api` and the CLI** (ADR-BDC-0015, REQ-CLI-0006 amended,
   REQ-API-0001 and 0005 amended, REQ-BDC-0803 amended). The request, data and error types of
   `scopes add|set|remove|list|ingest|announce`, `schedule *`, `cron run`, `tick` and `sync
   use-python|use-builtin` in `bd-cache-api`, decoded by the one decoder; the store-only verbs
   executed there (`ops/scopes.rs`), the others in `bd-cache-cli` (`src/request.rs`, the new
   `commands/`) and `bd-cache-py`. `scopes ingest` reads stdin through the 64 MiB + 1 byte limit
   and counts rows as they decode (REQ-BDC-0726). `cache scopes` is `scopes list`'s alias, now
   with each scope's filter; `cache evict` is unchanged; `scopes remove` is `evict` plus the
   registry removal. `ApiError::tick_refusal`, its kind table and the `refused[]` items are
   removed. The help text of `cache tick` is rewritten.
10. **Removals** (ADR-BDC-0010, ADR-BDC-0012; REQ-BDC-0701, 0702, 0704..0707 and the REQ-SYNC ids
    above superseded; REQ-BDC-0810 amended). In `crates/bd-cache-sync/src`: the consumer
    messaging module, `phases.rs`, the consumer parts of `announce.rs` and `active.rs`, the stop
    rule and the `refused` / `pass-through` replies; the sender's process test and
    `tests/stop_rule.rs`, `tests/tick_phases.rs` rewritten for scopes, and the phase-file fixtures
    (the files there whose paths the lint flags). In `bd-cache-model`: `src/sender.rs` and the
    `[tick]` table's message-routing and phase-file keys with their users; `[tick.hooks]` and
    `[tick.sync]` are `CONFIG_INVALID`. In `bd-cache-cli`: the uses of the consumer's message sender and `TickReply` in
    `src/lib.rs`, `commands/tick.rs` and `commands/signal.rs` (the `StopFlag` stays, now for `cron
    run` and `tick`), the refusal of the first consumer's own directory in `src/config.rs` with
    its test in `tests/process.rs`, and the hard-coded scope prefix in `tests/scenarios.rs`. The
    stop hook itself, returning continue, disable or delete, is b-6's (REQ-CRON-0005, ANC-007)
    and stays.
11. **Architecture rows** (ADR-BDC-0011, ADR-BDC-0015, ADR-PY-0006, ADR-SYNC-0008). The Crate
    Layout and Boundaries rows of `bd-cache-sync` (no sender, no phase files, no `toml` or
    `libc`), `bd-cache-py` (`bd-cache-sync` edge), `bd-cache-cli` (`tokio`), `sc-cron` (the
    general features) and `bd-cache-model` (no sender port), and the ADR implementation table.

## Acceptance

Each item has a positive test and a planted negative (AC-N). Unit tests run with no bd and no
Dolt, against fake-bd and the injected clock (NFR-BDC-0005); every bd call, hook and script run
is killed at its timeout capped by the time left before the deadline (NFR-BDC-0004).

1. `grep -rn` of the removal list finds nothing; AC-0 on this fence.
2. Schedule: `schedule set/show/enable/disable/clear` round-trip and a second built-in row is
   refused; `SCHEDULE_INVALID` for each refusal of deliverable 1, each with a planted negative:
   a relative interpreter, a missing one, one that is not executable, one that cannot import
   `bd_cache_rs` (a fake interpreter that exits 1 on `-c`), a non-dotted module, a
   `hook_reserve_ms` under 1000 with a hook, and `schedule enable` while the Python row is
   enabled. `schedule set` and `cron run` on a cache file whose `sc_cron_schedule_version` row
   names another version fail with a typed `STORE_FAILED` before serving any work (ADR-BDC-0014).
3. Single flight: `cron run --once` runs a due row and skips one not due (sc-cron ManualClock);
   two real `bdc` processes, the second replies `skipped: in_flight`; a holder killed with
   `kill -9` is taken over after expiry; an on-demand `tick` during a worker run is skipped;
   `use-python` during a worker-launched Python run fails with `TICK_IN_FLIGHT`; two simultaneous
   `cron run --once` processes run the Python row once.
4. Switch: `use-python` leaves exactly the Python row enabled and runs it at once; its reply
   carries the run id, the exit status and stderr; `use-builtin` reverses it. Planted negatives
   that each fail the command with `TICK_RUN_FAILED` and leave the built-in row enabled: a script
   that exits 1, one that outlives `--timeout-ms` (injected clock), a missing interpreter, a
   script whose `cron.stop` answers `disable`, a `SIGTERM` to `use-python` mid-run (Unix), and a
   group-writable cache file. An injected failure of the restore transaction gives
   `details.restored: false` and `use-builtin` then restores the row. A crash injected between
   the mode commit and the run leaves exactly one row enabled, and `schedule show` flags the
   Python row as never run. Two rows enabled at once is never observed by a second connection
   during the switch. A second Python sync row is refused.
5. Python row through the worker: with the Python row enabled, `cron run --once` runs the script
   with `cron.stop` then `cron.tick`; a script written with `bd_cache_rs.sync.main` ingests one
   scope and the next read sees it; no `bd list` runs and rows come only from the script's
   `ingest_scope`; the payload carries `cache_path`, `beads_dir` and `run_id`; the script sees
   only the allowlisted environment; a group-writable cache file runs nothing (`cron run` and
   `cron run --once`); a script that answers `cron.stop` with no JSON is recorded `Failed`.
6. One table holds the built-in Rust row and the Python row, each run with its own target; an
   unresolvable row is recorded `Failed` and stays active (sc-cron's per-row resolver).
7. Post-sync hook runner: kill at the deadline, causal through the shared `gate_file` helper, on
   Unix process groups and in a Windows job-object test; stdout and stderr caps; a large request
   written without a deadlock; the environment is only the allowlist plus the `--env` names; the
   cwd is a temp dir; an untrusted cache file gives `TICK_RUN_FAILED` and runs nothing (Unix).
   An oversized request is capped at 500 escalations and 4 MiB with `escalated_truncated` and
   `escalated_remaining` set; an oversized stderr is kept to 8 KiB in the reply.
8. Budget and hook: when the sync uses everything, the hook still gets `hook_reserve_ms`; a hook
   that times out still lets the run finish by its deadline; the built-in row's sc-cron timeout is
   at least `budget_ms + 10000`, and an overrunning run is killed by sc-cron only after its lease
   release (injected clock). The hook request shape; marks made by the hook persist; a crash
   after marking keeps those marks and re-offers the rest with `first_offered_at` unchanged; each
   failure gives `TICK_RUN_FAILED`; three consecutive failures start the backoff, shown in
   `schedule show`, and a success clears it.
9. `cron run` shutdown: with a hook and, separately, a Python row whose script starts a
   grandchild that ignores `SIGTERM`, a `SIGTERM` to `cron run` stops new runs, waits up to
   `--shutdown-timeout-ms`, kills both processes of the tree, releases the lease (the next `tick`
   is not `in_flight`) and exits 143; `SIGINT` exits 130. A Windows test sends Ctrl-Break and
   sees the job object's children gone and exit 130. `cron run --once` with a failing row exits
   3 with `TICK_RUN_FAILED`.
10. Logging: an in-process `cron run -v` with a ManualClock shows sc-cron's `launched` and
    `recorded` events and one event per run step with the row and `run_id`.
11. A real-interpreter end-to-end test against fake-bd: `bdc cache tick` syncs a label scope and
    an `All` scope; the post-sync hook receives both results and marks one escalation and a
    failed scope; on the next run marked items are not offered again and the rest are, with
    `first_offered_at` unchanged. Then `use-python` with a script that ingests one scope and
    exits 0, and `cron run --once` running it again; and with one that exits 1, which is rolled
    back. A hostile `bd_cache_rs.py` or a shadowing module in the project dir is not imported.
12. Rows: one invalid row among valid ones, in a built-in result and in a Python `ingest_scope`,
    stores no row from that result and fails the scope; an older incoming row never replaces a
    newer stored one, on both paths. An unreachable bd gives the scope an `outage` result, cached
    reads still answer, and the hook receives it. `scopes ingest` of stdin over 64 MiB is
    refused with `INGEST_INVALID` before the input is held whole.
13. sc-cron stays general: `cargo tree -p sc-cron` shows no bd-cache or sc-beads crate, and
    `grep -ri 'bead\|bd_cache\|bd-cache' crates/sc-cron/src` finds nothing (ANC-005); the exported
    supervisor kills a grandchild at expiry and on drop through the public API.
14. The CLI: `cache scopes` and `scopes list` give the same reply; `cache evict` keeps its 0.1
    reply; `scopes remove` of a scope with pending entries is `PRUNE_BLOCKED`; the new codes have
    REQ-BDC-0729's exits; `the_legacy_cache_file_is_config_invalid` is gone and a repository with
    the consumer's legacy directory opens as any other.
15. `tests/job_threading.rs` `join_idle` is causal. pytest passes with
    `-W error::DeprecationWarning`, the existing `import bd_cache_rs` tests pass unchanged, and
    `Cache.tick()` called from an asyncio loop thread returns without nesting a runtime.
16. AC-0 on this fence, AC-Q.

## Reference

PR #93 (stopped b-7 WIP, built from the cut plan), #94 and #95, all REFERENCE ONLY; reuse what
matches this doc.

## Does not close

The contract text for these verbs, the neutral tests and docs, health, and the CI lint gate
(b-9).
