# bd-cache-model Architecture

Status: draft. Crate-local decisions (domain code `MODEL`). Repo-level
context: [`../architecture.md`](../architecture.md). Created by sprint a-1.

The crate is pure (no I/O, no clock): timestamps are passed in as RFC 3339
strings. It depends on `sc-beads`, serde, serde_json and toml (for
`RepoConfig`'s opaque `schema_packages` and `tick` tables).

| Module | Contents |
|---|---|
| `op` | `Op`, `replay`, `normalize`, `phase_of`, precondition parse / canonical JSON / `unmet` |
| `pyjson` | `python_json`: Python `json.dumps(sort_keys=True)` text |
| `journal`, `scope`, `state` | journal types, `ScopeId`, store-side state and `StoreStatus` |
| `store`, `sender` | `CacheStore`, `DrainLock` / `DrainGuard`, `EscalationSender` |
| `config` | `RepoConfig`, `CacheConfig`, `DrainBudget` and the default constants |
| `reply`, `call`, `error` | reply shapes (provisional, REQ-BDC-0806 deferred), `CallKind` / `CallOutcome`, `CacheError` |
| `testing` (feature) | `FakeStore`, `FakeDrainLock`, the store contract suite and `store_contract_tests!` |

## ADR-MODEL-0001: Journal text is written exactly as Python writes it

**Status:** Draft

Decision: `args_json` and `precondition_json` are rendered by
`python_json`, which reproduces `json.dumps(..., sort_keys=True)` (sorted
keys, `", "` / `": "`, `ensure_ascii` escapes). A file shared with the
Python implementation keeps byte-identical journal text, and the
idempotency comparison (`(bead, op, args)`) compares parsed `Op`s, so key
order never matters. Fixture: `tests/fixtures/python-journal-args.json`,
generated with CPython.

Consequences: empty collections in `update` args are treated as "not
requested" and an update that requests nothing is `BAD_ARGS`; a
Python-written `{"add_labels": []}` (accepted by v3) would not parse. Integer
metadata values are limited to `i64`.

## ADR-MODEL-0002: Backend error mapping details

**Status:** Draft

Decision, beyond the sprint table: btit's `Spawn` (bd cannot start) maps
like `Unavailable` to `BD_UNAVAILABLE`; btit's `InvalidJson`,
`UnexpectedShape` and `ParseFailed` to `BD_CONTRACT`; `Refused(NotClosable
/ AlreadyClaimed)` to `PRECONDITION` with the matching `/v0` reason;
`Refused(Other)` and any other btit error map like `Failed`. A failed
`Check` is `BD_SCHEMA_FAILED` when stderr starts with `schema`, else
`BD_VERSION_FAILED` (see ADR-BEADS-0002). `Ready` failures are
`BD_LIST_FAILED`; `Sync` failures are `SYNC_FAILED` with cause
`BD_CACHE.BD_LIST_FAILED`.

## ADR-MODEL-0003: Store rule clarifications

**Status:** Draft

Decision: these points, not fixed by the sprint contract, are stated in the
`CacheStore` doc comment and checked by the suite: `escalate` does not
change `attempts` (the drain counts attempts with `record_attempt`, so the
cap is not double counted); `insert_entry` on a used key is
`IDEM_CONFLICT`; an unknown seq is `NOT_FOUND`; `resolve` on an absent entry
is `NOT_ESCALATED`; a rebuild keeps the scope registry, sync alerts and the
outage; prune keeps the scope registered and never deletes a bead with an
open entry; `status` keys journal counts `<phase>:<state>` with `None` for
entries without a phase, as v3 prints them. `FakeStore` stores the outage
and alerts as v3 `meta` keys, and its clones share one store.
