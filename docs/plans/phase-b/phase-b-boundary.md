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

## Lead rulings

| Ruling | Decision |
|---|---|
| S82 | ADR-BDC-0010 (fenix's draft on `lint/no-atm-boundary` @ 33a2b07) is approved with the lint adjustments L1–L3 below. It lands in b-9, together with its CI step, once the tree is clean. It is never merged red. |
| S83 | beads-cache owns synchronising bd on its cron tick (Rand, 2026-10-25: "beads-cache needs to handle the synchronization of bd on a cron tick. there can also be hooks necessary to report/escelate based on atm-bd requirements (note atm-bd is simply the first consumer of beads-cache)."). Each `cache tick` drains, refreshes the configured scopes, then calls the registered hooks through the beads-cache hook protocol (below). The schedule lives in beads-cache's `[tick]` config, driven by `sc-cron`. Hooks are how a consumer reports and escalates. The cache itself sends nothing and has no consumer-specific stop rule. atm-bd is the first consumer, not the only one. |
| S87 | The tick's sync method is overridable by a consumer-supplied provider (Rand, 2026-10-25: "we should probably consider allowing the sync method to be overridden by atm-bd supplied python call (because a hardcoded method will likely not work until it is run over time)."). The built-in `bd list` / `bd blocked` sync stays the default. A registered sync provider replaces it per tick, through the beads-cache sync protocol (below). Whatever the provider returns enters the store through the same decoder and the same atomic per-scope write as the built-in sync, so a provider can change how rows are fetched but cannot bypass validation. The same ingest path is exposed in PyO3 (`cache.ingest_scope(...)`), so Python hooks can feed the cache directly. |
| S84 | The store file format is frozen: v3 DDL, the additive columns, `user_version` 3, and the drain lock path and semantics. A consumer opens its existing file through `cache_path`. Table and column names stay as they are (`phase`, `announced_at`), because renaming them would break the file. The API and docs name them neutrally ("scope", "notified"). Cross-implementation compatibility with the consumer's own implementation is the consumer's test, against the published wheel. Ours is a neutral golden of the format. |
| S85 | PR #86 (health, `refresh scopes --timeout-ms`) is kept, and becomes b-8's branch. Its cutover runbook and the consumer-specific contract §10 text are removed. Migration of the consumer's own files is the consumer's job. |
| S86 | The stopped fix layers `fix/fen-tick` and `fix/fen-py` are abandoned unmerged. The neutral parts of their work are redone in b-6 and b-7. |

## Lint review (S82 adjustments)

The draft is sound: the rules are case-insensitive, there are no inline pragmas,
exemptions are fixed in the script, and the 20 tests use deterministic temp repos.
Adjustments:

- **L1. Drop the `docs/team-protocol.md` exemption.** It is agent tooling, not
  product. Move it to `.claude/team-protocol.md`, which is out of scope by
  construction, and update any reference to it.
- **L2. Close the scope-naming gap.** Hard-coding `phase-<x>` scopes in code (for
  example `format!("phase-{}")` or a `"phase-"` prefix literal) is not caught.
  Add a narrow rule for a `phase-` prefix literal or format string. Test both the
  hit and a non-hit (`phase-b` in prose).
- **L3. Published crates.** `sc-beads`, `sc-bd-cli` and `sc-cron` carry hits too,
  mostly comments and provenance. They are fixed like any other crate; the fix
  ships in their next release with the lockstep version (S61).

Rule breadth: no rule needs narrowing at handoff. If a sprint finds a legitimate
neutral term that hits, narrow the regex and add a test. Never add an exemption.

## The tick and hook protocol (S83)

Configuration, in beads-cache's own `.beads/bd-cache.toml`:

```toml
[tick]
schedule = "*/1 * * * *"       # sc-cron expression for the cache's own cron
scopes = ["<bd label>", ...]   # synced each tick; a scope is a bd label, any name
budget_ms = 25000              # the tick's deadline

[[tick.hooks]]                 # zero or more, run in order; a consumer registers its script
name = "report"                # free text, recorded in the reply
command = ["python3", "-m", "some.module"]   # argv, no shell
timeout_ms = 10000
```

`bdc cache tick` (run by the cache's cron, or on demand) does the following,
under the tick budget:

1. Drain the journal (`cache flush`).
2. Refresh every configured scope: by the built-in sync (`cache refresh scopes`), or by
   the registered sync provider (S87).
3. Call each registered hook in order, with the request below on stdin, in its
   own process group, killed at its `timeout_ms` or at the remaining budget.
   Each later hook sees the marks made by the earlier ones.
4. Apply each hook's response: mark the entries and scopes it reports as notified.

It replies with an envelope recording each step. A hook failure (spawn failure,
timeout, non-zero exit, or malformed or wrong-protocol output) is recorded as
`TICK_HOOK_FAILED` (per hook) with details. Nothing is marked, and the drain and refresh
results stand.

Request (one JSON object on stdin):

```json
{"protocol": "bd-cache.hook.v1", "tick_id": "...", "hook": "<name>", "now": "<rfc3339>",
 "cache_path": "...",
 "scopes": [{"scope": "...", "state": "ok|stale|outage|failed",
             "last_ok_at": "...", "error": {"code": "...", "message": "..."},
             "since": "...", "notified": false}],
 "escalated": [{"seq": 1, "bead": "...", "op": "...", "actor": "...",
                "error": {"code": "...", "message": "..."}, "created_at": "..."}],
 "pending": {"count": 0, "oldest_age_s": null}}
```

- `escalated` lists only entries not yet notified.
- A scope's `notified` is false while its current outage or failure, identified
  by `since`, has not been marked.

Response (one JSON object on stdout, exit 0):

```json
{"protocol": "bd-cache.hook.v1",
 "notified": {"entries": [1], "scopes": ["..."]},
 "message": "optional, recorded"}
```

- Unknown seqs and scopes are ignored and listed back in the reply, never as an
  error.
- The same marks are available without a hook, from the CLI and PyO3:
  `cache journal announce <seq>` (existing) and a new `cache scopes announce
  <scope>`. A consumer's own scheduler can call those instead.

Scope notification state is a new additive nullable column (`sync_state.notified_at`,
compared against the failure's `since`). That is allowed under S84; there are no
renames.

## The sync provider protocol (S87)

```toml
[tick.sync]                    # optional; absent means the built-in sync
command = ["python3", "-m", "some.module"]   # argv, no shell
timeout_ms = 20000
fallback = "builtin"           # "builtin": run the built-in sync for any scope the provider failed; "none": record the failure
```

Each tick, in step 2, the cache calls the provider once with every configured
scope, instead of running `bd` itself.

Request (one JSON object on stdin):

```json
{"protocol": "bd-cache.sync.v1", "tick_id": "...", "now": "<rfc3339>",
 "bd": {"binary": "<resolved path>", "project": "<dir>", "dolt_mode": "server|embedded"},
 "scopes": [{"scope": "...", "last_ok_at": "...", "revision": "..."}]}
```

Response (one JSON object on stdout, exit 0):

```json
{"protocol": "bd-cache.sync.v1",
 "scopes": [{"scope": "...", "status": "ok",
             "rows": ["<bd list --json row>", "..."],
             "blocked": ["<bd blocked --json row>", "..."],
             "revision": "optional"},
            {"scope": "...", "status": "failed",
             "error": {"code": "...", "message": "...", "outage": true}}]}
```

- Each `ok` scope is a complete snapshot of that scope, the same contract as the
  built-in sync: rows not listed leave the scope.
- The rows are decoded with the strict `sc-beads` row decoder used for `bd`'s own
  output, and written in one transaction per scope. A scope with any row that
  fails to decode is rejected whole; its stored rows are untouched and it is
  recorded as `failed` with `TICK_SYNC_INVALID`.
- Scopes the provider omits, a timeout, a non-zero exit, and malformed or
  wrong-protocol output count as provider failures for the affected scopes, and
  are then handled by `fallback`.
- `outage: true` on a failed scope feeds the same outage state as the built-in
  sync (b-6 deliverable 3), so the hooks see one model.
- `cache.ingest_scope(scope, rows, blocked, revision=None)` in PyO3, and
  `bdc cache refresh ingest --scope <s>` (JSON on stdin), use the same decoder and
  write path. A consumer can run its own sync on its own schedule and push the
  results.

## Sprints

| Sprint | Branch | Closure | Owns (hard fence) | must_follow |
|---|---|---|---|---|
| b-6 tick | `sprint/b-6-tick` | boundary | `crates/bd-cache-sync/**`, `crates/bd-cache-model/src/{config,error}.rs` and their tests, `crates/bd-cache-cli/src/{args.rs,config.rs,commands/tick.rs,commands/mod.rs}` and the CLI tick tests, `docs/bd-cache-sync/**` | — |
| b-7 neutral store and binding | `sprint/b-7-neutral` | boundary | `crates/bd-cache-store/**` (comments, tests and compat suite only; the format is frozen), `crates/bd-cache-py/**`, `crates/bd-cache/**`, `crates/sc-*/**` (comments and provenance only), `docs/bd-cache-store/**`, `docs/bd-cache-py/**`, `docs/bd-cache/**`, `docs/sc-*/**`, `scripts/bd-compat/**`, `.github/workflows/python.yml` | — |
| b-8 docs and API | `fix/fen-api` (PR #86) | integration | `crates/bd-cache-api/**`, `crates/bd-cache-cli/src/commands/manage.rs` and its tests, `docs/architecture.md` (except the ADR-BDC-0010 section), `docs/requirements.md`, `docs/bd-cache-cli/**`, `docs/bd-cache-api/**`, `docs/baseline/**`, `docs/research/**`, `README.md`, `CHANGELOG.md`; after b-7 merges, the `ingest_scope` method in `crates/bd-cache-py/src` | b-6, b-7 (for the ingest wiring only; the docs work starts at once) |
| b-9 lint | `lint/no-atm-boundary` | integration | `scripts/lint_no_atm.py`, `scripts/test_lint_no_atm.py`, the ADR-BDC-0010 section, `.github/workflows/ci.yml`, the `docs/team-protocol.md` move (L1), plus any residual hit anywhere | b-6, b-7, b-8 |

Width: 3 in wave 1 (b-6, b-7, b-8). b-8 merges after b-6 and b-7, then b-9 runs alone.

### b-6 tick

Deliverables:

1. Remove the messaging sender (`src/atm.rs`), the message texts in `announce.rs`,
   the consumer's phase-file discovery (`phases.rs` and the active-phase rule in
   `active.rs`), the consumer-shaped stop rule, `[tick] team` / `fallback_lead` /
   `phases_dir` / `phases`, and the `refused` / `pass-through` replies. Keep the
   neutral parts of the tick: the budget, the sc-cron schedule, single-flight, and
   the `TICK_*` codes that still apply.
   - A resolve hint in any neutral output names `cache journal resolve`.
   - Remove the `.atm-bd/` legacy-path refusal from `bd-cache-cli/src/config.rs`.
     A consumer passes `cache_path` explicitly.
2. Add `[tick] scopes` / `schedule` / `[[tick.hooks]]` to `bdc cache tick`, with the hook protocol above,
   plus `cache scopes announce` and the scope `notified` state.
   - Hook tests drive a test-support hook binary (Rust, no Python dependency).
     Cover these cases: success with marks, unknown seq, timeout (kill the process
     group, causal through a gate file), non-zero exit, malformed output, wrong
     protocol, no hook configured, two hooks in order (the second sees the first's
     marks), and the budget cutting the last hook.
3. Add an explicit outage flag in the error details (`details.outage: bool`). It is
   true only when bd could not be reached (spawn failure, timeout, lock, transport).
   A `BD_LIST_FAILED` from a bd that answered is a scope `failed`, not an `outage`.
   The 0.1 binding's exported code constants are unchanged.
4. Make `tests/job_threading.rs` join_idle causal (no `sleep` opener).
5. Add the sync provider (S87): `[tick.sync]`, the `bd-cache.sync.v1` protocol, and
   the ingest path, shared with the built-in sync's decoder and per-scope
   transaction. Tests use a test-support provider binary. Cover these cases: a
   snapshot replaces the scope; one bad row rejects the scope whole and leaves the
   stored rows untouched; an omitted scope; timeout (causal); `fallback = "builtin"`
   runs the built-in sync for the failed scope only; `fallback = "none"`; and
   `outage: true` reaching the hook request. `bdc cache refresh ingest` lives in
   `commands/manage.rs`, which b-8 owns: b-6 provides the `bd-cache` / `bd-cache-sync`
   ingest function, and b-8 wires the CLI verb and the PyO3 `ingest_scope` after b-6
   and b-7 merge. b-8 merges last of the three, and is allowed this one addition in
   `crates/bd-cache-py/src` once b-7 has merged.
6. Rewrite crate docs and requirements (`docs/bd-cache-sync/`): supersede ADR-SYNC-0007
   and its fix-a-7-3 / a-7-5 amendments with ADR-SYNC-0008 (the tick and hooks).

REQs: REQ-BDC-0720..0726 (new, written by b-8 from this section) replace
REQ-BDC-0701..0707.

### b-7 neutral store and binding

Deliverables:

1. Remove the vendored compat suite (`crates/bd-cache-store/tests/compat/`,
   `compat.rs`, PROVENANCE).
   - Replace it with neutral format goldens: the exact v3 DDL plus the additive
     columns, `user_version`, the drain lock path and its exclusion semantics.
   - Also test that a file written by another process, with only v3 columns
     filled, is read correctly (`membership_known` and the other NULL extensions).
2. In the Python binding:
   - replace the `atm_core_05831df_*` call corpus with a neutral call corpus that
     exercises the same 0.1 verbs and shapes;
   - make `src/lib.rs` and the README document the binding's own API, with no
     mapping from another implementation;
   - make `tests/test_additive.py:82-88` causal (a gate fake bd, no wall-clock
     bound);
   - document that there is no runner injection (wrap the binary through
     `bd_binary`), that a detached `bdc cache flush` must log its output, and that
     there is one handle per process.
3. Neutralise comments and provenance in `bd-cache-store`, `bd-cache` and `sc-*`.
   Fixture provenance cites the bd version and the command that produced it, not
   a consumer.

### b-8 docs and API (on PR #86)

Deliverables:

1. Keep #86's health and `--timeout-ms` work.
   - Delete `docs/bd-cache-cli/cutover-runbook.md`.
   - Rewrite contract §10 as "0.1 → 0.2 API differences", with no consumer named.
     It covers I1 (`outage` on Ok), I10 (`issues_list` default exclusions), the
     optional `changed` / `already_closed`, and one handle per process.
2. In `docs/requirements.md`, supersede REQ-BDC-0701..0707 and write REQ-BDC-0720..0726
   from the tick section above. Remove the consumer as design reference everywhere
   in the top-level docs. Delete `docs/baseline/BD-CACHE.md`; the store format golden
   (b-7) is the reference.
3. In `docs/architecture.md`, make ADR-BDC-0006 state `sc-cron`'s provenance without
   naming the consumer. Record S82–S86 in the phase-b-plan rulings table (the table
   is on this branch already).
4. In the contract, rewrite `cache tick` and `[tick]`, and add the hook protocol, the sync
   provider protocol, `cache refresh ingest`, `cache scopes announce` and the PyO3
   `ingest_scope`, matching b-6
   by the names above.

### b-9 lint

Deliverables:

1. Apply L1–L3. Mark ADR-BDC-0010 Approved (S82).
2. Merge b-6, b-7 and b-8 in. The lint exits 0 on the tree; fix any residual hits.
3. Add the CI step. It goes green on all three OSes.

## Disposition of the consumer's review findings

| Finding | Disposition |
|---|---|
| B1 sender identity, B3 message text, I5 cron shim | Superseded: the cache sends nothing (b-6). The resolve hint names `cache journal resolve`. |
| B2 legacy-path refusal | Superseded: the refusal is removed; the consumer passes `cache_path` (b-6). |
| B4 release and runbook | The runbook is the consumer's job. The 0.2.0 release waits for b-9 and Rand's go. |
| I1, I10, M3 | b-8, in the contract. |
| I2 outage classification | b-6, deliverable 3. |
| I3, I6, M1 | b-7, in the binding docs. |
| I4 outage-notified state | b-6: scope `notified` and `cache scopes announce`. |
| I7 health, I10 timeout | Done in #86, kept by b-8. |
| I8 timing tests | latency, harness and process.rs were already fixed (#84, #85). job_threading is b-6; test_additive is b-7. |
| I9 cross-implementation drain | The consumer's own test against the wheel (S84). Ours is the neutral format golden (b-7). |
| M2, M4 | b-8, a neutral note in the contract. |
| M5 | Upstream (sc-runtime). |
| M6 | No change: an oversized key fails safely as BAD_ARGS. |

## QA

Each sprint goes through the S57 QA gate and must have green CI. Planted negatives
are required for every behavioural change. b-9 adds a final check: the lint exits 0
and its CI step is green.
