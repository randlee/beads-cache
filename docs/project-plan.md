# beads-cache Project Plan

Status: phase a in progress (see the Phases and Sprints tables).

## Context

beads-cache extracts the beads SQLite cache in use today in atm-core's
`atm-bd-orchestration` skill into its own project, baselined on that
implementation and delivered first as an MVP. It is built against
sc-runtime, which is being specified in parallel, so phases here may wait
on sc-runtime milestones; record each such dependency in the phase row.

- Baseline (the contract): `.claude/skills/atm-bd-orchestration/scripts/BD-CACHE.md`
  in atm-core, introduced by https://github.com/randlee/atm-core/pull/2219.
  Reference implementation: `bd_cache.py`, `bd_models.py` and `bd_cache_cron.py`
  in the same directory. Acceptance suite: `tests/test_bd_cache.py` (44 tests)
  and `tests/test_bd_cache_cron.py` (13), with fixtures in
  `tests/fixtures/bd-1.3.1/`. Design tracker:
  https://github.com/randlee/atm-bd-orchestration/issues/557. Where BD-CACHE.md
  and the Python code disagree, report it on that issue.
- Upstream dependency: sc-runtime (https://github.com/randlee/sc-runtime).

## Conventions

Naming follows the Plan Naming section of
`.claude/project/quality-policy.md`. Phase plans live in
`docs/plans/phase-<phase>/`; the integration branch is `develop`; `main` receives releases from `develop`.

## Phases

| Phase | Goal | Plan | Status |
|---|---|---|---|
| `a` | Crates with no sc-runtime dependency: model, the shared `sc-beads` traits and `sc-bd-cli` implementation (seeded from btit, published for btit to use), SQLite store, core library, an `sc-cron` prototype and the sync task on it (see Crate Layout in `docs/architecture.md`) | [phase-a-plan](plans/phase-a/phase-a-plan.md) | in progress |

## Sprints

| Sprint | Phase | Deliverable | Branch | Status |
|---|---|---|---|---|
| [a-1](plans/phase-a/sprint-a-1-contracts.md) | a | contracts: sc-beads, bd-cache-model, bd-cache skeleton | `sprint/a-1-contracts` | in progress |
| [a-2](plans/phase-a/sprint-a-2-sc-bd-cli.md) | a | sc-bd-cli | `sprint/a-2-sc-bd-cli` | planned |
| [a-3](plans/phase-a/sprint-a-3-sc-cron.md) | a | sc-cron prototype | `sprint/a-3-sc-cron` | in progress |
| [a-4](plans/phase-a/sprint-a-4-store.md) | a | bd-cache-store | `sprint/a-4-store` | planned |
| [a-5](plans/phase-a/sprint-a-5-core-write.md) | a | bd-cache write path | `sprint/a-5-core-write` | planned |
| [a-6](plans/phase-a/sprint-a-6-core-read.md) | a | bd-cache read path | `sprint/a-6-core-read` | planned |
| [a-7](plans/phase-a/sprint-a-7-sync-integration.md) | a | bd-cache-sync and integration | `sprint/a-7-sync-integration` | planned |
| [a-8](plans/phase-a/sprint-a-8-python-bindings.md) | a | bd-cache-py bindings | `sprint/a-8-python-bindings` | in-review |
