# beads-cache Architecture

Status: draft. No architecture has been approved yet.

## Overview

beads-cache extracts the beads SQLite cache from atm-core, where it runs
today as part of the `atm-bd-orchestration` skill alongside an integrated TUI
and other orchestration features. The design is baselined on that in-use
implementation, limited to the cache functionality and delivered first as an
MVP; where beads-cache departs from the atm-core baseline, an ADR here
records why.

beads-cache is built against sc-runtime, which is being specified in
parallel. sc-runtime is an upstream dependency: its interfaces and
consumer contract constrain this architecture, and gaps go back to the
sc-runtime team as requests rather than local workarounds.

- Baseline (the contract): `.claude/skills/atm-bd-orchestration/scripts/BD-CACHE.md`
  in atm-core, introduced by https://github.com/randlee/atm-core/pull/2219.
  Reference implementation: `bd_cache.py`, `bd_models.py` and `bd_cache_cron.py`
  in the same directory. Acceptance suite: `tests/test_bd_cache.py` (41 tests)
  and `tests/test_bd_cache_cron.py` (9), with fixtures in
  `tests/fixtures/bd-1.3.1/`. Design tracker:
  https://github.com/randlee/atm-bd-orchestration/issues/557. Where BD-CACHE.md
  and the Python code disagree, report it on that issue.
- Upstream dependency: sc-runtime (https://github.com/randlee/sc-runtime).

CLI: the primary command is `bd-cache`, with the alias `bdc`.

Backend research: [bd serve HTTP API](research/bd-serve-http.md) and
[DoltHub sync without a local server](research/dolthub-sync.md).

To be filled: main components and how data flows between them.

## Crate Layout

To be filled once the Cargo workspace exists. List each crate, its
responsibility, and which crates it may depend on.

## Boundaries and Dependencies

To be filled: allowed dependency directions and public interfaces between
crates.

## Architecture Decision Records

Each ADR is its own section, `## ADR-<DOMAIN>-nnnn: Title`, where `<DOMAIN>`
is the product's domain code (to be chosen), or the crate's domain code in
per-crate docs. ADR template: `**Status:**` (Draft, Proposed, Approved,
Active, Superseded), context, decision, and consequences. An ADR changes only
through a later ADR that names the one it amends or supersedes.

None yet.

## Per-Crate Architecture

When the workspace declares per-crate docs (see
`.claude/project/quality-policy.md`), each crate's architecture lives in
`docs/<crate>/architecture.md` and is indexed here.
