# beads-cache Project Plan

Status: no phases planned yet.

## Context

beads-cache extracts the beads SQLite cache in use today in atm-core's
`atm-bd-orchestration` skill into its own project, baselined on that
implementation and delivered first as an MVP. It is built against
sc-runtime, which is being specified in parallel, so phases here may wait
on sc-runtime milestones; record each such dependency in the phase row.

- Baseline (the contract): `.claude/skills/atm-bd-orchestration/scripts/BD-CACHE.md`
  in atm-core, introduced by https://github.com/randlee/atm-core/pull/2219.
  Reference implementation: `bd_cache.py`, `bd_models.py` and `bd_cache_cron.py`
  in the same directory. Acceptance suite: `tests/test_bd_cache.py` (41 tests)
  and `tests/test_bd_cache_cron.py` (9), with fixtures in
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
| _none yet_ | | | |

## Sprints

| Sprint | Phase | Deliverable | Branch | Status |
|---|---|---|---|---|
| _none yet_ | | | | |
