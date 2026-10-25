# beads-cache Project Plan

Status: phase a in progress (see the Phases and Sprints tables).

## Context

beads-cache is the beads SQLite cache: a cache, a CLI for configuration and
management, and a PyO3 API (ADR-BDC-0010), delivered first as an MVP. It is built against
sc-runtime, which is being specified in parallel, so phases here may wait
on sc-runtime milestones; record each such dependency in the phase row.

- Store format: BD-CACHE v3 (`PRAGMA user_version` 3), pinned by the DDL golden
  in `crates/bd-cache-store/tests/compat_format/` (ADR-STORE-0001).
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
| [a-1](plans/phase-a/sprint-a-1-contracts.md) | a | contracts: sc-beads, bd-cache-model, bd-cache skeleton | `sprint/a-1-contracts` | complete |
| [a-2](plans/phase-a/sprint-a-2-sc-bd-cli.md) | a | sc-bd-cli | `sprint/a-2-sc-bd-cli` | complete |
| [a-3](plans/phase-a/sprint-a-3-sc-cron.md) | a | sc-cron prototype | `sprint/a-3-sc-cron` | complete |
| [a-4](plans/phase-a/sprint-a-4-store.md) | a | bd-cache-store | `sprint/a-4-store` | complete |
| [a-5](plans/phase-a/sprint-a-5-core-write.md) | a | bd-cache write path | `sprint/a-5-core-write` | complete |
| [a-6](plans/phase-a/sprint-a-6-core-read.md) | a | bd-cache read path | `sprint/a-6-core-read` | complete |
| [a-7](plans/phase-a/sprint-a-7-sync-integration.md) | a | bd-cache-sync and integration | `sprint/a-7-sync-integration` | in-review |
| [a-8](plans/phase-a/sprint-a-8-python-bindings.md) | a | bd-cache-py bindings | `sprint/a-8-python-bindings` | complete |
