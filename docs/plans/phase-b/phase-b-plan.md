# Phase B — bdc CLI, bd-cache-api, Python 0.2, release channels

Status: b-1..b-5 complete and merged into `integrate/phase-b` (b-1 #75, b-2 #76, b-3a #74,
b-3 #78, b-4 #77, b-5 #80); b-6 complete and merged (#92); b-7 complete and merged (#99); b-8
complete and merged (#100); b-9 complete and merged (#101, `fa05157`). b-1..b-9 merged;
phase-end review fixes in review (#102). Rand, 2026-10-26: phase B closes without the
binaries prerelease, and the contract is 0.2.0, not 1.0.0. Phase B closes when #82 merges into
`develop`. b-7, b-8 and b-9 are the sprints of the
boundary extension [phase-b-boundary.md](phase-b-boundary.md) (r3, lead ruling S82, with
Rand's later Amendments A1–A8). The contract, ADR-BDC-0009 and the REQ-BDC-0801, 0802, 0803
and 0806 amendments were approved by Rand (lead ruling S60, 2026-10-25). The phase-B extension's
register (ADR-BDC-0011..0015, ADR-STORE-0006, ADR-SYNC-0008, ADR-CRON-0009, ADR-PY-0006,
REQ-BDC-0720..0732, REQ-CRON-0016..0018 and the amendments they name) was approved by Rand
(2026-10-26, Amendment A8) in the planning PR #97.

Phase id: `b`. Plan branch: `plan/phase-b` (PR to `develop`; the extension is planned on
`plan/phase-b-next`, PR #97). Phase branch: `integrate/phase-b`, cut from `develop` after PR #69
(the contract) merges. Sprint branches `sprint/b-<n>-<slug>`, cut from `integrate/phase-b`.

The architecture and requirement docs live on the planning branch (A8): PR #97 carries
`integrate/phase-b`'s delivered docs (c711eef) with this plan's register on top, so every branch
reads one register. Sprints edit only the implementation rows and notes their fence names.

## Scope

Implement the bd-cache / bdc CLI contract,
[`docs/bd-cache-cli/contract.md`](../../bd-cache-cli/contract.md), contract
version 0.2.0 (Rand, 2026-10-26; it was 1.0.0 when approved at `8115a0a`), as approved at `8115a0a` and marked Approved on #69 at `6dec77e`. The contract is the source of truth. Sprint docs cite its
sections and do not restate them; a sprint that finds a gap raises it with
the lead, who changes the contract. Sprints never decide it locally.

- The core and store changes the contract needs (contract §9, "Core
  changes").
- A new crate, `bd-cache-api`. It holds one request, data and error
  vocabulary for every front end (contract §1 rule 6, §4, §5, §6, §9).
- A new crate, `bd-cache-cli`, with the binaries `bd-cache` and `bdc`, built
  on sc-command (contract §2, §3, §8).
- Python wheel 0.2.0, which adds `cache.beads.*` and `cache.manage.*` and
  leaves 0.1 unchanged (contract §10, §11).
- Binary releases: GitHub, Homebrew, Scoop and WinGet, plus a prerelease
  that carries the binaries.

## Prerequisites

| Id | Prerequisite | Blocks | Fallback if open at dispatch |
|---|---|---|---|
| P-1 | Done: contract approved, lead ruling S60 (2026-10-25). Plan against `8115a0a` or later. PR #69 merges to `develop` before `integrate/phase-b` is cut. | every sprint | none: cut `integrate/phase-b` only after #69 merges. |
| P-2 | Done: ADR-BDC-0009 and the contract's Appendix A amendments (REQ-BDC-0801, 0802, 0803; 0806 superseded), approved under S60 and marked Approved on #69. | — | — |
| P-3 | sc-publish#133: a crates-only release with no assets completes. Open. | nothing in b-5 directly | b-5 declares binaries, so every GitHub Release has assets and does not take the empty-asset path. b-5 stays on the current pin `16457e0` unless P-4 needs a bump. |
| P-4 | sc-publish staging bug: real releases upload the consumer's `release/` config as release assets. No issue filed yet; b-5 files it on randlee/sc-publish. | the first stable `release.yml` run with binaries (an immutable release cannot be corrected) | b-5 closes on the prerelease (`prerelease-archive.yml`) plus a clean `install.py --dry-run` and `validate-manifest` with the channels declared. The first stable release and its channel dispatches wait for a kit pin that contains the fix. |
| P-5 | sc-runtime request (non-blocking, contract §2 and Q7): an sc-config file source with an explicit path, where a missing file is allowed. | nothing | bdc applies the `BD_CACHE__*` env overrides itself (contract §2). |
| P-6 | External channel setup, by Rand: enable immutable releases on randlee/beads-cache; add repository secrets `HOMEBREW_TAP_TOKEN` (randlee/homebrew-tap, the tap the first consumer already uses), `SCOOP_BUCKET_TOKEN` (randlee/scoop-bucket) and `WINGET_GITHUB_TOKEN` (classic token with `public_repo`, from the owner of the winget-pkgs fork); fork microsoft/winget-pkgs and get the first version of the WinGet package merged by hand (`wingetcreate new`). | b-5's channel dispatches | b-5 declares the channels and proves them with the dry run; a channel whose setup is missing is dispatched later by tag, with no code change. |
| P-7 | sc-command at sc-runtime tag `v0.1.0` (`ae3e10b`) builds on all three CI OSes. | b-3a | b-3a checks this first. On a failure, pin by `rev` and raise the issue with the sc-runtime team; no local fork. |
| P-8 | Gated step before b-7: PR #97 merges into `develop`, then `develop` merges into `integrate/phase-b` (bc-lead owns the merge). That merge removes `integrate/phase-b`'s superseded `docs/plans/phase-b/sprint-b-7-sync-job.md` and `sprint-b-8-boundary.md`, which this plan's b-7..b-9 replace, and takes the planning branch's side of every doc conflict. This PR deletes the phase-a planted-bug evidence files in applying A8's no-consumer-wording ruling; that deletion awaits Rand's confirmation (Open items). | b-7, b-8, b-9 | none: b-7 is not cut until the merge is on `integrate/phase-b`. |

## Boundary map

| Boundary | Produced by | Contract | Consumed by |
|---|---|---|---|
| `bd-cache` / `bd-cache-model` / `bd-cache-store` additions | b-1 | [b-1 Public contract](sprint-b-1-core-store.md#public-contract) | b-2, b-3, b-4 |
| `bd-cache-sync::compose_with_config` | b-1 | b-1 Public contract | b-3 |
| `bd-cache-api` | b-2 | contract §4, §5, §6, §9; b-2's shared fixtures | b-3, b-4, the later MCP server |
| `bd-cache-cli` skeleton and `fake-bd` | b-3a | contract §8 Testing | b-3 |
| `bd-cache-cli` | b-3 | contract §2, §3, §8 | b-5, hooks, agents |
| `bd-cache-py` 0.2 | b-4 | contract §10 | the first consumer (its maintainer) |
| `sc-cron` hook target, `enabled` flag, stop decisions | b-6 | REQ-CRON-0013..0015 | b-7, b-8 |
| store extensions, scope filters, `CacheScheduleStore`, `set_enabled_many`, `replace_and_set_enabled` | b-7 | ADR-STORE-0006, ADR-BDC-0013, ADR-BDC-0014, REQ-CRON-0017 | b-8, b-9 |
| the sync rows, the switch, the post-sync hook, `cron run`, the new verbs, the Python methods and entry points; sc-cron's resolver and exported supervisor | b-8 | ADR-SYNC-0008, ADR-BDC-0015, ADR-PY-0006, ADR-CRON-0009 | b-9, consumers |
| the contract text of the extension, health, the lint gate | b-9 | contract §2, §4, §6, §10–§12; REQ-BDC-0732; ADR-BDC-0012 | consumers, release |

The crate edges are the Boundaries table in `docs/architecture.md`. PR #69
adds the rows for `bd-cache-api` and `bd-cache-cli`, and CI's boundary check
already reads that table for workspace edges. b-2 extends the check to
reject `sc-command` and `clap` in `bd-cache-api` and `bd-cache-py`, which
the workspace-edge check cannot see.

## Sprints and waves

| Sprint | Closure | Target boundary | Wave | Track | must_follow |
|---|---|---|---|---|---|
| [b-1](sprint-b-1-core-store.md) | boundary | core, model, store (+ one sync function) | 1 | core | — |
| [b-2](sprint-b-2-api.md) | boundary | `bd-cache-api` | 1 (merges after b-1) | api | — |
| [b-3a](sprint-b-3a-cli-harness.md) | boundary | `bd-cache-cli` skeleton, `fake-bd` | 1 | cli | — |
| [b-3](sprint-b-3-cli.md) | integration | `bd-cache-cli` | 2 | cli | b-1, b-2, b-3a |
| [b-4](sprint-b-4-python-0-2.md) | integration | `bd-cache-py` 0.2 | 2 | python | b-1, b-2 |
| [b-5](sprint-b-5-release-channels.md) | integration | release manifest and channels | 3 | release | b-3, b-4 |
| [b-6](sprint-b-6-sc-cron.md) | boundary | `sc-cron` (Amendment A2) | 4 | core | — |
| [b-7](sprint-b-7-store-contract.md) | boundary | store, scope filters, schedule store, mechanical caller edits (WP-6a) | 5 | core | b-6 |
| [b-8](sprint-b-8-tick-python.md) | integration | the sync rows, the switch, the Python surface, the removals, end to end (WP-6b, WP-9) | 6 | core | b-7 |
| [b-9](sprint-b-9-boundary-lint.md) | integration | neutral tests, contract text, health, lint gate (WP-7, WP-8, WP-10) | 7 (merges last) | docs | b-8 |

- Width: 3 in wave 1 (b-1, b-2, b-3a), 2 in wave 2 (b-3, b-4), 1 in waves 3 to 7.
- Critical path: b-1 → b-3 → b-5, then b-6 → b-7 → b-8 → b-9 (the extension, after P-8 for b-7).
- ~~Boundary amendment (2026-10-25): b-6 and b-8 run in parallel, b-7 follows b-6, and b-8 merges
  last.~~ *Superseded (A7, A8):* that schedule named `integrate/phase-b`'s earlier b-7 and b-8
  (b-6's `parallel_safe: [b-8]` refers to that earlier b-8, whose lint work is now b-9's). b-6 is
  merged; b-7, b-8 and b-9 run one after another with no parallel sprint, each green on its own.
  Each sprint is gated by the plan anchors in `docs/plans/phase-b/anchors/anchors.json`.
- b-2 codes against b-1's Public contract from day one. It merges to
  `integrate/phase-b` after b-1, and its write, journal and status
  operations are wired once b-1 has merged.

Change from the lead's split: the test harness is split out as **b-3a, and
moved to wave 1**. It depends only on `sc-bd-cli`'s `FakeBd` and on
sc-command, so it can start at once. It also proves the sc-command git pin
on all three OSes (P-7) before b-3 relies on it. b-3 keeps the CLI behaviour,
the named negative scenarios and the latency test, which need the harness.
The `compose_with_config` seam in `bd-cache-sync` moves to b-1, so b-3 edits
only its own crate.

## Shared files

| File | Owner | Rule |
|---|---|---|
| `Cargo.toml` `[workspace].members` and `[workspace.dependencies]` | b-2 (`bd-cache-api`, schemars), b-3a (`bd-cache-cli`, sc-command git dep at tag `v0.1.0`, clap) | Members stay listed literally (the kit rejects globs), in alphabetical order. The second of the two to merge resolves the trivial conflict. |
| `Cargo.toml` `[workspace.package].version` | b-4 | `0.2.0`, in lockstep with the wheel's static version. The shared crates take 0.2.0 too (S61). |
| `Cargo.lock` | every sprint that changes dependencies | Committed. Resolve a merge conflict by taking the target branch's lock and re-running `cargo metadata`, never `cargo update`. |
| `docs/architecture.md` | the planning branch (ADR text); b-2 (`bd-cache-api` rows), b-3 (`bd-cache-cli` rows, ADR-BDC-0009 implementation status), b-7 (`bd-cache-model`, `bd-cache-store`, `sc-beads`, `sc-bd-cli` rows), b-8 (`bd-cache-sync`, `bd-cache-py`, `bd-cache-cli`, `sc-cron`, `bd-cache-model` rows and the ADR implementation table) | A sprint edits only the rows its fence names; ADR text changes only on the planning branch. |
| `docs/bd-cache-cli/contract.md` | PR #69 / the lead; the planning branch (the phase-B pointer); b-7 and b-8 (contract §4 error-table rows only: b-7 adds the rows of `INGEST_INVALID`, `SCHEDULE_INVALID`, `TICK_RUN_FAILED` and `TICK_IN_FLIGHT` with the `error_table.rs` match those rows need; b-8 removes the `TICK_<REFUSAL>` row with that test's expectation); b-9 (§2, §4, §6, §10, §11, §12 from the approved register) | Read-only for every other sprint. Changes are contract revisions. The b-7 and b-8 exception is bounded to those §4 rows, so `crates/bd-cache-api/tests/error_table.rs` stays green after each sprint. |
| `docs/requirements.md` and every per-crate requirement and ADR text | plan branch only | A sprint that finds the register wrong raises it with the lead. |
| `.github/workflows/ci.yml` | b-2 (the "Crate boundaries" step: no `sc-command` / `clap` in `bd-cache-api` or `bd-cache-py`), b-3 (one new step: the release-profile NFR-BDC-0002 latency test), b-9 (the lint step, the no-`bd`/`dolt` and no-Dolt-client checks) | Nothing else. |
| `.github/workflows/python.yml` | b-4; b-9 (the compat jobs removed) | — |
| `release/**`, kit-installed workflows, `README.sc-publish.md` | b-5 | Kit files change only through `install.py`, never by hand. |
| `crates/bd-cache-sync/src/compose.rs` | b-1 | Adds `compose_with_config` only. |
| `crates/bd-cache-py/**` | b-4; then b-7 (mechanical edits), b-8 (`src/**`, the packaging files, `python/**`, the new pytest files), b-9 (every existing test file and the corpus) | b-1 may touch only test construction sites of the extended `JournalEntry` and `GetReply`. |
| `crates/sc-cron/**` | b-6; then b-7 (`store.rs`, `sqlite.rs`, `scheduler.rs` and the `RecordingStore` impls in `tests/worker.rs` and `tests/common/**`: `set_enabled_many` and `replace_and_set_enabled` only), b-8 (the resolver, the exported supervisor, the `Stop` removal), b-9 (comments, provenance, tests) | The sprints run in order, so the fences never overlap in time. |
| `crates/bd-cache-model/**` | b-7; then b-8 (`sender.rs`, `config.rs`, `lib.rs`: the removals) | — |
| `crates/bd-cache-api/**`, `crates/bd-cache-cli/**` | b-7 (mechanical edits, the code rows); b-8 (the new verbs, the removals); b-9 (`manage.rs` health, the outage scenario) | As the sprint docs' `owned_paths`. |

## Requirement ownership

Amended ids are as approved in the contract's Appendix A (S60). New ids are
added to `docs/requirements.md` by this plan. Each REQ and NFR the phase delivers or changes has
exactly one owning sprint; the others a sprint touches are in its `constrained_by`. An id that
an executed sprint delivered and the extension amends keeps its first owner for the text it had;
its phase-B amendment, marked "(amended)" in the extension's rows, has one owner of its own.

| Owner | Requirements |
|---|---|
| b-1 | REQ-BDC-0809 |
| b-2 | REQ-BDC-0803 (as amended), 0807, 0808, 0811; crate-level REQ-API-* |
| b-3a | crate-level REQ-CLI-* for the harness |
| b-3 | REQ-BDC-0801, 0802 (as amended), 0805, 0810; NFR-BDC-0002; crate-level REQ-CLI-* |
| b-4 | REQ-BDC-0812; crate-level REQ-PY-0011.. |
| b-5 | REQ-BDC-0813 |
| b-6 | REQ-CRON-0005, 0009, 0011, 0013..0015 |
| b-7 | REQ-BDC-0721, 0726, 0728, 0729; REQ-BDC-0501, 0602, 0603, 0804, 0812, 0901, 0909 and NFR-BDC-0007 (amended); REQ-STORE-0003, 0007 (amended); REQ-CRON-0017 |
| b-8 | REQ-BDC-0720, 0722..0725, 0727, 0730, 0731; REQ-BDC-0703, 0803, 0810 (amended); REQ-BDC-0701, 0702, 0704..0707 (superseded); REQ-SYNC-0001..0004, 0006..0012 (amended or superseded); REQ-CRON-0005 (amended), 0016, 0018; REQ-MODEL-0005, REQ-CLI-0006, 0008, REQ-PY-0002, 0005, 0008 and REQ-API-0001, 0005 (amended) |
| b-9 | REQ-BDC-0732; REQ-STORE-0006 (amended); REQ-PY-0009 (superseded) |
| superseded | REQ-BDC-0806 (S59, S60) |

So REQ-BDC-0803's phase-B amendment is b-8's (it removes the `TICK_<REFUSAL>` family),
REQ-BDC-0810's is b-8's and REQ-BDC-0812's is b-7's; REQ-CRON-0005's `Stop` removal is b-8's;
REQ-API-0001 and 0005, REQ-CLI-0006 and 0008 and REQ-PY-0002, 0005 and 0008 are amended by b-8.

## Lead rulings

All S-rulings are bc-lead, 2026-10-25, recording Rand's answers; S64–S81 were made during the
delivery of b-1..b-6 and are recorded in the requirements and ADRs they amend.

| Ruling | Decision | Where |
|---|---|---|
| S60 | The contract, ADR-BDC-0009 and the REQ-BDC-0801/0802/0803/0806 amendments are approved. | P-1, P-2 |
| S61 | One workspace version for everything. Wheel 0.2.0 means `[workspace.package].version` is 0.2.0, so sc-beads, sc-cron and sc-bd-cli also publish as 0.2.0. The crates are not decoupled and the kit is not changed. The bump goes in b-4, because the wheel build and `verify-version-lockstep` need the pyproject and workspace versions to match in the commit that ships the wheel. | b-4 deliverable 5 |
| S62 | A third additive nullable store column, `changed`, is accepted. It is named in contract §9 (amended on this branch). | b-1 deliverable 5 |
| S63 | Channel names: WinGet id `randlee.bd-cache`; Homebrew formula `bd-cache` in randlee/homebrew-tap; Scoop `bucket/bd-cache.json` in randlee/scoop-bucket. | b-5 |
| S64 | A write's one store transaction covers the journal entry and its `changed`; `before` and `after` are snapshots on either side of it. | REQ-BDC-0809 amendment |
| S65 | An entry's `note` is `resolved by <actor>: <note>`. | REQ-BDC-0809 amendment, ADR-STORE-0001 b-1 |
| S66 | The core facade's pending-entry and backend-check helpers. | `docs/bd-cache/` (ADR-CORE), `crates/bd-cache/tests/facade_s66.rs` |
| S67 | A scope with no completed sync is not served; a list never answers an empty list for it. | contract §7, `docs/bd-cache/read-path.md` |
| S68 | `changed` is unknown for an entry stored without it unless the entry is still pending, when it is predicted; it is never guessed. | REQ-BDC-0809 amendment |
| S69 | Over the bd CLI a refused close escalates as `PRECONDITION`. | contract §5.4 |
| S70 | `bd` is never read from the TOML. | contract §2 |
| S71 | bdc's own `CONFIG_INVALID` sources (`bdc_env_invalid`, `bdc_legacy_cache_file`). | contract §4 (the second is removed by b-8/b-9, ADR-BDC-0015) |
| S72 | `bd_cache_sync::compose_with_config_and_backend`, so the injected-invoker path honours env and flag overrides. | `docs/bd-cache-sync/architecture.md` |
| S73 | Recorded on PR #78 (b-3) with S69–S74; its text is in that PR. | PR #78 |
| S74 | `bd-cache-sync` is a workspace dependency. | root `Cargo.toml` (b-3) |
| S75 | A `cache refresh scopes` that cannot reach bd reports per-scope outage results. | contract §6 |
| S76, S77 | The extended `PATH` takes absolute entries only, the user's first. | ADR-BDCLI-0005 |
| S78, S79 | The release-profile latency gate runs on Unix only; Windows is report-only. | `docs/bd-cache-cli/architecture.md` |
| S80 | The Unix latency scenario amendment (QA FTQ-004). | `docs/bd-cache-cli/requirements.md` |
| S81 | A cross-process project lock, and process control without helper programs. | ADR-BDCLI-0006 |
| S82–S90 | The boundary extension r3: ADR-BDC-0010 and the lint, the tick on the sc-cron table, the frozen v3 store plus extensions, #86 kept, PRs #94 and #95 abandoned, the Python `sync` replacement through `ingest_scope`, the post-sync hook, registration and trust, one execution path and the tick lease (S87, S89 and S90 as amended by A7 and A8). | [phase-b-boundary.md](phase-b-boundary.md) §Lead rulings |
| A1–A6 | Rand's rulings after r3: `cron.tick` / `cron.stop` allowed; sc-cron general with the `enabled` flag and continue / disable / delete; one tick row per repository; enable and disable through the cron table; scopes as bd filters including `--all`; every deliverable justified by a REQ or ADR. | [phase-b-boundary.md](phase-b-boundary.md) §Amendments after r3 |
| A7 | Rand, 2026-10-26: the sync switch is two cron rows (a built-in row and the consumer's Python row, at most one enabled) with a CLI command as an app-level convenience that runs the Python row at once; sc-cron stays a general-purpose cron assembly with a Rust or Python target per row; PyO3 bindings for sc-cron come with sc-runtime. | [phase-b-boundary.md](phase-b-boundary.md) §A7 |
| A8 | Rand, 2026-10-26, on the PR #97 QA round: schedule storage through sc-cron's `ScheduleStore` in the cache file (`user_version` 3); architecture and requirements on the planning branch; the register approved; general sc-cron features allowed; `cron run`'s shutdown; no deliverable cut without his OK; no consumer wording except the lint's list; the 0.1 bare-name rule; the Python row's immediate run with rollback. | [phase-b-boundary.md](phase-b-boundary.md) §Amendments after r3; ADR-BDC-0012..0015 |

## Open items

- P-4: the sc-publish `release/` staging issue. b-5 files it, and the stable
  release waits for the fix.
- P-6: the external setup that Rand does: immutable releases, the three
  channel secrets, the winget-pkgs fork, and the first `randlee.bd-cache`
  submission made by hand.
- Ruled (Rand, 2026-10-26): `CONTRACT_VERSION` is 0.2.0, not 1.0.0, with wheel 0.2.0: "we are
  no-where near 1.0.0. we haven't done a single successful test w/ an actual consumer." The
  contract is pre-stable until then (contract §11).
- Ruled (Rand, 2026-10-26): phase B closes without the prerelease that carries `bd-cache` and
  `bdc` (REQ-BDC-0813, b-5 deliverable 4); the binaries are cut later from `develop` or `main`.
- This PR deleted `docs/plans/phase-a/evidence/a-7-planted-bugs.{json,py}` and
  the baseline design document (`docs/baseline/`) in applying A8's no-consumer-wording ruling; both stay in git
  history. Awaiting Rand's confirmation; neither is restored unless he says so.

## Deferred

- The MCP server, and everything in contract §12a. That includes `bdc schema`,
  `x-cli-flag`, `details_schema`, tool annotations and the MCP parity run.
- `bdc blocked`, `bdc issues batch-get` and the `id` filter on `issues list`,
  until a beads release ships them (contract §5.5). Until then the blocked
  set comes from `issues list` (contract §5.3, "Blocked beads").
- `cache journal list --state applied|resolved`, until the store has a
  listing for those states. Until then it is `UNSUPPORTED`.
- The other items of contract §12.

## QA consumption

As in phase A: each sprint doc lists `requirements.owns`,
`requirements.constrained_by`, `adrs` and `owned_paths`. Each acceptance
criterion has a positive observable and a planted negative: a deliberate
break that must make at least one test fail. The quality manager plants each
negative on a scratch branch and records the result in the QA report.
Reviewers follow `.claude/project/quality-policy.md`. A sprint doc cites the
green CI run on its last code-changing commit.
