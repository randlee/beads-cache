# bd-cache-py Architecture

Status: Approved (lead ruling, bc-lead, 2026-10-24). Crate-local decisions (domain code `PY`).
Repo-level context: [`../architecture.md`](../architecture.md). Requirements:
[`requirements.md`](requirements.md). Created by sprint a-8.

`bd-cache-py` is the Python module `bd_cache_rs` (maturin / PyO3). It exposes
the cache in process, so a consumer can call it from its own scheduler
without `sc-runtime`. It is a thin layer: it holds no cache rule, it opens the
concrete composition and converts arguments, replies and errors.

It depends on `bd-cache`, `bd-cache-api` (0.2), `bd-cache-store`, `bd-cache-model`, `sc-bd-cli` and
`sc-beads` (the Boundaries table of [`../architecture.md`](../architecture.md))
and, third party, on `pyo3`, `serde` and `serde_json` (to convert replies) and
`dunce` (canonical paths), all through `[workspace.dependencies]`; `serde` and
`serde_json` are allowed third-party dependencies of this crate (lead ruling R3)
and `dunce` was added for it (lead ruling R12), bc-lead, 2026-10-24.

| Module | Contents |
|---|---|
| `lib` | the `#[pymodule]` `bd_cache_rs`: the `Cache` class, `CacheError`, `UNREACHABLE`, `__version__` |
| `compose` | pure Rust: `open` builds `Cache<SqliteStore, FlockDrainLock, BdCli>` from the repository (`load_repo_config`, validated overrides, the canonical project directory); the process-wide `ProjectLocks` set |
| `slot` | pure Rust: the closable, mutex-guarded owner of the `Cache` (close, panic handling; poisoning is defensive) |
| `handle` | `PyCache`, the Python `Cache`: argument parsing, the sixteen verbs, GIL release |
| `api` | the 0.2 namespaces `Beads` / `Manage` over `bd-cache-api` (ADR-PY-0005), `CONTRACT_VERSION` |
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
third-party `pyo3`, `serde`, `serde_json` and `dunce` (the last for canonical
paths, lead ruling R12). `serde` is used only for the
`Serialize` bound of replies.

One `ProjectLocks` set per process (a `OnceLock` in `compose`) is shared by every
handle, keyed by the canonical project directory (`dunce::canonicalize`: the
std canonical path, without the Windows `\\?\` verbatim prefix where the plain
form is valid; `dunce` is a workspace dependency added for this crate, lead
ruling R12, bc-lead, 2026-10-24; an unresolvable directory is refused): `bd` on embedded Dolt must not run twice at once in a project, and two
handles on one project (different cache files) would otherwise each lock
separately. It is the only static; it is correctness state and the libraries
(`sc-bd-cli`) still take the set as an argument. The deadlines of `sync` and
`drain` are minted from the `Arc<dyn Clock>` that the `Cache` and `BdCli` share.

Consequences: a different backend (HTTP, later) needs a new composition and a
new handle type; the Python verbs stay the same. The default cache file is
`.bd-cache/<dolt_database>.bd-cache.sqlite3`; the first consumer keeps its
existing file in its own directory by setting `cache_path`.

## ADR-PY-0002: Replies are builtin objects; one exception class

**Status:** Approved (lead ruling, bc-lead, 2026-10-24); amended by lead rulings S38/S42 (bc-lead, 2026-10-24, fix-a-7-12): the config kind is reserved

Context: the first consumer reads the baseline Python implementation's JSON
dicts today. The `bd-cache-model`
reply types derive `Serialize`.

Decision: a reply is `serde_json::to_value` of the Rust reply, converted
recursively to `dict` / `list` / `str` / `int` / `float` / `bool` / `None`.
No Python class mirrors a reply, so the shapes below are the whole contract
and change only with the wheel's version. The shapes are the Rust ones
(`bd-cache-model`), not the baseline Python implementation's; the Mapping table records each
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
`BAD_OP` stays for its other callers), so the baseline Python implementation's `BD_CACHE.BAD_OP`
becomes `ValueError` here. An invalid configuration is a `ValueError` whose `kind` is
`parse`, `scope`, `argument` or `config`; `config` is reserved; not produced
while ConfigLoadError has no further variants (S38/S42), and `path` may be
`None`. An unreadable config
file is an `OSError` of kind `read`.

Consequences: `error.extra` of the baseline Python implementation is
`error.payload`; its
`as_json()` is `error.body`.

## ADR-PY-0003: Threading

**Status:** Approved (lead ruling, bc-lead, 2026-10-24); the one-connection rule is restated for the phase-B methods by ADR-PY-0006

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
verb (a panic in a `Drop`; defensive, nothing in production does this) is handled
the same way: the value is dropped, the poison is cleared while the guard is
held, and the first call reports it.
Nothing reuses a connection after a panic. Replies are converted after the GIL is taken again. SQLite
allows one connection per process per file (NFR-BDC-0007), so a second open
of the same file in the process is `STORE_IN_USE`; threads that want to run
verbs in parallel open different files.

Consequences: no background thread, no unsafe, no panic path for cross-thread
use. A long verb blocks other threads' verbs on the same handle (not their
Python code).

## ADR-PY-0004: Packaging, naming and test strategy

**Status:** Approved (lead ruling, bc-lead, 2026-10-24); amended by ADR-PY-0006 (mixed layout, the native module's name, the test strategy)

Decision:

- Names: crate `bd-cache-py`, library and Python module `bd_cache_rs`,
  distribution `bd-cache-rs` (`bd_cache` stays the baseline Python implementation's module name, so both
  import in one interpreter during the cutover). The version is the
  workspace's; `bd_cache_rs.__version__` versions the reply shapes.
- Wheels: abi3 (`abi3-py39`), one wheel per platform serves CPython 3.9 and
  later (the first consumer's cron needs 3.11 for `tomllib`; hooks may run older).
  `pyo3` has no `extension-module` feature here: maturin sets
  `PYO3_BUILD_EXTENSION_MODULE`, so `cargo clippy` and `cargo test` link
  against a Python and work for this crate too.
- CI: `ci.yml` (a-1) excludes `bd-cache-py` from the workspace gates because
  it needs a Python to link; `.github/workflows/python.yml` is its gate:
  clippy and `cargo test` for the crate on macOS, Linux and Windows (lead ruling
  R4), a wheel on macOS arm64 and Linux x86_64, and the pytest suite against
  the installed wheel on Python 3.9, 3.11 and 3.13. The wheel and pytest stay
  off Windows because the first consumer's hooks run on POSIX only.
- Tests: pytest drives every verb against the real composition. The `bd`
  program is `tests/fake_bd_main.py`, a recording fake the Rust `BdCli`
  spawns like the real one (strict rows, exit 13 guard refusals, down, delayed
  or blocked-until-released answers on request, start and end of every call
  logged, each start with the child's `BEADS_DIR` and working directory). It
  answers as `sc-bd-cli`'s `FakeBd` does (PE-ARCH-007): `--set-metadata`
  values are coerced as `coerce_metadata` does, checked against the shared
  table `tests/fixtures/metadata-coercion.json` (generated from that function),
  and `bd blocked` / `bd ready` follow the P-6 recording (ADR-BEADS-0004),
  checked against `crates/sc-beads/tests/fixtures/p6-blocked/`. An autouse
  fixture removes any ambient `BEADS_DIR` (PE-FTQ-001); a test that needs one
  sets it, with its working directory, in a child process. Tests about time are causal: a call is blocked until a Python thread
  creates a release file, which that thread does only after seeing something
  that needs the GIL free; durations are loose sanity bounds only. The mapping
  test reads the vendored call log recorded from the first consumer at
  `05831df`, made by the companion extraction script (both under `tests/`,
  the files whose paths the lint flags). It runs under the interpreter that
  runs pytest through a shell wrapper, so no `bd` is needed.
- Publishing: not in phase a. A later release step publishes with the `pypi`
  and `testpypi` environments (`PYPI_TOKEN`, `TEST_PYPI_TOKEN`).

## ADR-PY-0005: The namespaces, the error mapping and the 0.1 to 0.2 table

**Status:** Approved (lead ruling, bc-lead, 2026-10-25); created by sprint b-4; amended by ADR-PY-0006 (a `bd-cache-sync` dependency for the phase-B methods; the scope rule)

Context: contract section 10 adds the commands of `bd-cache-api` to the wheel
without touching a 0.1 verb, because the first consumer builds against the 0.1 dicts.

Decision:

- **Namespaces.** `api.rs` defines the classes `Beads` and `Manage`; each holds the
  `Cache` handle and has one `(**kwargs)` method per command. The method turns the
  keyword arguments into a JSON object (`None`, `bool`, `int`, finite `float`, `str`,
  `list`/`tuple`, `dict` with `str` keys; anything else is `TypeError`), calls
  `bd_cache_api::decode`, then `execute` on the handle's `Cache` under the same
  mutex and GIL release as a 0.1 verb, then `Data::to_json` and the usual
  `to_py`. `Request` cannot be built any other way (the operations are
  crate-private), so the binding cannot skip a check. `CacheFacts` (cache file,
  schema version, `dolt_database`, `project_id`, beads directory, repository root)
  is read once when the handle opens (`compose.rs`).
- **Resolved by `__getattr__`.** `cache.beads` and `cache.manage` are not class
  attributes, so `dir(Cache)` is exactly the 0.1 set and the 0.1 test that pins
  it passes unedited.
- **Errors.** `error::cache_err` (a core `CacheError`) and `error::api_err` (an
  `ApiError`) both set `kind`, `message`, `details`, `suggested_action` and
  `retryable`; a core error goes through `ApiError::from`, so the table row
  decides `retryable` for both generations. The 0.1 attributes keep their meaning
  (REQ-PY-0012 lists what an `ApiError` puts in them). Only a Python type mistake
  is not a `CacheError`.
- **No tick.** `cache tick` belongs to `bd-cache-sync`, which this crate does not
  depend on.
- **Boundaries.** `bd-cache-py` gains `bd-cache-api`; it takes no `sc-command` and
  no `clap` (the CI "Crate boundaries" step rejects both).
- **Version.** One workspace version (lead ruling S61): 0.2.0 for the wheel and
  every crate, set with `pyproject.toml` in one commit, because the kit's
  `verify-version-lockstep` needs them equal.

Consequences: a new command in `bd-cache-api` needs one line in `api.rs` (a Rust
test checks the method names against `Command::ALL`); a 0.1 verb never changes
for it.

### The 0.1 to 0.2 table (contract section 10)

| 0.1 verb (unchanged) | New in 0.2 | How the 0.1 shape differs |
|---|---|---|
| `get(bead, miss_timeout)` | `cache.beads.issues_get(id=, miss_timeout_ms=)` | `{row: {bead, stale, blocked}, scope_last_ok_at, pending, outage}`. **Unit change:** 0.1 takes seconds (float), 0.2 milliseconds (int). `miss_timeout_ms=0` is `NOT_CACHED` (retryable); 0.1 `miss_timeout=0` is `BD_UNAVAILABLE` |
| `list(phase, status, label)` | `cache.beads.issues_list(...)` | `{scope: "bk", last_ok_at, rows}`; no paging |
| `ready(phase, fresh_within)` | `cache.beads.ready(...)` | the beads are in `ready`; the basis is top-level; seconds |
| `write(...)` | `cache.beads.issues_update`, `issues_close`, `dependencies_add` | `{seq, duplicate}`; an unknown op raises `ValueError`; BD-CACHE spellings |
| none | `cache.beads.dependencies_list`, `cache.beads.context` | new |
| `refresh(ids, timeout)` | `cache.manage.refresh_issues(...)` | a bare list; seconds |
| `sync(phase, timeout)` | `cache.manage.refresh_scopes(...)` | one report, not `results[]` |
| `drain(start_gate_s, hard_s)` | `cache.manage.flush(...)` | seconds; `phase`; `reason` is a string |
| `prune(phase)` | `cache.manage.evict(...)` | same fields |
| `entry(seq)`, `unannounced_escalations()` | `cache.manage.journal_get(...)`, `journal_list(unannounced=True)` | `idem_key`, `phase`, `precondition`, BD-CACHE args; `last_error` is a string |
| `resolve(...)` returns `None`; `mark_announced(seq)` | `cache.manage.journal_resolve`, `journal_announce` | no read-back |
| `status()`, `open_work()` | `cache.manage.status()`, `health()`, `scopes()` | raw `meta`; `<phase>:<state>` keys |
| `sync_alert`, `set_sync_alert` | unchanged; the setter stays binding-only | none |
| `CacheError` | adds `kind`, `message`, `details`, `suggested_action`, `retryable` | additive |

Every method, old and new, accepts a bare (`bk`) or label (`phase-bk`) scope id.

## ADR-PY-0006: The phase-B methods, the mixed layout and the bd-cache-sync edge

**Status:** Approved (Rand, 2026-10-26); implemented by sprints b-8 (layout, methods, edge) and
b-9 (the test strategy). Amends ADR-PY-0004 (packaging and tests) and ADR-PY-0005 (its "No tick"
and "Boundaries" bullets and its scope sentence); restates ADR-PY-0003's one-connection rule for
the new methods; supersedes REQ-PY-0009 with the vendored call log.

Context: phase B adds `Cache.tick()`, `schedule_*`, `sync_use_python` / `sync_use_builtin`,
`ingest_scope`, `scope_announce` and `scopes_*` (REQ-BDC-0720..0731), and two Python entry points
run as `-m` modules: `bd_cache_rs.run` for the post-sync hook and `bd_cache_rs.sync` for a
consumer's sync script (ADR-SYNC-0008). ADR-PY-0005 says the crate does not depend on
`bd-cache-sync`; the pyproject builds one extension module named `bd_cache_rs`, which cannot also
hold Python submodules; and S84 removes the vendored call log the mapping test reads.

Decision:
- **Edge.** `bd-cache-py` depends on `bd-cache-sync` (a new Boundaries row edge, made by b-8 with
  the code). `Cache.tick()`, `schedule_*` and `sync_use_*` call `bd-cache-sync`; the store-only
  methods go through `bd-cache-api` like the 0.2 namespaces (ADR-BDC-0015).
- **One connection.** Every new method runs on the handle's own `Cache` and connection, under its
  mutex and inside `Python::detach` (ADR-PY-0003); `schedule_*` and `sync_use_*` use the handle's
  `CacheScheduleStore` (ADR-BDC-0014), so the process opens no second connection.
- **Runtime.** `Cache.tick()` and `sync_use_python()` release the GIL (`Python::detach`), then
  build a current-thread tokio runtime on a helper thread the call spawns and joins, and drop it
  before returning: no runtime outlives the call, and a caller on an asyncio event loop thread or
  inside a tokio runtime of its own never has a runtime nested in its context. An asyncio caller
  that does not want to block its loop calls them through `loop.run_in_executor`, as for every
  other verb. Both run the post-sync hook and the Python row as subprocesses (S90); nothing runs
  user code in process.
- **Layout.** maturin's mixed layout: the native module is `bd_cache_rs._native` (`module-name =
  "bd_cache_rs._native"`, `#[pymodule] fn _native`), and `crates/bd-cache-py/python/bd_cache_rs/`
  holds `__init__.py` (re-exporting every 0.1 and 0.2 name from `_native`, with the same
  `__all__`, `__version__` and `CONTRACT_VERSION`), `run.py` (`python -m bd_cache_rs.run`) and
  `sync.py` (`bd_cache_rs.sync.main`). `import bd_cache_rs`, `bd_cache_rs.Cache` and every
  exported name keep working unchanged, which the existing import tests pin. The pyproject
  description loses its consumer wording in the same change.
- **Scopes.** The 0.1 verbs keep the bare-name rule of ADR-BDC-0013; every other scope argument,
  0.2 namespaces included, takes a scope name as written.
- **Tests (amends ADR-PY-0004).** The mapping test and its vendored call log are deleted (b-9);
  a neutral 0.1 call corpus generated in-repo against the fake bd (`tests/corpus/`) covers every
  0.1 verb and reply shape, and the real-interpreter tests of the hook runner, `bd_cache_rs.run`,
  `bd_cache_rs.sync` and the switch run against `fake_bd_main.py`.

Consequences: the Boundaries and Crate Layout rows of `bd-cache-py` gain `bd-cache-sync` in b-8;
the "Mapping from the baseline Python implementation" section below is removed with its test in
b-9 (REQ-PY-0009 superseded).

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

## Mapping from the baseline Python implementation

The names the first consumer's hooks and cron script use in the baseline at
`05831df` (`sanity_task_close_hook.py`, `close_fields_backfill.py`,
`close-vars.py`, the cron script, and the baseline command lines their
messages tell people to run), the binding that replaces each, and what differs. `cache` is a
`bd_cache_rs.Cache`. Timeouts that the baseline defaulted to `BD_TIMEOUT`
(30 s) are explicit arguments here.

| Baseline Python implementation | Called by | Binding and differences |
|---|---|---|
| `BdCache(root, runner=, clock=, db_path=)` | all hooks, cron | `bd_cache_rs.Cache(root, bd_binary=, cache_path=)`; `runner` (a `subprocess.run` stand-in) becomes `bd_binary`, a program path (tests pass a recording program); `clock` has no equivalent; `db_path` is `cache_path`; the default file is `.bd-cache/<db>.bd-cache.sqlite3`, not the first consumer's own directory, so set `cache_path` to keep the Python file; raises `CacheError` (`NO_DATABASE`, `WRONG_DATABASE`, `SCHEMA_VERSION`) as before for a repository that exists; a `repo_root` that is missing or not a directory is a `ValueError` (`kind` `argument`), with or without `project_dir` |
| `cache.get(bead, miss_timeout=)` | sanity hook, backfill, close-vars | `cache.get(bead, miss_timeout=)`; the reply is `{row, ...}`, the row dict of Python is `reply["row"]["bead"]`; `reply["row"]["blocked"]` is new |
| `cache.cached(bead)` | cron (`active_phases`) | `cache.get(bead, miss_timeout=0)`: a hit returns, a miss raises `BD_UNAVAILABLE` without a `bd` call (where Python returned `None`) |
| `cache.bd_rows(ids, timeout)` then `cache.store(row, force=True)` | sanity hook (`refresh`), cron, close-vars | one call, `cache.refresh(ids, timeout)`: returns a list of views (not a dict by id) already stored with force and with pending writes replayed, so Python's `view(row)` is not needed |
| `cache.store(row)` | cron, close-vars | covered by `cache.refresh(ids, timeout)`; the sync stores rows by itself |
| `cache.view(row)` | close-vars | covered by `cache.refresh(ids, timeout)` and `cache.get(bead)`, which replay pending writes |
| `cache.list(phase, status, label)` | backfill | `cache.list(phase, status=, label=)`; the reply is `{scope, last_ok_at, rows, outage}` with views, not a list of rows; an unknown phase is `SCOPE_UNKNOWN` (sync it first) |
| `cache.write(bead, op, args, idem_key=, actor=, precondition=)` | sanity hook, backfill | `cache.write(bead, op, args, precondition, idem_key, actor)` (keywords work); `args` and `precondition` may be dicts or JSON text; the reply is `{seq, duplicate}` (Python adds `ok` and `state`; read `cache.entry(seq)["state"]`); a duplicate with another op is `IDEM_CONFLICT` with `payload["seq"]`; an unknown `op` is a `ValueError` here where the baseline raised `BD_CACHE.BAD_OP` (ADR-PY-0002) |
| `cache.drain(deadline=, monotonic=)` and the detached baseline `drain` command (`drain_now`) | cron, sanity hook | `cache.drain(start_gate_s, hard_s)`: two seconds-from-now bounds (start gate and hard deadline, REQ-BDC-0402) instead of one monotonic deadline; the cron passes `budget - 5` and `budget - 2`. The hook drains in process (`cache.drain(...)` on a thread of its own or left to the cron) instead of spawning a process; the report has no `ok` or `backend`, and carries `deferred_in_flight` and `outage` |
| `cache.sync(phase, timeout=)` | cron, backfill | `cache.sync(phase, timeout)`; the report is `{scope, rebuilt, model_current, stored, blocked, deferred, outage}`; an unreachable backend returns the report with `outage` set instead of raising `SYNC_FAILED` (ADR-BDC-0005) |
| `cache.prune(phase)` | the baseline `prune` command | `cache.prune(phase)`; `{scope, beads_deleted, out_of_scope_swept}` |
| `cache.resolve(seq, note, actor)` | the baseline `resolve` command (named in the escalation text) | `cache.resolve(seq, note, actor)`; returns `None` (read `cache.entry(seq)` for the state) |
| `cache.status()` and `status['db']` | cron `doctor` | `cache.status()` and `cache.path` (a `pathlib.Path`; `status['db']`); the reply has `meta`, `journal`, `phases`, `escalated` (full entries), `outage` and `stale`; no `ok`, `cache_schema_version` or `model_bd_version` |
| `cache.path` | sanity hook (`.with_suffix(".drain.log")`) | `cache.path`, a `pathlib.Path` |
| `cache.unannounced_escalations()` | cron | `cache.unannounced_escalations()`; entries use the `entry` shape (`phase`, `args` as a dict, `state`) |
| `cache.mark_announced(seq)` | cron | `cache.mark_announced(seq)`; an unknown seq is `NOT_FOUND` |
| `cache.sync_alert(phase)` | cron | `cache.sync_alert(phase)` |
| `cache.set_sync_alert(phase, cause)` | cron | `cache.set_sync_alert(phase, cause)` |
| `cache.clear_sync_alert(phase)` | cron | `cache.set_sync_alert(phase, None)` |
| `cache.outage()` | cron | `cache.status()["outage"]`: `{since, last_error}`, or `None`; there is no `announced_at` (below) |
| `cache.mark_outage_announced()` | cron | `cache.set_sync_alert(phase, "backend_unavailable")` for each active phase whose alert was sent; `cache.sync_alert(phase)` says whether it was; the end of the outage resets it (`cache.set_sync_alert(phase, None)`) without a message (ADR-BDC-0005) |
| `cache.record_outage(error)`, `cache.clear_outage()` | cron | none: the cache records and clears the outage itself on every backend call that is unreachable or answers (ADR-BDC-0005) |
| `cache.has_open_work()` | cron (`should_stop`) | `cache.open_work()`: `{pending, escalated_unannounced}`; open work is `any(...)` of the values |
| `CacheError`, `error.code`, `error.detail`, `error.fix` | all | `bd_cache_rs.CacheError`, same attributes |
| `error.extra` | cron (`extra.get("cause")`) | `error.payload` (`cause` and `phase` of `SYNC_FAILED`, `unmet`, `seq`, `known`, `source`) |
| `error.as_json()` | cron | `error.body` |
| `bd_cache.UNREACHABLE` | cron | `bd_cache_rs.UNREACHABLE`, the same four codes (a frozenset) |
| `bd_cache.__file__` | sanity hook (`drain_now` spawns the baseline `drain` command) | none: the hook calls `cache.drain(start_gate_s, hard_s)` in process, so there is no script to spawn (see the `drain` row) |
| `cache.dependents(bead, type)` | none at `05831df` | not exposed (not one of the verbs of REQ-BDC-0910) |

The baseline CLI (`get|list|write|drain|sync|prune|resolve|status`) is the
drop-in of REQ-BDC-0806, deferred to the `bdc` binary; the table above covers
the same operations through the handle.

## Known limits

- One process holds one connection per cache file (NFR-BDC-0007): a hook that
  opens a handle while the cron's handle is open in the same process gets
  `STORE_IN_USE`; separate processes are fine.
- The two implementations must never drain the same journal at the same time
  (phase plan, the first consumer's adoption and cutover): stop the Python cron before the
  bindings' `drain` runs.
- Verbs on one handle run one at a time, whatever the thread (ADR-PY-0003).
