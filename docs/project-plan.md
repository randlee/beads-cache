# beads-cache Project Plan

Status: phase a complete; phase b in progress (see the Phases and Sprints tables).

## Context

beads-cache extracts the beads SQLite cache in use today in its first
consumer into its own project, baselined on that
implementation and delivered first as an MVP. It is built against
sc-runtime, which is being specified in parallel, so phases here may wait
on sc-runtime milestones; record each such dependency in the phase row.

- Baseline (the contract): the baseline design, the first consumer's design
  document for the cache. Reference implementation: the baseline Python
  implementation (the cache, its models and its cron module). Acceptance
  suite: the baseline acceptance suite (44 tests) and the baseline cron suite
  (13), with fixtures for bd 1.3.1. Where the baseline design and the Python
  code disagree, the disagreement is reported to the first consumer's design
  tracker. Their locations are recorded in git history.
- Upstream dependency: sc-runtime (https://github.com/randlee/sc-runtime).

## Conventions

Naming follows the Plan Naming section of
`.claude/project/quality-policy.md`. Phase plans live in
`docs/plans/phase-<phase>/`; the integration branch is `develop`; `main` receives releases from `develop`.

## Phases

| Phase | Goal | Plan | Status |
|---|---|---|---|
| `a` | Crates with no sc-runtime dependency: model, the shared `sc-beads` traits and `sc-bd-cli` implementation (seeded from btit, published for btit to use), SQLite store, core library, an `sc-cron` prototype and the sync task on it (see Crate Layout in `docs/architecture.md`) | [phase-a-plan](plans/phase-a/phase-a-plan.md) | complete |
| `b` | The `bdc` CLI contract, `bd-cache-api`, the `bdc` binaries, Python wheel 0.2 and the release channels; then the boundary extension: no consumer concern in beads-cache, the bd sync on sc-cron rows in the cache file with a Python replacement and a post-sync hook, scopes as bd filters, and the lint gate | [phase-b-plan](plans/phase-b/phase-b-plan.md), [phase-b-boundary](plans/phase-b/phase-b-boundary.md) | in progress (b-1..b-7 merged into `integrate/phase-b`; b-8 in review; b-9 planned) |

## Sprints

| Sprint | Phase | Deliverable | Branch | Status |
|---|---|---|---|---|
| [a-1](plans/phase-a/sprint-a-1-contracts.md) | a | contracts: sc-beads, bd-cache-model, bd-cache skeleton | `sprint/a-1-contracts` | complete |
| [a-2](plans/phase-a/sprint-a-2-sc-bd-cli.md) | a | sc-bd-cli | `sprint/a-2-sc-bd-cli` | complete |
| [a-3](plans/phase-a/sprint-a-3-sc-cron.md) | a | sc-cron prototype | `sprint/a-3-sc-cron` | complete |
| [a-4](plans/phase-a/sprint-a-4-store.md) | a | bd-cache-store | `sprint/a-4-store` | complete |
| [a-5](plans/phase-a/sprint-a-5-core-write.md) | a | bd-cache write path | `sprint/a-5-core-write` | complete |
| [a-6](plans/phase-a/sprint-a-6-core-read.md) | a | bd-cache read path | `sprint/a-6-core-read` | complete |
| [a-7](plans/phase-a/sprint-a-7-sync-integration.md) | a | bd-cache-sync and integration | `sprint/a-7-sync-integration` | complete |
| [a-8](plans/phase-a/sprint-a-8-python-bindings.md) | a | bd-cache-py bindings | `sprint/a-8-python-bindings` | complete |
| [b-1](plans/phase-b/sprint-b-1-core-store.md) | b | core, model and store changes for the contract | `sprint/b-1-core-store` | complete (#75) |
| [b-2](plans/phase-b/sprint-b-2-api.md) | b | bd-cache-api | `sprint/b-2-api` | complete (#76) |
| [b-3a](plans/phase-b/sprint-b-3a-cli-harness.md) | b | bd-cache-cli skeleton and fake-bd | `sprint/b-3a-cli-harness` | complete (#74) |
| [b-3](plans/phase-b/sprint-b-3-cli.md) | b | bd-cache-cli | `sprint/b-3-cli` | complete (#78) |
| [b-4](plans/phase-b/sprint-b-4-python-0-2.md) | b | bd-cache-py 0.2 | `sprint/b-4-python-0-2` | complete (#77) |
| [b-5](plans/phase-b/sprint-b-5-release-channels.md) | b | release manifest and channels | `sprint/b-5-release-channels` | complete (#80) |
| [b-6](plans/phase-b/sprint-b-6-sc-cron.md) | b | sc-cron: Python hook target, enabled flag, stop decisions | `sprint/b-6-sc-cron` | complete (#92) |
| [b-7](plans/phase-b/sprint-b-7-store-contract.md) | b | store extensions, scope filters, schedule store | `sprint/b-7-store-contract` | complete (merged as PR #99 into `integrate/phase-b`) |
| [b-8](plans/phase-b/sprint-b-8-tick-python.md) | b | the sync rows, the switch, the Python surface, end to end | `sprint/b-8-tick-python` | complete (PR to `integrate/phase-b`) |
| [b-9](plans/phase-b/sprint-b-9-boundary-lint.md) | b | neutral tests, contract text, health, lint gate | `sprint/b-9-boundary-lint` | planned |
