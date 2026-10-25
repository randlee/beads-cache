---
id: b-4
phase: b
status: ready
closure_type: integration
target_boundary: bd-cache-py 0.2
track: python
wave: 2
branch: sprint/b-4-python-0-2
worktree: sprint/b-4-python-0-2
recommended_agent: rust-developer
recommended_model: sonnet
must_follow: [b-1, b-2]
parallel_safe: [b-3]
requirements:
  owns: [REQ-BDC-0812, "REQ-PY-0011.. (created here)"]
  constrained_by: [REQ-BDC-0004, REQ-BDC-0803, REQ-BDC-0807, REQ-BDC-0808, REQ-BDC-0809, REQ-BDC-0811, REQ-BDC-0910, REQ-PY-0001, REQ-PY-0009, REQ-PY-0010]
adrs: [ADR-BDC-0009, ADR-PY-0001, ADR-PY-0002, ADR-PY-0003, ADR-PY-0004, "ADR-PY-0005 (created here: the namespaces and the ApiError → CacheError mapping)"]
owned_paths:
  - "crates/bd-cache-py/**"
  - "docs/bd-cache-py/**"
  - ".github/workflows/python.yml"
  - "Cargo.toml"     # [workspace.package].version = "0.2.0" only
  - "Cargo.lock"
---

# Sprint b-4 — Python wheel 0.2

## Outcome

Wheel `bd-cache-rs` 0.2.0 adds `cache.beads.*` and `cache.manage.*` over
`bd-cache-api` (contract §10). Every 0.1 verb and every 0.1 dict is
unchanged, and 0.2 issues no `DeprecationWarning`. fenix builds against 0.1,
so this sprint is additive only.

## Deliverables

1. `cache.beads` and `cache.manage` namespace objects on the cache handle.
   - A method's name is the command path joined with `_`; surface (b) drops
     the `cache_` prefix.
   - Each method takes the request as keyword arguments, sends it through
     `bd-cache-api`'s decoder, and returns `data` as a dict.
   - `cache tick` is not exposed.
   - File: `crates/bd-cache-py/src/api.rs`, registered in `src/lib.rs`.
2. Scope ids: every method, old and new, accepts a bare id (`bk`) or a label
   (`phase-bk`).
3. `CacheError` gains the attributes `kind`, `message`, `details`,
   `suggested_action` and `retryable`. These map from `ApiError`, using
   b-2's error table for `retryable`. A Python type mistake still raises
   `TypeError`. File: `src/error.rs`.
4. `bd_cache_rs.CONTRACT_VERSION` (REQ-BDC-0811).
5. Version 0.2.0 (lead ruling S61, bc-lead, 2026-10-25). There is one
   workspace version for everything, so `[workspace.package].version`
   becomes 0.2.0, and sc-beads, sc-cron and sc-bd-cli also publish as
   0.2.0. The crates are not decoupled and the kit is not changed.
   - The bump is in b-4, not b-5. The wheel build's `sync-python-version` and
     the kit's `verify-version-lockstep` both need the static
     `pyproject.toml` version to equal the workspace version, so the two
     change together in the commit that ships wheel 0.2.
   - b-5 then cuts its prerelease from 0.2.0; the tag script patch-bumps
     as needed.
6. Tests:
   - `tests/test_api_fixtures.py` replays b-2's shared fixtures through the
     namespaces.
   - `tests/test_additive.py` covers the 0.1 compatibility checks.
   - Docs: `docs/bd-cache-py/{requirements,architecture}.md` (REQ-PY-0011..,
     ADR-PY-0005) and the 0.1 → 0.2 table from contract §10.

## Acceptance criteria

| # | Root | Positive observable | Planted negative |
|---|---|---|---|
| 1 | REQ-BDC-0812 | The 0.1 test files run unedited and pass (`git diff` of the 0.1 test files against the base is empty). The 0.1 dict key sets equal those pinned by b-1. | Rename `scope_last_ok_at` in the 0.1 `get` dict → a 0.1 test fails. |
| 2 | REQ-BDC-0812 | `python -W error::DeprecationWarning -m pytest` passes. | Add a `DeprecationWarning` to `get` → the run fails. |
| 3 | REQ-BDC-0807 | Each shared fixture gives the same `data`, or the same `CacheError.code`, through Python as through `bd-cache-api`. | Validate an argument in Python before the decoder, with a different code → that fixture fails. |
| 4 | contract §10 units | `cache.beads.issues_get(id=…, miss_timeout_ms=0)` on an uncached bead raises `CacheError` with code `BD_CACHE.NOT_CACHED` and `retryable` true. 0.1's `get(bead, miss_timeout=0.0)` keeps its 0.1 behaviour. | Read `miss_timeout_ms` as seconds → the timing assertion fails (a 200 ms wait becomes 200 s, cut off by the test's timeout). |
| 5 | contract §10 scope ids | `cache.beads.issues_list(scope="bk")` and `scope="phase-bk"` return the same rows, and so does the 0.1 `list("phase-bk")`. | Accept only the label form → the bare-id case raises `SCOPE_UNKNOWN`. |
| 6 | REQ-BDC-0811 | `CONTRACT_VERSION` equals `cache.beads.context()["x_cache"]["contract_version"]`. | Hard-code a different version string → the test fails. |
| 7 | packaging | The wheel builds in `python.yml` at 0.2.0, and the kit's `verify-version-lockstep` passes. | Bump only `pyproject.toml` → `verify-version-lockstep` fails. |

## Dependencies

b-1 (the reply fields, kept out of the 0.1 dicts) and b-2 (the crate and
its fixtures).
