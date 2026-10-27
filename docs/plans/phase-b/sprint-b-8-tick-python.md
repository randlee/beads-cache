---
id: b-8
phase: b
status: complete
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
  owns: [REQ-BDC-0720, REQ-BDC-0722, REQ-BDC-0723, REQ-BDC-0724, REQ-BDC-0725, REQ-BDC-0727, REQ-BDC-0730, REQ-BDC-0731, "REQ-BDC-0703, 0803, 0810 (amended)", "REQ-BDC-0701, 0702, 0704..0707 (superseded; their code removed)", "REQ-SYNC-0001, 0004, 0010, 0012 (amended)", "REQ-SYNC-0002, 0003, 0006..0009, 0011 (superseded; their code removed)", "REQ-CRON-0005 (amended)", REQ-CRON-0016, REQ-CRON-0018, "REQ-MODEL-0005 (amended)", "REQ-CLI-0006, REQ-CLI-0008 (amended)", "REQ-PY-0002, 0005, 0008 (amended)", "REQ-API-0001, REQ-API-0005 (amended)"]
  not_binding: "REQ-PY-0009: superseded with the vendored call log, which b-9 removes"
  constrained_by: [REQ-BDC-0001, REQ-BDC-0006, REQ-BDC-0007, REQ-BDC-0008, REQ-BDC-0401, REQ-BDC-0402, REQ-BDC-0502, REQ-BDC-0602, REQ-BDC-0721, REQ-BDC-0726, REQ-BDC-0728, REQ-BDC-0729, REQ-BDC-0801, REQ-BDC-0802, REQ-BDC-0807, REQ-BDC-0808, REQ-BDC-0811, REQ-BDC-0812, REQ-BDC-0901, REQ-BDC-0909, REQ-BDC-0910, NFR-BDC-0001, NFR-BDC-0002, NFR-BDC-0003, NFR-BDC-0004, NFR-BDC-0005, NFR-BDC-0006, NFR-BDC-0007, "REQ-SYNC-0005, 0013", "REQ-CRON-0001..0004, 0006..0015, 0017", "REQ-API-0002..0004, 0006..0008", "REQ-CLI-0001..0005, 0007, 0009..0012", "REQ-PY-0001, 0003, 0004, 0006, 0007, 0010..0014", "REQ-MODEL-0001..0004"]
adrs: [ADR-BDC-0001, ADR-BDC-0002, ADR-BDC-0003, ADR-BDC-0004, ADR-BDC-0005, ADR-BDC-0006, ADR-BDC-0007, ADR-BDC-0009, ADR-BDC-0010, ADR-BDC-0011, ADR-BDC-0012, ADR-BDC-0013, ADR-BDC-0014, ADR-BDC-0015, ADR-SYNC-0001, ADR-SYNC-0002, ADR-SYNC-0006, ADR-SYNC-0008, "ADR-CRON-0001..0009", "ADR-PY-0001..0006", "ADR-CLI-0001..0005", ADR-API-0001, ADR-API-0002, "ADR-STORE-0001, 0003, 0006", "ADR-MODEL-0001..0004", ADR-CORE-0001, ADR-CORE-0002, ADR-CORE-0205]
adrs_superseded: "ADR-SYNC-0003, 0004, 0005 and 0007 are superseded by ADR-SYNC-0008 (their code is removed by this sprint); ADR-API-0003 is superseded (b-3); none of them binds this sprint"
owned_paths:
  - "crates/bd-cache-sync/**"
  - "crates/bd-cache-model/{Cargo.toml,src/sender.rs,src/config.rs,src/lib.rs} and crates/bd-cache-model/tests/config.rs (the removals)"
  - "crates/sc-cron/src/{scheduler.rs,worker.rs,hook.rs,process.rs,types.rs,lib.rs}, crates/sc-cron/tests/{worker.rs,python_hook.rs,stop_hook.rs,two_process.rs,reload_poll.rs,common/**} and new sc-cron test files"
  - "crates/bd-cache-api/src/{decode.rs,ops/mod.rs,request/**,data/**,error.rs,lib.rs}, the new src/ops/scopes.rs, the scopes reply member in src/ops/manage.rs, crates/bd-cache-api/tests/** for the new verbs, and the TICK_<REFUSAL> expectation of tests/error_table.rs"
  - "docs/bd-cache-cli/contract.md (the removal of the §4 TICK_<REFUSAL> row only)"
  - "crates/bd-cache-cli/{Cargo.toml,src/lib.rs,src/args.rs,src/config.rs,src/request.rs,src/scenario.rs,src/log.rs,src/help.rs,src/render/**,src/bin/**}, src/commands/{mod.rs,tick.rs,signal.rs} and the new commands/{schedule,cron,sync,scopes}.rs"
  - "crates/bd-cache-cli/tests/{cli.rs,process.rs,scenarios.rs,fixtures.rs,harness.rs,fixtures/**,support/**} and new test files for the new verbs"
  - "crates/bd-cache-py/src/**, crates/bd-cache-py/{README.md,pyproject.toml,Cargo.toml}, the new crates/bd-cache-py/python/**"
  - "the new pytest files crates/bd-cache-py/tests/{test_tick.py,test_schedule.py,test_switch.py,test_run.py,test_sync_harness.py,test_ingest.py,test_scopes.py}"
  - "docs/architecture.md (the bd-cache-sync, bd-cache-py, bd-cache-cli, sc-cron and bd-cache-model rows of the Crate Layout and Boundaries tables, and the ADR implementation table)"
  - "docs/bd-cache-sync/**, docs/sc-cron/**, docs/bd-cache-api/**, docs/bd-cache-cli/{architecture.md,requirements.md} (implementation notes)"
  - "the carried register and plan edits (lead ruling, QA-1 fix round; each hunk listed in the PR): docs/requirements.md (the REQ-BDC-0727 and 0732 period text), docs/architecture.md (the ADR-BDC-0013 alert-verb text and the ADR implementation table), docs/bd-cache/read-path.md (the ADR-CORE-0205 alert-verb paragraph), docs/project-plan.md and docs/plans/phase-b/{phase-b-plan.md,sprint-b-7-store-contract.md} (the status lines)"
  - "the alert verbs refuse an unregistered scope (the ADR-BDC-0013 Amendment 1 exception ends with the phase files): crates/bd-cache/src/lib.rs and crates/bd-cache/tests/ingest.rs; crates/bd-cache-store/tests/location.rs (the location test without the phase files)"
  - "the sync rows' run record (REQ-BDC-0731, the ADR-STORE-0006 sprint b-8 amendment) and the bounded pending and escalation reads (REQ-BDC-0727): crates/bd-cache-model/src/{state.rs,store.rs,testing/store.rs,testing/contract.rs}, crates/bd-cache-store/src/{schema.rs,store.rs}, crates/bd-cache-store/tests/{format_golden.rs,open.rs,fixtures/format-golden.txt}, crates/bd-cache/src/lib.rs, docs/bd-cache-store/architecture.md (the ADR-STORE-0006 amendment)"
  - "the model types the fixes need: crates/bd-cache-model/src/error.rs (PreconditionReason::PythonSyncEnabled and InsideRun), src/budget.rs (TickBudget::default, ROW_TIMEOUT_MARGIN public), src/ingest.rs (IngestSnapshot::from_values and from_reader, REQ-BDC-0726)"
  - "crates/bd-cache-cli/src/commands/runtime.rs, crates/bd-cache-cli/src/bin/fake-python.rs, tests/tree_kill.rs (the Windows job-object and Ctrl-Break tests) and tests/release_hygiene.rs (fake-python is a gated test bin like fake-bd); crates/sc-bd-cli/src/process_group.rs (a stale comment); crates/sc-cron/{Cargo.toml,README.md} (process-wrap from the workspace entry; its links); crates/sc-cron/src/store.rs is unchanged"
  - "the root Cargo.toml [workspace.dependencies] entries of process-wrap and rustix (B8Q1-036), and .github/workflows/python.yml (bdc and fake-bd built beside the wheel for the end-to-end pytest, B8Q1-016)"
  - "docs/bd-cache-py/architecture.md (the ADR-PY-0006 sprint b-8 amendment: the in-run static and the composition through bd-cache-sync, lead ruling ARCH-002)"
  - "the new pytest file crates/bd-cache-py/tests/test_cli_e2e.py"
---

# Sprint b-8 — the sync rows, the switch, the Python surface and end to end (WP-6b, WP-9)

## Outcome

beads-cache syncs bd itself, on a row of the sc-cron table in the cache file. A consumer can
switch to its own Python sync row with one command, which runs the script at once and rolls back
on failure, and the built-in sync gives a post-sync hook every result and error; the cache sends
nothing (ADR-BDC-0010). The design is ADR-SYNC-0008, ADR-BDC-0015, ADR-PY-0006 and
ADR-CRON-0009, with REQ-BDC-0720..0731, which hold the parameters.

## Deliverables

1. **Schedule** (REQ-BDC-0720, REQ-BDC-0731, ADR-SYNC-0008). `bdc cache schedule
   set|show|enable|disable|clear` manage the built-in row in `CacheScheduleStore` (b-7), as
   REQ-BDC-0720 gives them. At most one built-in row per repository (ANC-008): a second
   `schedule set` replaces the one row in place (generation + 1), and a built-in or Python
   sync payload on a row of any other name is refused by the resolver and recorded `Failed`,
   while other sc-cron rows coexist in the table; `enable` and
   `disable` flip only its `enabled` flag (REQ-CRON-0014); `enable` while the Python row is
   enabled is `SCHEDULE_INVALID`. `schedule set` validates all before it writes (the import check
   through sc-cron's supervisor), then writes the row with `replace_and_set_enabled`, its
   `enabled` flag kept (ADR-SYNC-0008). Every mode-changing command reads the rows and writes
   them under the cache file's mode lock (ADR-SYNC-0008 as amended in QA-1), so no
   interleaving leaves both rows enabled. `schedule show` reports what REQ-BDC-0731 lists,
   each row's run id, consecutive failures and first failure kept as typed data in the cache
   file's `sync_run` table (ADR-STORE-0006 as amended in QA-1).
2. **Entry points and shutdown** (REQ-BDC-0720, REQ-BDC-0730, S90). `bdc cache cron run`
   (sc-cron's worker with bd-cache-sync's resolver, `MissedTickPolicy::Skip`, run through
   bd-cache-sync's `SystemSchedules`; `bd-cache-cli` gains `tokio`, not `sc-cron`, QA-1 ruling
   RBQA-F003), `cron
   run --once`, `bdc cache tick` and PyO3 `Cache.tick()` run the built-in row's code;
   `Cache.tick()` releases the GIL and owns its runtime as ADR-PY-0006 says. Scheduled times come
   only from the rows (ANC-002). `cron run` stops on `SIGINT`, `SIGTERM` and `SIGHUP` (Ctrl-C and
   Ctrl-Break on Windows) with `--shutdown-timeout-ms`: on a signal only the stop flag is set
   and no run starts; the runs in flight, their bd calls included, get the grace period, and
   only when it expires are the bd calls stopped and the trees still going killed (Rand's
   ruling 5); the lease is released and a stop takes at most twice the timeout, as
   REQ-BDC-0730 says (`kill -9` is covered only by lease expiry); `tick` and `sync use-python`
   stop their bd calls and trees at once;
   `cron run --once` exits 0, or 3 with `TICK_RUN_FAILED`; `cron run` logs as REQ-BDC-0730 says.
   Every new command (`schedule`, `cron`, `tick`, `sync`, `scopes`) is on the `bdc cache`
   cache-management surface; none joins the bd /v0-shaped beads surface (ADR-BDC-0009,
   ADR-BDC-0015).
3. **The built-in run** (REQ-BDC-0703 amended, REQ-BDC-0722, REQ-BDC-0727). `TickLease` (the RAII
   guard over b-7's lease calls) and `Budget` (the sync and hook deadlines); lease, drain, sync of
   every configured scope by its filter, post-sync hook and reply; the hook keeps
   `hook_reserve_ms`; no write transaction and no drain lock while the hook runs.
   - Rows are stored only through b-7's strict row decoder (REQ-BDC-0007), all or nothing per
     scope result: one invalid row stores no row from that result and fails the scope
     (REQ-BDC-0008); a stored row whose `updated_at` is newer is never replaced (REQ-BDC-0502).
   - When bd cannot be reached, a scope's result is `outage`, its error object carrying
     `outage: true` (REQ-BDC-0729's `details.outage`, flat in the embedded object as
     ADR-SYNC-0008's request shows), its rows stay as they were, reads keep answering from the
     cache, and the hook is told (ADR-BDC-0005).
   - While the Python row is enabled, `tick` and `Cache.tick()` are `PRECONDITION` and start no
     bd process.
4. **The Python sync row and the switch** (REQ-BDC-0724, REQ-BDC-0725, REQ-BDC-0722, ANC-003).
   - `bdc cache sync use-python` writes the one Python row enabled and the built-in row disabled
     in one transaction, runs it at once under its lease and on failure restores the prior
     state, as REQ-BDC-0724 says; `bdc cache sync use-builtin` reverses it.
   - `PythonSyncJob`: the Python row's target, resolved per launch with the trust check, running
     the script with sc-cron's protocol (`cron.stop`, then `cron.tick`) under the lease, with
     REQ-BDC-0725's payload and the environment allowlist.
   - With the Python row enabled, no `bd list` runs for the configured scopes; the script fills
     the cache through `ingest_scope`, with the same rules as the built-in sync (REQ-BDC-0007,
     0008, 0502), and its run is judged by its exit status. beads-cache itself reaches the backend
     only through bd on both paths and reads no Dolt table (REQ-BDC-0006).
   - `.beads/bd-cache.toml` cannot name code to run (`CONFIG_INVALID`).
5. **Post-sync hook** (REQ-BDC-0723, REQ-BDC-0727, REQ-BDC-0728). The optional hook a consumer
   names in the built-in row gets the sync results and errors after each sync; it runs through
   sc-cron's exported supervisor (REQ-CRON-0018) with REQ-BDC-0723's command, environment, cwd,
   caps and kill at its deadline (NFR-BDC-0004), REQ-BDC-0727's capped request, at-least-once
   delivery and backoff, and the trust check before every launch (`TrustedCacheFile`). The run
   marks nothing. bd-cache-sync drops `toml` and `libc` and adds only `rustix` (Unix, its
   `process` module, from the workspace entry) for the trust check's effective uid, and the
   `bd-cache-api` edge (QA-1: it runs the front-end verbs for every front end and maps their
   errors once) (ADR-SYNC-0008).
6. **sc-cron's general features** (REQ-CRON-0016, REQ-CRON-0018, REQ-CRON-0005 amended,
   ADR-CRON-0009). `TargetResolver` and `Scheduler::spawn_worker_with`, called once per launch,
   an error recorded `Failed` with the row left active; `spawn_worker_for` and `spawn_worker`
   kept. `sc_cron::process` public, `PythonHook::with_env`, `HookOutput::stop_decision` /
   `run_outcome`. `StopDecision::Stop` removed with its match arms. sc-cron's rustdoc cites only
   its own requirements; test fixtures use neutral names. sc-cron gets no beads-cache name,
   path, type, protocol or option.
7. **Python packaging and entry points** (ADR-PY-0006, REQ-BDC-0723, REQ-BDC-0725). The mixed
   layout of ADR-PY-0006, existing imports kept; `python -m bd_cache_rs.run post_sync
   <module:function>` and `bd_cache_rs.sync.main(func)` as ADR-SYNC-0008 says; `flush`, `drain`,
   `tick` and `schedule_*` raise `PRECONDITION` inside a run; the pyproject description loses its
   consumer wording.
8. **PyO3 surface** (ADR-PY-0006, REQ-PY-0002, 0005, 0008 amended, REQ-BDC-0726). `ingest_scope`,
   `scope_announce`, `scopes_*`, `schedule_*`, `sync_use_python` / `sync_use_builtin` and `tick()`
   on the handle's own connection (ADR-PY-0003), and the new codes in the exports. New scope
   arguments take scope names as written; the 0.1 verbs keep the bare-name rule (b-7). The 0.1
   verbs, arguments and dicts are unchanged, and `dir(Cache)` is still the 0.1 set. `bd-cache-py`
   gains `bd-cache-sync` and `tokio`, opens its handle through bd-cache-sync's one composition
   root (`compose_tuned`, one process-wide `ProjectLocks`), drops `dunce`, and its Boundaries
   and Crate Layout rows say so.
9. **The new verbs in `bd-cache-api` and the CLI** (ADR-BDC-0015, REQ-CLI-0006 amended,
   REQ-API-0001 and 0005 amended, REQ-BDC-0803 amended). The new verbs' request, data and error
   types in `bd-cache-api`, decoded by `decode.rs` and listed in `Command::ALL` (the
   `Command::parse("cache tick")` doctest reversed); the store-only verbs run there
   (`ops/scopes.rs`, in `ops/mod.rs`), the others in `bd-cache-cli` (`src/request.rs`, the new
   `commands/`) and `bd-cache-py`. `scopes ingest` reads stdin through
   the 64 MiB + 1 byte limit and counts rows as they decode (REQ-BDC-0726). `cache scopes` is
   `scopes list`'s alias, now with each scope's filter; `cache evict` is unchanged; `scopes
   remove` is `evict` plus the registry removal. `ApiError::tick_refusal`, its kind table, the
   `refused[]` items, the contract's §4 `TICK_<REFUSAL>` row and its `error_table.rs` expectation
   are removed. The help text of `cache tick` is rewritten. With the phase files gone, the
   alert verbs (`sync_alert`, `set_sync_alert`, `set_sync_alert_within`) refuse an
   unregistered scope like every other scope verb (ADR-BDC-0013, ADR-CORE-0205: their
   Amendment 1 exception ends; `crates/bd-cache/src/lib.rs`, `tests/ingest.rs`).
10. **Removals** (ADR-BDC-0010, ADR-BDC-0012; REQ-BDC-0701, 0702, 0704..0707 and the REQ-SYNC ids
    above superseded; REQ-BDC-0810 amended). In `crates/bd-cache-sync/src`: the consumer
    messaging module, `phases.rs`, the consumer parts of `announce.rs` and `active.rs`, the stop
    rule and the `refused` / `pass-through` replies; the sender's process test and
    `tests/stop_rule.rs`, `tests/tick_phases.rs` rewritten for scopes, and the phase-file fixtures
    (the files there whose paths the lint flags). In `bd-cache-model`: `src/sender.rs` and the
    `[tick]` table's message-routing and phase-file keys with their users; `[tick.hooks]` and
    `[tick.sync]` are `CONFIG_INVALID`. In `bd-cache-cli`: the uses of the consumer's message
    sender and `TickReply` in `src/lib.rs`, `commands/tick.rs` and `commands/signal.rs` (the
    `StopFlag` stays, for `cron run` and `tick`), the refusal of the first consumer's own
    directory in `src/config.rs` with its test in `tests/process.rs`, and the hard-coded scope
    prefix in `tests/scenarios.rs`. The stop hook itself, returning continue, disable or delete,
    is b-6's (REQ-CRON-0005, ANC-007) and stays.
11. **Architecture rows** (ADR-BDC-0011, ADR-BDC-0015, ADR-PY-0006, ADR-SYNC-0008). The Crate
    Layout and Boundaries rows of the crates this sprint changes, with their new edges, and the
    ADR implementation table.

## Acceptance

Each item has a positive test and a planted negative (AC-N). Unit tests run with no bd and no
Dolt, against fake-bd and the injected clock (NFR-BDC-0005); every bd call, hook and script run
is killed at its timeout capped by the time left before the deadline (NFR-BDC-0004).

1. `grep -rn` of the removal list finds nothing.
2. Schedule: `schedule set/show/enable/disable/clear` round-trip and a second built-in row is
   refused; `SCHEDULE_INVALID` for each `schedule set` refusal of REQ-BDC-0720 and 0724 (a fake
   interpreter exits 1 on `-c`) and for `schedule enable` while the Python row is enabled, each
   with a planted negative. `schedule set` and `cron run` on a cache file whose
   `sc_cron_schedule_version` row names another version fail with a typed `STORE_FAILED` before
   serving any work (ADR-BDC-0014).
3. Single flight: `cron run --once` runs a due row and skips one not due (sc-cron ManualClock);
   two real `bdc` processes, the second replies `skipped: in_flight`; a holder killed with
   `kill -9` is taken over after expiry; an on-demand `tick` during a worker run is skipped;
   `use-python` during a worker-launched Python run fails with `TICK_IN_FLIGHT`; two simultaneous
   `cron run --once` processes run the Python row once.
4. Switch: `use-python` leaves exactly the Python row enabled and runs it at once; its reply
   carries the run id, the exit status and stderr; `use-builtin` reverses it. Failing the command
   with `TICK_RUN_FAILED` and restoring the prior mode: a script that exits 1, one that outlives
   `--timeout-ms` (injected clock), a missing interpreter, a `cron.stop` answer of `disable`, a
   `SIGTERM` mid-run (Unix), a group-writable cache file. A failed restore gives
   `details.restored: false`, then `use-builtin` restores. A crash between the mode commit and
   the run leaves one row enabled, and `schedule show` flags the Python row as never run. Each
   rule of REQ-BDC-0724, with a planted negative: a second connection polling while the first
   Python row is created never sees two enabled rows; `schedule set` keeps a disabled row
   disabled, and writes it disabled while mode is Python; `use-python` with no built-in row
   fails to mode `none`, and `use-builtin` is then `SCHEDULE_INVALID`; a failed re-run from mode
   Python restores the old script; a second `use-python` with a new script replaces the row in
   place, fencing a run of the old one. Over 8 KiB of stderr holding an `--env` value gives a
   reply cut to 8 KiB showing `<redacted:NAME>`; planted negative: drop the cap or the redaction.
5. Python row through the worker: with the Python row enabled, `cron run --once` runs the script
   with `cron.stop` then `cron.tick`; a script written with `bd_cache_rs.sync.main` ingests one
   scope and the next read sees it; no `bd list` runs and rows come only from the script's
   `ingest_scope`; the payload carries `cache_path`, `beads_dir` and `run_id`; the script sees
   only the allowlisted environment; a group-writable cache file runs nothing (`cron run` and
   `cron run --once`); a script that answers `cron.stop` with no JSON is recorded `Failed`.
6. One table holds the built-in Rust row and the Python row, each run with its own target; an
   unresolvable row is recorded `Failed` and stays active (sc-cron's per-row resolver).
7. Post-sync hook runner: kill at the deadline, causal through the shared `gate_file` helper, on
   Unix process groups and in a Windows job-object test; output caps; a large request written
   without a deadlock; only the allowlisted environment; a temp cwd; an untrusted cache file runs
   nothing (Unix); an oversized request capped at 500 escalations and 4 MiB.
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
   is not `in_flight`) and exits 143; `SIGINT` exits 130 and `SIGHUP` 129; a second signal during
   the wait changes neither the wait nor the exit code. A Windows test sends Ctrl-Break and sees
   the job object's children gone and exit 130. `cron run --once` with a failing row exits 3
   with `TICK_RUN_FAILED`.
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
    `grep -ri 'bead\|bd_cache\|bd-cache' crates/sc-cron/src` finds nothing (ANC-005).
14. The CLI: `cache scopes` and `scopes list` give the same reply; `cache evict` keeps its 0.1
    reply; `scopes remove` of a scope with pending entries is `PRUNE_BLOCKED`; the new codes have
    REQ-BDC-0729's exits, and `error_table.rs` passes with the `TICK_<REFUSAL>` row gone;
    `the_legacy_cache_file_is_config_invalid` is gone and a repository with the consumer's legacy
    directory opens as any other.
15. `tests/job_threading.rs` `join_idle` is causal. pytest passes with
    `-W error::DeprecationWarning`, the existing `import bd_cache_rs` tests pass unchanged, and
    `Cache.tick()` called from an asyncio loop thread returns without nesting a runtime.
16. AC-0 on this fence, AC-Q.

## Reference

PRs #93 (stopped b-7 WIP), #94 and #95, REFERENCE ONLY; reuse what matches this doc.

## Does not close

The contract text for these verbs, the neutral tests and docs, health, and the CI lint gate
(b-9).
