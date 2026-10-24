# bd-cache-py Architecture

Status: Approved (lead ruling, bc-lead, 2026-10-24). Crate-local decisions (domain code `PY`).
Repo-level context: [`../architecture.md`](../architecture.md). Requirements:
[`requirements.md`](requirements.md). Created by sprint a-8.

`bd-cache-py` is the Python module `bd_cache_rs` (maturin / PyO3). It exposes
the cache in process, so atm-bd can call it from its own scheduler (ATM cron)
without `sc-runtime`. It is a thin layer: it holds no cache rule, it opens the
concrete composition and converts arguments, replies and errors.

It depends on `bd-cache`, `bd-cache-store`, `bd-cache-model`, `sc-bd-cli` and
`sc-beads` (the Boundaries table of [`../architecture.md`](../architecture.md))
and, third party, on `pyo3` (workspace) with `serde` and `serde_json`
(already in the workspace) to convert replies; `serde` and `serde_json` are allowed third-party dependencies of this crate (lead ruling R3, bc-lead, 2026-10-24).

| Module | Contents |
|---|---|
| `lib` | the `#[pymodule]` `bd_cache_rs`: the `Cache` class, `CacheError`, `UNREACHABLE`, `__version__` |
| `compose` | pure Rust: `open` builds `Cache<SqliteStore, FlockDrainLock, BdCli>` from the repository (`load_repo_config`, validated overrides, the canonical project directory); the process-wide `ProjectLocks` set |
| `slot` | pure Rust: the closable, mutex-guarded owner of the `Cache` (close, poison handling) |
| `handle` | `PyCache`, the Python `Cache`: argument parsing, the sixteen verbs, GIL release |
| `convert` | `serde_json::Value` to Python objects, JSON text for `write`, seconds to `Duration` |
| `error` | `CacheError`, `ClosedError` and the `BD_CACHE.*` error to exception mapping |

## ADR-PY-0001: A thin layer over the concrete composition

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Context: REQ-BDC-0910 asks for the cache in process from Python. The core is
generic over its store, lock and backend, and the bindings must pick types.

Decision: the handle owns `Cache<SqliteStore, FlockDrainLock, BdCli>` and a
`SystemClock` that `Cache` and `BdCli` share (the core's rule that deadlines
are measured on one clock). `Cache.__init__` follows the repository
configuration rules of `docs/architecture.md`: it loads `.beads/bd-cache.toml`
with `bd_cache_store::load_repo_config`, applies the keyword overrides,
opens the store (which locates the file from `.beads/metadata.json`; no `bd`
starts), and runs `bd` as `bd_binary` in `project_dir` (default: the
repository root). Each verb parses its arguments into the validated newtypes
(`BeadId`, `Actor`, `IdemKey`, `ScopeId`) and calls the `Cache` method of the
same name. The binding adds no rule: a defect in a rule is fixed in
`bd-cache`, and the pytest suite checks that the verbs reach it with the
right arguments (the argv `bd` saw) and return what it returned.

Dependencies (lead ruling R2, R3): the crate takes `bd-cache`, `bd-cache-store`,
`bd-cache-model`, `sc-bd-cli` and `sc-beads` through `[workspace.dependencies]`
(`bd-cache-store` and `sc-bd-cli` were added there for this crate), and the
third-party `pyo3`, `serde` and `serde_json`. `serde` is used only for the
`Serialize` bound of replies.

One `ProjectLocks` set per process (a `OnceLock` in `compose`) is shared by every
handle, keyed by the canonical project directory (std `canonicalize`, with the
Windows `\\?\` prefix removed by a small helper, so no `dunce` dependency is added;
an unresolvable directory is refused): `bd` on embedded Dolt must not run twice at once in a project, and two
handles on one project (different cache files) would otherwise each lock
separately. It is the only static; it is correctness state and the libraries
(`sc-bd-cli`) still take the set as an argument. The deadlines of `sync` and
`drain` are minted from the `Arc<dyn Clock>` that the `Cache` and `BdCli` share.

Consequences: a different backend (HTTP, later) needs a new composition and a
new handle type; the Python verbs stay the same. The default cache file is
`.bd-cache/<dolt_database>.bd-cache.sqlite3`; atm-bd keeps its
`.atm-bd/` file by setting `cache_path`.

## ADR-PY-0002: Replies are builtin objects; one exception class

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Context: atm-bd reads `bd_cache.py`'s JSON dicts today. The `bd-cache-model`
reply types derive `Serialize`.

Decision: a reply is `serde_json::to_value` of the Rust reply, converted
recursively to `dict` / `list` / `str` / `int` / `float` / `bool` / `None`.
No Python class mirrors a reply, so the shapes below are the whole contract
and change only with the wheel's version. The shapes are the Rust ones
(`bd-cache-model`), not `bd_cache.py`'s; the Mapping table records each
difference. A typed `CacheError` raises `bd_cache_rs.CacheError`, one class
made with `create_exception!` (an abi3 wheel built for Python 3.9 cannot
subclass `Exception` from a Rust class before Python 3.12). Each raised
instance carries `code`, `detail`, `fix`, `reason`, `payload`, `body` and
`exit_code` as attributes; `payload` is `CacheError::to_json` without the
fixed keys, `body` is the whole object (REQ-BDC-0802, 0803). A value that is
not a valid argument is `ValueError`, never a `CacheError`: the cache has no
code for a caller's malformed id. That includes an `op` outside
`bd_cache_model::OPS` and a `list` status outside bd's seven: the binding
checks them before the core (lead ruling R7, bc-lead, 2026-10-24; the core's
`BAD_OP` stays for its other callers), so `bd_cache.py`'s `BD_CACHE.BAD_OP`
becomes `ValueError` here.

Consequences: `error.extra` of `bd_cache.py` is `error.payload`; its
`as_json()` is `error.body`.

## ADR-PY-0003: Threading

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Context: ADR-CORE-0001: a `Cache` is `Send`, `!Sync`, owns one connection,
and every verb is synchronous. A verb can wait on `bd` for up to its deadline.
The first draft made the handle `#[pyclass(unsendable)]`; a handle touched or
dropped on another thread then panicked inside PyO3 (seen on CI: a test thread,
and a Python object freed by whichever thread held the last reference).
That cannot be allowed for an object a scheduler may hand between threads.

Decision (amends the sprint doc's "unsendable" wording, per lead ruling of
2026-10-24 on PR #11): the handle owns `Mutex<Option<Inner>>`, the "behind
their own mutex" option ADR-CORE-0001 names. The handle is `Send + Sync` to
PyO3; any thread may call a verb, `close()` or drop it. Verbs run one at a
time. Every verb takes the lock and runs inside `Python::detach`, so a thread
waiting for the lock or for `bd` holds no GIL. `closed` is an atomic flag set
as `close()` begins; `close()`, `__exit__` and `closed` never hold the GIL
while they wait: `close()` waits for the in-flight verb inside `Python::detach`
and drops the connection there. A verb runs under `catch_unwind` inside the
slot: if it panics, the slot takes and drops the connection (it may be
mid-statement) and sets `closed` before the call returns, so the cache file can
be opened again at once, and the call raises `ClosedError` carrying the panic
message and telling the caller to open a new `Cache`; later calls raise it too.
No `PanicException` reaches Python. (Built with `panic = "abort"` a panic
aborts the process, which is what a panic means.) A lock poisoned outside a
verb (a panic in a `Drop`) is handled the same way: the value is dropped, the
poison is cleared while the guard is held, and the first call reports it.
Nothing reuses a connection after a panic. Replies are converted after the GIL is taken again. SQLite
allows one connection per process per file (NFR-BDC-0007), so a second open
of the same file in the process is `STORE_FAILED`; threads that want to run
verbs in parallel open different files.

Consequences: no background thread, no unsafe, no panic path for cross-thread
use. A long verb blocks other threads' verbs on the same handle (not their
Python code).

## ADR-PY-0004: Packaging, naming and test strategy

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Decision:

- Names: crate `bd-cache-py`, library and Python module `bd_cache_rs`,
  distribution `bd-cache-rs` (`bd_cache` stays `bd_cache.py`'s name, so both
  import in one interpreter during the cutover). The version is the
  workspace's; `bd_cache_rs.__version__` versions the reply shapes.
- Wheels: abi3 (`abi3-py39`), one wheel per platform serves CPython 3.9 and
  later (atm-bd's cron needs 3.11 for `tomllib`; hooks may run older).
  `pyo3` has no `extension-module` feature here: maturin sets
  `PYO3_BUILD_EXTENSION_MODULE`, so `cargo clippy` and `cargo test` link
  against a Python and work for this crate too.
- CI: `ci.yml` (a-1) excludes `bd-cache-py` from the workspace gates because
  it needs a Python to link; `.github/workflows/python.yml` is its gate:
  clippy and `cargo test` for the crate on macOS, Linux and Windows (lead ruling
  R4), a wheel on macOS arm64 and Linux x86_64, and the pytest suite against
  the installed wheel on Python 3.9, 3.11 and 3.13. The wheel and pytest stay
  off Windows because the atm-bd hooks run on POSIX only.
- Tests: pytest drives every verb against the real composition. The `bd`
  program is `tests/fake_bd_main.py`, a recording fake the Rust `BdCli`
  spawns like the real one (strict rows, exit 13 guard refusals, down, delayed
  or blocked-until-released answers on request, start and end of every call
  logged). Tests about time are causal: a call is blocked until a Python thread
  creates a release file, which that thread does only after seeing something
  that needs the GIL free; durations are loose sanity bounds only. The mapping
  test reads `tests/atm_core_05831df_calls.json`, extracted from atm-core with
  `tests/atm_core_05831df_extract.py`. It runs under the interpreter that
  runs pytest through a shell wrapper, so no `bd` is needed.
- Publishing: not in phase a. A later release step publishes with the `pypi`
  and `testpypi` environments (`PYPI_TOKEN`, `TEST_PYPI_TOKEN`).

## Reply shapes

Versioned with the wheel (`0.1.0`). `ts` is an RFC 3339 UTC `str` with second
resolution; `bead` is a bd row dict (all `bd list --json` keys of the strict
model); `view` is `{bead, stale, blocked}`; `outage` is `None` or
`{since: ts, last_error: str}`; `entry` is below.

| Verb | Reply |
|---|---|
| `get` | `{row: view, scope_last_ok_at: ts or None, pending: int, outage}` |
| `refresh` | `[view, ...]` (bd's rows, stored with force, pending writes replayed) |
| `write` | `{seq: int, duplicate: bool}` |
| `list` | `{scope: str, last_ok_at: ts or None, rows: [view, ...], outage}` |
| `ready` | `{scope, last_ok_at, basis, locally_adjusted: bool, ready: [view, ...], read_fallback: str or None, missing_rows: [str, ...]}`; `basis` is `{kind: "fresh", fetched_at}`, `{kind: "cached", fetched_at, stale: bool}` or `{kind: "unknown"}` |
| `sync` | `{scope, rebuilt: bool, model_current: bool, stored: int, blocked: int, deferred: bool, outage}` |
| `drain` | `{busy: bool, applied: [seq], retrying: [seq], escalated: [{seq, bead, op, actor, phase, reason}], deferred: [seq], deferred_in_flight: [seq], outage}` |
| `entry`, each of `unannounced_escalations()` | `{seq, idem_key, phase: str or None, bead, op, args: dict, precondition: dict, actor, state, attempts: int, last_error: str or None, announced_at: ts or None, created_at: ts, applied_at: ts or None}`; `state` is `pending`, `applied`, `escalated` or `resolved` |
| `resolve`, `mark_announced`, `set_sync_alert` | `None` |
| `prune` | `{scope, beads_deleted: int, out_of_scope_swept: int}` |
| `status` | `{meta: {str: str}, journal: {"<phase>:<state>": int}, phases: [{phase, last_started_at, last_ok_at, last_error, beads}], escalated: [entry, ...], outage, stale: [str, ...]}` |
| `sync_alert` | `str` or `None` |
| `open_work` | `{pending: int, escalated_unannounced: int}` |

## Mapping from `bd_cache.py`

The names the atm-bd hooks and `bd_cache_cron.py` use at atm-core `05831df`
(`sanity_task_close_hook.py`, `close_fields_backfill.py`, `close-vars.py`,
`bd_cache_cron.py`, and the `bd_cache.py` command lines their messages tell
people to run), the binding that replaces each, and what differs. `cache` is a
`bd_cache_rs.Cache`. Timeouts that `bd_cache.py` defaulted to `BD_TIMEOUT`
(30 s) are explicit arguments here.

| `bd_cache.py` | Called by | Binding and differences |
|---|---|---|
| `BdCache(root, runner=, clock=, db_path=)` | all hooks, cron | `bd_cache_rs.Cache(root, bd_binary=, cache_path=)`; `runner` (a `subprocess.run` stand-in) becomes `bd_binary`, a program path (tests pass a recording program); `clock` has no equivalent; `db_path` is `cache_path`; the default file is `.bd-cache/<db>.bd-cache.sqlite3`, not `.atm-bd/`, so set `cache_path` to keep the Python file; raises `CacheError` (`NO_DATABASE`, `WRONG_DATABASE`, `SCHEMA_VERSION`) as before |
| `cache.get(bead, miss_timeout=)` | sanity hook, backfill, close-vars | `cache.get(bead, miss_timeout=)`; the reply is `{row, ...}`, the row dict of Python is `reply["row"]["bead"]`; `reply["row"]["blocked"]` is new |
| `cache.cached(bead)` | cron (`active_phases`) | `cache.get(bead, miss_timeout=0)`: a hit returns, a miss raises `BD_UNAVAILABLE` without a `bd` call (where Python returned `None`) |
| `cache.bd_rows(ids, timeout)` then `cache.store(row, force=True)` | sanity hook (`refresh`), cron, close-vars | one call, `cache.refresh(ids, timeout)`: returns a list of views (not a dict by id) already stored with force and with pending writes replayed, so Python's `view(row)` is not needed |
| `cache.store(row)` | cron, close-vars | covered by `cache.refresh(ids, timeout)`; the sync stores rows by itself |
| `cache.view(row)` | close-vars | covered by `cache.refresh(ids, timeout)` and `cache.get(bead)`, which replay pending writes |
| `cache.list(phase, status, label)` | backfill | `cache.list(phase, status=, label=)`; the reply is `{scope, last_ok_at, rows, outage}` with views, not a list of rows; an unknown phase is `SCOPE_UNKNOWN` (sync it first) |
| `cache.write(bead, op, args, idem_key=, actor=, precondition=)` | sanity hook, backfill | `cache.write(bead, op, args, precondition, idem_key, actor)` (keywords work); `args` and `precondition` may be dicts or JSON text; the reply is `{seq, duplicate}` (Python adds `ok` and `state`; read `cache.entry(seq)["state"]`); a duplicate with another op is `IDEM_CONFLICT` with `payload["seq"]`; an unknown `op` is a `ValueError` here where `bd_cache.py` raised `BD_CACHE.BAD_OP` (ADR-PY-0002) |
| `cache.drain(deadline=, monotonic=)` and the detached `bd_cache.py drain` command (`drain_now`) | cron, sanity hook | `cache.drain(start_gate_s, hard_s)`: two seconds-from-now bounds (start gate and hard deadline, REQ-BDC-0402) instead of one monotonic deadline; the cron passes `budget - 5` and `budget - 2`. The hook drains in process (`cache.drain(...)` on a thread of its own or left to the cron) instead of spawning a process; the report has no `ok` or `backend`, and carries `deferred_in_flight` and `outage` |
| `cache.sync(phase, timeout=)` | cron, backfill | `cache.sync(phase, timeout)`; the report is `{scope, rebuilt, model_current, stored, blocked, deferred, outage}`; an unreachable backend returns the report with `outage` set instead of raising `SYNC_FAILED` (ADR-BDC-0005) |
| `cache.prune(phase)` | `bd_cache.py prune` | `cache.prune(phase)`; `{scope, beads_deleted, out_of_scope_swept}` |
| `cache.resolve(seq, note, actor)` | `bd_cache.py resolve` (named in the escalation text) | `cache.resolve(seq, note, actor)`; returns `None` (read `cache.entry(seq)` for the state) |
| `cache.status()` and `status['db']` | cron `doctor` | `cache.status()` and `cache.path` (a `pathlib.Path`; `status['db']`); the reply has `meta`, `journal`, `phases`, `escalated` (full entries), `outage` and `stale`; no `ok`, `cache_schema_version` or `model_bd_version` |
| `cache.path` | sanity hook (`.with_suffix(".drain.log")`) | `cache.path`, a `pathlib.Path` |
| `cache.unannounced_escalations()` | cron | `cache.unannounced_escalations()`; entries use the `entry` shape (`phase`, `args` as a dict, `state`) |
| `cache.mark_announced(seq)` | cron | `cache.mark_announced(seq)`; an unknown seq is `NOT_FOUND` |
| `cache.sync_alert(phase)` | cron | `cache.sync_alert(phase)` |
| `cache.set_sync_alert(phase, cause)` | cron | `cache.set_sync_alert(phase, cause)` |
| `cache.clear_sync_alert(phase)` | cron | `cache.set_sync_alert(phase, None)` |
| `cache.outage()` | cron | `cache.status()["outage"]`: `{since, last_error}`, or `None`; there is no `announced_at` (below) |
| `cache.mark_outage_announced()` | cron | `cache.set_sync_alert(phase, "backend_unavailable")` for each active phase whose lead was told; `cache.sync_alert(phase)` says whether it was; the end of the outage resets it (`cache.set_sync_alert(phase, None)`) without a message (ADR-BDC-0005) |
| `cache.record_outage(error)`, `cache.clear_outage()` | cron | none: the cache records and clears the outage itself on every backend call that is unreachable or answers (ADR-BDC-0005) |
| `cache.has_open_work()` | cron (`should_stop`) | `cache.open_work()`: `{pending, escalated_unannounced}`; open work is `any(...)` of the values |
| `CacheError`, `error.code`, `error.detail`, `error.fix` | all | `bd_cache_rs.CacheError`, same attributes |
| `error.extra` | cron (`extra.get("cause")`) | `error.payload` (`cause` and `phase` of `SYNC_FAILED`, `unmet`, `seq`, `known`, `source`) |
| `error.as_json()` | cron | `error.body` |
| `bd_cache.UNREACHABLE` | cron | `bd_cache_rs.UNREACHABLE`, the same four codes (a frozenset) |
| `bd_cache.__file__` | sanity hook (`drain_now` spawns `python bd_cache.py drain`) | none: the hook calls `cache.drain(start_gate_s, hard_s)` in process, so there is no script to spawn (see the `drain` row) |
| `cache.dependents(bead, type)` | none at `05831df` | not exposed (not one of the verbs of REQ-BDC-0910) |

The CLI (`bd_cache.py get|list|write|drain|sync|prune|resolve|status`) is the
drop-in of REQ-BDC-0806, deferred to the `bdc` binary; the table above covers
the same operations through the handle.

## Known limits

- One process holds one connection per cache file (NFR-BDC-0007): a hook that
  opens a handle while the cron's handle is open in the same process gets
  `STORE_FAILED`; separate processes are fine.
- The two implementations must never drain the same journal at the same time
  (phase plan, atm-bd adoption and cutover): stop the Python cron before the
  bindings' `drain` runs.
- Verbs on one handle run one at a time, whatever the thread (ADR-PY-0003).
