# Phase B — bdc CLI, bd-cache-api, Python 0.2, release channels

Status: ready for dispatch. The contract, ADR-BDC-0009 and the REQ-BDC-0801,
0802, 0803 and 0806 amendments were approved by Rand (lead ruling S60,
2026-10-25).

Phase id: `b`. Plan branch: `plan/phase-b` (PR to `develop`). Phase branch:
`integrate/phase-b`, cut from `develop` after PR #69 (the contract) merges.
Sprint branches `sprint/b-<n>-<slug>`, cut from `integrate/phase-b`.

## Scope

Implement the bd-cache / bdc CLI contract,
[`docs/bd-cache-cli/contract.md`](../../bd-cache-cli/contract.md), contract
version 1.0.0, as approved at `8115a0a` and marked Approved on #69 at `6dec77e`. The contract is the source of truth. Sprint docs cite its
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
| P-6 | External channel setup, by Rand: enable immutable releases on randlee/beads-cache; add repository secrets `HOMEBREW_TAP_TOKEN` (randlee/homebrew-tap, the tap atm already uses), `SCOOP_BUCKET_TOKEN` (randlee/scoop-bucket) and `WINGET_GITHUB_TOKEN` (classic token with `public_repo`, from the owner of the winget-pkgs fork); fork microsoft/winget-pkgs and get the first version of the WinGet package merged by hand (`wingetcreate new`). | b-5's channel dispatches | b-5 declares the channels and proves them with the dry run; a channel whose setup is missing is dispatched later by tag, with no code change. |
| P-7 | sc-command at sc-runtime tag `prerelease/v0.1.0` (`ba971c1`) builds on all three CI OSes. | b-3a | b-3a checks this first. On a failure, pin by `rev` and raise the issue with the sc-runtime team; no local fork. |

## Boundary map

| Boundary | Produced by | Contract | Consumed by |
|---|---|---|---|
| `bd-cache` / `bd-cache-model` / `bd-cache-store` additions | b-1 | [b-1 Public contract](sprint-b-1-core-store.md#public-contract) | b-2, b-3, b-4 |
| `bd-cache-sync::compose_with_config` | b-1 | b-1 Public contract | b-3 |
| `bd-cache-api` | b-2 | contract §4, §5, §6, §9; b-2's shared fixtures | b-3, b-4, the later MCP server |
| `bd-cache-cli` skeleton and `fake-bd` | b-3a | contract §8 Testing | b-3 |
| `bd-cache-cli` | b-3 | contract §2, §3, §8 | b-5, hooks, agents |
| `bd-cache-py` 0.2 | b-4 | contract §10 | atm-bd (fenix) |

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

- Width: 3 in wave 1 (b-1, b-2, b-3a), 2 in wave 2 (b-3, b-4), 1 in wave 3.
- Critical path: b-1 → b-3 → b-5.
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
| `Cargo.toml` `[workspace].members` and `[workspace.dependencies]` | b-2 (`bd-cache-api`, schemars), b-3a (`bd-cache-cli`, sc-command git dep at tag `prerelease/v0.1.0`, clap) | Members stay listed literally (the kit rejects globs), in alphabetical order. The second of the two to merge resolves the trivial conflict. |
| `Cargo.toml` `[workspace.package].version` | b-4 | `0.2.0`, in lockstep with the wheel's static version. |
| `Cargo.lock` | every sprint that changes dependencies | Committed. Resolve a merge conflict by taking the target branch's lock and re-running `cargo metadata`, never `cargo update`. |
| `docs/architecture.md` | b-2 (`bd-cache-api` rows), b-3 (`bd-cache-cli` rows, ADR-BDC-0009 implementation status) | Nobody else edits it. |
| `docs/bd-cache-cli/contract.md` | PR #69 / the lead | Read-only for sprints. Changes are contract revisions. |
| `docs/requirements.md` | plan branch only | — |
| `.github/workflows/ci.yml` | b-2 (the "Crate boundaries" step: no `sc-command` / `clap` in `bd-cache-api` or `bd-cache-py`), b-3 (one new step: the release-profile NFR-BDC-0002 latency test) | Nothing else. |
| `.github/workflows/python.yml` | b-4 | — |
| `release/**`, kit-installed workflows, `README.sc-publish.md` | b-5 | Kit files change only through `install.py`, never by hand. |
| `crates/bd-cache-sync/src/compose.rs` | b-1 | Adds `compose_with_config` only. |
| `crates/bd-cache-py/**` | b-4 | b-1 may touch only test construction sites of the extended `JournalEntry` and `GetReply`. |

## Requirement ownership

Amended ids are as approved in the contract's Appendix A (S60). New ids are
added to `docs/requirements.md` by this plan.

| Owner | Requirements |
|---|---|
| b-1 | REQ-BDC-0809 |
| b-2 | REQ-BDC-0803 (as amended), 0807, 0808, 0811; crate-level REQ-API-* |
| b-3a | crate-level REQ-CLI-* for the harness |
| b-3 | REQ-BDC-0801, 0802 (as amended), 0805, 0810; NFR-BDC-0002; crate-level REQ-CLI-* |
| b-4 | REQ-BDC-0812; crate-level REQ-PY-0011.. |
| b-5 | REQ-BDC-0813 |
| superseded | REQ-BDC-0806 (S59, S60) |

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
