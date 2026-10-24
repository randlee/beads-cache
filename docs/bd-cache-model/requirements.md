# bd-cache-model Requirements

Status: approved (lead ruling, 2026-10-23). Crate-local requirements (domain code `MODEL`) refining
[`../requirements.md`](../requirements.md). Created by sprint a-1.

## REQ-MODEL-0001: Journal ops are validated, canonical and pure

**Status:** Approved (lead ruling, 2026-10-23)

`Op::parse` accepts `close {reason?}`, `update` (a nonempty subset of
status, assignee, add/remove labels, set/unset metadata, append notes; the
`/v0` `metadata: {set, unset}` form too) and `dep_add {depends_on, type}`;
an unknown op is `BAD_OP`; unknown or mistyped keys, an empty update, a
metadata value bd would coerce and an unknown dependency type are
`BAD_ARGS`. `Op::to_args_json` is byte-identical to Python's
`json.dumps(args, sort_keys=True)`; equal ops compare equal.

**Refines:** REQ-BDC-0301, 0303, 0304
**Acceptance evidence:** `test_bad_ops_are_typed`, `metadata_coercion_guard`,
`args_shape_errors_are_bad_args`, `python_written_args_round_trip_byte_for_byte`,
`equal_ops_compare_equal_regardless_of_key_order` (`tests/ops.rs`).

## REQ-MODEL-0002: Overlay, achieved and replay follow the op table

**Status:** Approved (lead ruling, 2026-10-23)

`Op::overlay` applies the op table's overlay; `Op::achieved` holds when the
row already shows the effect; `replay` overlays a bead's `Pending` entries
in seq order and skips escalated, resolved and applied ones.

**Refines:** REQ-BDC-0305, 0202 (constraint)
**Acceptance evidence:** `close_overlay_and_achieved`,
`update_overlay_applies_fields_in_order_and_achieved_checks_each`,
`dep_add_overlay_adds_the_edge_once_and_achieved_sees_it`,
`replay_applies_pending_entries_in_seq_order_and_skips_the_rest`.

## REQ-MODEL-0003: Typed errors

**Status:** Approved (lead ruling, 2026-10-23)

`CacheError` has one variant per `BD_CACHE.*` code with its payload, plus
`SCOPE_UNKNOWN` and `STORE_FAILED` (a-1 amendment 1); `exit_code()` is 3; `to_json()` is
`{ok: false, code, detail, fix, reason?, ...payload}`; `from_backend` maps
backend errors by `CallKind` per the table in `src/error.rs`.

**Refines:** REQ-BDC-0802, 0803
**Acceptance evidence:** `golden_json_for_every_code`, `reasons_use_v0_names`,
`from_backend_matches_the_table_for_every_call_kind` (`tests/goldens.rs`).

## REQ-MODEL-0004: Store and lock ports with a contract suite

**Status:** Approved (lead ruling, 2026-10-23)

`CacheStore` and `DrainLock` are the store ports; every rule in the
`CacheStore` doc comment has a function in `testing::contract`, which
`FakeStore` + `FakeDrainLock` pass and every implementation must pass.

**Refines:** NFR-BDC-0005, ADR-BDC-0004
**Acceptance evidence:** `tests/store_contract.rs` (17 rules).

## REQ-MODEL-0005: Repository configuration

**Status:** Approved (lead ruling, 2026-10-23)

`RepoConfig` is the `.beads/bd-cache.toml` schema (unknown keys refused);
`cache_config` derives `CacheConfig` with defaults (30 s call cap, 2 s miss
fill, 5 attempts); `DrainBudget::before_tick_end` places the start gate 5 s
and the hard deadline 2 s before a tick end.

**Refines:** REQ-BDC-0101 (constraint), 0402 (constraint), 0909 (constraint)
**Acceptance evidence:** `tests/config.rs`.
