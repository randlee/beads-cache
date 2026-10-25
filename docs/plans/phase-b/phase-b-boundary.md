# Phase B, boundary extension: no consumer concerns in beads-cache

Status: approved by lead ruling S82 (bc-lead, 2026-10-25, recording Rand's rulings).
This extends [phase-b-plan.md](phase-b-plan.md) with sprints b-6 to b-9. Phase B
does not close, and PR #82 (integrate/phase-b → develop) does not merge, until b-9
is green.

## Why

The consumer's phase-B review (fenix, for atm-bd) found the cron and cutover path
blocking. While answering it, Rand ruled, verbatim:

- "this is a strict boundary. there should be no atm concerns or patterns that
  exactly match atm."
- "beads-cache is a cache w/ cli for configuration. it also exposes python api
  through py03 for you to use."
- "the beads-cache cron must support running a registered python script that is
  part of the atm-bd package. atm-bd would include beads-cache as a dependency for
  the plug-in and it would be consumed both by phthon hooks and by direct reads
  from tui."
- "blocking 'cron@tool' and other atm strings from source or tests".

Phases a and b ported the consumer's cron into `bd-cache-sync`. The tick sends
messages, reads the consumer's team and lead configuration and its phase files,
mirrors the consumer scheduler's stop rule and reply shape, and syncs only
`phase-<x>` scopes. The test suites vendor the consumer's implementation. The
consumer's lint, at its handoff, reports 899 hits in 112 files.


Rand later restated the goals (2026-10-25), verbatim:

> long term sync would be done by beads-cache at a time scheduled in the cron
> scheduler in sqlite cron table. In the event of the 'hard-coded sync' not
> working, the hard-coded sync could be disabled and replaced w/ a python method
> registered by consumer (atm-bd). We would also likely want a 'post-sync' hook
> that could contain sync results and errors and allow consumer (atm-bd) to do
> custom things at that point i.e. atm send for escellation.

This revision (r2) follows those goals and the critical plan review of r1
(4 blocking, 12 important). Every item from that review is answered in
[Review disposition](#review-disposition).

## Lead rulings

| Ruling | Decision |
|---|---|
| S82 | ADR-BDC-0010 (fenix's draft on `lint/no-atm-boundary` @ 33a2b07) is approved, with the lint adjustments L2–L5 below. The lint script and its tests land with this plan, with no CI step, so every sprint can gate on it (`--paths`). The CI step lands in b-10, once the tree is clean. It is never merged red. |
| S83 | beads-cache owns the bd sync. The schedule is a job in the `sc-cron` sqlite cron table (`SqliteScheduleStore`), stored in the cache file and set with `bdc cache schedule set`. A due job runs one tick in this order: drain, then sync each managed scope, then the post-sync hook. The cache itself sends nothing, and there is no consumer-specific stop rule. atm-bd is the first consumer, not the only one. |
| S84 | The store format is frozen as v3 plus the additive extensions listed in b-6's format golden. Extensions are added only by ruling, through the existing extension-column mechanism (checked with `PRAGMA table_info` under `BEGIN IMMEDIATE`; `user_version` stays 3). Table and column names stay as they are (`phase`, `announced_at`, `scope.kind DEFAULT 'phase'`); the API and docs name them neutrally. This revision adds `sync_state.failing_since`, `sync_state.notified_since` and a `registration` table. Cross-implementation compatibility is the consumer's own test, run against a CI-built wheel artifact before Rand's 0.2.0 go. |
| S85 | PR #86 (health, `refresh scopes --timeout-ms`) is kept, and becomes b-8's branch. Its cutover runbook and the consumer-specific contract text are removed. |
| S86 | The stopped layers `fix/fen-tick` and `fix/fen-py` are abandoned unmerged. Their neutral parts are redone in b-6, b-7 and b-9. |
| S87 | The built-in ("hard-coded") sync can be disabled per scope and replaced with a Python method that the consumer registers. The method feeds the cache only through `Cache.ingest_scope`, which applies the same strict decoder and per-scope transaction as the built-in sync, so a method cannot bypass validation. |
| S88 | The post-sync hook is one registered Python method. It is called after the sync with the tick's results and errors and the cache's notification state, and it marks what it has handled through the cache API. Delivery is at-least-once: anything not marked is offered again on the next tick. |
| S89 | Python methods are registered in the cache file (the `registration` table, through `bdc cache hooks …` or PyO3), never in `.beads/bd-cache.toml`, which may be checked in. A cloned repository therefore cannot make `bdc` run code. Registration names an interpreter and a `module:function`, and is a trusted, explicit act by the consumer. |

## Lint review (S82 adjustments)

The draft is sound. The rules are case-insensitive, there are no inline pragmas,
and the exemptions are fixed in the script and scoped to a section, not a whole
file (`EXEMPT_SECTIONS`). Its 20 tests use deterministic temporary repos.

Adjustments, made on this plan branch:

- **L1.** (Withdrawn by Rand.) `docs/team-protocol.md` stays where it is, and
  stays exempt: it is the protocol for the team working on this repo.
- **L2.** Add a narrow rule for hard-coded scope prefixes: a `"phase-"` literal or a
  `phase-{` format string. Test a hit, and test a non-hit for the frozen column
  name `phase`, `DEFAULT 'phase'` and `phase-b` in prose.
- **L3.** Fix `sc-beads`, `sc-bd-cli` and `sc-cron` like any other crate. Their edits
  are to comments, provenance and tests. They take no version bump (the lockstep
  0.2.0 is already set, S61).
- **L4.** Scan tracked file paths as well as file contents. `atm_core_05831df_*`,
  `atm_process.rs` and `.atm-bd/` are path hits.
- **L5.** Add `cron[@_-]tool` variants to `tool-principal`. Add `--paths <prefix>...`,
  so a sprint can lint its own fence. State in the ADR that strings built at
  runtime cannot be caught; review guards against those.

`.claude/` and the other agent tooling are deliberately out of scope: the repo's
own agents coordinate over ATM. If a legitimate neutral term hits, narrow the
regex and add a test. Never add an exemption.

## Tick, sync and post-sync hook (S83, S87, S88)

### Scheduling

- The cache's cron job lives in the `sc-cron` sqlite cron table inside the cache
  file. It is managed with `bdc cache schedule set <cron-expr> [--budget-ms N]`,
  `schedule show` and `schedule clear`, and through PyO3 (`Cache.schedule_set`
  and the others).
- Something must run due jobs. That is `bdc cache cron run`, a long-running worker
  built on `sc-cron`'s `Scheduler` with single-flight and `MissedTickPolicy::Skip`,
  or `bdc cache cron run --once`, which runs whatever is due and exits, for an OS
  scheduler. `bdc cache tick` runs one tick now, whatever the schedule says.

### One tick (deadline = start + budget_ms; default 25000)

1. **Drain** the journal, bounded by the remaining budget minus the hook reserve.
2. **Sync** each managed scope, bounded by the remaining budget minus the hook
   reserve. Each scope is synced by its method:
   - `builtin`, the default: `bd list` / `bd blocked`, as today.
   - `python`: the registered `sync` method is called once, with every python-managed
     scope. It ingests through `Cache.ingest_scope`. Any scope it neither ingests
     nor reports as failed is recorded as failed, `TICK_SYNC_MISSING`.
   - `external`: the tick does not sync the scope. It is fed only by the consumer's
     own `ingest_scope` calls, so the tick never overwrites rows the consumer pushed.

   The method is set per scope with `bdc cache scopes set <scope> --method builtin|python|external`.
   This is the per-scope switch that disables the built-in sync.
3. **Post-sync hook**, if one is registered, gets the full remaining budget. That
   is at least `hook_reserve_ms` (default 5000), which steps 1–2 cannot consume, so
   escalation still runs during an outage.
4. **Reply:** one envelope recording each step's result.

### Calling a Python method

`bdc` is a Rust binary, so it runs a registered method as
`<interpreter> -m bd_cache_rs.run <kind> <module:function>`. It writes the request
JSON on stdin, concurrently with reading the output. The process runs in the
cache's working directory with a minimal environment: PATH, HOME, the locale,
`BEADS_DIR` and the variables named in the registration's allowlist. It runs in
its own process group, using sc-bd-cli's bounded runner, so it is killed at the
deadline on all three OSes. b-6 adds a Windows test. Stdout and stderr are each
capped at 1 MiB, and stderr is kept in the reply truncated to 8 KiB. Request
bodies are never logged.

`bd_cache_rs.run` (b-9) does the following:
- opens a `Cache` on the same file;
- imports the function and calls `func(cache, request)`;
- prints `{"protocol": "bd-cache.run.v1", "ok": true, "result": <json or null>}`,
  or the error with its traceback summary, and exits 0, or 1 on error.

A consumer process that itself runs the tick through PyO3 (`Cache.tick()`) calls
the registered methods in-process, with the same request and result shapes.

### Request shapes (`bd-cache.run.v1`)

Every request carries `protocol`, `kind`, `tick_id`, `now`, `deadline` and
`cache_path`.

`sync`:

```json
{"kind": "sync", "bd": {"binary": "...", "project": "...", "dolt_mode": "server|embedded"},
 "scopes": [{"scope": "...", "last_ok_at": "...", "revision": "<opaque, echoed from the last ingest>"}]}
```

`post_sync`:

```json
{"kind": "post_sync",
 "drain": {"applied": 0, "escalated": 0, "error": null},
 "scopes": [{"scope": "...", "method": "builtin|python|external",
             "result": "ok|failed|outage|skipped|missing",
             "error": {"code": "...", "message": "...", "outage": true},
             "failing_since": "<rfc3339 or null>", "notified_since": "<rfc3339 or null>"}],
 "escalated": [{"seq": 1, "bead": "...", "op": "...", "actor": "...", "attempts": 3,
                "error": {"code": "...", "message": "..."}, "created_at": "...",
                "first_offered_at": "..."}],
 "pending": {"count": 0, "oldest_age_s": null}}
```

- `escalated` lists only entries that are escalated and not yet marked notified.
- A scope needs notifying when `failing_since` is set and `notified_since` differs from it.

### Marking (S88)

The hook marks through the cache API. These are the same calls the CLI exposes:

- `Cache.journal_announce(seq)` / `bdc cache journal announce <seq>`, the existing call.
  It refuses an entry that is not escalated (`PRECONDITION`).
- `Cache.scope_announce(scope, since)` / `bdc cache scopes announce <scope> --since <ts>`, new.
  It is compare-and-set: it marks only if `since` equals the scope's current
  `failing_since`; otherwise `PRECONDITION`. A mark meant for an old outage
  therefore can never silence a new one.

Delivery is at-least-once. A hook that sends, then crashes or times out, is offered
the same items on the next tick, with `first_offered_at` unchanged, so it can
deduplicate. Hooks must be idempotent.

### Ingest (S87)

`Cache.ingest_scope(scope, rows, blocked, revision=None, allow_empty=False)` and
`bdc cache scopes ingest <scope>` (JSON on stdin):

- Every row passes the strict `sc-beads` row decoder, and its labels must include
  the scope (the same membership rule as `bd list --label`).
- `blocked` rows must be a subset of `rows`.
- An empty snapshot is refused unless `allow_empty` is set.
- At most 50 000 rows and 64 MiB per call.
- Any violation rejects the whole call with `INGEST_INVALID`, and the stored scope
  is untouched.
- An accepted call is one transaction: it is a complete snapshot of the scope, so
  a bead not listed leaves the scope. A bead that is also in another scope keeps
  its other membership, exactly as in the built-in sync. The row is replaced, and
  the newest snapshot wins.
- `revision` is opaque and stored per scope. It is echoed in the next `sync` request,
  so a method can carry state from tick to tick.

### New codes

| Code | Kind | CLI exit | Raised by |
|---|---|---|---|
| `TICK_SYNC_MISSING` | sync failure (per scope, in the reply) | 0 (the tick ran) | the tick, for a python scope the method neither ingested nor failed |
| `TICK_RUN_FAILED` | hook/method failure (in the reply) | 0 | the tick, on spawn failure, timeout, exit 1, malformed output or a wrong protocol |
| `INGEST_INVALID` | `BadArgs` | 3 | `ingest_scope` / `scopes ingest` |
| `REGISTRATION_INVALID` | `BadArgs` | 3 | `hooks register`: a bad interpreter path, or a function that does not import (checked at registration) |

`details.outage: bool` is added to errors that can come from `bd`. It is true only
when `bd` could not be reached: spawn failure, timeout, lock or transport. A
`BD_LIST_FAILED` from a `bd` that answered has `outage: false`. The 0.1 binding's
existing constants are not changed; the four new codes are added to its exports
in b-9.

### Registration (S89)

- `bdc cache hooks register sync|post_sync --python <interpreter> --function <module:function> [--env NAME]...`,
  plus `hooks show` and `hooks clear`. These are mirrored in PyO3 as `Cache.hooks_register`.
- Registration is stored in the cache file's `registration` table.
- `.beads/bd-cache.toml` cannot register code. A `[tick.hooks]` or `[tick.sync]` key
  there is `CONFIG_INVALID`, and that is tested.

## Sprints

| Sprint | Branch | Closure | Owns (hard fence) | must_follow |
|---|---|---|---|---|
| b-6 tick | `sprint/b-6-tick` | integration (vertical: store API → sync → CLI; one feature across three crates, splitting it would leave an unusable tick) | `crates/bd-cache-store/src/**`, `crates/bd-cache-store/tests/` except `compat*`, `crates/bd-cache/**`, `crates/bd-cache-sync/**`, `crates/bd-cache-model/src/{config,error}.rs` and their tests, `crates/bd-cache-cli/src/{args.rs,config.rs,commands/tick.rs,commands/mod.rs}` and new `commands/{schedule,cron,hooks,scopes}.rs` with their tests, `docs/bd-cache-sync/**`, `docs/bd-cache-store/**`, `docs/bd-cache/**` | — |
| b-7 neutral tests and crates | `sprint/b-7-neutral` | boundary | `crates/bd-cache-store/tests/compat*` (removal), `crates/bd-cache-py/tests/**` and `crates/bd-cache-py/{README.md,src/lib.rs}` (comments and tests only), `crates/sc-*/**` (comments, provenance and tests only), `docs/sc-*/**`, `docs/bd-cache-py/**`, `scripts/bd-compat/**`, `.github/workflows/{python.yml,bd-compat.yml}` | — |
| b-8 docs and API | `fix/fen-api` (PR #86) | boundary | `crates/bd-cache-api/**`, `crates/bd-cache-cli/src/commands/manage.rs` and its tests, `docs/architecture.md` (except the ADR-BDC-0010 section), `docs/requirements.md`, `docs/bd-cache-cli/**`, `docs/bd-cache-api/**`, `docs/baseline/**`, `docs/research/**`, `docs/plans/phase-b/phase-b-plan.md`, `README.md`, `CHANGELOG.md` | b-6 (for the contract text only) |
| b-9 Python surface | `sprint/b-9-py` | boundary | `crates/bd-cache-py/src/**` except the comment edits b-7 made; `crates/bd-cache-py/{pyproject.toml,python/**}` (the mixed layout for `bd_cache_rs.run`); the new pytest files | b-6, b-7 |
| b-10 lint gate | `lint/no-atm-boundary` | integration | `scripts/lint_no_atm.py` and its tests, the ADR-BDC-0010 section, `.github/workflows/ci.yml`, and any residual hit | b-6, b-7, b-8, b-9 |

Wave 1: b-6, b-7 and b-8 (b-8's code and doc work starts at once; its contract text
follows b-6's merged names). Wave 2: b-9. Wave 3: b-10. Critical path:
b-6 → b-9 → b-10.

Shared-file rules:
- **`Cargo.lock`:** on a conflict, take the target branch's lock and run `cargo metadata`.
  Never run `cargo update`.
- **Root `Cargo.toml`:** no sprint adds a crate. Dev-dependencies go in each crate's
  own `Cargo.toml`.
- **`ci.yml`:** owned by b-10 only. b-7 removes compat jobs only from the workflows it owns.
- **The 0.2.0 version:** unchanged.

Every sprint shares these acceptance criteria:
- **AC-0:** `python3 scripts/lint_no_atm.py --paths <the sprint's fence>` exits 0.
- **AC-Q:** S57 QA PASS and green CI on the three OSes.
- **AC-N:** a planted negative for every behaviour change, restored from an
  in-memory copy.

### b-6 tick

REQs: REQ-BDC-0720..0729 (new; b-8 writes the text from this section) supersede
REQ-BDC-0701..0707. ADRs: ADR-SYNC-0008 (the tick, schedule, methods, hook,
registration and trust; it supersedes ADR-SYNC-0007 and its a-7-3 and a-7-5
amendments). ADR-STORE-0005 covers the extension columns and table.

Delete: `crates/bd-cache-sync/src/{atm.rs,phases.rs}`, the consumer parts of
`announce.rs`, `active.rs` and the stop rule, `tests/{atm_process.rs,stop_rule.rs}`,
`[tick] team`, `fallback_lead`, `phases_dir` and `phases`, the `refused` /
`pass-through` replies, and the `.atm-bd/` refusal in `bd-cache-cli/src/config.rs`.

Acceptance:
1. `grep -rn` of the delete list finds nothing, and AC-0 passes.
2. Schedule:
   - `schedule set/show/clear` round-trip through the `sc-cron` table in the cache file.
   - `cron run --once` runs a due tick, and skips one that is not due. This is
     tested with sc-cron's ManualClock.
   - Single-flight: a second worker on the same file runs no overlapping tick.
3. Methods:
   - `builtin` syncs as today.
   - `python` calls the method once with every python scope. A scope it neither
     ingests nor fails is `TICK_SYNC_MISSING`.
   - `external` is never synced, and a pushed snapshot survives a tick.
   - Each case is covered by a test-support Python-free stand-in. Tests drive the
     runner seam with a Rust fake, so no interpreter is needed in b-6. b-9 adds the
     real interpreter test.
4. Budget:
   - When the sync consumes everything, the hook still runs with at least
     `hook_reserve_ms`.
   - A method that times out is killed (causal, through the shared `gate_file`
     helper in `bd-cache-cli/tests/support`), and the tick still finishes within
     the deadline.
5. Hook:
   - The post-sync request matches the shape above.
   - The marks work.
   - A stale `scope_announce` is refused.
   - A crash after the side effect re-offers the item, with `first_offered_at`
     unchanged.
   - Spawn failure, exit 1, oversize output, malformed output and the wrong
     protocol each give `TICK_RUN_FAILED`, and leave nothing marked.
6. Ingest:
   - each rule in the Ingest section has a planted negative (foreign scope label,
     blocked not a subset of rows, an empty snapshot, the caps);
   - a bead in two scopes keeps its other membership.
7. Outage truth table: one test per row (spawn failure, timeout, lock, transport,
   and an answered `BD_LIST_FAILED`).
8. Format: the b-6 format golden contains the exact v3 DDL and every extension.
   Opening a v3-only file adds the columns. A file with the columns opened by a
   reader that does not know them still reads.
9. Registration: `.beads/bd-cache.toml` cannot register code (`CONFIG_INVALID`).
   `hooks register` checks the import (`REGISTRATION_INVALID`). The environment is
   limited to the allowlist.
10. `tests/job_threading.rs` `join_idle` is causal.

Does not close: the real-interpreter tests (b-9), the contract text (b-8), the CI lint
step (b-10).

### b-7 neutral tests and crates

Delete: `crates/bd-cache-store/tests/compat/`, `crates/bd-cache-store/tests/compat.rs`,
`crates/bd-cache-py/tests/atm_core_05831df_*`, and the compat jobs in
`python.yml` / `bd-compat.yml`. (`docs/baseline/BD-CACHE.md` is b-8's to delete.)

Acceptance:
1. AC-0 passes on the fence.
2. The `atm_core_05831df_*` corpus is replaced by a neutral corpus that exercises
   every 0.1 verb and reply shape. It is generated by a script in the repo, against
   the fake bd. The test count does not drop: the old and new verb coverage lists
   are compared in the PR.
3. `tests/test_additive.py:82-88` is causal: the fake bd waits on a gate, with no
   wall-clock bound.
4. The binding docs cover:
   - its own API, with no mapping from another implementation;
   - that there is no runner injection (wrap the binary through `bd_binary`);
   - that a detached `bdc cache flush` must log its output;
   - one handle per process.

Does not close: the format golden (b-6), the new Python methods (b-9).

### b-8 docs and API (on PR #86)

Acceptance:
1. #86's health and `--timeout-ms` work are kept and grep clean (AC-0 on
   `crates/bd-cache-api` and `manage.rs`). `docs/bd-cache-cli/cutover-runbook.md`
   is deleted.
2. Contract:
   - §10 becomes "0.1 → 0.2 API differences", with no consumer named: `outage` on
     Ok, the `issues_list` default exclusions, optional `changed` / `already_closed`,
     one handle per process, install, and `cache_path` for an existing file.
   - It adds `schedule`, `cron run`, `scopes set/announce/ingest`, `hooks`, the
     `bd-cache.run.v1` shapes and the new-codes table, under b-6's merged names.
3. `docs/baseline/BD-CACHE.md` is deleted; b-6's format golden is the reference.
   `docs/requirements.md` supersedes REQ-BDC-0701..0707 and adds REQ-BDC-0720..0729.
   `docs/architecture.md` ADR-BDC-0006 states sc-cron's provenance neutrally, and
   ADR-BDC-0011 records the S87/S89 seam:
   - Alternatives considered: a subprocess protocol returning rows (rejected: two
     validation paths), and the consumer pushing only (kept, as `external`).
   - Why a registered method: it fits Rand's goal and carries state through
     `revision`.
4. The phase-b-plan rulings table records S64–S89.
5. AC-0 passes on the fence.

### b-9 Python surface

Acceptance:
1. PyO3 adds:
   - `ingest_scope`, `scope_announce`, `scopes_set`, `schedule_*`, `hooks_register`
     / `show` / `clear` and `tick()`, which runs registered methods in-process;
   - the four new codes in the exports.
2. `python -m bd_cache_rs.run` implements `bd-cache.run.v1`, through the maturin
   mixed layout. The existing `import bd_cache_rs` tests pass unchanged.
3. A real-interpreter end-to-end test drives a real `bdc cache tick` against the fake
   bd. It covers:
   - a registered `sync` method ingesting one scope and failing another;
   - the `post_sync` hook receiving both;
   - marking one escalation and the failed scope;
   - a second tick re-offering nothing marked and everything not marked.
4. pytest runs with `-W error::DeprecationWarning`, and AC-0 passes.

### b-10 lint gate

Acceptance:
1. All four sprints are merged in. `python3 scripts/lint_no_atm.py` exits 0 on the
   whole tree.
2. ADR-BDC-0010 is Approved (S82).
3. The CI step "Strict consumer boundary (ADR-BDC-0010)" is green on the three OSes.

## Disposition of the consumer's review findings

| Finding | Disposition |
|---|---|
| B1 sender identity, B3 message text, I5 cron shim | Superseded: the cache sends nothing. Escalation goes through the post-sync hook (S88). The resolve hint names `cache journal resolve`. |
| B2 legacy-path refusal | Superseded. The refusal is removed (b-6); the consumer passes `cache_path`. |
| B4 release and runbook | The install and `cache_path` steps go in the neutral §10 (b-8). The consumer's own cutover is the consumer's. The release waits for b-10 and Rand's go. |
| I1, I10, M3 | b-8, contract §10 and §5.3. |
| I2 outage classification | b-6 AC-7. |
| I3, I6, M1 | b-7 AC-4. |
| I4 outage-notified state | b-6: `failing_since` / `notified_since` and the compare-and-set `scope_announce`. |
| I7 health, I10 timeout | Done in #86, kept by b-8 (AC-1). |
| I8 timing tests | latency, harness and process.rs were fixed in #84/#85. job_threading is b-6 AC-10. test_additive is b-7 AC-3. |
| I9 cross-implementation | The consumer's test, run against the CI wheel artifact before the 0.2.0 go (S84). |
| M2, M4 | b-8, a neutral contract note. |
| M5 | Upstream (sc-runtime). |
| M6 | No change: an oversized key fails safely as BAD_ARGS. |

## Review disposition

This answers the critical plan review of r1.

| Review item | Resolution in r2 |
|---|---|
| B1 notified column has no owner | The store src, API, extensions and golden all move into b-6's fence. b-7 only deletes the compat suite. |
| B2 no `since` | `failing_since` and `notified_since` are stored columns. `scope_announce` is compare-and-set on `since`. |
| B3 ingest and verb wiring cross fences | b-6 owns `crates/bd-cache` and new CLI command files, and does not touch `manage.rs`. All PyO3 additions are b-9 (after b-6/b-7). |
| B4 no ACs | Per-sprint acceptance, deletions, "does not close", and AC-0 (lint on the fence). |
| I1 marking order and delivery | Marks go through the API, inside the hook. Delivery is at-least-once, with `first_offered_at`. There is one hook, so there is no cross-hook ordering. |
| I2 mark validation | `journal_announce` refuses a non-escalated entry. `scope_announce` is compare-and-set. |
| I3 budget | The deadline nesting is stated, with a `hook_reserve_ms` floor and a test. |
| I4 process, IO and trust | Minimal environment, caps, concurrent IO, sc-bd-cli's bounded runner on all three OSes, a Windows test, and registration only in the cache file (S89). |
| I5 out-of-scope rows | Label membership, the `blocked` subset rule, `allow_empty` and caps, each with a planted negative. |
| I6 ADR and double mechanism | ADR-BDC-0011 (b-8) and ADR-SYNC-0008 (b-6). The `external` method stops the tick from clobbering pushed scopes. `revision` carries state between ticks. |
| I7 codes | The new-codes table above, with the owners: b-6 (Rust), b-9 (py), b-8 (contract). |
| I8 frozen format | S84 is amended. The migration goes through the existing extension mechanism, with tests (b-6 AC-8). The CI wheel artifact is the consumer's handoff. |
| I9 lockfile | The shared-file rules above. |
| I10 dropped findings | ACs named in the consumer-findings table. |
| I11 S87 not recorded | b-8 AC-4 records S64–S89. |
| I12 compat CI references | b-7 removes them from its workflows. `ci.yml` is b-10's. |
| Lint coverage | L1 withdrawn (team-protocol stays exempt). L4 (paths), L5 (`cron` variants, `--paths`), runtime-string limits in the ADR, and the frozen names tested as non-hits (L2). Linting runs per sprint through AC-0. |
| M1–M4 | Scopes are any bd label, and `kind 'phase'` is frozen (S84). `revision` is opaque. The critical path is stated. The shared `gate_file` helper is named. |

## QA

Every sprint goes through the S57 QA gate and needs green CI. A planted negative
is required for every behaviour change.
