---
id: b-2
phase: b
status: ready
closure_type: boundary
target_boundary: bd-cache-api
track: api
wave: 1
branch: sprint/b-2-api
worktree: sprint/b-2-api
recommended_agent: rust-developer
recommended_model: opus
must_follow: []          # codes against b-1's Public contract; merges after b-1
parallel_safe: [b-1, b-3a]
requirements:
  owns: [REQ-BDC-0803, REQ-BDC-0807, REQ-BDC-0808, REQ-BDC-0811]
  constrained_by: [REQ-BDC-0004, REQ-BDC-0802, REQ-BDC-0809, REQ-BDC-0902, REQ-BDC-0907, REQ-BDC-0908, REQ-BDC-0911, REQ-BDC-0912]
adrs: [ADR-BDC-0002, ADR-BDC-0003, ADR-BDC-0005, ADR-BDC-0009, ADR-MODEL-0002, "ADR-API-* (created here)"]
owned_paths:
  - "crates/bd-cache-api/**"
  - "docs/bd-cache-api/**"
  - "Cargo.toml"            # the bd-cache-api member, its workspace dep, schemars
  - "Cargo.lock"
  - "docs/architecture.md"  # the bd-cache-api Crate Layout row and its ADR-BDC-0009 status only
  - ".github/workflows/ci.yml"  # the Crate boundaries step only
---

# Sprint b-2 — bd-cache-api

## Outcome

A new crate, `bd-cache-api`, holds the contract's request, data and error
vocabulary and its operations over `Cache` (contract §9). Every front end
calls it: the CLI (b-3), the bindings (b-4) and, later, the MCP server. Its
dependencies are core, model and sc-beads only, with no sc-command and no
clap (the Boundaries row from #69).

## Deliverables

1. **Request and data models** for every command in contract §5.3 and §6,
   except `cache tick`, whose reply type belongs to `bd-cache-sync`. Files:
   `crates/bd-cache-api/src/request/{beads,manage}.rs` and
   `src/data/{beads,manage,entry,x_cache}.rs`. The rules on absent and null
   follow contract §1 rule 3.
2. **One decoder**, from a JSON request to a typed request (REQ-BDC-0807,
   contract §1 rules 5 and 6).
   - An unknown member is `BAD_ARGS` with `details.member`.
   - A served `/v0` name the cache does not serve is `UNSUPPORTED` with
     `details.unsupported`.
   - The idempotency key is required.
   - File: `src/decode.rs`.
3. **Operations**: one function per command over `&mut Cache<S, L, B>`,
   returning `Result<Data, ApiError>`. The write operations use
   `write_returning`; `journal get` uses `entry_by_key`; `status` uses
   `journal_counts`. `context` takes the `metadata.json` facts as an input
   struct, because the api crate never opens files.
   - Files: `src/ops/{beads,manage}.rs`.
4. **Filters, paging and order** (REQ-BDC-0808, contract §5.3).
   - The filters run on top of `Cache::list(scope)`.
   - The orders are total.
   - The keyset cursor is opaque `v1.`.
   - Inside a scope the default is no limit.
   - Files: `src/filter.rs`, `src/page.rs`.
5. **`/v0` name mapping.** The stored BD-CACHE args are mapped to the `/v0`
   patch for `Entry.args` (for example, `set_metadata` becomes
   `metadata.set`), and the patch is mapped back for `write_returning`
   (contract §6 `Entry`). File: `src/v0.rs`.
6. **The error table and its mappings** (REQ-BDC-0803, contract §4).
   - `ApiError {kind, code, message, details, suggested_action}`.
   - `ERROR_TABLE`: code, kind, exit and retryable.
   - `From<CacheError>`, field by field as in contract §4.
   - `ApiError::config_invalid(source, …)`, so the front ends can map the
     store's and sync's config errors without this crate depending on them.
   - `ApiError::tick_refusal(code)`, for the `TICK_*` codes.
   - File: `src/error.rs`.
7. **`CONTRACT_VERSION`** = `"1.0.0"`, included in the `context` and
   `cache status` data (REQ-BDC-0811). File: `src/lib.rs`.
8. **Shared fixtures**: request JSON plus the expected `data` or error, under
   `crates/bd-cache-api/tests/fixtures/`. They run here against `FakeStore` and
   `FakeBackend`. b-3 replays them through `run()`, and b-4 through Python.
9. **CI edge check**: extend the "Crate boundaries" step in
   `.github/workflows/ci.yml`. The existing step covers workspace edges only,
   so add a check that `bd-cache-api` and `bd-cache-py` have no direct
   dependency on `sc-command` or `clap` (contract §9).
10. **Docs**: `docs/bd-cache-api/requirements.md` (REQ-API-*) and
   `architecture.md` (ADR-API-*, the module table). Also the `bd-cache-api`
   row in `docs/architecture.md`, with Proposed removed.

## Acceptance criteria

| # | Root | Positive observable | Planted negative |
|---|---|---|---|
| 1 | REQ-BDC-0807 | Each fixture's decode gives the fixture's code: `BAD_ARGS` for an unknown member, `UNSUPPORTED` for `claim`, `expected_version` or `include_comments`, and `BAD_ARGS` for an empty patch or `status: "closed"`. | Ignore unknown members → the unknown-member fixture returns `ok`, and the test fails. |
| 2 | REQ-BDC-0004 | A write request without `idempotency_key` is refused. | Derive a key from the request content → the fixture fails. |
| 3 | REQ-BDC-0808 | Over 250 rows with priority and `created_at` ties, the pages walked with `next_cursor` and `limit: 7` concatenate to exactly the unpaged order, with no duplicates and no gaps, for both sorts. A tampered cursor is `BAD_ARGS` with `v0_code: invalid_cursor`. | Drop the `id` tie-break → the page walk shows a duplicate or a gap, and the test fails. |
| 4 | contract §5.3 `issues list` | By default closed rows, templates and `gate` rows are excluded; `status` or `all` lifts the exclusion; a scope is required (`SCOPE_UNKNOWN` without one); a `phase-<x>` label selects the scope and still filters. | Exclude nothing by default → the default-exclusion fixture fails. |
| 5 | REQ-BDC-0803 | `ERROR_TABLE` equals the contract §4 table, through a test that parses the table out of `docs/bd-cache-cli/contract.md`. Every `CacheError` code maps to a code in the table. | Change `SCOPE_UNKNOWN`'s kind to `not_found` → the parse test fails. |
| 6 | contract §4 mapping | `CacheError {reason, unmet, phase}` maps to `details.v0_code`, `details.unmet` and `details.scope` (label form). | Leave `phase` in bare form → the fixture fails. |
| 7 | contract §6 `Entry` | A stored `set_metadata` entry reads back as `args.metadata.set`. A `/v0` patch decoded and journaled reads back identically. | Map `unset_metadata` to `metadata.set` → the round-trip test fails. |
| 8 | REQ-BDC-0809 / §5.1 | A duplicate write returns `x_cache.duplicate: true` and the stored `changed`. | Take `changed` from `after != before` on a duplicate → the fixture fails. |
| 9 | boundary | The CI boundary step passes, and it now also rejects a direct dependency (any kind) of `bd-cache-api` or `bd-cache-py` on `sc-command` or `clap`. | Add `clap` to `bd-cache-api` → the boundary step fails; add `bd-cache-store` → the existing workspace-edge check fails. |

## Dependencies

b-1's Public contract. Merges to `integrate/phase-b` after b-1. b-3 and b-4
consume this crate and its fixtures.
