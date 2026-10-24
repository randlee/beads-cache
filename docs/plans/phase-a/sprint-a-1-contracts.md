---
id: a-1
phase: a
status: planned
closure_type: contract
target_boundary: bd-cache-model + sc-beads
track: cache
wave: 1
branch: sprint/a-1-contracts
worktree: sprint/a-1-contracts
recommended_agent: rust-developer
recommended_model: opus
must_follow: []
parallel_safe: [a-3]
baseline_sections:   # replaced by `requirements` after P-1
  - "BD-CACHE.md: Bead shape (the contract with bd)"
  - "BD-CACHE.md: Journal operations"
  - "BD-CACHE.md: CLI (error code table)"
  - "BD-CACHE.md: Rust shape"
adrs: [ADR-BDC-0003, ADR-BDC-0004, ADR-BDC-0007]
owned_paths:
  - "Cargo.toml"
  - "rust-toolchain.toml"
  - "crates/bd-cache-model/**"
  - "crates/sc-beads/**"
---

# Sprint a-1 — contracts: cache model and sc-beads

## Outcome and closure

Freeze every interface phase A shares. `sc-beads` is the backend contract
crate only (trait, types, errors, test double); implementations live in
`sc-bd-cli` (a-2) and, once bd ships it, `sc-bd-http`. The crate carries the
`sc-beads` backend trait with the cache extensions, strict and tolerant row types, `BeadsError` and `Refusal`,
the `FakeBackend` test double, and the complete `bd-cache-model` crate. The
model crate is pure (no I/O), so its behaviour (overlay, achieved, argument
validation) lands here with its types. Closes when the workspace builds, the
fake passes the contract tests, and every later sprint can be written against
these signatures.

## Decision prerequisites

- P-3: the shared API shape with beads-task-issue-tracker (sync calls,
  `BeadsError`, tolerant vs strict rows). The facade is ported from
  `btit-beads` (`backend.rs`, `error.rs`) and `btit-types` (issue types)
  before the cache extensions are added, so btit's existing methods keep
  their signatures.
- Strict `Bead` generation: serde types generated from the `bd schema`
  fixture (bd 1.3.1), `deny_unknown_fields`, plus the four `--json` extras
  (`comment_count`, `dependency_count`, `dependent_count`, `parent`). Record
  the generator command and the pinned schema sha in the module header.

## Public contract produced

The normative signatures (expanding the phase plan's sketch):

- `sc_beads::BeadsBackend`: btit's methods unchanged, plus `fingerprint`,
  `list_by_ids`, `list_since(filter, cursor)`, guarded `close` with reason,
  guarded `update(patch)`, `dep_add(kind)`; every call takes a `Deadline`.
- `sc_beads::{Bead, BdRawIssue, Edge, DepType, Fingerprint, ScopeFilter,
  SyncCursor (opaque), Deadline, Guards, Patch, Actor}`.
- `sc_beads::BeadsError` with `Unavailable` (timeout, cannot start,
  connection refused), `Refused(Refusal)`, `Contract`, `Failed`.
- `sc_beads::testing::FakeBackend`: deterministic, emits validated rows,
  records the command plan, injects failures, timeouts and exit-13 refusals.
- `bd_cache_model::{Op, JournalEntry, JournalState, Precondition, CacheError}`
  with `Op::overlay`, `Op::achieved`, argument validation including bd's
  metadata coercion rule, and `CacheError::code()` / `fix()` for every
  `BD_CACHE.*` code in BD-CACHE.md, plus the `/v0` `reason` names
  (ADR-BDC-0003).

## Deliverables

1. Workspace `Cargo.toml` (`members = ["crates/*"]`, shared lints, edition,
   rust-version) and `rust-toolchain.toml`.
2. `sc-beads`: trait, row types, errors and `FakeBackend` (ADR-BDC-0004,
   0007), with every trait and feature btit uses today kept, so btit can
   switch to the crates.io release.
3. `bd-cache-model`, complete (ADR-BDC-0003).
4. bd-1.3.1 contract fixtures (`bd-schema.json`, `list-rows.json`) copied from
   atm-core with provenance.
5. Contract tests every `BeadsBackend` implementation must pass, run here
   against `FakeBackend`.

## Acceptance criteria

| Criterion | Root |
|---|---|
| `test_pinned_model_matches_recorded_schema`: the fixture schema sha equals the pinned sha | BD-CACHE.md Bead shape |
| `test_recorded_bd_rows_validate`: every fixture row parses as strict `Bead` | BD-CACHE.md Bead shape |
| A row with an unknown key, missing required key or wrong type is `BeadsError::Contract` naming bead and field | BD-CACHE.md Bead shape |
| `test_bad_ops_are_typed`: bad op, args, precondition and dep type are typed errors | BD-CACHE.md Journal operations |
| `test_metadata_values_round_trip_through_bd_coercion` (validation half): only int, bool, or a string that bd will not coerce is accepted | BD-CACHE.md Journal operations |
| `overlay` and `achieved` match the op table for close, every update field and dep_add | BD-CACHE.md Journal operations |
| Every `BD_CACHE.*` code in the CLI table has a `CacheError` variant with its fix text | BD-CACHE.md CLI |
| Contract tests pass against `FakeBackend`; `cargo build --workspace` passes | ADR-BDC-0004 |
| btit's `BeadsBackend`, `CliBackend` and `DoltOperations` traits and its cargo features are present with unchanged signatures | ADR-BDC-0007 |
| `cargo publish --dry-run -p sc-beads` passes (crates.io metadata complete) | ADR-BDC-0007 |

## Ported-case ownership (phase A)

| Sprint | Python cases |
|---|---|
| a-1 | pinned_model_matches_recorded_schema, recorded_bd_rows_validate, bad_ops_are_typed, metadata coercion (validation) |
| a-2 | backend_check_does_not_need_bd_context, metadata coercion (argv: bools as `true`/`false`), exit 13 mapping |
| a-4 | cache_file_is_named_for_the_dolt_database, missing_metadata_is_typed, beads_dir_env_names_the_database, file_from_another_project_is_refused, newer_cache_schema_is_refused, dependents_use_the_edge_index, escalations_are_unannounced_until_marked, sync_alert_round_trip, pending_work_counts_as_open |
| a-5 | every other `test_bd_cache.py` case (27) |
| a-6 | all 9 `test_bd_cache_cron.py` cases |

## This sprint does not close

- Running `bd` (a-2), SQLite storage (a-4), drain and sync behaviour (a-5).

## Required validation

`cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D
warnings`, `cargo test --workspace --all-features`.
