# bd-cache-api Requirements

Status: Approved (lead ruling S60, bc-lead, 2026-10-25; phase B closed,
2026-10-25; written in sprint b-2). Crate-local requirements (domain code `API`)
refining [`../requirements.md`](../requirements.md) and
[`../bd-cache-cli/contract.md`](../bd-cache-cli/contract.md) (contract 1.0.0).

## REQ-API-0001: One decoder, nothing ignored

**Status:** Approved (lead ruling S60, bc-lead, 2026-10-25; phase B closed, 2026-10-25)

`decode(command, json)` is the only path from a JSON request to a typed
request. An unknown member is `BAD_ARGS` with `details.member`; a `/v0`
parameter or member of the pinned spec (beads v1.3.2) that the cache does not
serve is `UNSUPPORTED` with `details.unsupported` and `details.name`; unknown
members are reported before unsupported ones. An empty patch and
`status: "closed"` are `BAD_ARGS`; a patch is validated as the journal parses
it (`Op::parse_new`).

Decode, then `execute`, is the only path: `Request` and its per-command
structs have crate-private fields (read through accessors), the unit
requests are `#[non_exhaustive]`, and the per-command operations are
crate-private, so no front end can build or alter a request the decoder did
not check (QA RCR-001). Operation-level `UNSUPPORTED` is left to the front
end, which builds it with `ApiError::unsupported(Unsupported::Operation, name)`.

**Refines:** REQ-BDC-0807
**Acceptance evidence:** `decode_refusals` (`tests/fixtures.rs`, fixture `decode_refusals.json`); the `compile_fail` doctests in `src/lib.rs` (a literal `IssuesUpdate`, a field changed after decoding, `ops::issues_update`); `an_operation_the_cache_does_not_serve_is_unsupported_operation` (`tests/error_table.rs`).

## REQ-API-0002: The idempotency key is required, never derived

**Status:** Approved (lead ruling S60, bc-lead, 2026-10-25; phase B closed, 2026-10-25)

`issues update`, `issues close` and `dependencies add` decode only with a
nonblank `idempotency_key`; a write without one is `BAD_ARGS`
(`details.member: idempotency_key`) and journals nothing.

**Refines:** REQ-BDC-0004
**Acceptance evidence:** `idempotency_key_required` (`tests/fixtures.rs`).

## REQ-API-0003: Total orders and an opaque keyset cursor

**Status:** Approved (lead ruling S60, bc-lead, 2026-10-25; phase B closed, 2026-10-25)

`issues list` orders `(created_at desc, id asc)` (default) or
`(priority asc, created_at desc, id asc)`; `ready` uses the second. Inside a
scope the default is no limit, and `limit: 0` is unlimited. An explicit
limit pages with `has_more` and `next_cursor` (absent on the last page); a
walk visits every row once. The cursor is `v1.` + base64url of
`[sort, key…, check]`; one this crate did not mint, or one for the other
sort, is `BAD_ARGS` with `v0_code: invalid_cursor`.

**Refines:** REQ-BDC-0808
**Acceptance evidence:** `a_created_walk_concatenates_to_the_unpaged_order`,
`a_priority_walk_concatenates_to_the_unpaged_order`,
`a_tampered_or_foreign_cursor_is_bad_args_invalid_cursor` (`tests/paging.rs`).

## REQ-API-0004: Filters run in the api over one scope

**Status:** Approved (lead ruling S60, bc-lead, 2026-10-25; phase B closed, 2026-10-25)

A list-like read names a scope with `scope` or its first `phase-<x>` label
(else `SCOPE_UNKNOWN` with `details.known` in label form); every label is
still an AND filter. `issues list` excludes closed rows, templates and `gate`
rows unless `status` or `all` is set. A scope with no completed sync since it
was last evicted, or ever, is not served: `issues list` and `ready` return
`SCOPE_UNKNOWN` with `suggested_action` `bdc cache refresh scopes --scope
<s>`, never an empty list (lead ruling S67, contract §7).

**Refines:** REQ-BDC-0902, contract §5.1, §5.3, §7
**Acceptance evidence:** `issues_list_filters` (`tests/fixtures.rs`);
`evict_is_blocked_by_pending_writes`, `a_configured_scope_never_synced_is_not_served` (`tests/manage.rs`).

## REQ-API-0005: The error table and the CacheError mapping

**Status:** Approved (lead ruling S60, bc-lead, 2026-10-25; phase B closed, 2026-10-25)

`ERROR_TABLE` equals the contract §4 table (code, kind, exit, retryable,
`v0_code`); every `CacheError` code is in it. `From<CacheError>` maps
`detail` → `message`, `fix` → `suggested_action`, `reason` →
`details.v0_code`, `unmet` → `details.unmet`, `phase` → `details.scope` in
label form, and the other payload fields to `details` under their names.
`ApiError::config_invalid` and `ApiError::tick_refusal` map the store's,
sync's and the tick's codes without depending on those crates.

**Refines:** REQ-BDC-0803
**Acceptance evidence:** `the_error_table_equals_the_contract`,
`every_cache_error_code_is_in_the_table_with_its_kind`,
`reason_unmet_and_phase_map_to_details` (`tests/error_table.rs`).

## REQ-API-0006: Entry args in /v0 names

**Status:** Approved (lead ruling S60, bc-lead, 2026-10-25; phase B closed, 2026-10-25)

`Entry.args` is the `/v0` form of the stored op (`set_metadata` →
`metadata.set`, `unset_metadata` → `metadata.unset`, a dep add as
`edges: [edge]`); the store is not rewritten. A `/v0` patch decoded and
journaled reads back identically.

**Refines:** contract §6 `Entry`
**Acceptance evidence:** `tests/entry_args.rs`.

## REQ-API-0007: Write replies come from write_returning

**Status:** Approved (lead ruling S60, bc-lead, 2026-10-25; phase B closed, 2026-10-25)

A write reply carries the view after the write, `changed` (a close:
`already_closed = !changed`) and `x_cache {seq, state, duplicate,
idempotency_key}`; on a duplicate, `changed` is the value stored with the
original entry. When that value is unknown (lead ruling S68: a duplicate of
an entry with no stored `changed` that is no longer pending), `changed` is
absent, never `null` and never guessed; `already_closed` is `!changed`, so
it is absent too.

**Refines:** REQ-BDC-0809, contract §5.1
**Acceptance evidence:** `writes` (`tests/fixtures.rs`); `tests/unknown_changed.rs`.

## REQ-API-0008: Contract version and no front-end dependency

**Status:** Approved (lead ruling S60, bc-lead, 2026-10-25; phase B closed, 2026-10-25)

`CONTRACT_VERSION` is `"1.0.0"` and is reported by `context` and
`cache status`. The crate depends on `bd-cache`, `bd-cache-model`,
`sc-beads`, serde, serde_json and schemars only: no sc-command, no clap, in
any dependency kind, and none of its normal or build dependencies pulls
sc-command or clap in transitively (CI "Crate boundaries" and "Crate
boundaries, transitive").

**Refines:** REQ-BDC-0811, contract §9
**Acceptance evidence:** fixtures `reads`, `writes` (`contract_version`); the two CI boundary steps.
