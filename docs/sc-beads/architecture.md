# sc-beads Architecture

Status: draft. Crate-local decisions (domain code `BEADS`). Repo-level
context: [`../architecture.md`](../architecture.md), ADR-BDC-0004 and
ADR-BDC-0007. Created by sprint a-1.

## Layout

| Module | Contents | Origin |
|---|---|---|
| crate root (`types/`) | `ProjectRef`, `BdRawIssue`, `ListQuery`, `CreatePayload`, `UpdatePayload`, `RelationType`, CLI probe types, filesystem types | btit-types, verbatim |
| `backend`, `error`, `compat`, `detect`, `gates`, `issues`, `parse`, `logging` | btit's traits, `BeadsError`, pure detection / gate / normalization / tolerant-parse logic, the gated `log_*!` macros | btit-beads, verbatim except logging (ADR-BEADS-0001) |
| `cache` | `CacheBackend`, `BeadsCacheError`, `BeadId`, `Actor`, `Edge`, `BlockedRow`, `Fingerprint`, `ScopeFilter`, `SyncCursor`, `Guards`, `MetaValue`, `Patch`, `Refusal` | new |
| `row` (+ generated `row/generated.rs`) | strict `Bead`, `BeadComment`, `BondedFrom`, `DepType`, the pinned fingerprint constants, the validators | new, generated from `tests/fixtures/bd-1.3.1/bd-schema.json` |
| `clock` | `Clock`, `SystemClock`, `Deadline`, RFC 3339 helpers | new |
| `testing` (feature) | `ManualClock`, `FakeBackend`, `bead()`, the `CacheBackend` contract suite and `backend_contract_tests!` | new |

The crate depends on no workspace crate (`../architecture.md`, Boundaries)
and only on serde, serde_json, sha2 and tracing. It does no I/O.

## ADR-BEADS-0001: Port btit verbatim; logging through tracing

**Status:** Draft

Context: REQ-BDC-0906 requires every btit trait and feature to be kept;
ADR-BDC-0007 moves logging from btit's global macros to `tracing`.

Decision: btit-types and btit-beads at `8d8e82ae9f85` are copied unchanged
(imports rewritten from `btit_types::` / `btit_beads::` to `crate::`),
including their unit tests, the API-freeze tests and the
`compatibility_info` test. The crate stays on edition 2021, as btit is, so
the ported code needs no body edits (edition 2024 rejects three of its
binding patterns). The public surface is pinned twice: by the ported
compile-time API-freeze tests and by `tests/fixtures/btit-api.txt`, the
public lines of the 13 btit source files, checked by
`tests/btit_api_snapshot.rs` (sc-beads may add, never change or drop).
The single recorded deviation: the `log_*!` macros keep their names, gates
and statics but emit through `tracing`, and `logging::tracing` replaces the
`logging::log` re-export.

Consequences: a btit consumer switches with import changes, plus
`logging::log::` → `logging::tracing::` where it used the re-export.

## ADR-BEADS-0002: Cache errors are a separate type

**Status:** Draft (owner amendment to sprint a-1, recorded in ADR-BDC-0007)

Context: P-3 asked whether `BeadsError` is the shared error type. Adding
cache variants to it would change btit's pinned enum.

Decision: `BeadsError` is unchanged. `BeadsCacheError` carries the cache
cases (`Unavailable { reason, timed_out }`, `Refused(Refusal)`,
`Contract { bead, field, detail }`, `Failed { code, stderr }`) plus
`Btit(BeadsError)`, with `From<BeadsError>`. Every `CacheBackend` method
returns it; btit's traits keep returning `BeadsError`. `is_unavailable()`
also covers btit's `Spawn` (bd cannot start). For `fingerprint`, a `Failed`
stderr starts with the failing subcommand (`version:` / `schema:`) so the
cache can tell `BD_VERSION_FAILED` from `BD_SCHEMA_FAILED`.

## ADR-BEADS-0003: Strict rows validated from generated field tables

**Status:** Draft

Context: serde errors do not name the field of a type mismatch, and
REQ-BDC-0007 needs the bead and field of every violation.

Decision: the generator (`tests/strict_rows_generated.rs`) emits, from the
schema fixture, the `Bead` struct and one field table per object type
(name, required, kind: string, date-time, integer, boolean, enum, string
array, object array, any). `parse_bd_row` walks the JSON against the table
(field paths such as `dependencies[0].type`), then reshapes and
deserializes. Optional keys accept `null`, as pydantic's `Optional` does;
integers and booleans are not coerced from strings (stricter than pydantic's
lax mode). `dependencies` keep only the edge triple (`Edge`), because the
store rebuilds them from its edges table; `Edge` deserializes tolerantly so
rows stored by the Python cache read back. `cargo test` fails while
`row/generated.rs` differs from the generator's output; regenerate with
`SC_BEADS_REGENERATE=1 cargo test -p sc-beads --test strict_rows_generated`.
Fields are in key order, so a serialized row has sorted keys like Python's
`json.dumps(sort_keys=True)`.

## ADR-BEADS-0004: FakeBackend follows the recorded probe

**Status:** Draft

Decision: the P-6 probe was repeated for a-1 with bd 1.3.0 and recorded in
`tests/fixtures/p6-blocked/` (provenance there); the contract rule
`blocked_and_ready_match_the_p6_probe` requires every `CacheBackend`,
`FakeBackend` included, to reproduce its `bd blocked` and `bd ready`
answers before, after closing and after reopening an outside blocker.
Unprobed edge types: `conditional-blocks` and `waits-for` are treated as
blocking like `blocks` (bd's `AffectsReadyWork` set; `DepType::is_blocking`
also counts `parent-child`), erring towards "not ready" (REQ-BDC-0904). The
recording showed `bd dep add` does not bump the depender's `updated_at`; the
fake does the same. Clones of a `FakeBackend` share one bd so a test keeps a
handle after moving the backend into a cache.
