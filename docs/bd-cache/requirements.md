# bd-cache Requirements

Status: draft. Crate-local requirements (domain code `CORE`) refining
[`../requirements.md`](../requirements.md). This file (sprint a-1) holds the
skeleton and shared-helper requirements; the write path's are in
[`write-path.md`](write-path.md) (sprint a-5) and the read path's in
[`read-path.md`](read-path.md) (sprint a-6).

## REQ-CORE-0001: One call helper for every backend call

**Status:** Draft

Every backend call goes through `call(kind, hard, f)`, which hands `f` the
earlier of `hard` and `CacheConfig::call_timeout` and classifies a failure:
a timeout after `hard` expired is `Cut` (no outage, no attempt); an
unreachable backend on `Check`, `PreRead`, `Sync`, `Refresh`, `MissFill` or
`Ready` is an `Outage` and is recorded; on `Write` or `ReadBack` it is an
`Error(BD_UNAVAILABLE)`; anything else is the mapped `Error`. Only an
answering list call clears the outage.

**Refines:** ADR-BDC-0005, NFR-BDC-0004 (constraint)
**Acceptance evidence:** `tests/helpers_call.rs`.

## REQ-CORE-0002: The outage keeps its start

**Status:** Draft

Recording an outage keeps `since` from the first failure and stores the
latest error; clearing removes it.

**Refines:** ADR-BDC-0005
**Acceptance evidence:** `test_outage_keeps_its_start_time_and_latest_error`.

## REQ-CORE-0003: Backend check and rebuild

**Status:** Draft

`backend_check::run` fingerprints the backend through `call(Check, ..)` and
applies the store's rebuild rule, reporting the outcome and
`model_current`.

**Refines:** REQ-BDC-0105
**Acceptance evidence:** `test_schema_change_rebuilds_rows_and_keeps_the_journal`,
`test_model_current_when_live_schema_matches_pin`,
`test_first_backend_check_records_schema_without_dropping_cached_rows`
(`tests/helpers_backend_check.rs`).

## REQ-CORE-0004: Views and bounded miss fills

**Status:** Draft

`view::current_view` returns the stored row with the bead's pending entries
replayed and starts no bd call; `miss::fill` makes one `list_by_ids` under
the caller's timeout or `miss_fill_timeout`, stores the row with no scope,
and returns `BD_UNAVAILABLE` when cut or unreachable.

**Refines:** REQ-BDC-0202 (constraint), 0902 (constraint), 0903 (constraint)
**Acceptance evidence:** `tests/helpers_view_miss.rs`.
