# bd-cache-model Architecture

Status: approved (lead ruling, 2026-10-23). Crate-local decisions (domain code `MODEL`). Repo-level
context: [`../architecture.md`](../architecture.md). Created by sprint a-1.

The crate is pure (no I/O, no clock): timestamps are passed in as
`sc_beads::Timestamp` values (RFC 3339 UTC, second precision). It depends on `sc-beads`, serde, serde_json and, privately, toml
(ADR-MODEL-0004).

| Module | Contents |
|---|---|
| `op` | `Op`, `replay`, `normalize`, `phase_of`, precondition parse / canonical JSON / `unmet` |
| `pyjson` | `python_json`: Python `json.dumps(sort_keys=True)` text |
| `journal`, `scope`, `state` | journal types, `ScopeId`, store-side state and `StoreStatus` |
| `store`, `sender` | `CacheStore`, `DrainLock` / `DrainGuard`, `EscalationSender` |
| `config` | `RepoConfig`, `CacheConfig`, `DrainBudget` and the default constants |
| `reply`, `call`, `error` | reply shapes (provisional, REQ-BDC-0806 deferred), `CallKind` / `CallFailure`, `CacheError` |
| `testing` (feature) | `FakeStore`, `FakeDrainLock`, the store contract suite and `store_contract_tests!` |

## ADR-MODEL-0001: Journal text is written exactly as Python writes it

**Status:** Approved (lead ruling, 2026-10-23)

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

**Status:** Approved (lead ruling, 2026-10-23)

Decision, beyond the sprint table: btit's `Spawn` (bd cannot start) maps
like `Unavailable` to `BD_UNAVAILABLE`; btit's `InvalidJson`,
`UnexpectedShape` and `ParseFailed` to `BD_CONTRACT`; `Refused(NotClosable
/ AlreadyClaimed)` to `PRECONDITION` with the matching `/v0` reason;
`Refused(Other)` and any other btit error map like `Failed`.
`CheckFailed { step }` is `BD_VERSION_FAILED` or `BD_SCHEMA_FAILED` by its
step, and a plain `Failed` on `Check` is `BD_VERSION_FAILED`; no stderr is
inspected (amended in fix-a-1-1, RBP-F006). `Ready` failures are
`BD_LIST_FAILED`; `Sync` failures are `SYNC_FAILED` with cause
`SyncCause::BdListFailed`. Every mapped error keeps the backend error as
its `source` (`BackendCause`) and in its JSON as `source: {code, detail}`
(fix-a-1-1, RBP-F005). Precondition keys are `GuardKey`.

### ADR-MODEL-0002 amendment (a-1 amendment 1): store failures are STORE_FAILED

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Context: `CacheStore` methods may fail for the store's own reasons (SQLite
errors, file I/O, a stored value that does not decode), but `CacheError` had
no code for them. `bd-cache-store` reported them as `NO_DATABASE`, with the
source chain flattened into the detail text. The baseline
(`docs/baseline/BD-CACHE.md`, CLI table) has no v3 code for store or SQLite
failures, so none could be reused.

Decision: add the additive code `BD_CACHE.STORE_FAILED`,
`CacheError::StoreFailed { detail, source: Option<StoreCause> }`.
- `detail` names the failing step and the file.
- `StoreCause` wraps the underlying error in an `Arc<dyn Error + Send + Sync>`,
  following the `BackendCause` pattern: it compares by text, `CacheError`
  stays `Clone + Eq`, `Error::source()` returns the wrapped error, and
  `to_json` adds `source: {detail}`.
- `CacheError::store_failed(detail, source)` builds one.
- It exits 3 like every typed error. Its fix tells the operator to check the
  file (permissions, free disk space, another holder), or to move a corrupt
  file aside so the next sync rebuilds it from bd; pending writes in that
  file are lost.

A store reports its own failures as `STORE_FAILED`. `NO_DATABASE` keeps its
baseline meaning: missing or invalid `.beads/metadata.json`.

Consequences: `bd-cache-store` (a-4) switches its store-internal failures from
`NO_DATABASE` to `STORE_FAILED` with the source kept. The code is additive to
the v3 CLI contract (ADR-BDC-0003), so it needs a note on #557.

## ADR-MODEL-0003: Store rule clarifications

**Status:** Approved (lead ruling, 2026-10-23)

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

### ADR-MODEL-0003 amendment (a-1 amendment 1): status reports stale rows

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Context: sprint a-6 needs status to report stale rows (REQ-BDC-0603, as
amended in c4a2f8b).

Decision: `StoreStatus` gains `stale: Vec<BeadId>`. `status()` reports the
same set as `stale_rows`, read as one consistent snapshot with the rest of
the status. The store contract rule `status_lists_stale_rows_until_reread`
checks that a failed read-back is listed and that a re-read clears it.

Consequences: every `CacheStore` (FakeStore, a-4's SqliteStore) must report
`stale` in `status()` from the same snapshot, and passes the new rule.

### ADR-MODEL-0003 amendment (fix-a-1-1): checked model values and a failure-only call result

**Status:** Approved (lead ruling, 2026-10-23; ruling L5 for `CallFailure`)

`IdemKey::new` rejects an empty key; scopes are
`ScopeId::Phase(PhaseName)` with a validated name (also in
`ScopeUnknown.known`, `EscalatedItem.phase` and `phase_of`); journal,
sync-state, outage and ready-base instants are `Timestamp`s. `CallOutcome`
is replaced by the failure-only `CallFailure { Cut, Outage(CacheError),
Error(CacheError) }`: `call` returns `Result<T, CallFailure>`, an outage
carries its `BD_UNAVAILABLE` error (so a caller never wraps the text again),
and `CallFailure::into_error` turns a cut into `BD_UNAVAILABLE`. An
unassigned update overlays as no assignee, as bd stores it.
fix-a-1-2 (QA-002): the newer-row guard compares `updated_at` as an instant
(second precision), never as text; the contract rule
`newer_row_guard_compares_instants` checks it with offset and fractional
timestamps. fix-a-1-3 (ARCH-201; lead ruling, bc-lead, 2026-10-23): when either `updated_at` does not
parse, the stored row is not newer and the incoming row is stored; text is
never ordered (`FakeBackend`'s cursor filter likewise lists such a row).
`resolve` on an absent seq is `NOT_ESCALATED`; `NOT_FOUND` applies to the
other seq-taking methods (ARCH-202; lead ruling, bc-lead, 2026-10-23).

## ADR-MODEL-0004: toml is a private parsing dependency

**Status:** Approved (lead ruling, 2026-10-23; ruling L2 on ARCH-007)

Context: `RepoConfig` exposed `toml::Value` and `toml::Table`, and
`from_toml` exposed `toml::de::Error`, which made the toml crate part of the
model's public contract.

Decision: no toml type appears in the API. `schema_packages` is
`Vec<SchemaPackageRef>`, an opaque entry the later schema-package phase will
open up (`docs/architecture.md`, Repository configuration). `tick` is a
typed `TickConfig` (`team`, `fallback_lead`, `phases_dir`, `phases`;
unknown keys refused), and `from_toml` returns the crate's `ConfigError`.
toml stays a normal dependency, used only inside `config.rs`; the
`bd-cache-model -> toml` edge is listed in the Crate Layout of
`docs/architecture.md`.

Consequences: a toml upgrade is not a breaking change for the model's
users. a-7 adds a tick key by amending `TickConfig` (an a-1 amendment).
