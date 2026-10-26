# bd-cache Architecture

Status: approved (lead ruling, 2026-10-23). Crate-local decisions (domain code `CORE`). Repo-level
context: [`../architecture.md`](../architecture.md). Created by sprint a-1.

## Index

| Document | Owner | Carries |
|---|---|---|
| this file | a-1 | layout, shared helpers, ADR-CORE-0001 (threading), ADR-CORE-0002 (skeleton) |
| [`write-path.md`](write-path.md) | a-5 | the write path's REQ-CORE and ADR-CORE entries (`write`, `drain`) |
| [`read-path.md`](read-path.md) | a-6 | the read path's REQ-CORE and ADR-CORE entries (`get`, `entry`, `refresh`, `list`, `ready`, `sync`, `resolve`, `prune`, `status`), including the ready adjustment table |

Requirements: [`requirements.md`](requirements.md).

## Layout

`Cache<S: CacheStore, L: DrainLock, B: CacheBackend>` owns its config,
store, lock, backend and an `Arc<dyn Clock>` shared with the backend. The
crate depends only on `bd-cache-model` and `sc-beads`, never the store or
`sc-bd-cli`.

| File | Owner | Contents |
|---|---|---|
| `lib.rs` | a-1 | `Cache`, `new`, the public verbs (each delegates to its module), the store passthroughs (`unannounced_escalations`, `mark_announced`, `sync_alert`, `set_sync_alert`, `open_work`), `clock()` (a-7: the clock the cache and backend share), `scope_synced` (a-7, S30: one `sync_state` row through the port), `mark_announced_within` and `set_sync_alert_within` (a-7, S33: the bounded passthroughs of the two marker writes), `CLOSED_STATUS`, the `testing` hooks |
| `outage.rs` | a-1 | `call`, `classify`, `note_unavailable`, `note_ok` |
| `view.rs`, `backend_check.rs`, `miss.rs` | a-1 | `current_view`, `run`, `fill` |
| `write.rs`, `drain.rs` | a-5 | write path and drain (`docs/bd-cache/write-path.md`); b-1: `write_returning`, with `write` over the same journaling |
| `get.rs`, `refresh.rs`, `list.rs`, `sync.rs`, `ready.rs`, `resolve.rs`, `prune.rs`, `status.rs` | a-6 | read path (`docs/bd-cache/read-path.md`); b-1: `GetReply.scope` and `entry_by_key` (`get.rs`), `journal_counts` (`status.rs`) |
| `tests/common/` | a-1 | `World` (the Python suite's world) and fixtures |
| `tests/helpers_*.rs` | a-1 | helper tests |

## ADR-CORE-0001: Threading model

**Status:** Approved (lead ruling, 2026-10-23)

Context: the store holds one SQLite connection per process (NFR-BDC-0007),
and backend calls block for up to their deadline.

Decision: a `Cache` is `Send` and deliberately `!Sync` (a
`PhantomData<Cell<()>>` marker, whatever the store type is). Every verb runs
synchronously on the caller's thread; reads take `&self`, verbs that store
or call bd take `&mut self`. There is no internal locking and no background
thread: a scheduler (`bd-cache-sync` on sc-cron, or a consumer's
own scheduler through the bindings) owns one `Cache` per cache file and
calls `drain` and `sync` from its tick. Async front ends move the `Cache`
into a blocking task, or keep it behind their own mutex. Across processes,
the drain lock (`DrainLock`) admits one drainer, and the store's
transactions keep each write atomic. The backend and the cache share one
`Arc<dyn Clock>`, so every deadline is measured on one clock.

Consequences: no `Cache` is ever shared between threads by reference; a
second process (a hook) opens its own `Cache` on the same file and contends
only for the drain lock.

## ADR-CORE-0002: Skeleton by delegation

**Status:** Approved (lead ruling, 2026-10-23)

Decision: each public verb in `lib.rs` delegates to a `pub(crate)` function
with a fixed signature in its module file (`write::write`, `get::entry`,
...). a-1 ships those files as stubs (`unimplemented!`, marked with the
owning sprint); a-5 and a-6 replace only their own files, so neither edits
`lib.rs` and they never conflict. The shared helpers are crate-private; the
`testing` feature exposes thin hooks (`bd_cache::testing`: `call`,
`note_unavailable`, `note_ok`, `current_view`, `backend_check`, `miss_fill`,
and `store` / `store_mut` / `backend` / `clock` accessors) so integration
tests can drive them, enabled for this crate's own tests through a
dev-dependency on itself. Without the feature the helpers are allowed to be
unused (`cfg_attr` on their modules) until a-5 and a-6 call them, and CI
checks the libraries with default features (`cargo clippy --lib`). The store
passthroughs (`unannounced_escalations`, `mark_announced`, `sync_alert`,
`set_sync_alert`, `open_work`) are implemented in `lib.rs` by a-1. The ten
stub files are in a-1's `owned_paths` until ownership hands over to a-5
(`write.rs`, `drain.rs`) and a-6 (the other eight). `call` returns
`Result<T, CallFailure>` (ruling L5; ADR-MODEL-0003 amendment).

### ADR-CORE-0002 amendment (a-7): `scope_synced` joins the passthroughs

**Status:** Approved (lead ruling S30/S32/S33, bc-lead, 2026-10-24)

Context: the sync task trusts a cached root only when the phase's own scope has
synced (ADR-SYNC-0005, S24), and needs that answer without a bead read.

Decision: the passthrough list (`unannounced_escalations`, `mark_announced`,
`sync_alert`, `set_sync_alert`, `open_work`) gains `Cache::scope_synced`, which
reads one `sync_state` row through the port. It is a-7's edit to `lib.rs`
(S30); a-5 and a-6 are unaffected. S33 adds `Cache::mark_announced_within` and
`Cache::set_sync_alert_within`, the passthroughs of the port's bounded marker writes
(ADR-MODEL-0003 a-7 amendment), also a-7's edit to `lib.rs`.

Consequences: the list of store passthroughs in the paragraph above is
`unannounced_escalations`, `mark_announced`, `sync_alert`, `set_sync_alert`,
`open_work`, `scope_synced`, `mark_announced_within` and `set_sync_alert_within`.

### ADR-CORE-0002 amendment (a-5 QA-1): the temporary helper overrides leave with a-5

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Context: the six `cfg_attr(..., allow(dead_code))` / `allow(unused_imports)`
overrides on the helper modules, the `call` re-export and `Cache.lock` were
temporary, until a-5 and a-6 call the helpers. a-5's write path and drain call
every one of them, so the overrides are stale once a-5 lands.

Decision: sprint a-5 may remove exactly those six overrides and their comment
from `lib.rs`; no other `lib.rs` edit is allowed to a-5 or a-6. a-6 does not
touch `lib.rs`, so whichever of a-5 and a-6 merges second takes the removal
from `integrate/phase-a` with no conflict. Without the `testing` feature the
helpers are then used by a-5's code, and CI's default-feature
`cargo clippy --lib` keeps checking it.

Consequences: the "allowed to be unused until a-5 and a-6 call them" sentence
above no longer applies once a-5 merges.

## Implementation notes (sprint b-7)

- Scopes are named bd filters (ADR-BDC-0013). `Cache::new` writes the configured scopes to
  the registry (the file wins); `scopes_add`, `scopes_set` and `scopes_remove` manage the
  others and refuse a file scope with `CONFIG_INVALID` (`scopes.rs`). `sync(name)` takes the
  name as written and registers an unknown one on demand as the label scope with that label;
  `sync_legacy` and `legacy_scope` are the 0.1 paths, where `x` and the legacy-prefixed `x`
  mean the phase-a scope `x` and never a filter scope of that name (REQ-BDC-0812).
- A sync lists the scope's filter (`Scope::backend_filter`), checks every row against it
  (`selects`), and stores bd's ready and blocked answers intersected with the scope's members
  (`ready::store_base`, ADR-BDC-0002 as amended). Its result sets the failure state
  (`SyncState::record_result`): a failure or an outage sets `failing_since`, an ok clears it;
  an outage is recorded per scope with its error (REQ-BDC-0728, 0729).
- `ingest_scope` (`ingest.rs`) stores a checked `IngestSnapshot` with no bd call: an
  unregistered scope is `SCOPE_UNKNOWN`, an empty snapshot `INGEST_INVALID` unless allowed,
  and the ready ids are the open, unblocked, undeferred rows (REQ-BDC-0726).
- The store-side calls of the lease, the hook state, `scope_announce` and `mark_offered` are
  passthroughs for the sync crate (REQ-BDC-0722, 0727, 0728); their guards and backoff are
  sprint b-8's.
- Tests: `scope_filters.rs` (`sync_label_filter_scope`, `sync_all_scope`, the status
  criterion, on-demand registration, the file-wins and 0.1 rules), `ingest.rs` (each Ingest
  rule and the built-in snapshot regression), `failure_state.rs` (one test per
  `failing_since` transition, compare-and-set marking, reads during an outage).
