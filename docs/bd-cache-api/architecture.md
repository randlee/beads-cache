# bd-cache-api Architecture

Status: Approved (lead ruling S60, bc-lead, 2026-10-25; phase B closed,
2026-10-25; written in sprint b-2). Crate-local decisions (domain code `API`) under
ADR-BDC-0009 and [`../bd-cache-cli/contract.md`](../bd-cache-cli/contract.md).

## Modules

| Module | Holds |
|---|---|
| `request/{beads,manage}.rs` | One typed request per command of contract §5.3 and §6 (not `cache tick`), with schemars schemas; `Command`, `Request` |
| `decode.rs` | `decode(command, json)`: the one decoder and the only constructor of a `Request` (REQ-API-0001, 0002) |
| `ops/{beads,manage}.rs` | One crate-private function per command over `&mut Cache<S, L, B>`, returning `Result<Data, ApiError>`; the public `execute`; `CacheFacts` |
| `filter.rs` | Scope selection and the list and ready filters (REQ-API-0004) |
| `page.rs` | The two total orders and the keyset cursor (REQ-API-0003) |
| `v0.rs` | `/v0` patch ↔ stored BD-CACHE args (REQ-API-0006) |
| `data/{beads,manage,entry,x_cache}.rs` | The `data` of each command, `Entry`, `x_cache`, `OpError` |
| `error.rs` | `ApiError`, `ErrorKind`, `ERROR_TABLE`, `From<CacheError>` (REQ-API-0005) |
| `tests/fixtures/*.json` | The shared fixtures b-3 and b-4 replay (format in its README) |

## ADR-API-0001: Decode by hand, not with serde

**Status:** Approved (lead ruling S60, bc-lead, 2026-10-25; phase B closed, 2026-10-25)

**Context.** Contract §1 rule 5 needs three outcomes per member (served,
`/v0` but not served, unknown) with distinct codes and `details`; serde's
`deny_unknown_fields` gives one opaque error.

**Decision.** `decode.rs` checks every member name against a served list and a
not-served list per command, unknown first, then reads each value with typed
helpers that name the member on failure. `null` is the same as absent.

**Consequences.** The per-command lists are the single statement of what the
cache serves; adding a member is one edit there. Request types do not
implement `Deserialize`, have crate-private fields with read accessors, and
cannot be built outside the crate, so `decode` is the only constructor and
`execute` the only public operation (QA RCR-001). An unused list of
unserved operation names was removed: its names did not match contract
§5.4, and a front end, which owns its command names, maps them itself.

## ADR-API-0002: The cursor carries a checksum

**Status:** Approved (lead ruling S60, bc-lead, 2026-10-25; phase B closed, 2026-10-25)

**Context.** A keyset cursor that decodes to a valid but altered key would page
from the wrong place without an error; the contract requires a tampered cursor
to be `BAD_ARGS` (`invalid_cursor`).

**Decision.** The cursor is `v1.` + base64url (no padding, canonical) of
`[sort, key…, check]`, where `check` is FNV-1a 64 of the rest. Not a secret:
it detects edits, not forgery.

**Consequences.** The encoding stays outside semver (contract §11).

## ADR-API-0003: Interim adapter for sprint b-1

**Status:** Superseded: b-1 merged (#75); the adapter was deleted and the
operations call b-1's `write_returning`, `entry_by_key`, `journal_counts`,
`GetReply.scope` and entry fields directly. The two items b-1's contract did
not cover became `Cache::pending_entries` and `Cache::backend_check` (lead
ruling S66).

**Context.** b-2 is built in parallel with b-1, which adds `write_returning`,
`entry_by_key`, `journal_counts`, `GetReply.scope` and the entry columns.

**Decision.** `b1_stub.rs` provides the same signatures over today's facade,
reaching the store through `bd_cache::testing` (the `testing` feature is
enabled for this crate until then). When b-1 lands it is deleted and the
callers use b-1's items. Two items it provides are not in b-1's Public
contract and need a lead decision: a facade read of the pending entries
(`cache journal list --state pending`) and a public backend check
(`cache health --probe`).

**Consequences.** Until then a write reply is not built in one store
transaction, and a duplicate's `changed` is recomputed (equal to the stored
value while earlier entries are pending).

## Implementation notes (sprint b-8, ADR-BDC-0015)

- `src/request/sync.rs` holds the phase-B requests; `Command::ALL` lists them (34 commands) and
  `Command::is_front_end` names the ones a front end runs over `bd-cache-sync` (`cache schedule
  *`, `cron run`, `tick`, `sync *`); `execute` refuses those with `INTERNAL`.
- `src/ops/scopes.rs` runs the store-only verbs; `cache scopes` and `cache scopes list` return
  the same `CacheScopesData`; `cache scopes remove` is `scopes_remove` (prune and the registry
  removal in one store transaction).
- `ApiError::tick_refusal`, its kind table and the contract's §4 `TICK_<REFUSAL>` row are
  removed; `tests/error_table.rs` checks that no row names the family.

