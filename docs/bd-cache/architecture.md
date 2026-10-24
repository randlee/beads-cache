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
| `lib.rs` | a-1 | `Cache`, `new`, the public verbs (each delegates to its module), the store passthroughs (`unannounced_escalations`, `mark_announced`, `sync_alert`, `set_sync_alert`, `open_work`), the `testing` hooks |
| `outage.rs` | a-1 | `call`, `classify`, `note_unavailable`, `note_ok` |
| `view.rs`, `backend_check.rs`, `miss.rs` | a-1 | `current_view`, `run`, `fill` |
| `write.rs`, `drain.rs` | a-5 | stubs until a-5 |
| `get.rs`, `refresh.rs`, `list.rs`, `sync.rs`, `ready.rs`, `resolve.rs`, `prune.rs`, `status.rs` | a-6 | stubs until a-6 |
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
thread: a scheduler (`bd-cache-sync` on sc-cron, or atm-bd's
`bd_cache_cron.py` through the bindings) owns one `Cache` per cache file and
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
