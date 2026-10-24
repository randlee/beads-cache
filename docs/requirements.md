# beads-cache Requirements

Status: draft. No requirements have been approved yet.

## Scope

beads-cache extracts the beads SQLite cache that is in use today inside
atm-core, where it is part of the `atm-bd-orchestration` skill (which also
carries an integrated TUI and other orchestration features). The baseline is
the cache as atm-core runs it now: requirements here are drawn from that
implementation and scoped to the cache functionality, delivered first as a
minimum viable product (MVP). Anything left out of the MVP is recorded below
as out of scope, not dropped silently.

beads-cache is limited to the SQLite beads cache application. The hooks,
TUI and other orchestration features of `atm-bd-orchestration` stay in
atm-core. The application runs a sync task that keeps the cache in step with
a dolt server, which may be local or remote.
Its value is as a filtered local cache: it holds only the beads in its
configured sync scopes (several `phase-*` scopes at once today), because bd
databases are too large to copy whole. Besides the `bd-cache`/`bdc` CLI, a
later front end is an HTTPS MCP server that serves the local cache.

beads-cache is built against sc-runtime, which is being specified in
parallel. Needs that sc-runtime does not yet meet go to the sc-runtime team
as requests; beads-cache does not work around them locally.

Baseline (the contract): `.claude/skills/atm-bd-orchestration/scripts/BD-CACHE.md`
in atm-core, introduced by https://github.com/randlee/atm-core/pull/2219.
Reference implementation: `bd_cache.py`, `bd_models.py` and `bd_cache_cron.py`
in the same directory. Acceptance suite: `tests/test_bd_cache.py` (41 tests)
and `tests/test_bd_cache_cron.py` (9), with fixtures in
`tests/fixtures/bd-1.3.1/`. Design tracker:
https://github.com/randlee/atm-bd-orchestration/issues/557. Where BD-CACHE.md
and the Python code disagree, report it on that issue.
Upstream dependency: sc-runtime (https://github.com/randlee/sc-runtime).

### Out of Scope for the MVP

To be filled as requirements are drawn from the atm-core baseline.

## Requirement ID Convention

- Functional requirements: `REQ-<DOMAIN>-nnnn`; non-functional
  requirements: `NFR-<DOMAIN>-nnnn`. `<DOMAIN>` is the product's domain code
  (to be chosen), or the crate's domain code in per-crate docs.
- Each requirement is its own section: `## REQ-<DOMAIN>-nnnn: Title`.
- IDs are never reused. A retired requirement keeps its ID with
  `**Status:** Retired` and a pointer to its replacement.
- Requirement template: each requirement carries `**Status:**` (Draft, Approved, Active, Retired),
  a one-sentence testable statement, and its acceptance evidence.

## Functional Requirements

To be filled.

## Non-Functional Requirements

To be filled.

## Per-Crate Requirements

When the workspace declares per-crate docs (see
`.claude/project/quality-policy.md`), each crate's requirements live in
`docs/<crate>/requirements.md` and are indexed here.
