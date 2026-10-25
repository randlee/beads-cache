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

This revision (r4, lean) follows those goals. It replaces r1–r3, which grew to six sprints by redesigning a scheduler, runner and lease that `sc-cron` already provides
The r1–r3 review items that still apply are carried into the ACs; the rest are listed under
[Deferred](#deferred).

Rand, on sc-cron (2026-10-25), verbatim: "what you described for sc-cron sounds
like the correct general purpose design and should retain that general purpose
shape for porting (copying) to sc-runtime. NOTE: cron table should have an
'enable' bool flag so scheduled tasks can be enabled/disabled w/out needing to
rewrite the entire row."

## Lead rulings

| Ruling | Decision |
|---|---|
| S82 | ADR-BDC-0010 (fenix's draft, `lint/no-atm-boundary` @ 33a2b07) is approved, with the lint adjustments L2, L4 and L5 (L1 is withdrawn; `docs/team-protocol.md` stays exempt). The lint and its tests land with this plan, with no CI step, so every sprint can gate on `--paths`. b-6 turns on the CI step once the tree is clean, and it guards b-7 and b-8 from then on. |
| S83 | Scheduling is `sc-cron`. A schedule row is a cron string, a timeout, a JSON payload, a hook target and an `enabled` flag. The hook target is `Rust` (an in-process `Job`) or `Python` (an interpreter plus a module, run as a subprocess with one JSON object on stdin). `sc-cron` stays general purpose: it knows nothing of beads or of any consumer, so sc-runtime can copy it as it is. |
| S84 | The bd sync is one schedule row. Its hook target is the switch between the "hard-coded" built-in sync (`Rust`: beads-cache's sync job) and a consumer's replacement (`Python`: the consumer's module, which feeds the cache through `ingest_scope`). Disabling the built-in sync means repointing that row, or setting `enabled = false` and adding the consumer's own row. |
| S85 | The built-in sync job calls an optional post-sync Python hook, named in its row's payload, with the sync results and errors and the escalations not yet notified. The hook marks what it handled through the cache API. The cache itself sends nothing. Delivery is at-least-once. |
| S86 | The store format is frozen as v3 plus the existing extensions. This phase adds only `sync_state.failing_since` and `sync_state.notified_since` (both nullable), and `sc-cron`'s own tables, which already exist. Names stay as they are. |
| S87 | PR #86 (health, `refresh scopes --timeout-ms`) is merged into b-6. Its cutover runbook and consumer-specific contract text are removed there. The stopped layers `fix/fen-tick` and `fix/fen-py` are abandoned. |

## Sprints

| Sprint | Branch | Owns | must_follow |
|---|---|---|---|
| b-6 removal and lint | `sprint/b-6-removal` | everything except `crates/sc-cron/**` and `docs/sc-cron/**` | — |
| b-7 sc-cron general shape | `sprint/b-7-sc-cron` | `crates/sc-cron/**`, `docs/sc-cron/**` | — |
| b-8 sync job and post-sync hook | `sprint/b-8-sync` | `crates/bd-cache-store/**`, `crates/bd-cache/**`, `crates/bd-cache-sync/**`, `crates/bd-cache-cli/**`, `crates/bd-cache-py/**`, and their docs | b-6, b-7 |

b-6 and b-7 run in parallel. b-8 follows both.

Every sprint must also meet these:
- `python3 scripts/lint_no_atm.py --paths <fence>` exits 0. From b-6 on, it is the CI step.
- S57 QA PASS, and green CI on the three OSes.
- A planted negative for every behaviour change.
- On a `Cargo.lock` conflict, take the target branch's lock and run `cargo metadata`.

### b-6 removal and lint

1. Merge `fix/fen-api` (#86) in. Delete `docs/bd-cache-cli/cutover-runbook.md`, and rewrite
   contract §10 as "0.1 → 0.2 API differences", with no consumer named.
2. Delete the consumer code:
   - from `crates/bd-cache-sync`: `atm.rs`, `phases.rs`, the consumer parts of
     `announce.rs` and `active.rs`, the stop rule, and their tests;
   - from `bd-cache-model`: `sender.rs` and the `[tick] team`, `fallback_lead`,
     `phases_dir` and `phases` config;
   - the `refused` / `pass-through` replies;
   - the `.atm-bd/` refusal;
   - the hard-coded `phase-` scope prefix in `ScopeId` (`bd-cache-model/src/scope.rs`)
     and in `bd-cache-api` `manage.rs`. A scope is any bd label.

   The tick keeps only what is neutral: drain plus sync of configured scopes. b-8
   reshapes it into the sync job.
3. Delete the vendored compat suite (`crates/bd-cache-store/tests/compat*`) and
   `crates/bd-cache-py/tests/atm_core_05831df_*`. Replace them with a neutral format
   golden (the exact v3 DDL plus the extensions) and a neutral 0.1 call corpus,
   generated in the repo against the fake bd, with no loss of verb coverage.
4. Neutralise the docs, requirements, ADRs and comments. That covers
   ADR-SYNC-0007, REQ-BDC-0701..0707, `docs/baseline/BD-CACHE.md` and the
   provenance notes, and includes the `sc-*` crates other than sc-cron, which b-7
   neutralises.
5. Make the remaining timing tests causal: `bd-cache-py/tests/test_additive.py:82-88`
   and `bd-cache-sync/tests/job_threading.rs` `join_idle`.
6. Add the CI step "Strict consumer boundary (ADR-BDC-0010)". `lint_no_atm.py`
   exits 0 on the whole tree, except `crates/sc-cron` and `docs/sc-cron`, which b-7
   cleans. The step runs once both are merged; if b-7 lands second, b-7 turns it on.

### b-7 sc-cron general shape

Everything stays general: no beads, bd-cache or consumer types or names.

1. `HookTarget = Rust(Job) | Python { interpreter, module }`.
   - Python runs as `<interpreter> -I -m <module>`, with the cwd set to a temp dir.
     The schedule's JSON payload and the event go on stdin as one object:
     `{"protocol": "sc-cron.hook.v1", "event": "cron.tick", "schedule": "<name>",
     "payload": {...}, "deadline": "..."}`, plus `cron.stop` before a row is
     stopped. These are the same event names as sc-runtime's plan, and Rand ruled
     they are allowed.
   - The process is killed at the row's timeout, as a Unix process group or a
     Windows job object.
   - Output is capped, and the outcome is recorded as the run's result.
2. An `enabled` column (default 1). `Scheduler::set_enabled(name, bool)` changes only
   that column. A disabled row never fires, and keeps its generation, expression and
   payload.
3. Accept standard five-field crontab expressions (minute first), as well as the
   existing six- and seven-field forms.
4. Neutralise the docs and comments (no consumer provenance). Keep the existing
   generation fencing as the single-flight guarantee, and test it across two
   processes.
5. Tests:
   - a Python hook through a stub interpreter (argv and stdin shape, timeout kill,
     caps);
   - enable and disable without rewriting the row (a planted negative: an update
     that rewrites the row is caught);
   - five-field parse;
   - two-process fencing.
6. Tell sc-runtime that this crate is the shape to copy.

### b-8 sync job and post-sync hook

1. The built-in sync job is a `Rust` `Job`. It drains, syncs the scopes listed in its
   payload (`{"scopes": [...], "post_sync": {"interpreter": "...", "module": "..."}}`),
   then runs the post-sync hook, if one is set, through b-7's Python runner.
2. `bdc cache schedule set|show|enable|disable|clear` manages the sync row. Commands
   that set the target are `--rust-sync` (the default) or `--python <interpreter>
   --module <m>`. `bdc cache cron run [--once]` runs due rows. `bdc cache tick`
   runs the sync job once, now.
3. `ingest_scope(scope, rows, blocked, revision=None)` takes a complete snapshot of
   a scope, through the strict row decoder, in one transaction. A row that fails to
   decode rejects the whole call (`INGEST_INVALID`), and a bead keeps its
   membership in other scopes. It is available in Rust, in the CLI
   (`cache scopes ingest`) and in PyO3. It is how a Python sync replacement feeds
   the cache.
4. The post-sync request carries:
   - each scope's result, error, `failing_since` and `notified`;
   - the escalated entries not yet notified;
   - the pending count.

   The hook marks entries with the existing `journal announce`, and scopes with
   the new `scopes announce <scope> --since <failing_since>`, which marks only if
   `since` still matches. Anything the hook leaves unmarked is offered again on the
   next run.
5. Add `details.outage: bool` to errors that can come from bd. It is true only when
   bd could not be reached. A `BD_LIST_FAILED` from a bd that answered has
   `outage: false`.
6. Update the contract, requirements and ADR-SYNC-0008 for the sync job, the hook
   and `ingest_scope`.
7. One end-to-end test with a real interpreter, run through `bdc cache tick` against
   the fake bd:
   - the built-in sync, then a post-sync hook that marks one escalation and the
     failed scope;
   - the next tick does not offer what was marked, and does offer the rest again;
   - the row repointed to a Python sync module that ingests one scope.

## Deferred

These were designed in r1–r3 and are deferred until a real failure needs them:
- a tick lease beyond sc-cron's generation fencing;
- cache-file ownership and permission checks;
- an `external` scope method and a scope registry;
- ingest row and byte caps, and `allow_empty`;
- a separate registration table (the hook target lives in the cron row, inside the
  cache file, not in the checked-in TOML);
- `first_offered_at`.

## Disposition of the consumer's review findings

| Finding | Where it goes |
|---|---|
| B1, B3, I5 | Superseded: the cache sends nothing (S85). |
| B2 | The refusal is removed (b-6). |
| B4 | The neutral §10 (b-6). The release waits for b-8 and Rand's go. |
| I1, I3, I6, I10, M1–M4 | b-6 docs. |
| I2 | b-8 item 5. |
| I4 | b-8 item 4 (`failing_since` / `notified_since`). |
| I7, I10 | Done in #86, merged by b-6. |
| I8 | The remaining tests are fixed in b-6 item 5. |
| I9 | The consumer's test, against a CI-built wheel. |
| M5 | Upstream. |
| M6 | No change. |
