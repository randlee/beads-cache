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
  owns: [REQ-BDC-0720, REQ-BDC-0723, REQ-BDC-0725, REQ-BDC-0727, "REQ-BDC-0703 (amended)", "REQ-BDC-0701, 0702, 0704..0707 (superseded)"]
  constrained_by: [REQ-CRON-0016, REQ-BDC-0001, REQ-BDC-0006, REQ-BDC-0007, REQ-BDC-0008, REQ-BDC-0502, REQ-BDC-0812, NFR-BDC-0004, NFR-BDC-0005, REQ-CRON-0013, REQ-CRON-0014]
adrs: [ADR-SYNC-0008, ADR-CRON-0009, ADR-BDC-0005, ADR-BDC-0006, ADR-BDC-0010]
owned_paths:
  - "crates/bd-cache-sync/**"
  - "crates/bd-cache-cli/src/{args.rs,config.rs,commands/tick.rs,commands/mod.rs}"
  - "crates/bd-cache-cli/src/commands/{schedule,cron,hooks,scopes}.rs and their tests"
  - "docs/bd-cache-sync/**"
  - "crates/sc-cron/** (the ADR-CRON-0009 changes and the Stop alias removal only; no change to
    scheduling semantics)"
  - "crates/bd-cache-py/src/**"
  - "crates/bd-cache-py/{README.md,pyproject.toml,Cargo.toml,python/**}"
  - "the new pytest files"
---

# Sprint b-8 — the tick, the Python surface and end to end (WP-6b, WP-9)

## Outcome

beads-cache syncs bd itself, on a tick scheduled in the sc-cron table of the cache file. A
consumer hands scopes to its own registered Python `sync` method and gets a `post_sync` hook
with every result and error; the cache sends nothing (ADR-BDC-0010). The design is
[phase-b-boundary.md](phase-b-boundary.md) §Scheduling, §Single flight, §One tick, §Calling a
Python method, §Request and result shapes and §Registration, with Amendments A2–A5 and
decision D1.

## Deliverables

1. **Schedule** (REQ-BDC-0720, Amendments A3 and A4). `bdc cache schedule set <cron-expr>
   [--budget-ms N] [--hook-reserve-ms N]`, `show`, `enable`, `disable` and `clear`, with the
   defaults and ranges of §Scheduling. At most one tick row per repository; `enable` and
   `disable` flip only sc-cron's `enabled` column (REQ-CRON-0014).
2. **Entry points** (REQ-BDC-0720, S90). `bdc cache cron run` (sc-cron `Scheduler`,
   `MissedTickPolicy::Skip`), `cron run --once`, `bdc cache tick` and PyO3 `Cache.tick()` run
   the same Rust tick; `Cache.tick()` releases the GIL.
3. **One tick** (REQ-BDC-0703 amended, REQ-BDC-0722). Lease, drain, sync, post-sync hook and
   reply, in the order and budget of §One tick; the hook keeps `hook_reserve_ms`. The tick
   holds no write transaction and no drain lock while a method runs.
4. **Scope methods** (REQ-BDC-0725, REQ-BDC-0721, decision D1). `builtin` scopes run `bd list`
   with the scope's filter and `bd blocked`; `python` scopes go to the registered `sync` method
   once, instead of the built-in `bd list` sync for those scopes (ANC-003); a consumer switches a
   scope with `bdc cache scopes set <scope> --method python`. `external` scopes are never synced. `bdc cache scopes add|set|remove|list|ingest|announce`.
   - Both the built-in sync and a Python method's `ingest_scope` store rows only through b-7's
     strict row decoder (REQ-BDC-0007), all or nothing per scope result: one invalid row stores
     no row from that result and the scope's result is failed (REQ-BDC-0008). Neither replaces a
     stored row whose `updated_at` is newer (REQ-BDC-0502).
   - When bd cannot be reached, a `builtin` scope's result is `outage` (`details.outage: true`),
     its rows stay as they were, reads keep answering from the cache, and the post-sync hook is
     told (ADR-BDC-0005).
5. **Registered methods** (REQ-BDC-0723, REQ-BDC-0724, S89). `bdc cache hooks register
   sync|post_sync --python <absolute interpreter> --function <module:function> [--env NAME]...`,
   `hooks show`, `hooks clear`. Each method runs as
   `<interpreter> -I -m bd_cache_rs.run <kind> <module:function>` from a temp cwd, through
   sc-cron's protocol-free supervised-process call (deliverable 6, decision D2); the
   `bd-cache.run.v1` request is built in bd-cache-sync. bd-cache checks that the interpreter is
   absolute (`REGISTRATION_INVALID`). The tick
   refuses to run registered code from a cache file not owned by the current user, or group- or
   world-writable (`TICK_RUN_FAILED`); the Windows ACL check is a documented gap.
6. **sc-cron generality fixes to b-6** (REQ-CRON-0016, ADR-CRON-0009, ANC-005; the b-6 findings
   of the sc-cron generality review). In sc-cron, with no beads-cache name, path, type or
   requirement id:
   - a public supervised-process call: program, arguments, an environment policy (inherit, or
     an allowlist plus names set to values), a working directory (temporary by default), stdin
     bytes, separate stdout and stderr caps, and an expiry, with the process-group / job-object
     kill at the expiry and on drop;
   - `HookTarget::Command` for any executable, with `PythonHook` as a constructor for it, and a
     per-row target resolver for a worker;
   - the `sc-cron.hook.v1` stdin adds the schedule's `id`, `generation` and scheduled time;
   - sc-cron's rustdoc cites only its own requirements; test fixtures use neutral schedule
     names and payloads.

   bd-cache-sync sets its own policy on that call: it passes PATH, HOME, the locale and
   `BEADS_DIR`, sets `BD_CACHE_PATH`, uses a 1 MiB output cap with stderr kept to 8 KiB, and
   never logs request bodies.
7. **`bd-cache.run.v1`** (REQ-BDC-0725, REQ-BDC-0727). `python -m bd_cache_rs.run` (maturin
   mixed layout) opens its own `Cache` on `BD_CACHE_PATH`, calls `func(cache, request)` and
   prints the result envelope of §Calling a Python method. `flush`, `drain`, `tick` and
   `schedule` raise `PRECONDITION` inside a run.
8. **Post-sync hook** (REQ-BDC-0727, REQ-BDC-0728). The request and result shapes of §Request
   and result shapes; the tick itself marks nothing; delivery is at-least-once.
9. **PyO3 surface** (REQ-BDC-0812, REQ-BDC-0726). `ingest_scope`, `scope_announce`,
   `scopes_*`, `schedule_*`, `hooks_*` and `tick()`, and the four new codes in the exports.
   Every scope argument takes any configured scope name, with no prefix rule (ANC-009). The
   0.1 verbs, arguments and dicts are unchanged.
10. **Store** (ADR-STORE-0001). b-8 adds or changes no store table, column or index; it uses
    b-7's extensions and creates the sc-cron schedule tables through sc-cron.
11. **Removals** (ADR-BDC-0010; REQ-BDC-0701, 0702, 0704..0707 superseded). In
    `crates/bd-cache-sync/src`: the consumer messaging module, `phases.rs`, the consumer parts of
    `announce.rs` and `active.rs`, and the stop rule; the sender's process test and
    `tests/stop_rule.rs` (the messaging module and the process test are the files there whose
    paths the lint flags); the `refused` / `pass-through` replies; the refusal of the first
    consumer's own directory in `bd-cache-cli/src/config.rs`; and
    sc-cron's `StopDecision::Stop` alias kept by b-6 for the old stop rule. The stop hook itself,
    returning continue, disable or delete, is b-6's (merged; REQ-CRON-0005, ANC-007) and stays
    unchanged. `.beads/bd-cache.toml`
    keys `[tick.hooks]` and `[tick.sync]` are `CONFIG_INVALID`.

## Acceptance

Each item has a positive test and a planted negative (AC-N). Unit tests run with no bd and no
Dolt, against fake-bd and the injected clock (NFR-BDC-0005); every bd call and method run is
killed at its timeout capped by the time left before the deadline (NFR-BDC-0004).

1. `grep -rn` of the removal list finds nothing.
2. Schedule and lease: `schedule set/show/enable/disable/clear` round-trip and a second tick
   row is refused; `cron run --once` runs a due tick and skips one not due (sc-cron
   ManualClock); two real `bdc` processes, the second replies `skipped: in_flight`; a holder
   killed with `kill -9` is taken over after expiry; an on-demand `tick` during a worker tick is
   skipped.
3. A `python` scope never runs `bd list` for that scope; its rows come only from the method's
   `ingest_scope`. Methods, through a Rust test-support runner that speaks `bd-cache.run.v1` and opens a real
   second handle: `builtin` syncs as before, with a label scope and an `--all` scope; `python`
   ingested, failed and missing scopes are judged by `tick_id`; `external` is never synced and
   pushed rows survive a tick.
4. Runner: kill at the deadline, causal through the shared `gate_file` helper, on Unix process
   groups and in a Windows job-object test; stdout and stderr caps; a large request written
   without a deadlock; the environment is only the allowlist; the cwd is a temp dir; an
   untrusted cache file gives `TICK_RUN_FAILED` and runs nothing (Unix).
5. Budget: when the sync uses everything, the hook still gets `hook_reserve_ms`; a method that
   times out still lets the tick finish by its deadline.
6. Hook: the request shape; marks made by the runner persist; a crash after marking keeps those
   marks and re-offers the rest with `first_offered_at` unchanged; each failure gives
   `TICK_RUN_FAILED`.
7. `REGISTRATION_INVALID` for a relative interpreter and for a function that does not import. A
   hostile `bd_cache_rs.py` or a shadowing module in the project dir is not imported.
8. A real-interpreter end-to-end test runs a real `bdc cache tick` against fake-bd: the `sync`
   method ingests one scope and fails another; `post_sync` receives both and marks one
   escalation and the failed scope; on the next tick marked items are not offered again and the
   rest are, with `first_offered_at` unchanged.
9. Rows: one invalid row among valid ones, in a built-in result and in a Python
   `ingest_scope`, stores no row from that result and fails the scope; an older incoming row
   never replaces a newer stored one, on both paths. An unreachable bd gives the scope an
   `outage` result, cached reads still answer, and the hook receives it.
10. sc-cron stays general: `cargo tree -p sc-cron` shows no bd-cache or sc-beads crate, and
    `grep -ri 'bead\|bd_cache\|bd-cache' crates/sc-cron/src` finds nothing (ANC-005).
11. `tests/job_threading.rs` `join_idle` is causal. pytest passes with
   `-W error::DeprecationWarning`, and the existing `import bd_cache_rs` tests pass unchanged.
12. AC-0 on this fence, AC-Q.

## Reference

PR #93 (stopped b-7 WIP, built from the cut plan), #94 (fen-tick) and #95 (fen-py), all
REFERENCE ONLY; reuse what matches this doc.

## Does not close

The contract text for these verbs, the neutral tests and docs, and the CI lint gate (b-9).
