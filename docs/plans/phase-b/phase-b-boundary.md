# Phase B, boundary extension: no consumer concerns in beads-cache

Status: r3, approved by lead ruling S82 (bc-lead, 2026-10-25, recording Rand's rulings),
restored unchanged from `b1a4b11` except for the [Amendments](#amendments-after-r3) Rand
ruled after r3 (each marked in the text) and the [Sprints](#sprints) mapping. r3's own
sprints are kept as work packages WP-6a, WP-6b, WP-7, WP-8, WP-9 and WP-10; sprints b-7,
b-8 and b-9 deliver them (b-6, sc-cron, was added by Amendment A2 and is merged). Phase B
does not close, and PR #82 (integrate/phase-b → develop) does not merge, until b-9 is green.

## Why

The consumer's phase-B review (by the first consumer's maintainer) found the cron and
cutover path blocking. While answering it, Rand ruled:

- The boundary is strict: beads-cache has no concerns or patterns that match the first
  consumer (Rand, 2026-10-25, paraphrased).
- "beads-cache is a cache w/ cli for configuration. it also exposes python api
  through py03 for you to use."
- The beads-cache cron must support running a registered Python script that is part of
  the consumer's package. The consumer would include beads-cache as a dependency for the
  plug-in, and it would be consumed both by Python hooks and by direct reads from a TUI
  (Rand, 2026-10-25, paraphrased).
- The consumer's principal names and its other strings are blocked from source and tests
  (Rand, 2026-10-25, paraphrased).

Phases a and b ported the consumer's cron into `bd-cache-sync`. The tick sends
messages, reads the consumer's team and lead configuration and its phase files,
mirrors the consumer scheduler's stop rule and reply shape, and syncs only
`phase-<x>` scopes. The test suites vendor the consumer's implementation. The
consumer's lint, at its handoff, reports 899 hits in 112 files.


Rand later restated the goals (Rand, 2026-10-25, paraphrased): long term, beads-cache
does the sync at a time scheduled in the sqlite cron table. If the hard-coded sync does
not work, it can be disabled and replaced with a Python method the consumer registers.
A post-sync hook would likely receive the sync results and errors and let the consumer
do its own things at that point, such as sending an escalation.

This revision (r3) follows those goals and the critical plan reviews of r1 and r2
(r1: 4 blocking, 12 important; r2: 4 blocking, 9 important). Every item is answered in
[Review disposition](#review-disposition).

## Lead rulings

| Ruling | Decision |
|---|---|
| S82 | ADR-BDC-0010 (the first consumer's maintainer's draft @ 33a2b07) is approved, with the lint adjustments L2–L5 below (L1 is withdrawn). The lint script and its tests, including `--paths`, merge into `integrate/phase-b` with this plan (PR #87, bc-lead owns the merge), before WP-6a, WP-7 and WP-8 start, so every sprint gates on AC-0. The CI step lands in WP-10, once the tree is clean. It is never merged red. |
| S83 | beads-cache owns the bd sync. The schedule is a job in the `sc-cron` sqlite cron table (`SqliteScheduleStore`), stored in the cache file and set with `bdc cache schedule set`. A due job runs one tick in this order: drain, then sync each managed scope, then the post-sync hook. The cache itself sends nothing, and there is no consumer-specific stop rule. The first consumer is not the only one. |
| S84 | The store format is frozen as v3 plus the additive extensions listed in ADR-STORE-0006 and WP-6a's format golden. Extensions are added only by ruling, through the existing extension mechanism: `PRAGMA table_info` / `CREATE TABLE IF NOT EXISTS` under `BEGIN IMMEDIATE`; `user_version` stays 3. Existing names stay (`phase`, `announced_at`, `scope.kind DEFAULT 'phase'`), and the API names them neutrally. The full extension list for this phase is in [Store extensions](#store-extensions). A reader that does not know an extension still reads the file. Cross-implementation compatibility is the consumer's own test, run against a CI-built wheel artifact before Rand's 0.2.0 go. |
| S85 | PR #86 (health, `refresh scopes --timeout-ms`) is kept, and becomes WP-8's branch. Its cutover runbook and the consumer-specific contract text are removed. |
| S86 | The stopped layers `fix/fen-tick` and `fix/fen-py` are abandoned unmerged. Their neutral parts are redone in WP-6, WP-7 and WP-9. |
| S87 | The built-in ("hard-coded") sync can be disabled per scope and replaced with a Python method that the consumer registers. The method feeds the cache only through `Cache.ingest_scope`, which applies the same strict decoder and per-scope transaction as the built-in sync, so a method cannot bypass validation. |
| S88 | The post-sync hook is one registered Python method. It is called after the sync with the tick's results and errors and the cache's notification state, and marks what it handled through the cache API. Delivery is at-least-once: anything not marked is offered again on the next tick. The tick itself marks nothing. |
| S89 | Python methods are registered in the cache file (the `registration` table, through `bdc cache hooks …` or PyO3), never in `.beads/bd-cache.toml`, which may be checked in. Registration records an absolute interpreter path and a `module:function`. Each method runs as `<interpreter> -I -m bd_cache_rs.run`, with the cwd set to a private temp dir, so neither the project directory nor `PYTHON*` variables can inject modules. The tick refuses to run registered code from a cache file that is not owned by the current user, or that is group- or world-writable. Windows ACL checks are a documented gap. |
| S90 | One execution path: `bdc cache tick`, `cron run`, `cron run --once` and PyO3 `Cache.tick()` all run the same Rust tick, and it always runs registered methods as subprocesses. There is no in-process Python method call. `Cache.tick()` releases the GIL. Ticks are single-flight across processes through a lease row in the cache file. |

## Amendments after r3

Rand's rulings after r3 (2026-10-25), quoted. Where one changes r3, the text is marked
*Amended (Ax)*. Every deliverable cites the REQ or ADR that justifies it (A6).

| Id | Ruling (verbatim) | Effect on r3 |
|---|---|---|
| A1 | "cron.tick and cron.stop are not banned." | The lint does not flag `cron.tick` or `cron.stop`. |
| A2 | "I expect 'sc-cron' to be a general tool that can be used for scheduling jobs." and "should retain that general purpose shape for porting (copying) to sc-runtime. NOTE: cron table should have an 'enable' bool flag so scheduled tasks can be enabled/disabled w/out needing to rewrite the entire row." and the stop hook returns "'disable', 'delete', 'continue'". | Sprint b-6 (merged, #92): sc-cron gains a Python hook target (`<interpreter> -I -m <module>`, JSON on stdin, process-group or job-object kill, output caps), an `enabled` column, `continue`/`disable`/`delete` stop decisions and five-field expressions. r3's "sc-cron itself is not changed" no longer holds; the tick lease is still the tick's own. WP-6b and WP-9 run every registered method through sc-cron's Python runner, with module `bd_cache_rs.run`. |
| A3 | "limiting 'builtin' sync to one per repo makes sense." | At most one tick row per repository in the cache file's cron table; `schedule set` replaces it, and other sc-cron rows are allowed. |
| A4 | "sync row's hook target switches - can't this be handled through cron scheduling? i.e. built-in is disabled, python hook is enabled. (nothing for you to do outside basic sc-cron design)" | `bdc cache schedule enable` / `disable` flip only the tick row's `enabled` flag. A consumer moves scopes to its Python method with `scopes set <scope> --method python` (§Scopes). Decision D1 records the choice. |
| A5 | Active phases would need to be defined with filters: 'phase' is the first consumer's concept, and other applications would have different scopes, including --all (Rand, 2026-10-25, paraphrased). | A scope is a name plus a bd filter: labels (all or any), status, type, assignee, or `--all` (§Scopes). |
| A6 | "Add an anchor for only allowing code that is justified by req/adr to keep project tight." | Each sprint deliverable cites its REQ or ADR (anchor ANC-011). |

**D2 (lead default; Rand may overrule).** From the sc-cron generality review: registered
methods do not run through sc-cron's schedule hook. sc-cron offers a protocol-free
supervised-process call (REQ-CRON-0016, ADR-CRON-0009), and bd-cache-sync builds
`bd-cache.run.v1`, the `bd_cache_rs.run` argv, the environment it passes and its caps on top of
it. This amends A2's "WP-6b and WP-9 run every registered method through sc-cron's Python
runner" and §Calling a Python method's runner note.

**D1 (lead default; Rand may overrule).** r3's design stays: one tick runs the built-in sync
for `builtin` scopes, calls the registered `sync` method once for the `python` scopes, then the
`post_sync` method (S87–S90), all as sc-cron Python hooks. The alternative, a separate Python
sc-cron row replacing the tick, is not taken: it would lose the per-scope choice, the shared
budget with the hook reserve and the single lease.

## Lint review (S82 adjustments)

The draft is sound. The rules are case-insensitive, there are no inline pragmas,
and the exemptions are fixed in the script and scoped to a section, not a whole
file (`EXEMPT_SECTIONS`). Its 20 tests use deterministic temporary repos.

Adjustments, made on this plan branch:

- **L1.** (Withdrawn by Rand.) `docs/team-protocol.md` stays where it is, and
  stays exempt: it is the protocol for the team working on this repo.
- **L2.** Add a narrow rule for hard-coded scope prefixes: a string literal that is the
  phase prefix with its dash, or a format string that appends a value to it. Test a hit, and test a non-hit for the frozen column
  name `phase`, `DEFAULT 'phase'` and `phase-b` in prose.
- **L3.** Fix `sc-beads`, `sc-bd-cli` and `sc-cron` like any other crate. Their edits
  are to comments, provenance and tests. They take no version bump (the lockstep
  0.2.0 is already set, S61).
- **L4.** Scan tracked file paths as well as file contents. The vendored call-log tests,
  the sender's process test and the first consumer's own directory are path hits.
- **L5.** Add `cron[@_-]tool` variants to `tool-principal`. Add `--paths <prefix>...`,
  so a sprint can lint its own fence. State in the ADR that strings built at
  runtime cannot be caught; review guards against those.

`.claude/` and the other agent tooling are deliberately out of scope: the repo's
own agents use it to coordinate. *Amended (A1):* `cron.tick` and `cron.stop` are not
flagged. If a legitimate neutral term hits, narrow the
regex and add a test. Never add an exemption.

## Tick, sync and post-sync hook (S83, S87, S88, S90)

### Scopes

- The scope registry is the existing `scope` table, with a new `method` column
  (`builtin` by default) and a per-scope `revision`.
- `bdc cache scopes add <scope> [--method builtin|python|external]`, `scopes set <scope>
  --method …`, `scopes remove <scope>` and `scopes list` manage it. PyO3 mirrors them.
- *Amended (A5):* a scope is a name plus a bd filter: labels (all or any), status,
  type, assignee, or `--all`. The built-in sync runs `bd list` with that filter.
- Removing a scope deletes its membership rows. It does not delete bead rows that
  another scope still holds.
- A tick with no scopes still drains and still runs the hook.

### Scheduling

- The cache's cron job lives in the `sc-cron` sqlite tables, inside the cache file.
  It is managed with `bdc cache schedule set <cron-expr> [--budget-ms N]
  [--hook-reserve-ms N]`, `schedule show`, `schedule enable`, `schedule disable` and
  `schedule clear`. *Amended (A3, A4):* there is at most one tick row per repository, and
  `enable` / `disable` flip only sc-cron's `enabled` column on it.
  - `--budget-ms` defaults to 25000 (range 1000–600000).
  - `--hook-reserve-ms` defaults to 5000 (range 0 to budget−1000).
- Entry points: `bdc cache cron run` (a long-running worker on `sc-cron`'s
  `Scheduler`, `MissedTickPolicy::Skip`), `bdc cache cron run --once` (runs what
  is due and exits, for an OS scheduler), `bdc cache tick` (one tick now), and
  PyO3 `Cache.tick()`. All four run the same Rust tick function.

### Single flight (S90)

- The tick takes a lease row (`tick_lease`: holder pid, host, `tick_id`, expiry)
  inside `BEGIN IMMEDIATE`.
- The expiry is the deadline plus 10 s of grace.
- A tick that finds a live lease does nothing and replies `skipped: in_flight`.
- An expired lease, such as one left by a holder killed with `kill -9`, is taken
  over.
- The lease is the tick's, not the scheduler's. *Amended (A2):* sc-cron itself was
  changed by sprint b-6, but not for the lease; its generation fencing is unchanged.

### One tick (deadline = start + budget_ms)

1. **Lease.** Taken, or the tick is skipped.
2. **Drain** the journal, bounded by the remaining budget minus the hook reserve.
   The drain lock is released when the drain ends.
3. **Sync**, bounded by the remaining budget minus the hook reserve:
   - `builtin` scopes run `bd list` / `bd blocked`, as today.
   - `python` scopes go to the registered `sync` method, called once with all of
     them.
   - `external` scopes are not synced. Only the consumer's own `ingest_scope` calls
     fill them, so the tick never overwrites rows the consumer pushed.
4. **Post-sync hook**, if one is registered, gets the full remaining budget. That
   is at least `hook_reserve_ms`, which steps 2–3 cannot consume, so escalation
   still runs during an outage.
5. **Reply:** one envelope recording each step. Then the lease is released.

While a method runs, the tick holds no write transaction and no drain lock.
Cross-process handles on one file are allowed (the store refuses only a second
handle in the same process, `STORE_IN_USE`), so the method's own `Cache` handle
reads and writes normally.

### Calling a Python method (S89, S90)

- **The command.** The method runs as `<absolute interpreter> -I -m bd_cache_rs.run
  <kind> <module:function>`, with the cwd set to a fresh private temp dir.
- **Environment.** It is limited to PATH, HOME, the locale, `BEADS_DIR`,
  `BD_CACHE_PATH` (the cache file) and the registration's `--env` allowlist.
- **I/O.** The request goes to stdin, written concurrently with reading the output.
  Stdout and stderr are each capped at 1 MiB, and stderr is kept in the reply
  truncated to 8 KiB.
- **Killing.** It runs in its own process group (Unix) or job object (Windows),
  killed at its deadline.
- *Amended (A2):* the method runs through sc-cron's Python hook runner (b-6), as module
  `bd_cache_rs.run`. b-8 adds what r3 asks that the runner lacks: the environment
  allowlist, an absolute interpreter only, `BD_CACHE_PATH`, and 1 MiB output caps
  (b-6 caps at 64 KiB).
- **Logging.** Request bodies are never logged or echoed.

`bd_cache_rs.run` (WP-9) does the following:
- opens its own `Cache` on `BD_CACHE_PATH`;
- imports the function and calls `func(cache, request)`;
- prints `{"protocol": "bd-cache.run.v1", "ok": true, "result": …}`, or
  `{"ok": false, "error": {"type", "message"}}` with the message truncated to 4 KiB
  and no request data, then exits 0, or 1 on error.

A method may call the read, ingest and announce APIs. It must not flush, drain,
tick or schedule. Those calls raise `PRECONDITION` inside a run.

### Request and result shapes (`bd-cache.run.v1`)

Every request carries `protocol`, `kind`, `tick_id`, `now`, `deadline` and
`cache_path`.

**`sync` request:**

```json
{"kind": "sync", "bd": {"binary": "...", "project": "...", "dolt_mode": "server|embedded"},
 "scopes": [{"scope": "...", "last_ok_at": "...", "revision": "<opaque>"}]}
```

**`sync` result**, the method's return value:

```json
{"scopes": [{"scope": "...", "status": "failed",
             "error": {"code": "...", "message": "...", "outage": true}}]}
```

- A scope counts as ingested when an `ingest_scope` call stamped with this
  `tick_id` (taken from the request) committed for it during the run. That is
  judged by the store, not by what the method claims.
- A python scope that is neither ingested nor listed as failed is
  `TICK_SYNC_MISSING`.
- A method's `outage` is recorded exactly as given.

**`post_sync` request:**

```json
{"kind": "post_sync",
 "drain": {"applied": 0, "escalated": 0, "error": null},
 "scopes": [{"scope": "...", "method": "builtin|python|external",
             "result": "ok|failed|outage|skipped|missing",
             "error": {"code": "...", "message": "...", "outage": true},
             "failing_since": "<opaque token or null>", "notified": false}],
 "escalated": [{"seq": 1, "bead": "...", "op": "...", "actor": "...", "attempts": 3,
                "error": {"code": "...", "message": "..."}, "created_at": "...",
                "first_offered_at": "..."}],
 "pending": {"count": 0, "oldest_age_s": null}}
```

**`post_sync` result:** optional free JSON, recorded truncated to 8 KiB.

- `escalated` lists only entries that are escalated and not yet notified.
- `first_offered_at` is set the first time an entry is offered and never changes
  after that.

### Failure state and marking (S88)

**`failing_since` lifecycle:**
- Set to the time of the first non-ok result after an `ok` (or after the scope is
  added).
- Kept across consecutive failures, including a change between outage and failed.
- Cleared on `ok`.
- It is an opaque token to callers; they round-trip it and never parse it.
- `notified` is true when `notified_since` equals `failing_since`.

**Marking, through the cache API:**
- `Cache.journal_announce(seq)` / `bdc cache journal announce <seq>`: the existing
  call, with its behaviour unchanged.
- `Cache.scope_announce(scope, since)` / `bdc cache scopes announce <scope> --since <token>`,
  new: compare-and-set. It marks only if `since` equals the current `failing_since`;
  otherwise `PRECONDITION`. A mark meant for an old failure therefore never silences
  a new one.

Delivery is at-least-once. Marks a hook makes before it crashes persist. Items it
did not mark are offered again, with `first_offered_at` unchanged. Hooks must be
idempotent.

### Ingest (S87)

`Cache.ingest_scope(scope, rows, blocked, revision=None, allow_empty=False)` and
`bdc cache scopes ingest <scope>` (JSON on stdin). A call made inside a run is
stamped with the run's `tick_id`.

- Every row passes the strict `sc-beads` row decoder.
- Membership is declared by the snapshot: every listed row belongs to the scope.
  For a `builtin` scope the tick's own sync also requires the scope label, as
  `bd list --label` does today. A `python` or `external` scope may define
  membership by any criteria, which is why the override exists.
- `blocked` rows follow the built-in rule exactly. WP-6a pins this rule with a
  regression test that runs a real built-in snapshot (rows and blocked) through
  ingest.
- An empty snapshot is refused unless `allow_empty` is set.
- At most 50 000 rows and 64 MiB per call.
- Any violation rejects the whole call with `INGEST_INVALID`, and the stored scope
  is untouched.
- An accepted call is one transaction and a complete snapshot of the scope.
- A bead that is also in another scope keeps that other membership. The newest
  snapshot wins the row's content.
- `revision` is opaque. It is stored per scope and echoed in the next `sync`
  request, so a method can carry state from tick to tick.

### Store extensions

*Amended (planning PR):* r3 named this ADR ADR-STORE-0005, which already exists (the
store's dependencies); it is ADR-STORE-0006 throughout.

These are the full additions for this phase (ADR-STORE-0006; every one is in the
golden):

| Kind | Name | Default and meaning |
|---|---|---|
| column | `scope.method` TEXT | NULL = `builtin` |
| column | `scope.revision` TEXT | NULL |
| column | `sync_state.failing_since` TEXT | NULL = not failing |
| column | `sync_state.notified_since` TEXT | NULL |
| column | `journal.first_offered_at` TEXT | NULL = never offered (`attempts` already exists) |
| column | `journal.ingest_tick_id` / `scope.last_ingest_tick_id` TEXT | NULL |
| table | `registration` (kind PK, interpreter, function, env_json, registered_at, registered_by) | — |
| table | `tick_lease` (id=1 PK, tick_id, pid, host, expires_at) | — |
| tables | the `sc-cron` `SqliteScheduleStore` schema, as that crate defines it | created on the first `schedule set` |

### New codes

| Code | Kind | CLI exit | Raised by |
|---|---|---|---|
| `TICK_SYNC_MISSING` | per-scope result in the reply | 0 | the tick |
| `TICK_RUN_FAILED` | per-method result in the reply | 0 | the tick: spawn failure, timeout, exit 1, oversize output, malformed output, wrong protocol, untrusted cache file |
| `INGEST_INVALID` | `BadArgs` | 3 | `ingest_scope` / `scopes ingest` |
| `REGISTRATION_INVALID` | `BadArgs` | 3 | `hooks register`: a relative or missing interpreter, or a function that does not import (checked by running the interpreter) |

`details.outage: bool` is added to errors that can come from `bd`. It is true only
when bd could not be reached: spawn failure, timeout, lock or transport. A
`BD_LIST_FAILED` from a bd that answered has `outage: false`. The 0.1 binding's
existing constants are unchanged; the four new codes are added to its exports in
WP-9.

### Registration (S89)

- `bdc cache hooks register sync|post_sync --python <absolute interpreter> --function <module:function> [--env NAME]...`,
  plus `hooks show` and `hooks clear`. PyO3 mirrors them as `Cache.hooks_*`.
- Registration is stored in the cache file's `registration` table.
- `.beads/bd-cache.toml` cannot register code. A `[tick.hooks]` or `[tick.sync]` key
  there is `CONFIG_INVALID`.

## Sprints

r3's work packages are delivered by these sprints, in merge order. Each sprint doc lists its
deliverables with the REQ or ADR behind each (A6).

| Sprint | Delivers | Branch | Closure | must_follow | Reference PRs |
|---|---|---|---|---|---|
| [b-6](sprint-b-6-sc-cron.md) | A2 (sc-cron) | `sprint/b-6-sc-cron` | boundary | — | merged, #92 |
| [b-7](sprint-b-7-store-contract.md) | WP-6a | `sprint/b-7-store-contract` | boundary | b-6 | #93 (stopped b-7 WIP) |
| [b-8](sprint-b-8-tick-python.md) | WP-6b, WP-9, and the b-6 runner fixes A2 needs | `sprint/b-8-tick-python` | integration | b-7 | #93, #94, #95 |
| [b-9](sprint-b-9-boundary-lint.md) | WP-7, WP-8, WP-10 | `sprint/b-9-boundary-lint` | integration | b-8 (merges last) | #91, #86, #96 |

The critical path is b-7 → b-8 → b-9. b-9's code and test work (WP-7, the #86 part of WP-8)
can start beside b-7; its contract text follows b-8's merged names, and its lint step lands last.
Each fence below is the work package's (r3), and a sprint owns the union of its packages' fences.

| Work package | Owns (hard fence, including each crate's `Cargo.toml`) |
|---|---|
| WP-6a store and contract | `crates/bd-cache-store/**` except `tests/compat*`, `crates/bd-cache/**`, `crates/bd-cache-model/**`, `docs/bd-cache-store/**`, `docs/bd-cache/**`, `docs/bd-cache-model/**` |
| WP-6b tick | `crates/bd-cache-sync/**`, `crates/bd-cache-cli/src/{args.rs,config.rs,commands/tick.rs,commands/mod.rs}` and new `commands/{schedule,cron,hooks,scopes}.rs` with their tests, `docs/bd-cache-sync/**`; for the A2 runner fixes, `crates/sc-cron/src/hook.rs`, `crates/sc-cron/src/process.rs` and their tests |
| WP-7 neutral tests and crates | `crates/bd-cache-store/tests/compat*` (removal), `crates/bd-cache-py/tests/**` (the existing files), `crates/sc-*/**` except the A2 runner files (comments, provenance and tests only, with no version bump), `docs/sc-*/**`, `docs/bd-cache-py/**`, `scripts/bd-compat/**`, `.github/workflows/**` except `ci.yml` and the kit's workflows |
| WP-8 docs and API | `crates/bd-cache-api/**`, `crates/bd-cache-cli/src/commands/manage.rs` and its tests, `docs/bd-cache-cli/**`, `docs/bd-cache-api/**`, `docs/baseline/**`, `docs/research/**`, `README.md`, `CHANGELOG.md` |
| WP-9 Python surface and end to end | `crates/bd-cache-py/src/**`, `crates/bd-cache-py/{README.md,pyproject.toml,Cargo.toml,python/**}`, and the new pytest files |
| WP-10 lint gate | `.github/workflows/ci.yml`, and any residual hit, as a last resort only |

*Amended (planning PR):* the register and design text r3 gave WP-8 and WP-10 —
`docs/requirements.md` (REQ-BDC-0701..0707 superseded, REQ-BDC-0720..0729 added,
REQ-BDC-0804 amended), `docs/architecture.md` (ADR-BDC-0010, ADR-BDC-0006 provenance,
ADR-BDC-0011), ADR-STORE-0006 and ADR-SYNC-0008 (Proposed), and the phase-b-plan rulings
table — is in the phase-B planning PR, so the sprints implement an approved register. A sprint
that finds the register wrong raises it with the lead; it never edits it locally.

Shared-file rules:
- On a `Cargo.lock` conflict, take the target branch's lock and run `cargo metadata`.
  Never run `cargo update`.
- No sprint adds a crate or edits the root `Cargo.toml`.
- `ci.yml` is WP-10's alone.
- `docs/plans/**` is exempt from the lint and is the plan branch's.

Common acceptance for every sprint:
- **AC-0:** `python3 scripts/lint_no_atm.py --paths <the sprint's fence>` exits 0.
- **AC-Q:** S57 QA PASS, and green CI on the three OSes.
- **AC-N:** a planted negative for every behaviour change, restored from an
  in-memory copy.

### WP-6a store and contract

ADR: ADR-STORE-0006 (*Amended (planning PR):* written there as Proposed; this work package
implements it and it is marked Approved when the sprint merges) (the extensions). It deletes the consumer parts of
`bd-cache-model` (`sender.rs` and the `[tick]` table's message-routing and phase-file
settings) and the neutralised comments in the store.

Acceptance:
1. Every extension in the table exists. The golden has the exact v3 DDL plus the
   extensions. Opening a v3-only file adds them. A file with them, opened by a
   reader that does not know them, still reads.
2. Store API:
   - `ingest_scope`, with each Ingest rule given a planted negative;
   - the built-in snapshot regression test;
   - the scope registry;
   - `scope_announce` compare-and-set, including a stale `since` refused;
   - the `failing_since` lifecycle, with one test per transition;
   - `first_offered_at` set once;
   - the `tick_lease` take, refusal and expiry takeover;
   - registration read and write.
3. The new codes and `details.outage` are in the model's error table. The outage
   truth table has one test per row: spawn failure, timeout, lock, transport, and
   an answered `BD_LIST_FAILED`.
4. A second process opens the same file and ingests while a first holds a handle
   with no transaction open. The first then reads the result (cross-process
   handles).

### WP-6b tick

ADR: ADR-SYNC-0008 (*Amended (planning PR):* written there as Proposed; this work package
implements it and it is marked Approved when the sprint merges), covering the tick, the schedule, the lease, the methods, the
runner seam and the trust model, and recording the rejected alternatives. It
supersedes ADR-SYNC-0007 and its a-7-3 and a-7-5 amendments.

Deletes:
- in `crates/bd-cache-sync/src`: the consumer messaging module, `phases.rs`, and the
  consumer parts of `announce.rs` and `active.rs`, plus the stop rule;
- the sender's process test and `tests/stop_rule.rs` (the messaging module and the
  process test are the files there whose paths the lint flags);
- the `refused` / `pass-through` replies;
- the refusal of the first consumer's own directory in `bd-cache-cli/src/config.rs`.

Acceptance:
1. `grep -rn` of the delete list finds nothing, and AC-0 passes.
2. Schedule and lease:
   - `schedule set/show/clear` round-trip. `cron run --once` runs a due tick and
     skips one that is not due (sc-cron ManualClock).
   - Two real `bdc` processes: the second replies `skipped: in_flight`.
   - A holder killed with `kill -9` is taken over after expiry.
   - An on-demand `tick` during a worker tick is skipped.
3. Methods, through a Rust test-support runner binary that speaks
   `bd-cache.run.v1` and opens a real second handle:
   - `builtin` syncs as before;
   - `python`: ingested, failed and missing scopes are judged by `tick_id`;
   - `external` is never synced, and pushed rows survive a tick.
4. Runner:
   - kill at the deadline, causal through the shared `gate_file` helper, on Unix
     process groups and in a Windows job-object test;
   - stdout and stderr caps;
   - a large request written concurrently without a deadlock;
   - the environment is limited to the allowlist;
   - the cwd is a temp dir;
   - an untrusted cache file (wrong owner, or group/world-writable) gives
     `TICK_RUN_FAILED` and runs nothing (Unix).
5. Budget: when the sync uses everything, the hook still gets `hook_reserve_ms`. A
   method that times out still lets the tick finish within the deadline.
6. Hook:
   - the request shape;
   - marks made by the runner persist;
   - a crash after marking keeps those marks and re-offers the rest, with
     `first_offered_at` unchanged;
   - each failure gives `TICK_RUN_FAILED`, and the tick itself marks nothing.
7. `.beads/bd-cache.toml` cannot register code (`CONFIG_INVALID`).
8. `tests/job_threading.rs` `join_idle` is causal.

This sprint does not close the real-interpreter behaviour, the
`REGISTRATION_INVALID` import check or the PyO3 surface (WP-9), or the contract text
(WP-8).

### WP-7 neutral tests and crates

Deletes: `crates/bd-cache-store/tests/compat/`, `tests/compat.rs`,
the vendored call-log tests in `crates/bd-cache-py/tests/` (the files whose paths the
lint flags), and the compat jobs in the workflows
it owns.

Acceptance:
1. AC-0 passes.
2. A neutral corpus, generated in-repo against the fake bd, covers every 0.1 verb
   and reply shape. The verb-coverage list before and after is compared in the PR,
   with no loss.
3. `tests/test_additive.py:82-88` is causal: a gate fake bd, and no wall-clock bound.
4. `docs/bd-cache-py/` covers:
   - no runner injection (wrap the binary with `bd_binary`);
   - that a detached `bdc cache flush` must log its output;
   - one handle per process;
   - the binding's own API, with no mapping from another implementation.

### WP-8 docs and API (on PR #86)

Acceptance:
1. #86's health and `--timeout-ms` work are kept. AC-0 passes on its crates.
   `docs/bd-cache-cli/cutover-runbook.md` is deleted (the baseline design document was
   deleted by the planning PR).
2. Contract:
   - §10 becomes "0.1 → 0.2 API differences", naming no consumer. It covers
     `outage` on Ok, the `issues_list` default exclusions, optional `changed` /
     `already_closed`, one handle per process, install, and `cache_path` for an
     existing file.
   - It adds the scopes, schedule, cron, hooks and ingest verbs, the
     `bd-cache.run.v1` shapes and the new-codes table, under WP-6b's merged names.
3. `docs/requirements.md` supersedes REQ-BDC-0701..0707 and adds REQ-BDC-0720..0729,
   from this section.
4. `docs/architecture.md`: ADR-BDC-0006 states sc-cron's provenance neutrally.
   ADR-BDC-0011 is a short decision record pointing at ADR-SYNC-0008 and
   ADR-STORE-0006.
5. The phase-b-plan rulings table records S64–S90.

*Amended (planning PR):* items 3 and 4, and S82–S90 with A1–A6 for item 5, are written in
the phase-B planning PR; WP-8 adds S64–S81 to the rulings table. WP-8 checks them
against the merged names and raises any difference with the lead. The provenance wording already in
those two files (overviews, `Source:` lines, earlier amendment notes) is restated neutrally by
WP-8, with no change of meaning.

### WP-9 Python surface and end to end

Acceptance:
1. PyO3:
   - `ingest_scope`, `scope_announce`, `scopes_*`, `schedule_*`, `hooks_*` and
     `tick()`, which releases the GIL and spawns runners;
   - the four new codes in the exports;
   - `run`-context guards, so `flush`, `tick` and `schedule` raise `PRECONDITION`.
2. `python -m bd_cache_rs.run` implements `bd-cache.run.v1`, using the maturin mixed
   layout. The existing `import bd_cache_rs` tests pass unchanged.
3. `REGISTRATION_INVALID`: a relative interpreter, and a function that does not
   import. A hostile `bd_cache_rs.py` or a shadowing module in the project dir is
   not imported (`-I`, temp cwd).
4. A real-interpreter end-to-end test runs a real `bdc cache tick` against the fake bd:
   - the registered `sync` method ingests one scope and fails another;
   - `post_sync` receives both, and marks one escalation and the failed scope;
   - on the next tick, marked items are not offered again, and unmarked items are
     offered again with `first_offered_at` unchanged.
5. pytest passes with `-W error::DeprecationWarning`, and AC-0 passes.

### WP-10 lint gate

Acceptance:
1. All sprints are merged. `python3 scripts/lint_no_atm.py` exits 0 on the whole
   tree, and any residual fix is listed in the PR.
2. The CI step "Strict consumer boundary (ADR-BDC-0010)" is green on the three OSes.

## Disposition of the consumer's review findings

| Finding | Disposition |
|---|---|
| B1 sender identity, B3 message text, I5 cron shim | Superseded. The cache sends nothing; escalation goes through the post-sync hook (S88). The resolve hint names `cache journal resolve`. |
| B2 legacy-path refusal | Superseded. The refusal is removed (WP-6b), and the consumer passes `cache_path`. |
| B4 release and runbook | The install and `cache_path` steps go in the neutral §10 (WP-8); the consumer's own cutover is the consumer's. The release waits for WP-10 and Rand's go. |
| I1, I10, M3 | WP-8, in the contract. |
| I2 outage classification | WP-6a AC-3. |
| I3, I6, M1 | WP-7 AC-4. |
| I4 outage-notified state | WP-6a: `failing_since`, `notified_since` and the compare-and-set mark. |
| I7 health, I10 timeout | Done in #86, and kept by WP-8 (AC-1). |
| I8 timing tests | latency, harness and process.rs were fixed earlier. join_idle is WP-6b AC-8. test_additive is WP-7 AC-3. |
| I9 cross-implementation | The consumer's test, against the CI wheel artifact (S84). |
| M2, M4 | WP-8, a neutral note. |
| M5 | Upstream. |
| M6 | No change. |

## Review disposition

### r1 review (resolved in r2, carried forward)

| r1 item | Where it is resolved |
|---|---|
| B1 | WP-6a owns the store. |
| B2 | the `failing_since` lifecycle. |
| B3 | WP-6a/WP-6b own `crates/bd-cache` and the new command files, and all PyO3 is in WP-9. |
| B4 | the ACs. |
| I1, I2 | §Failure state and marking. |
| I3 | the budget and `hook_reserve_ms`. |
| I4 | §Calling a Python method and S89. |
| I5 | §Ingest. |
| I6 | ADR-SYNC-0008, and the `external` method. |
| I7 | the codes table. |
| I8 | §Store extensions. |
| I9 | the shared-file rules. |
| I10 | the consumer findings table. |
| I11 | WP-8 AC-5. |
| I12 | WP-7 owns its workflows; `ci.yml` is WP-10's. |

### r2 review

| r2 item | Resolution in r3 |
|---|---|
| R1 second store handle | Cross-process handles are allowed (`STORE_IN_USE` is per process). The tick holds no transaction or drain lock while a method runs. WP-6a AC-4 and WP-6b AC-3 use a real second handle. A method may not flush, drain or tick. |
| R2 in-process `Cache.tick()` | Removed (S90). `Cache.tick()` spawns the same runner, with the GIL released. |
| R3 cross-process single flight | The `tick_lease` row, used by all four entry points, replying `skipped: in_flight`. WP-6b AC-2 covers two processes, `kill -9` and the on-demand case. `sc-cron` is unchanged. |
| R4 trust | `-I`, a temp cwd, an absolute interpreter, and a refusal on an untrusted cache file (S89). WP-6b AC-4 and WP-9 AC-3 are the negatives. The Windows ACL check is a documented gap. |
| I1 extension list | §Store extensions, complete, and in the golden. |
| I2 managed scope | §Scopes: the registry, its verbs, and an empty set. |
| I3 sync result | The result schema. Ingestion is judged by the `tick_id` stamp in the store. |
| I4 membership rule | Membership is declared by the snapshot for python and external scopes; the label rule applies only to the built-in sync. The regression test is in WP-6a. |
| I5 WP-6 size and closure | Split into WP-6a and WP-6b. WP-9 is the integration sprint and owns the real-interpreter ACs. |
| I6 fences | `crates/bd-cache-py/src` and the README are WP-9's alone. Each `Cargo.toml` follows its `src`. The model and other workflows are assigned. `crates/bd-cache` (where `Cache` lives) is WP-6a's, so there is no collision with WP-8. |
| I7 marking | "The tick itself marks nothing". The `failing_since` lifecycle. `since` is opaque. `journal_announce` is unchanged, with no new refusal. |
| I8 lint ownership | The script lands with this plan (S82). WP-10 owns only the ADR section, the CI step and residual hits. |
| I9 ADR, tests, error output | The seam is in ADR-SYNC-0008 (WP-6b); ADR-BDC-0011 only points to it. Windows, caps, IO and environment ACs are in WP-6b AC-4. Runner error output is truncated, with no request data. |
| M1, M2 | WP-9 AC-4 is reworded. The budget and reserve defaults and ranges are in §Scheduling. |

## QA

Every sprint goes through the S57 QA gate and needs green CI. A planted negative
is required for every behaviour change.
