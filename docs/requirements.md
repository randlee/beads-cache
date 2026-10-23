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

beads-cache is built against sc-runtime, which is being specified in
parallel. Needs that sc-runtime does not yet meet go to the sc-runtime team
as requests; beads-cache does not work around them locally.

Baseline: the beads cache in atm-core's `atm-bd-orchestration` skill (link to be added).
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
