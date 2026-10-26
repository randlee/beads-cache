---
id: b-8
phase: b
status: planned
closure_type: integration
target_boundary: bd-cache-sync, bd-cache-cli, bd-cache-py, sc-cron runner
track: core
wave: 6
branch: sprint/b-8-tick-python
worktree: sprint/b-8-tick-python
recommended_agent: rust-developer
recommended_model: opus
must_follow: [b-7]
parallel_safe: [b-9]
requirements:
  owns: [REQ-BDC-0720, REQ-BDC-0723, REQ-BDC-0724, REQ-BDC-0725, REQ-BDC-0727, "REQ-BDC-0703 (amended)", "REQ-BDC-0701, 0702, 0704..0707 (superseded)"]
  constrained_by: [REQ-CRON-0016, REQ-BDC-0001, REQ-BDC-0006, REQ-BDC-0007, REQ-BDC-0008, REQ-BDC-0502, REQ-BDC-0812, NFR-BDC-0004, NFR-BDC-0005, REQ-CRON-0013, REQ-CRON-0014]
adrs: [ADR-SYNC-0008, ADR-CRON-0009, ADR-BDC-0005, ADR-BDC-0006, ADR-BDC-0010]
owned_paths:
  - "crates/bd-cache-sync/**"
  - "crates/bd-cache-cli/src/{args.rs,config.rs,commands/tick.rs,commands/mod.rs}"
  - "crates/bd-cache-cli/src/commands/{schedule,cron,sync,scopes}.rs and their tests"
  - "docs/bd-cache-sync/**"
  - "crates/sc-cron/** (the ADR-CRON-0009 changes only: per-row target, Stop alias removal, rustdoc
    and fixtures; no change to scheduling semantics)"
  - "crates/bd-cache-py/src/**"
  - "crates/bd-cache-py/{README.md,pyproject.toml,Cargo.toml,python/**}"
  - "the new pytest files"
---

# Sprint b-8 — the sync rows, the switch, the Python surface and end to end (WP-6b, WP-9)

## Outcome

beads-cache syncs bd itself, on a row in the sc-cron table of the cache file. A consumer can
switch to its own Python sync row with one command, and the built-in sync gives a post-sync hook
every result and error; the cache sends nothing (ADR-BDC-0010). The design is
[phase-b-boundary.md](phase-b-boundary.md) §A7, §Scheduling, §Single flight, §One tick,
§Calling a Python method and §Request and result shapes, with Amendments A2–A5 and A7.

## Deliverables

1. **Schedule** (REQ-BDC-0720, Amendments A3, A4). `bdc cache schedule set <cron-expr>
   [--budget-ms N] [--hook-reserve-ms N] [--post-sync <module:function> --python <absolute
   interpreter>]`, `show`, `enable`, `disable` and `clear` manage the built-in row, with the
   defaults and ranges of §Scheduling. At most one built-in row per repository (ANC-008);
   `enable` and `disable` flip only sc-cron's `enabled` column (REQ-CRON-0014).
2. **Entry points** (REQ-BDC-0720, S90). `bdc cache cron run` (sc-cron `Scheduler`,
   `MissedTickPolicy::Skip`), `cron run --once`, `bdc cache tick` and PyO3 `Cache.tick()` run
   the built-in row's code; `Cache.tick()` releases the GIL. The scheduled sync times come only
   from the rows in the cron table (ANC-002): `cron run` and `cron run --once` run a row when it
   is due, and `bdc cache tick` and `Cache.tick()` are explicit on-demand runs that do not
   change the rows' times. Every new command (`schedule`, `cron`, `tick`, `sync`, `scopes`) is
   on the `bdc cache` cache-management surface; none is added to the bd /v0-shaped beads
   surface (ADR-BDC-0009).
3. **The built-in run** (REQ-BDC-0703 amended, REQ-BDC-0722). Lease, drain, sync of every
   configured scope with `bd list` (the scope's filter) and `bd blocked`, post-sync hook and
   reply, in the order and budget of §One tick; the hook keeps `hook_reserve_ms`. The run holds
   no write transaction and no drain lock while the hook runs.
   - Rows are stored only through b-7's strict row decoder (REQ-BDC-0007), all or nothing per
     scope result: one invalid row stores no row from that result and fails the scope
     (REQ-BDC-0008); a stored row whose `updated_at` is newer is never replaced (REQ-BDC-0502).
   - When bd cannot be reached, a scope's result is `outage` (`details.outage: true`), its rows
     stay as they were, reads keep answering from the cache, and the hook is told
     (ADR-BDC-0005).
4. **The Python sync row and the switch** (REQ-BDC-0724, REQ-BDC-0725, ANC-003, Amendment A7).
   - `bdc cache sync use-python --python <absolute interpreter> --module <module> [--cron
     <expr>]` disables the built-in row, creates or updates the Python sync row (an sc-cron
     Python hook row) and enables it, then runs it at once. The reply carries the run's exit
     status and captured stderr, so the caller sees any error the script emits; a failed run
     makes the command fail (`TICK_RUN_FAILED`) and restores the built-in row as the enabled
     one. `bdc cache sync use-builtin` reverses the switch. Neither leaves both rows enabled.
   - With the Python row enabled, no `bd list` runs for the configured scopes; the script fills
     the cache through `ingest_scope`, with the same rules as the built-in sync
     (REQ-BDC-0007, 0008, 0502), and its run is judged by its exit status.
   - A relative interpreter or a module that is not a dotted name is `SCHEDULE_INVALID`.
     `.beads/bd-cache.toml` cannot name code to run (`CONFIG_INVALID`).
   - `bdc cache scopes add|set|remove|list|ingest|announce`.
5. **Post-sync hook** (REQ-BDC-0723, REQ-BDC-0727, REQ-BDC-0728). The hook named in the
   built-in row's payload runs as `<interpreter> -I -m bd_cache_rs.run post_sync
   <module:function>`, run by bd-cache-sync with its own process supervision (not through
   sc-cron): a private temp cwd; only PATH, HOME, the locale and `BEADS_DIR` passed, with
   `BD_CACHE_PATH` set; stdout capped at 1 MiB and stderr kept to 8 KiB; killed with everything
   it started at its deadline (NFR-BDC-0004); request bodies never logged. It gets the
   `post_sync` request of §Request and result shapes; the run itself marks nothing; delivery is
   at-least-once. No row's or hook's code runs from a cache file not owned by the current user,
   or group- or world-writable (`TICK_RUN_FAILED`); the Windows ACL check is a documented gap.
6. **sc-cron generality fixes to b-6** (REQ-CRON-0016, ADR-CRON-0009, ANC-005; the b-6 findings
   of the sc-cron generality review). A worker resolves each row's hook target (Rust `Job` or
   Python module) from the row, so the built-in row and the Python row share one table; the
   `StopDecision::Stop` alias is removed; sc-cron's rustdoc cites only its own requirements;
   test fixtures use neutral names. sc-cron gets no beads-cache name, path, type, protocol or
   option.
7. **`bd_cache_rs.run`** (REQ-BDC-0727). `python -m bd_cache_rs.run post_sync
   <module:function>` (maturin mixed layout) opens its own `Cache` on `BD_CACHE_PATH`, calls
   `func(cache, request)` and prints the result envelope of §Calling a Python method. `flush`,
   `drain`, `tick` and `schedule` raise `PRECONDITION` inside a run.
8. **PyO3 surface** (REQ-BDC-0812, REQ-BDC-0726). `ingest_scope`, `scope_announce`,
   `scopes_*`, `schedule_*`, `sync_use_python` / `sync_use_builtin` and `tick()`, and the new
   codes in the exports. Every scope argument takes any configured scope name, with no prefix
   rule (ANC-009). The 0.1 verbs, arguments and dicts are unchanged.
9. **Store** (ADR-STORE-0001). b-8 adds or changes no store table, column or index; it uses
   b-7's extensions and creates the sc-cron schedule tables through sc-cron.
10. **Removals** (ADR-BDC-0010; REQ-BDC-0701, 0702, 0704..0707 superseded). In
    `crates/bd-cache-sync/src`: the consumer messaging module, `phases.rs`, the consumer parts
    of `announce.rs` and `active.rs`, and the stop rule; the sender's process test and
    `tests/stop_rule.rs` (the messaging module and the process test are the files there whose
    paths the lint flags); the `refused` / `pass-through` replies; the refusal of the first
    consumer's own directory in `bd-cache-cli/src/config.rs`. The stop hook itself, returning
    continue, disable or delete, is b-6's (REQ-CRON-0005, ANC-007) and stays. The
    `.beads/bd-cache.toml` keys `[tick.hooks]` and `[tick.sync]` are `CONFIG_INVALID`.

## Acceptance

Each item has a positive test and a planted negative (AC-N). Unit tests run with no bd and no
Dolt, against fake-bd and the injected clock (NFR-BDC-0005); every bd call and hook run is
killed at its timeout capped by the time left before the deadline (NFR-BDC-0004).

1. `grep -rn` of the removal list finds nothing.
2. Schedule and lease: `schedule set/show/enable/disable/clear` round-trip and a second
   built-in row is refused; `cron run --once` runs a due row and skips one not due (sc-cron
   ManualClock); two real `bdc` processes, the second replies `skipped: in_flight`; a holder
   killed with `kill -9` is taken over after expiry; an on-demand `tick` during a worker run is
   skipped.
3. Switch: `use-python` leaves exactly the Python row enabled and runs it at once; its reply
   carries the script's exit status and stderr; a script that fails makes the command fail and
   leaves the built-in row enabled; `use-builtin` reverses it; with the Python row enabled, no
   `bd list` runs and rows come only from the script's `ingest_scope`.
4. One table holds the built-in Rust row and the Python row, each run with its own target
   (sc-cron per-row target).
5. Post-sync hook runner: kill at the deadline, causal through the shared `gate_file` helper, on
   Unix process groups and in a Windows job-object test; stdout and stderr caps; a large request
   written without a deadlock; the environment is only the allowlist; the cwd is a temp dir; an
   untrusted cache file gives `TICK_RUN_FAILED` and runs nothing (Unix).
6. Budget: when the sync uses everything, the hook still gets `hook_reserve_ms`; a hook that
   times out still lets the run finish by its deadline.
7. Hook: the request shape; marks made by the hook persist; a crash after marking keeps those
   marks and re-offers the rest with `first_offered_at` unchanged; each failure gives
   `TICK_RUN_FAILED`.
8. `SCHEDULE_INVALID` for a relative interpreter and a non-dotted module. A hostile
   `bd_cache_rs.py` or a shadowing module in the project dir is not imported.
9. A real-interpreter end-to-end test against fake-bd: `bdc cache tick` syncs a label scope and
   an `--all` scope; the post-sync hook receives both results and marks one escalation and a
   failed scope; on the next run marked items are not offered again and the rest are, with
   `first_offered_at` unchanged. Then `use-python` with a script that ingests one scope and
   exits 0, and with one that exits 1.
10. Rows: one invalid row among valid ones, in a built-in result and in a Python
    `ingest_scope`, stores no row from that result and fails the scope; an older incoming row
    never replaces a newer stored one, on both paths. An unreachable bd gives the scope an
    `outage` result, cached reads still answer, and the hook receives it.
11. sc-cron stays general: `cargo tree -p sc-cron` shows no bd-cache or sc-beads crate, and
    `grep -ri 'bead\|bd_cache\|bd-cache' crates/sc-cron/src` finds nothing (ANC-005).
12. `tests/job_threading.rs` `join_idle` is causal. pytest passes with
    `-W error::DeprecationWarning`, and the existing `import bd_cache_rs` tests pass unchanged.
13. AC-0 on this fence, AC-Q.

## Reference

PR #93 (stopped b-7 WIP, built from the cut plan), #94 (fen-tick) and #95 (fen-py), all
REFERENCE ONLY; reuse what matches this doc.

## Does not close

The contract text for these verbs, the neutral tests and docs, and the CI lint gate (b-9).
