---
id: b-8
phase: b
status: planned
closure_type: integration
target_boundary: repository
track: docs
wave: 4
branch: sprint/b-8-boundary
worktree: sprint/b-8-boundary
recommended_agent: rust-developer
recommended_model: sonnet
must_follow: []
parallel_safe: [b-6, b-7]
requirements:
  owns: ["REQ-BDC-0804 (amended)"]
  constrained_by: [REQ-BDC-0812, NFR-BDC-0005]
adrs: [ADR-BDC-0010, ADR-BDC-0009, ADR-STORE-0001]
owned_paths:
  - "docs/requirements.md"
  - "docs/architecture.md"
  - "docs/bd-cache-cli/**"
  - "docs/baseline/**"
  - "docs/research/**"
  - "crates/bd-cache-store/tests/compat*"
  - "crates/bd-cache-py/tests/**"
  - "crates/sc-beads/**"
  - "crates/sc-bd-cli/**"
  - "scripts/**"
  - ".github/workflows/ci.yml"
  - "README.md"
  - "CHANGELOG.md"
---

# Sprint b-8 — boundary cleanup and the no-ATM lint gate

## Outcome

No ATM concern remains in beads-cache source, tests, scripts or product docs. The no-ATM lint
runs in CI so none come back (ADR-BDC-0010). It merges last, after b-6 and b-7.

## Deliverables

1. **#86** (REQ-BDC-0804 amended).
   - Merge `fix/fen-api`: health with pending age, last drain and unresolved escalations, and
     `refresh scopes --timeout-ms`.
   - Delete its cutover runbook, and rewrite contract §10 as "0.1 → 0.2 API differences",
     naming no consumer.
   - Both commands are cache-management surface commands, apart from the bd /v0-shaped beads
     surface (ADR-BDC-0009). `health` only reads cache state through bd-cache-api.
     `--timeout-ms` is the deadline for that refresh: each bd call it makes is killed at its
     timeout capped by the time left (NFR-BDC-0004). Both commands reach the backend only
     through bd, never through Dolt tables (REQ-BDC-0006). Neither changes the sync job, its schedule or
     budget, how scopes are defined, or how the cache reaches the backend; those are b-7's.
2. **Neutral compatibility tests** (REQ-BDC-0804 amended, ADR-STORE-0001).
   - Delete the vendored compat suite and `crates/bd-cache-py/tests/atm_core_05831df_*`.
   - Replace them with a format golden (the exact v3 DDL plus the extensions) and a neutral
     0.1 call corpus generated against fake-bd, with no loss of verb coverage (REQ-BDC-0812).
3. **Register amendments** (REQ-BDC-0804 amended). Apply REQ-BDC-0701..0707 and 0804 as listed
   in `doc/plan/phase-current/register-amendments.md`. Neutralise the docs, ADRs, comments and
   provenance notes, and delete `docs/baseline/BD-CACHE.md`.
   - The `bdc` contract keeps its two surfaces (ADR-BDC-0009).
   - `docs/team-protocol.md` stays as it is, exempt from the lint.
4. **Causal tests** (NFR-BDC-0005). `bd-cache-py/tests/test_additive.py:82-88` and
   `job_threading.rs` `join_idle` become causal.
5. **Lint gate** (ADR-BDC-0010). Add the CI step "Strict consumer boundary (ADR-BDC-0010)";
   `python3 scripts/lint_no_atm.py` exits 0 on the whole tree.

## Tests

- The format golden matches the v3 DDL, and `PRAGMA user_version` stays 3.
- The neutral corpus covers every 0.1 verb.
- The lint and its unit tests pass.

## Does not close

The sync job (b-7) and sc-cron (b-6).
