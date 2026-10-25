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
| `journal`, `scope`, `state` | journal types, `ScopeId`, `PhaseName` (with `PhaseName::RULE`, the phase-name rule text), store-side state, `BaseEntry` and `StoreStatus` |
| `store`, `sender` | `CacheStore` (with `base_entry`, `has_entries_since`, `mark_announced_within`, `set_sync_alert_within`), `DrainLock` / `DrainGuard`, `EscalationSender` (with `send_within`), `SendError` and `SendErrorKind`, `MEMBER_NAME_RULE` (the member-name rule text) |
| `config` | `RepoConfig`, `CacheConfig`, `DrainBudget` and the default constants |
| `reply`, `call`, `error` | reply shapes (provisional, REQ-BDC-0806 deferred), `CallKind` / `CallFailure`, `CacheError` with its code classification (`UNREACHABLE_CODES`, `ENVELOPE_KEYS`, `is_unreachable`; ADR-MODEL-0002 amendment, a-8) |
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
A `Failed` on `Check` is `BD_VERSION_FAILED`. `Ready` failures are
`BD_LIST_FAILED`; `Sync` failures are `SYNC_FAILED` with cause
`SyncCause::BdListFailed`. Precondition keys are `GuardKey`.

### ADR-MODEL-0002 amendment (fix-a-1-1): typed check step and kept source

**Status:** Approved (lead ruling, bc-lead, 2026-10-23; RBP-F005, RBP-F006)

Context: the check step was read from a stderr prefix, and mapped errors
dropped the backend error.

Decision: `CheckFailed { step }` is `BD_VERSION_FAILED` or `BD_SCHEMA_FAILED`
by its step; no stderr is inspected (RBP-F006). Every mapped error keeps the
backend error as its `source` (`BackendCause`) and in its JSON as
`source: {code, detail}` (RBP-F005).

Consequences: `CheckFailed` is a `BeadsCacheError` variant (ADR-BEADS-0002);
callers can walk `Error::source()` to the bd failure.

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

### ADR-MODEL-0002 amendment (a-8, lead ruling R9): the code classification

**Status:** Approved (lead rulings R9 and R11, bc-lead, 2026-10-24)

Context: the Python bindings (a-8) and the drain (a-5) each kept their own copy
of which codes mean "bd could not answer" and which keys of a typed error's JSON
are envelope rather than payload. Copies drift when a variant is added.

Decision (additive; no change to the wire format):

1. **Items.** `bd_cache_model` gains `pub const UNREACHABLE_CODES: [&str; 4]`,
   `pub const ENVELOPE_KEYS: [&str; 5]` and `CacheError::is_unreachable(&self)
   -> bool`, re-exported from `lib.rs` (the two constants). A fourth item,
   `CacheError::payload_keys`, is `#[cfg(test)]` and crate-private test support:
   it has no consumer outside this crate's tests, so it is not public API and
   does not exist in a non-test build.
2. **Exhaustive.** `is_unreachable` is a `match` with no wildcard arm, enforced in
   every build: adding a `CacheError` variant does not compile until it is
   classified. `payload_keys` and the tests' `witness` (declared once with the
   variant-name list by the `arms!` macro, tied to the samples by
   `every_arm_has_a_sample`) are exhaustive too, but only in test builds, which CI
   runs on every push.
3. **The unreachable set.** `UNREACHABLE_CODES` is `BD_UNAVAILABLE`,
   `BD_VERSION_FAILED`, `BD_SCHEMA_FAILED` and `BD_LIST_FAILED`: the set of
   ADR-CORE-0101 (the drain's stop policy) and `bd_cache.py`'s `UNREACHABLE`.
   `is_unreachable` is that set by variant, a test keeps the constant and the
   method equal, and the drain (`bd-cache/src/drain.rs`) now calls the method
   instead of its own copy (R11).
4. **The envelope.** `ENVELOPE_KEYS` (`ok`, `code`, `detail`, `fix`, `reason`)
   plus `payload_keys` (and the `source` a mapped or store error adds) are
   exactly the keys of `CacheError::to_json`, for every variant and with and
   without a kept source (`the_envelope_source_and_payload_keys_match_to_json_for_every_sample`).
5. **Consumers.** `bd-cache-py` builds its `UNREACHABLE` frozenset from
   `UNREACHABLE_CODES` and splits an error's `payload` from the envelope with
   `ENVELOPE_KEYS`; `bd-cache` (drain) uses `is_unreachable`. `to_json` and every
   code, fix text and exit status are unchanged.

Consequences: one definition of each set; a new variant must be classified.

### ADR-MODEL-0002 amendment (phase-a end): transport, not-found, in-use, metadata cause

**Status:** Proposed (phase-end fix; PE-RBP-F001, F008, F013, F014)

Context: four failures reached the caller with a code whose fix was wrong.
Output `bd` printed that could not be read whole (a read error, or over
`sc-bd-cli`'s `MAX_OUTPUT_BYTES`) was `BD_CONTRACT` ("regenerate sc-beads'
rows"), though nothing was parsed. A guarded close whose guard check found no
row was a fabricated `Failed { code: 1 }`, so the drain retried a missing bead
`max_attempts` times. A second open of one cache file in one process was
`STORE_FAILED` ("move it aside"), though the file is fine. A `metadata.json`
that could not be read or parsed was `NO_DATABASE` with the cause flattened into
text and the `bd init` fix, even when the file exists but is not readable.

Decision (additive):

1. `BeadsCacheError` (`sc-beads`, `#[non_exhaustive]`) gains
   `Transport { what, source: io::Error }` (`SC_BEADS_TRANSPORT`, the read error
   as its `source`) and `NotFound { bead }` (`SC_BEADS_NOT_FOUND`). `sc-bd-cli`
   maps `BoundedError::Output` to `Transport` and a guard check that finds no row
   to `NotFound`. `Contract` stays for real row-contract breaks.
2. `from_backend` maps `Transport` by `CallKind` to the transient codes:
   `Write` → `BD_WRITE_FAILED`, `Sync` → `SYNC_FAILED` (cause `BD_LIST_FAILED`),
   every other kind, `Check` included, → `BD_LIST_FAILED` (bd ran, so
   `BD_VERSION_FAILED`'s "check that bd is installed" would be wrong). The kept
   backend copy keeps the `io::Error`'s kind and text. `NotFound` maps to
   `NOT_FOUND`, which the drain escalates without counting an attempt.
3. The additive code `BD_CACHE.STORE_IN_USE`, `CacheError::StoreInUse { detail }`:
   this process already has the cache file open (NFR-BDC-0007). Its fix: close
   that handle or reuse it; the file is fine. Not unreachable, no payload, exit 3.
   `bd-cache-store` returns it for a refused one-connection claim instead of
   `STORE_FAILED`.
4. `CacheError::NoDatabase` gains `source: Option<StoreCause>`: the I/O or JSON
   error reading `metadata.json`, rendered as `source: {detail}` like
   `STORE_FAILED`'s. When the source is an I/O error of kind `PermissionDenied`,
   `fix()` says the file exists and must be made readable, not `bd init`.

Consequences: `CacheError` has 21 codes (`golden_json_for_every_code`). The codes
are additive to the v3 CLI contract (ADR-BDC-0003). A consumer that matched
`STORE_FAILED` for a double open (the Python bindings' open path) now sees
`STORE_IN_USE`. Constructing `NoDatabase` needs the new field; matching it with
`{ .. }` is unchanged.

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

Context: QA-1 found raw strings for keys, scopes and instants, and a call
result with a dead success variant.

Decision: `IdemKey::new` rejects an empty key; scopes are
`ScopeId::Phase(PhaseName)` with a validated name (also in
`ScopeUnknown.known`, `EscalatedItem.phase` and `phase_of`); journal,
sync-state, outage and ready-base instants are `Timestamp`s. `CallOutcome`
is replaced by the failure-only `CallFailure { Cut, Outage(CacheError),
Error(CacheError) }`: `call` returns `Result<T, CallFailure>`, an outage
carries its `BD_UNAVAILABLE` error (so a caller never wraps the text again),
and `CallFailure::into_error` turns a cut into `BD_UNAVAILABLE`. An
unassigned update overlays as no assignee, as bd stores it.

Consequences: invalid keys, scopes and instants cannot be built; callers
match `CallFailure` instead of an outcome with an unreachable `Ok`.

### ADR-MODEL-0003 amendment (fix-a-1-2): instants, not text

**Status:** Approved (lead ruling, bc-lead, 2026-10-23; QA-002)

Context: the newer-row guard compared `updated_at` as text.

Decision: it compares `updated_at` as an instant (second precision), never
as text; the contract rule `newer_row_guard_compares_instants` checks it with
offset and fractional timestamps.

Consequences: every `CacheStore` must compare instants (a-4 passes the rule).

### ADR-MODEL-0003 amendment (fix-a-1-3): unparseable instants and absent seqs

**Status:** Approved (lead ruling, bc-lead, 2026-10-23; ARCH-201, ARCH-202)

Context: the guard fell back to text order when a value did not parse, and
the doc left `resolve` on an unknown seq ambiguous.

Decision: when either `updated_at` does not parse, the stored row is not
newer and the incoming row is stored; text is never ordered (`FakeBackend`'s
cursor filter likewise lists such a row). `resolve` on an absent seq is
`NOT_ESCALATED`; `NOT_FOUND` applies to the other seq-taking methods.

Consequences: a corrupt stored timestamp heals on the next upsert; a-4's
store follows the same rules through the contract suite.

### ADR-MODEL-0003 amendment (a-1 amendment 2): ready replies say when

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Context: a-6 QA-1 (arch-qa) found two gaps against the repo ADRs.
ADR-BDC-0005 says every read reply carries the scope's `last_ok_at`, but
`ReadyReply` had no such field. ADR-BDC-0002 says every `ready` reply states
its basis, including bd's fetch time, but `ReadyBasis::Fresh` carried none.

Decision:
- `ReadyReply` gains `last_ok_at: Option<Timestamp>`, the scope's last
  successful sync, as `ListReply` has (`GetReply` carries it as
  `scope_last_ok_at`).
- `ReadyBasis::Fresh` becomes `Fresh { fetched_at: Timestamp }`: the
  instant taken before the first `list_blocked` / `list_ready` call of that
  answer, the same instant a refreshed base stores. Every basis except
  `Unknown` now names when bd's answer was fetched.
- Serialized: `"last_ok_at"` on the reply, and
  `{"kind": "fresh", "fetched_at": ...}` for the basis (golden in
  `tests/goldens.rs`).

Consequences: a-6 (bd-cache read path) fills `ReadyReply.last_ok_at` from the
scope's sync state and `Fresh.fetched_at` with the instant it takes before
the first `list_blocked` / `list_ready` call, the same instant the refreshed
base stores. This closes the ADR-BDC-0002 and ADR-BDC-0005 gaps a-6 QA-1
found. The serialized basis changes from `{"kind": "fresh"}` to
`{"kind": "fresh", "fetched_at": ...}`; reply shapes are provisional
(REQ-BDC-0806, deferred) and not claimed equal to Python, so no #557 note is
needed. The `scope_last_ok_at` / `last_ok_at` naming split across read
replies is left for the bdc sprint (REQ-BDC-0806) to settle.

### ADR-MODEL-0003 amendment (a-7): ports gain `base_entry`, `has_entries_since`, `send_within`, `mark_announced_within` and `set_sync_alert_within`

**Status:** Approved (lead ruling, bc-lead, 2026-10-24, fix-a-7-2); the name rules added by lead ruling S32 (bc-lead, 2026-10-24, fix-a-7-8); `mark_announced_within` and `set_sync_alert_within` added by lead ruling S33 (bc-lead, 2026-10-24); `set_sync_alert_within` also used for the alert clears that send no message by lead ruling S35 (bc-lead, 2026-10-24, fix-a-7-10)

Context: the ports are a-1's contract, and a-4, a-5, a-6 and a-8 build on them.
Sprint a-7 changed them, by lead rulings S2, S4 and S11 (bc-lead, 2026-10-24) and
S14/S15 (fix-a-7-2), to make a cached `get` independent of the size of its scope
and journal (NFR-BDC-0001) and to bound the sync task's messages:

Decision:
- `CacheStore::base_entry(scope, id) -> Option<BaseEntry>`: one bead's entry of
  the scope's stored ready base (fetch instant, stale mark, the bead's
  `blocked_by`), equal to `BaseEntry::of(ready_base(scope), id)`. The provided
  body loads the whole base; an implementation with an index overrides it and
  reads the head and the entry in one snapshot (ADR-STORE-0004). The contract
  rule `base_entry_rules` pins it.
- `CacheStore::has_entries_since(at) -> bool`: "something is pending, or was
  applied at or after `at`". The provided body loads both lists; an
  implementation answers from indexes. An entry's applied time is its
  `applied_at`, else, for an entry applied by `bd_cache.py` (which sets only
  `updated_at`), its `updated_at`; an entry whose time cannot be read counts as
  applied since (conservative). `applied_since` follows the same rule (ADR-STORE-0004). Contract rule `has_entries_since_rules`.
  Port rule (lead rulings S25/S26, bc-lead, 2026-10-24): though both take `&self`, an
  implementation may write to date entries another writer left without an applied
  time; it must not wait for the write lock to do so, and must give the same answer
  whether or not the write runs. `SqliteStore` does it only above a threshold of
  undated entries (ADR-STORE-0004).
- `EscalationSender::send_within(lead, team, body, timeout)`: a provided method
  that ignores `timeout` and calls `send`; a sender that can bound its work
  overrides it. The caller checks its own budget after every send, so a sender
  that ignores the timeout costs at most the send in flight. `lead` and `team`
  stay `&str`: callers pass names they validated, and a sender that builds a
  command line keeps a defensive guard (S15).
- `SendError` gains `kind: SendErrorKind` (`InvalidAddress`, `NotStartable`,
  `TimedOut`, `Cancelled`, `Exited`, `Other`; `as_str`, `fix`, `Serialize`) and
  the constructors `SendError::new` and `with_kind`. **This is a breaking change
  to the struct literal** `SendError { detail }`; no other crate in the workspace
  or on the a-8 branch built one. `SendErrorKind` is `#[non_exhaustive]`.
- `MEMBER_NAME_RULE` (`sender.rs`) and `PhaseName::RULE` (`scope.rs`) state the name rules once; the sync task's refusals and the validators quote them (S32, fix-a-7-8).
- `CacheStore::mark_announced_within(seq, at, wait)` and `CacheStore::set_sync_alert_within(scope, cause, wait)` (lead ruling S33, bc-lead, 2026-10-24): the post-send markers and the alert clears that send no message (S35), bounded by the tick's remaining time. Their provided bodies ignore the wait and call `mark_announced` and `set_sync_alert`, so `FakeStore` is unaffected; `SqliteStore` bounds the busy timeout by `wait` (never above its own `BUSY_TIMEOUT`).

Consequences: every `CacheStore` passes the new contract rules (FakeStore
through the provided bodies, SqliteStore through its indexes and bounded writes),
`bounded_marker_writes_match_the_unbounded_forms` included (QA7-004: each `_within`
method with a non-zero and a zero wait on a free lock leaves the post-state of its
unbounded form, clearing an alert with `None` included); `EscalationSender`
implementations keep compiling. A store or wrapper that can wait overrides both
`_within` methods (forwarding the wait); the provided bodies are unbounded. The earlier wording that `EscalationSender` and
`RepoConfig.tick` were "frozen in a-1" meant "defined in a-1"; they change only
by an amendment like this one.

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
