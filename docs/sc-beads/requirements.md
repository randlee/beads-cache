# sc-beads Requirements

Status: draft. Crate-local requirements (domain code `BEADS`) refining the
repo-level ids in [`../requirements.md`](../requirements.md). Created by
sprint a-1.

## REQ-BEADS-0001: btit's backend contract is kept unchanged

**Status:** Draft

btit's `BeadsBackend`, `CliBackend`, `DoltOperations` and `CloseSuggestions`
traits, the btit-types data types and `BeadsError` (variants, fields, codes
and `Display` text) are those of randlee/beads-task-issue-tracker at
`8d8e82ae9f85`, so btit switches by changing imports only. btit-beads and
btit-types declare no cargo features, and neither does sc-beads beyond the
additive `testing` feature.

**Refines:** REQ-BDC-0906
**Acceptance evidence:** `tests/btit_api_snapshot.rs` against
`tests/fixtures/btit-api.txt`; the ported `tests/api_freeze.rs`,
`tests/types_api_freeze.rs` and `tests/compatibility_info.rs`; btit's unit
tests in the ported modules.

## REQ-BEADS-0002: Cache backend calls are deadline-bounded and typed

**Status:** Draft

`CacheBackend: BeadsBackend` offers `fingerprint`, `list_by_ids`,
`list_scope`, `list_blocked`, `list_ready`, `close_guarded`,
`update_guarded` and `dep_add_typed`; each takes a `Deadline` and returns
`BeadsCacheError` (`Unavailable`, `Refused`, `Contract`, `Failed`, or
`Btit` wrapping a `BeadsError`).

**Refines:** REQ-BDC-0006, NFR-BDC-0004 (constraint), ADR-BDC-0004
**Acceptance evidence:** the `CacheBackend` contract suite
(`testing::contract`, `tests/backend_contract.rs`), including
`every_call_stops_at_its_deadline`.

## REQ-BEADS-0003: Strict rows

**Status:** Draft

`Bead` is generated from the pinned `bd schema` (bd 1.3.1) plus the four
`--json` extras; `parse_bd_row` / `parse_bd_rows` / `parse_blocked_rows`
reject an unknown key, a missing or null required key, a wrong JSON type, a
value outside a declared enum and a malformed date-time with
`BeadsCacheError::Contract` naming the bead and the field path. One bad row
fails the whole list.

**Refines:** REQ-BDC-0007, REQ-BDC-0008 (constraint)
**Acceptance evidence:** `test_pinned_model_matches_recorded_schema`,
`test_recorded_bd_rows_validate`,
`a_bad_row_is_a_contract_error_naming_bead_and_field` (`tests/strict_rows.rs`);
`generated_strict_rows_are_current` (`tests/strict_rows_generated.rs`).

## REQ-BEADS-0004: Clock and Deadline

**Status:** Draft

All time comes from a `Clock` (`now_utc` as RFC 3339 seconds, `now_mono`);
a `Deadline` is an instant with `at`, `after`, `earliest`, `remaining` and
`expired`.

**Refines:** ADR-BDC-0005 (constraint)
**Acceptance evidence:** `clock` unit tests and doctests; `ManualClock` doctest.

## REQ-BEADS-0005: A deterministic bd for tests

**Status:** Draft

Behind the `testing` feature, `FakeBackend` models bd as the P-6 probe
recorded it (rows without `is_blocked`, `bd blocked` with parent-child
inheritance and outside blockers, `bd ready`, own-`updated_at` bumps on
close and reopen), records its command plan, injects `Unavailable` (timed
out or not), refusals and failures per call, and advances a `ManualClock`
by a configured cost per call.

**Refines:** NFR-BDC-0005, ADR-BDC-0002
**Acceptance evidence:** `blocked_and_ready_match_the_p6_probe` and the rest
of the contract suite against `FakeBackend`; `tests/fake_backend.rs`.
