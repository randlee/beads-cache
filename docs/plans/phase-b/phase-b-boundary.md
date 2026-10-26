# Phase B, boundary extension: no consumer concerns in beads-cache

Status: r3, approved by lead ruling S82 (bc-lead, 2026-10-25, recording Rand's rulings),
restored unchanged from `b1a4b11` except for the [Amendments](#amendments-after-r3) Rand
ruled after r3 (A1–A8, each marked in the text) and the [Sprints](#sprints) mapping. The final
design is the register, Approved by Rand (2026-10-26): ADR-BDC-0011..0015 and the ADRs they name
(ADR-STORE-0006, ADR-SYNC-0008, ADR-CRON-0009, ADR-PY-0006), REQ-BDC-0720..0732 and
REQ-CRON-0016..0018. Where r3's text below differs, it is struck (~~like this~~) or marked
*Superseded* inline, and the register wins. r3's own
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
| S83 | beads-cache owns the bd sync. The schedule is a job in the `sc-cron` sqlite cron table ~~(`SqliteScheduleStore`)~~, stored in the cache file and set with `bdc cache schedule set` (*Amended (A8):* the table is `bd-cache-store`'s `CacheScheduleStore`, which implements sc-cron's `ScheduleStore` over the cache's one connection; ADR-BDC-0014). A due job runs one tick in this order: drain, then sync each managed scope, then the post-sync hook. The cache itself sends nothing, and there is no consumer-specific stop rule. The first consumer is not the only one. |
| S84 | The store format is frozen as v3 plus the additive extensions listed in ADR-STORE-0006 and WP-6a's format golden. Extensions are added only by ruling, through the existing extension mechanism: `PRAGMA table_info` / `CREATE TABLE IF NOT EXISTS` under `BEGIN IMMEDIATE`; `user_version` stays 3. Existing names stay (`phase`, `announced_at`, `scope.kind DEFAULT 'phase'`), and the API names them neutrally. The full extension list for this phase is in [Store extensions](#store-extensions). A reader that does not know an extension still reads the file. Cross-implementation compatibility is the consumer's own test, run against a CI-built wheel artifact before Rand's 0.2.0 go. |
| S85 | PR #86 (health, `refresh scopes --timeout-ms`) is kept, and becomes WP-8's branch. Its cutover runbook and the consumer-specific contract text are removed. |
| S86 | The stopped layers of PRs #94 and #95 are abandoned unmerged. Their neutral parts are redone in WP-6, WP-7 and WP-9. |
| S87 | The built-in ("hard-coded") sync can be disabled ~~per scope~~ and replaced with a Python method that the consumer registers (*Amended (A7):* by the Python sync row). The method feeds the cache only through `Cache.ingest_scope`, which applies the same strict decoder and per-scope transaction as the built-in sync, so a method cannot bypass validation. |
| S88 | The post-sync hook is one registered Python method. It is called after the sync with the tick's results and errors and the cache's notification state, and marks what it handled through the cache API. Delivery is at-least-once: anything not marked is offered again on the next tick. The tick itself marks nothing. |
| S89 | Python methods are registered in the cache file ~~(the `registration` table, through `bdc cache hooks …` or PyO3)~~ (*Amended (A7):* in the sync rows' payloads), never in `.beads/bd-cache.toml`, which may be checked in. Registration records an absolute interpreter path and a `module:function`. Each method runs as `<interpreter> -I -m bd_cache_rs.run` (*Amended (A7):* the post-sync hook; the Python sync row runs its own module with sc-cron's protocol), with the cwd set to a private temp dir, so neither the project directory nor `PYTHON*` variables can inject modules. The tick refuses to run registered code from a cache file that is not owned by the current user, or that is group- or world-writable. Windows ACL checks are a documented gap. |
| S90 | One execution path: `bdc cache tick`, `cron run`, `cron run --once` and PyO3 `Cache.tick()` all run the same Rust tick, and it always runs registered methods as subprocesses. There is no in-process Python method call. `Cache.tick()` releases the GIL. Ticks are single-flight across processes through a lease row in the cache file (*Amended (A8):* every sync run, built-in or Python, takes it; REQ-BDC-0722). |

## Amendments after r3

Rand's rulings after r3 (2026-10-25), quoted. Where one changes r3, the text is marked
*Amended (Ax)*. Every deliverable cites the REQ or ADR that justifies it (A6).

| Id | Ruling (verbatim) | Effect on r3 |
|---|---|---|
| A1 | "cron.tick and cron.stop are not banned." | The lint does not flag `cron.tick` or `cron.stop`. |
| A2 | "I expect 'sc-cron' to be a general tool that can be used for scheduling jobs." and "should retain that general purpose shape for porting (copying) to sc-runtime. NOTE: cron table should have an 'enable' bool flag so scheduled tasks can be enabled/disabled w/out needing to rewrite the entire row." and the stop hook returns "'disable', 'delete', 'continue'". | Sprint b-6 (merged, #92): sc-cron gains a Python hook target (`<interpreter> -I -m <module>`, JSON on stdin, process-group or job-object kill, output caps), an `enabled` column, `continue`/`disable`/`delete` stop decisions and five-field expressions. r3's "sc-cron itself is not changed" no longer holds; the tick lease is still the tick's own. Superseded by A7: bd-cache-sync runs the post-sync hook itself; the Python sync is its own sc-cron row. |
| A3 | "limiting 'builtin' sync to one per repo makes sense." | At most one tick row per repository in the cache file's cron table; `schedule set` replaces it, and other sc-cron rows are allowed. |
| A4 | "sync row's hook target switches - can't this be handled through cron scheduling? i.e. built-in is disabled, python hook is enabled. (nothing for you to do outside basic sc-cron design)" | `bdc cache schedule enable` / `disable` flip only the tick row's `enabled` flag. Superseded by A7: the switch is between two rows. |
| A5 | Active phases would need to be defined with filters: 'phase' is the first consumer's concept, and other applications would have different scopes, including --all (Rand, 2026-10-25, paraphrased). | A scope is a name plus a bd filter: labels (all or any), status, type, assignee, or `--all` (§Scopes). |
| A6 | "Add an anchor for only allowing code that is justified by req/adr to keep project tight." | Each sprint deliverable cites its REQ or ADR (anchor ANC-011). |
| A7 | Rand, 2026-10-26, on the sync switch: two cron rows is how it should work, with a CLI command as an app-level convenience that disables the built-in sync, schedules the consumer's Python script and starts it; sc-cron stays nothing other than a general-purpose cron assembly (paraphrased). Agreed details: the switch runs the Python row at once, so the caller sees any error; sc-cron gets a per-row hook target. Hook targets stay Python or Rust, and maturin / PyO3 bindings for sc-cron come when it moves to sc-runtime (Rand, 2026-10-26). | Replaces r3's per-scope methods, the registration table and the in-tick Python `sync` call. See §A7 below. |
| A8 | Rand, 2026-10-26, on the PR #97 QA round (paraphrased): the schedule is stored through sc-cron's `ScheduleStore`, implemented by bd-cache-store in the cache file, `user_version` staying 3; architecture and requirements live on the planning branch; the new ADRs and REQs are approved; sc-cron stays general, with general features such as a multi-row enable or an exported supervisor allowed; `cron run` stops new triggers on a signal, waits for runs up to a timeout, kills hanging process trees and releases the lease; no deliverable is removed without his OK; no consumer wording except the lint's prohibited list; the 0.1 Python verbs keep the bare-name scope rule; the Python sync row runs at once on `use-python` and rolls back on failure, and PyO3 bindings for sc-cron come later. | ADR-BDC-0012..0015, ADR-PY-0006, REQ-BDC-0730..0732, REQ-CRON-0017..0018; the amendments marked *(A8)* below. |

### A7: two rows, a switch command, and a general sc-cron

- **Two rows.** The cache file's cron table holds at most one built-in sync row per repository
  (a Rust job: lease, drain, `bd list` for each configured scope, then the optional post-sync
  hook) and at most one Python sync row, which runs the consumer's script. The script fills the
  cache through `ingest_scope`, with the same validation as the built-in sync. At most one of
  the two rows is enabled.
- **The switch.** `bdc cache sync use-python --python <absolute interpreter> --module <module>
  [--cron <expr>]` disables the built-in row, creates or updates the Python row, enables it and
  runs it once at once. The run is immediate so the caller (often an agent) sees any error the
  script emits: the reply carries the run's exit status and captured stderr, and a failed run
  makes the command exit with an error and restores the built-in row as the enabled one
  (lead default). `bdc cache sync use-builtin` reverses it. Neither leaves both rows
  enabled. `schedule enable` / `disable` still flip one row's `enabled` flag. *Amended (A8):*
  `use-python` takes the lease, then sets the mode (both rows' flags) in one transaction, and
  `use-builtin` sets it back in one transaction; `schedule enable` of the built-in row while the
  Python row is enabled is refused (REQ-BDC-0720, REQ-BDC-0724).
- **Post-sync hook.** It belongs to the built-in row: its payload names the hook (interpreter
  and module), and bd-cache-sync runs it after the sync ~~with its own process supervision~~
  (*Amended (A8):* through sc-cron's exported supervisor, REQ-CRON-0018), killed with everything
  it started at its deadline. A Python sync script does its own
  post-sync work.
- **sc-cron.** It stays a general-purpose cron assembly. It gains only a general feature: a hook target per schedule row (Rust or Python). *Amended
  (A8):* and two more general ones, several rows' enabled flags in one transaction and its
  child-process supervisor exported with an environment policy (ADR-CRON-0009). Its own maturin / PyO3
  bindings come when it moves to sc-runtime, not in this phase. It gets
  no protocol, option or name shaped for beads-cache.
- **Superseded r3 text** (each marked *Superseded (A7)* below): the per-scope `method`
  (`builtin` / `python` / `external`); the `registration` table and `bdc cache hooks`; the
  registered `sync` method and its `bd-cache.run.v1` sync request and result; the `tick_id`
  stamp on ingest and `TICK_SYNC_MISSING`; `REGISTRATION_INVALID` (a bad interpreter or module
  given to the switch is `SCHEDULE_INVALID`). `bd_cache_rs.run` and the `post_sync` request
  stay for the post-sync hook. The lease, the budget with the hook reserve, `ingest_scope`,
  failure state and marking, and the trust check on the cache file stay.

### The lint's prohibited list (Rand, 2026-10-26)

No consumer wording appears anywhere in the repository except in the lint that fails when a
prohibited word is used (`scripts/lint_no_atm.py`; anchor ANC-001). The lint's prohibited list is
the one place the words are written. Every plan, anchor, requirement and doc names them only as 'a
prohibited word', so the whole tree, including `docs/plans/` and the anchor record
(`docs/plans/phase-b/anchors/`), passes the lint with no exemption beyond the lint itself, its tests
and `docs/team-protocol.md`.

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

The final design of everything in this section is the register (ADR-SYNC-0008, ADR-STORE-0006,
ADR-BDC-0013, ADR-BDC-0014, REQ-BDC-0720..0732). r3's text is kept below as the plan record,
with what A7 and A8 superseded struck or marked.

### Scopes

*Superseded (A7) in part:* scopes have no `method`; the built-in row syncs every configured
scope, and the Python row's script decides what it ingests.

- The scope registry is the existing `scope` table, with ~~a new `method` column
  (`builtin` by default) and~~ a per-scope `revision` (*Amended (A8):* and a `filter` column,
  ADR-BDC-0013).
- `bdc cache scopes add <scope>` (~~`[--method builtin|python|external]`~~), `scopes set <scope>`
  (~~`--method …`~~), `scopes remove <scope>` and `scopes list` manage it. PyO3 mirrors them.
  *Amended (A8):* a scope `.beads/bd-cache.toml` defines is the file's, and `scopes set|remove`
  of it is refused; `cache scopes` is `scopes list`'s alias, and `scopes remove` is `cache evict`
  plus removing the registry entry (ADR-BDC-0013, ADR-BDC-0015).
- *Amended (A5):* a scope is a name plus a bd filter: labels (all or any), status,
  type, assignee, or `--all`. The built-in sync runs `bd list` with that filter.
- Removing a scope deletes its membership rows. It does not delete bead rows that
  another scope still holds.
- A tick with no scopes still drains and still runs the hook.

### Scheduling

- The cache's cron job lives in the `sc-cron` sqlite tables, inside the cache file
  (*Amended (A8):* `bd-cache-store`'s `CacheScheduleStore`, ADR-BDC-0014).
  It is managed with `bdc cache schedule set <cron-expr> [--budget-ms N]
  [--hook-reserve-ms N]`, `schedule show`, `schedule enable`, `schedule disable` and
  `schedule clear`. *Amended (A3, A4):* there is at most one tick row per repository, and
  `enable` / `disable` flip only sc-cron's `enabled` column on it.
  - `--budget-ms` defaults to 25000 (range 1000–600000).
  - `--hook-reserve-ms` defaults to 5000 (range 0 to budget−1000; *Amended (A8):* at least
    1000 when a hook is named).
- Entry points: `bdc cache cron run` (a long-running worker on `sc-cron`'s
  `Scheduler`, `MissedTickPolicy::Skip`), `bdc cache cron run --once` (runs what
  is due and exits, for an OS scheduler), `bdc cache tick` (one tick now), and
  PyO3 `Cache.tick()`. All four run the same Rust tick function. *Amended (A8):* `cron run`
  stops cleanly on a signal (REQ-BDC-0730), and `tick` is refused while the Python row is enabled.

### Single flight (S90)

- The tick takes a lease row (`tick_lease`: holder pid, host, ~~`tick_id`~~ `run_id`, expiry)
  inside `BEGIN IMMEDIATE`. *Amended (A8):* every sync run takes it, the Python row's and the
  immediate run of `use-python` included, through the RAII `TickLease` (REQ-BDC-0722).
- The expiry is the deadline plus 10 s of grace.
- A tick that finds a live lease does nothing and replies `skipped: in_flight`.
- An expired lease, such as one left by a holder killed with `kill -9`, is taken
  over.
- The lease is the tick's, not the scheduler's. *Amended (A2):* sc-cron itself was
  changed by sprint b-6, but not for the lease; its generation fencing is unchanged.

### One tick (deadline = start + budget_ms)

*Amended (A7):* this is the built-in row's run. Step 3 syncs every scope with `bd list` /
`bd blocked`; there are no `python` or `external` scopes and no in-tick `sync` method.

1. **Lease.** Taken, or the tick is skipped.
2. **Drain** the journal, bounded by the remaining budget minus the hook reserve.
   The drain lock is released when the drain ends.
3. **Sync**, bounded by the remaining budget minus the hook reserve:
   - ~~`builtin` scopes~~ every scope runs `bd list` / `bd blocked`, as today.
   - ~~`python` scopes go to the registered `sync` method, called once with all of
     them.~~ *Superseded (A7).*
   - ~~`external` scopes are not synced. Only the consumer's own `ingest_scope` calls
     fill them, so the tick never overwrites rows the consumer pushed.~~ *Superseded (A7).*
4. **Post-sync hook**, if one is ~~registered~~ named in the row's payload, gets the full
   remaining budget. That is at least `hook_reserve_ms`, which steps 2–3 cannot consume, so
   escalation still runs during an outage.
5. **Reply:** one envelope recording each step. Then the lease is released.

While ~~a method~~ the hook runs, the tick holds no write transaction and no drain lock.
Cross-process handles on one file are allowed (the store refuses only a second
handle in the same process, `STORE_IN_USE`), so the hook's own `Cache` handle
reads and writes normally.

### Calling a Python method (S89, S90)

*Amended (A7):* only the post-sync hook is called this way, by bd-cache-sync (not through
sc-cron's hook runner). The consumer's Python sync is its own sc-cron row, run with sc-cron's
protocol and the harness `bd_cache_rs.sync.main` (ADR-SYNC-0008).

- **The command.** The method runs as `<absolute interpreter> -I -m bd_cache_rs.run
  <kind> <module:function>`, with the cwd set to a fresh private temp dir.
- **Environment.** It is limited to PATH, HOME, the locale, `BEADS_DIR`,
  `BD_CACHE_PATH` (the cache file) and the ~~registration's~~ row's `--env` allowlist
  (*Amended (A8):* kept, on `schedule set --post-sync` and `sync use-python`).
- **I/O.** The request goes to stdin, written concurrently with reading the output.
  Stdout and stderr are each capped at 1 MiB, and stderr is kept in the reply
  truncated to 8 KiB.
- **Killing.** It runs in its own process group (Unix) or job object (Windows),
  killed at its deadline.
- ~~*Amended (A2):* the method runs through sc-cron's Python hook runner (b-6), as module
  `bd_cache_rs.run`. b-8 adds what r3 asks that the runner lacks: the environment
  allowlist, an absolute interpreter only, `BD_CACHE_PATH`, and 1 MiB output caps
  (b-6 caps at 64 KiB).~~ *Superseded (A7, A8):* the hook runs through sc-cron's exported
  supervisor (REQ-CRON-0018) with the environment, caps and cwd above.
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

*Superseded (A7) in part:* the `sync` request and result are removed; the `post_sync` request
and result stay, in the shape ADR-SYNC-0008 gives (`docs/bd-cache-sync/architecture.md`). It
differs from r3's below: `run_id` replaces `tick_id`; a scope result has no `method` and no
`missing` value; `escalated` is capped (500 entries, oldest first, 4 MiB per request) with
`escalated_truncated` and `escalated_remaining` (A8).

~~Every request carries `protocol`, `kind`, `tick_id`, `now`, `deadline` and
`cache_path`.~~

~~**`sync` request:** `{"kind": "sync", "bd": {...}, "scopes": [{"scope", "last_ok_at",
"revision"}]}`.~~

~~**`sync` result**, the method's return value: `{"scopes": [{"scope", "status": "failed",
"error": {"code", "message", "outage"}}]}`.~~

- ~~A scope counts as ingested when an `ingest_scope` call stamped with this
  `tick_id` (taken from the request) committed for it during the run. That is
  judged by the store, not by what the method claims.~~ *Superseded (A7).*
- ~~A python scope that is neither ingested nor listed as failed is
  `TICK_SYNC_MISSING`.~~ *Superseded (A7).*
- ~~A method's `outage` is recorded exactly as given.~~ *Superseded (A7).*

~~**`post_sync` request** (r3): `{"kind": "post_sync", "drain", "scopes": [{"scope", "method",
"result": "ok|failed|outage|skipped|missing", "error", "failing_since", "notified"}],
"escalated": [...], "pending"}`.~~ *Superseded (A7, A8):* ADR-SYNC-0008's shape.

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
idempotent. *Amended (A8):* consecutive hook failures are counted and backed off
(REQ-BDC-0727).

### Ingest (S87)

*Amended (A7):* there is no `tick_id` stamp; a Python sync row's run is judged by its own
exit status, as any sc-cron row.

`Cache.ingest_scope(scope, rows, blocked, revision=None, allow_empty=False)` and
`bdc cache scopes ingest <scope>` (JSON on stdin). ~~A call made inside a run is
stamped with the run's `tick_id`.~~ *Superseded (A7).*

- Every row passes the strict `sc-beads` row decoder.
- Membership is declared by the snapshot: every listed row belongs to the scope.
  For a ~~`builtin`~~ scope the ~~tick's own~~ built-in sync also requires the scope's filter,
  as `bd list --label` does today. ~~A `python` or `external` scope may define
  membership by any criteria, which is why the override exists.~~ *Superseded (A7):* a Python
  sync row declares membership by the snapshot it ingests.
- `blocked` rows follow the built-in rule exactly. WP-6a pins this rule with a
  regression test that runs a real built-in snapshot (rows and blocked) through
  ingest.
- An empty snapshot is refused unless `allow_empty` is set.
- At most 50 000 rows and 64 MiB per call (*Amended (A8):* enforced while the input is
  read).
- Any violation rejects the whole call with `INGEST_INVALID`, and the stored scope
  is untouched.
- An accepted call is one transaction and a complete snapshot of the scope.
- A bead that is also in another scope keeps that other membership. The newest
  snapshot wins the row's content.
- `revision` is opaque. It is stored per scope ~~and echoed in the next `sync`
  request, so a method can carry state from tick to tick~~ and returned by `scopes list`, so a
  script can carry state from run to run (*Amended (A7)*).

### Store extensions

*Amended (A7, A8):* the final list is ADR-STORE-0006's (`docs/bd-cache-store/architecture.md`),
pinned by the format golden: `scope.revision`, `scope.filter`, `sync_state.failing_since`,
`sync_state.notified_since`, `journal.first_offered_at`, the tables `tick_lease`, `hook_state`
and `sc_cron_schedule`, and the `meta` row `sc_cron_schedule_version`. r3's list, kept as the
record:

*Amended (planning PR):* r3 named this ADR ADR-STORE-0005, which already exists (the
store's dependencies); it is ADR-STORE-0006 throughout.

| Kind | Name | Default and meaning |
|---|---|---|
| ~~column~~ | ~~`scope.method` TEXT~~ | ~~NULL = `builtin`~~ *Superseded (A7)* |
| column | `scope.revision` TEXT | NULL |
| column | `sync_state.failing_since` TEXT | NULL = not failing |
| column | `sync_state.notified_since` TEXT | NULL |
| column | `journal.first_offered_at` TEXT | NULL = never offered (`attempts` already exists) |
| ~~column~~ | ~~`journal.ingest_tick_id` / `scope.last_ingest_tick_id` TEXT~~ | ~~NULL~~ *Superseded (A7)* |
| ~~table~~ | ~~`registration` (kind PK, interpreter, function, env_json, registered_at, registered_by)~~ | *Superseded (A7)* |
| table | `tick_lease` (id=1 PK, ~~tick_id~~ run_id, pid, host, expires_at) | — |
| tables | ~~the `sc-cron` `SqliteScheduleStore` schema, as that crate defines it~~ | ~~created on the first `schedule set`~~ *Superseded (A8):* `sc_cron_schedule` with sc-cron's version-2 columns, served by `CacheScheduleStore` (ADR-BDC-0014) |

### New codes

*Amended (A7, A8):* the final table is REQ-BDC-0729's (`docs/requirements.md`): `INGEST_INVALID`,
`SCHEDULE_INVALID`, `TICK_RUN_FAILED` (as a command failure with exit 3 and as a hook-step result
inside a run's reply with exit 0) and `TICK_IN_FLIGHT`, each with its kind, exit, retryability,
details keys and suggested action; the `TICK_<REFUSAL>` family is removed (ADR-BDC-0015). r3's
table, kept as the record:

| Code | Kind | CLI exit | Raised by |
|---|---|---|---|
| ~~`TICK_SYNC_MISSING`~~ | ~~per-scope result in the reply~~ | ~~0~~ | *Superseded (A7)* |
| `TICK_RUN_FAILED` | per-method result in the reply | 0 | the tick: spawn failure, timeout, exit 1, oversize output, malformed output, wrong protocol, untrusted cache file (*Amended (A8):* also the failing exit 3 of `sync use-python` and `cron run --once`) |
| `INGEST_INVALID` | `BadArgs` (*Amended (A8):* kind `invalid_input`) | 3 | `ingest_scope` / `scopes ingest` |
| ~~`REGISTRATION_INVALID`~~ | ~~`BadArgs`~~ | ~~3~~ | *Superseded (A7):* `SCHEDULE_INVALID` |

`details.outage: bool` is added to errors that can come from `bd`. It is true only
when bd could not be reached: spawn failure, timeout, lock or transport. A
`BD_LIST_FAILED` from a bd that answered has `outage: false`. The 0.1 binding's
existing constants are unchanged; the ~~four~~ new codes are added to its exports in
WP-9.

### Registration (S89)

*Superseded (A7):* the Python sync is registered as its own cron row by `bdc cache sync
use-python`; the post-sync hook is named in the built-in row's payload; both live in the cache
file, never in `.beads/bd-cache.toml`.

- ~~`bdc cache hooks register sync|post_sync --python <absolute interpreter> --function
  <module:function> [--env NAME]...`, plus `hooks show` and `hooks clear`. PyO3 mirrors them as
  `Cache.hooks_*`.~~
- ~~Registration is stored in the cache file's `registration` table.~~
- `.beads/bd-cache.toml` cannot register code. A `[tick.hooks]` or `[tick.sync]` key
  there is `CONFIG_INVALID`.

## Sprints

r3's work packages are delivered by these sprints, in merge order. Each sprint doc lists its
deliverables with the REQ or ADR behind each (A6).

| Sprint | Delivers | Branch | Closure | must_follow | Reference PRs |
|---|---|---|---|---|---|
| [b-6](sprint-b-6-sc-cron.md) | A2 (sc-cron) | `sprint/b-6-sc-cron` | boundary | — | merged, #92 |
| [b-7](sprint-b-7-store-contract.md) | WP-6a; the scope filters, the schedule store and the mechanical caller edits (A5, A8) | `sprint/b-7-store-contract` | boundary | b-6 | #93 (stopped b-7 WIP) |
| [b-8](sprint-b-8-tick-python.md) | WP-6b, WP-9 as amended by A7 and A8, the b-6 sc-cron fixes, and the semantic removals | `sprint/b-8-tick-python` | integration | b-7 | #93, #94, #95 |
| [b-9](sprint-b-9-boundary-lint.md) | WP-7, WP-8, WP-10 | `sprint/b-9-boundary-lint` | integration | b-8 (merges last) | #91, #86, #96 |

Gate before b-7: PR #97 (this plan) merges into `develop`, then `develop` merges into
`integrate/phase-b`, and only then is b-7 cut from `integrate/phase-b` (phase-b-plan.md, P-8).

The sprints run one after another: b-7 → b-8 → b-9, with no sprint parallel to another
(*Amended (A8):* r3's "b-9 can start beside b-7" is withdrawn, because b-7's mechanical edits and
b-9's crate work touch the same crates). Each sprint must be CI-green on the three OSes on its
own. A sprint owns the paths in its doc's `owned_paths`; the table below is the same list.
*Amended (A8):* r3's per-work-package fences are replaced by these per-sprint fences, and the
mechanical caller edits move into b-7 while the semantic removals they would otherwise force
(the sender port, the `[tick]` keys and their users) move to b-8, which owns the callers.

| Sprint | Owns (hard fence, including each crate's `Cargo.toml`) |
|---|---|
| b-7 | `crates/bd-cache-store/**` except `tests/compat*`; `crates/bd-cache/**`; `crates/bd-cache-model/**` (b-8 later removes `src/sender.rs` and the `[tick]` keys); `crates/sc-beads/src/cache.rs`, `crates/sc-beads/src/testing/**`, `crates/sc-beads/tests/**` (the filter variants and fixtures); `crates/sc-bd-cli/src/argv.rs`, `crates/sc-bd-cli/src/testing/**`, `crates/sc-bd-cli/tests/**` (the filter argv); `crates/sc-cron/src/{store.rs,sqlite.rs,scheduler.rs}`, `crates/sc-cron/tests/{sqlite_store.rs,enabled.rs}` and the `RecordingStore` impls in `crates/sc-cron/tests/worker.rs` and `crates/sc-cron/tests/common/**` (`set_enabled_many` and `replace_and_set_enabled` only); `crates/bd-cache-api/tests/error_table.rs` (the `TICK_` row match only), the new `crates/bd-cache-api/tests/scope_names.rs` and `crates/bd-cache-py/tests/test_reads.py` (the 0.1 bare-name cases); `docs/bd-cache-cli/contract.md` (the §4 error-table rows of the four new codes only); the mechanical edits the `ScopeId`, `ScopeFilter` and store changes force in `crates/bd-cache-api/**`, `crates/bd-cache-cli/**` (with `src/bin/fake-bd.rs` answering the filters), `crates/bd-cache-py/**` and `crates/bd-cache-sync/**`, and the code rows of `crates/bd-cache-api/src/error.rs`; the `bd-cache-model`, `bd-cache-store`, `sc-beads` and `sc-bd-cli` rows of the Crate Layout and Boundaries tables of `docs/architecture.md`; `docs/bd-cache-store/**`, `docs/bd-cache/**`, `docs/bd-cache-model/**` (implementation notes) |
| b-8 | `crates/bd-cache-sync/**`; `crates/bd-cache-model/{Cargo.toml,src/sender.rs,src/config.rs,src/lib.rs}` and `tests/config.rs` (the removals); `crates/sc-cron/src/{scheduler.rs,worker.rs,hook.rs,process.rs,types.rs,lib.rs}` and `crates/sc-cron/tests/{worker.rs,python_hook.rs,stop_hook.rs,two_process.rs,reload_poll.rs,common/**}` plus new sc-cron test files; `crates/bd-cache-api/src/{decode.rs,ops/mod.rs,request/**,data/**,error.rs,lib.rs}`, the new `crates/bd-cache-api/src/ops/scopes.rs`, the `scopes` reply member in `src/ops/manage.rs`, `crates/bd-cache-api/tests/**` for the new verbs and the `TICK_<REFUSAL>` expectation of `tests/error_table.rs`; `docs/bd-cache-cli/contract.md` (the removal of the §4 `TICK_<REFUSAL>` row only); `crates/bd-cache-cli/Cargo.toml`, `crates/bd-cache-cli/src/{lib.rs,args.rs,config.rs,request.rs,scenario.rs,log.rs,help.rs,render/**,bin/**}`, `crates/bd-cache-cli/src/commands/{mod.rs,tick.rs,signal.rs}` and the new `commands/{schedule,cron,sync,scopes}.rs`, `crates/bd-cache-cli/tests/{cli.rs,process.rs,scenarios.rs,fixtures.rs,harness.rs,fixtures/**,support/**}` and new test files for the new verbs; `crates/bd-cache-py/src/**`, `crates/bd-cache-py/{README.md,pyproject.toml,Cargo.toml}`, the new `crates/bd-cache-py/python/**`, and the new pytest files `crates/bd-cache-py/tests/{test_tick.py,test_schedule.py,test_switch.py,test_run.py,test_sync_harness.py,test_ingest.py,test_scopes.py}`; the `bd-cache-sync`, `bd-cache-py`, `bd-cache-cli`, `sc-cron` and `bd-cache-model` rows of the Crate Layout and Boundaries tables and the ADR implementation table of `docs/architecture.md`; `docs/bd-cache-sync/**`, `docs/sc-cron/**`, `docs/bd-cache-api/**`, `docs/bd-cache-cli/{architecture.md,requirements.md}` (implementation notes) |
| b-9 | `crates/bd-cache-store/tests/compat*` (removal); every existing file of `crates/bd-cache-py/tests/` (the two vendored call-log files and `test_mapping_doc.py` removed), the new `crates/bd-cache-py/tests/corpus/**` and `tests/test_corpus.py`; `crates/sc-beads/**`, `crates/sc-bd-cli/**`, `crates/sc-cron/**` (comments, provenance, tests and fixtures only, no version bump); `crates/bd-cache-api/src/ops/manage.rs` and `crates/bd-cache-cli/src/commands/manage.rs` with their tests (#86 health); the new `crates/bd-cache-cli/tests/outage_scenario.rs`; `docs/bd-cache-cli/contract.md`, `docs/bd-cache-cli/cutover-runbook.md` (deleted if present), `docs/bd-cache-py/**`, `docs/research/**`; `scripts/bd-compat/**`, `scripts/lint_no_atm.py`, `scripts/test_lint_no_atm.py`; `.github/workflows/{ci.yml,python.yml,bd-compat.yml}` (not the kit's); `README.md`; `CHANGELOG.md` |

The register (`docs/requirements.md`, `docs/architecture.md`'s ADR text, and the REQ and ADR text
of every per-crate doc) is the planning branch's (A8): this PR writes it, Approved, and a sprint
that finds it wrong raises it with the lead and never edits it locally. Sprints edit only the
implementation rows and notes their fence names.

Shared-file rules:
- On a `Cargo.lock` conflict, take the target branch's lock and run `cargo metadata`.
  Never run `cargo update`.
- No sprint adds a crate or edits the root `Cargo.toml`. A crate's own `Cargo.toml` may add a
  workspace dependency it newly uses (`sc-cron` in `bd-cache-store`, b-7; `bd-cache-sync` and
  `tokio` in `bd-cache-py`, and `sc-cron` and `tokio` with `rt-multi-thread` and `signal` in
  `bd-cache-cli`, b-8), and may
  drop one it no longer uses (`toml` and `libc` in `bd-cache-sync`, b-8).
- `ci.yml` is b-9's alone.
- `docs/plans/**` is the plan branch's. *Amended (A8, ADR-BDC-0012):* it is not exempt from the
  lint; it names prohibited words only as "a prohibited word".

Common acceptance for every sprint:
- **AC-0:** `python3 scripts/lint_no_atm.py --paths <the sprint's fence>` exits 0.
- **AC-Q:** S57 QA PASS, and green CI on the three OSes.
- **AC-N:** a planted negative for every behaviour change, restored from an
  in-memory copy.

### WP-6a store and contract

ADR: ADR-STORE-0006 (the extensions). *Amended (A8):* ADR-STORE-0006 and every ADR and REQ this
plan writes are Approved by Rand (2026-10-26) in the planning PR; a sprint implements an approved
register. ~~It deletes the consumer parts of `bd-cache-model` (`sender.rs` and the `[tick]` table's
message-routing and phase-file settings)~~ (*Amended (A8):* moved to b-8, which owns their
callers) and the neutralised comments in the store.

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
   - ~~registration read and write.~~ *Superseded (A7).*
3. The new codes and `details.outage` are in the model's error table. The outage
   truth table has one test per row: spawn failure, timeout, lock, transport, and
   an answered `BD_LIST_FAILED`.
4. A second process opens the same file and ingests while a first holds a handle
   with no transaction open. The first then reads the result (cross-process
   handles).

### WP-6b tick

*Amended (A7):* items about per-scope methods, the `registration` table, `hooks register`,
the in-tick `sync` method, `tick_id` judging, `TICK_SYNC_MISSING` and `REGISTRATION_INVALID`
are replaced by the two rows and the switch; b-8's sprint doc is the binding list.

ADR: ADR-SYNC-0008 (*Amended (A8):* Approved by Rand, 2026-10-26, in the planning PR), covering the tick, the schedule, the lease, the methods, the
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
3. ~~Methods, through a Rust test-support runner binary that speaks
   `bd-cache.run.v1` and opens a real second handle: `builtin` syncs as before; `python`:
   ingested, failed and missing scopes are judged by `tick_id`; `external` is never synced, and
   pushed rows survive a tick.~~ *Superseded (A7):* the two rows and the switch (b-8 acceptance).
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
~~`REGISTRATION_INVALID`~~ (*Superseded (A7):* `SCHEDULE_INVALID`) import check or the PyO3
surface (WP-9), or the contract text (WP-8). *Amended (A8):* WP-6b and WP-9 are one sprint, b-8,
and the delete list's refusal of the first consumer's own directory amends REQ-BDC-0810 through
ADR-BDC-0015.

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
   - It adds the scopes, schedule, cron, ~~hooks~~ sync and ingest verbs, the
     `bd-cache.run.v1` shapes and the new-codes table, ~~under WP-6b's merged names~~
     (*Amended (A8):* from the approved register, ADR-BDC-0015 and REQ-BDC-0720..0732, which is
     the source of truth b-8 builds to; b-9 states versioning under §11).
3. `docs/requirements.md` supersedes REQ-BDC-0701..0707 and adds REQ-BDC-0720..0729,
   from this section.
4. `docs/architecture.md`: ADR-BDC-0006 states sc-cron's provenance neutrally.
   ADR-BDC-0011 is a short decision record pointing at ADR-SYNC-0008 and
   ADR-STORE-0006.
5. The phase-b-plan rulings table records S64–S90.

*Amended (planning PR, A8):* items 3, 4 and 5 (S64–S90 and A1–A8 in the rulings table) are
written in the phase-B planning PR, with the provenance wording of `docs/requirements.md` and
`docs/architecture.md` restated neutrally, with no change of meaning. WP-8 (b-9) checks the
register against the merged code and raises any difference with the lead.

### WP-9 Python surface and end to end

*Amended (A7):* items about per-scope methods, the `registration` table, `hooks register`,
the in-tick `sync` method, `tick_id` judging, `TICK_SYNC_MISSING` and `REGISTRATION_INVALID`
are replaced by the two rows and the switch; b-8's sprint doc is the binding list.

Acceptance:
1. PyO3:
   - `ingest_scope`, `scope_announce`, `scopes_*`, `schedule_*`, ~~`hooks_*`~~
     `sync_use_python` / `sync_use_builtin` and `tick()`, which releases the GIL and spawns
     runners;
   - the ~~four~~ new codes in the exports;
   - `run`-context guards, so `flush`, `tick` and `schedule` raise `PRECONDITION`.
2. `python -m bd_cache_rs.run` implements `bd-cache.run.v1`, using the maturin mixed
   layout. The existing `import bd_cache_rs` tests pass unchanged.
3. ~~`REGISTRATION_INVALID`~~ `SCHEDULE_INVALID`: a relative interpreter, and a function that does not
   import. A hostile `bd_cache_rs.py` or a shadowing module in the project dir is
   not imported (`-I`, temp cwd).
4. A real-interpreter end-to-end test runs a real `bdc cache tick` against the fake bd:
   - ~~the registered `sync` method ingests one scope and fails another;~~ *Superseded (A7):*
     the Python sync row, run by `use-python` and by `cron run --once` (b-8 acceptance);
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
