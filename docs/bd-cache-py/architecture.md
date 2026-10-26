# bd-cache-py Architecture

Status: Approved (lead ruling, bc-lead, 2026-10-24). Crate-local decisions (domain code `PY`).
Repo-level context: [`../architecture.md`](../architecture.md). Requirements:
[`requirements.md`](requirements.md). Created by sprint a-8.

`bd-cache-py` is the Python module `bd_cache_rs` (maturin / PyO3). It exposes
the cache in process, so a consumer can call it from its own scheduler
without `sc-runtime`. It is a thin layer: it holds no cache rule, it opens the
concrete composition and converts arguments, replies and errors.

It depends on `bd-cache`, `bd-cache-api` (0.2), `bd-cache-model`, `bd-cache-store`,
`bd-cache-sync`, `sc-bd-cli` and `sc-beads` (the Boundaries table of
[`../architecture.md`](../architecture.md)) and, third party, on `pyo3`, `serde` and
`serde_json` (to convert replies) and `tokio` (the current-thread runtime of `tick()` and the
sync methods, ADR-PY-0006), all through `[workspace.dependencies]`; `serde` and `serde_json` are
allowed third-party dependencies of this crate (lead ruling R3). `dunce` (lead ruling R12) was
dropped by sprint b-8 (ADR-PY-0006, QA-1 amendment): the process-wide `ProjectLocks` set is
`bd-cache-sync`'s, keyed by `sc-bd-cli`'s canonical path.

| Module | Contents |
|---|---|
| `lib` | the `#[pymodule]` `bd_cache_rs._native`: the `Cache` class, `CacheError`, `ClosedError`, `UNREACHABLE`, `CODES`, `CONTRACT_VERSION`, `__version__` |
| `compose` | pure Rust: `open` composes `Cache<SqliteStore, FlockDrainLock, BdCli>` through `bd_cache_sync::compose_tuned` (the one composition root, with `bd-cache-sync`'s process-wide `ProjectLocks`), from `load_repo_config` and the validated keyword overrides |
| `slot` | pure Rust: the closable, mutex-guarded owner of the `Cache` (close, panic handling; poisoning is defensive) |
| `handle` | `PyCache`, the Python `Cache`: argument parsing, the sixteen 0.1 verbs, GIL release |
| `api` | the 0.2 namespaces `Beads` / `Manage` over `bd-cache-api` (ADR-PY-0005) |
| `phase_b` | the phase-B methods of the handle (`tick()`, `schedule_*`, `sync_use_*`, `ingest_scope`, `scope_announce`, `scopes_*`), through `bd-cache-api` or `bd_cache_sync::execute` (ADR-PY-0006) |
| `convert` | `serde_json::Value` to Python objects, JSON text for `write`, seconds to `Duration` |
| `error` | `CacheError`, `ClosedError` and the `BD_CACHE.*` error to exception mapping |
| `python/bd_cache_rs/` | `__init__.py` (re-exports `_native`), `run.py` (`python -m bd_cache_rs.run`, the post-sync hook runner), `sync.py` (`bd_cache_rs.sync.main`, the Python sync row's harness), `_common.py` |

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
(`sc-bd-cli`) still take the set as an argument. *(Superseded by the QA-1 amendment of
ADR-PY-0006 below: the in-run flag is a second static, and the `ProjectLocks` set is
`bd-cache-sync`'s, with `dunce` dropped.)* The deadlines of `sync` and
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
difference. *(The Mapping table is removed by ADR-PY-0006 with its test; the shapes are
[Reply shapes](#reply-shapes) below and the 0.1 → 0.2 differences are contract §10.)* A typed `CacheError` raises `bd_cache_rs.CacheError`, one class
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
`None`. *(Amended below, sprint b-9: the `config` kind is removed.)* An unreadable config
file is an `OSError` of kind `read`.

Consequences: `error.extra` of the baseline Python implementation is
`error.payload`; its
`as_json()` is `error.body`.

### Amendment: the open-error kinds (sprint b-9, fix round 1)

**Status:** Approved (lead ruling, sprint b-9 fix round 1: B9Q1-040, B9Q1-041)

The reserved `config` kind is removed with its variant (`ConfigKind::Other`): the
`ValueError` kinds are `parse`, `scope` and `argument`, and nothing produced `config`. It
comes back only if `ConfigLoadError` becomes non-exhaustive. The `OSError` of an unreadable
config file (kind `read`) is built from the typed read error, so its `errno` and `filename`
are set; the cause of an open failure is kept as the error's source, never flattened into
text alone.

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
  distribution `bd-cache-rs` (the `_rs` suffix names the Rust binding, so the module never
  takes a name another package of the same purpose may use). The version is the
  workspace's; `bd_cache_rs.__version__` versions the reply shapes.
- Wheels: abi3 (`abi3-py39`), one wheel per platform serves CPython 3.9 and
  later (the oldest CPython the binding supports; it needs nothing newer).
  `pyo3` has no `extension-module` feature here: maturin sets
  `PYO3_BUILD_EXTENSION_MODULE`, so `cargo clippy` and `cargo test` link
  against a Python and work for this crate too.
- CI: `ci.yml` (a-1) excludes `bd-cache-py` from the workspace gates because
  it needs a Python to link; `.github/workflows/python.yml` is its gate:
  clippy and `cargo test` for the crate on macOS, Linux and Windows (lead ruling
  R4), a wheel on macOS arm64 and Linux x86_64, and the pytest suite against
  the installed wheel on Python 3.9, 3.11 and 3.13. The wheel and pytest stay
  off Windows: the pytest harness runs the fake `bd` through a POSIX shell wrapper,
  and the crate's `cargo test` covers Windows.
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
  test read a vendored call log recorded from another implementation's callers, made by a
  companion extraction script (both under `tests/`, the files whose paths the lint flags). *(Superseded by ADR-PY-0006: sprint b-9 deleted the
  mapping test, the call log and the script; the neutral 0.1 call corpus, `tests/corpus/` with
  `tests/test_corpus.py`, generated in this repository against the fake bd, replaces them.)*
  It runs under the interpreter that
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
*Amended by ADR-PY-0006:* only the 0.1 verbs keep this rule (ADR-BDC-0013's 0.1 compatibility);
the 0.2 namespaces and the phase-B methods take a scope name as written.

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
  the code) and on `tokio`, the workspace entry, whose `rt` feature builds the current-thread
  runtime below; b-8 names it in the crate's Crate Layout row. It needs no `sc-cron` edge: the
  schedule and switch calls go through `bd-cache-sync`. `Cache.tick()`, `schedule_*` and `sync_use_*` call `bd-cache-sync`; the store-only
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
  user code in process. Like the 0.1 `sync` verb, they hold the handle's mutex (and its one
  connection) for the whole call, so another thread on the same `Cache` waits until the run's
  deadline at most, and the call cannot be interrupted from Python. *(Superseded by the QA-1
amendment below: a `KeyboardInterrupt` during the call stops the run and is raised when it
returns.)*
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

### Amendment: the in-run flag and the one composition root (sprint b-8, QA-1)

**Status:** Approved (lead ruling ARCH-002 of the b-8 QA-1 fix round, sprint b-8, 2026-10-26).
Amends ADR-PY-0001 where it calls the `ProjectLocks` `OnceLock` the module's only static.

- **A second process-wide static: the in-run flag.** `phase_b::IN_RUN` (an `AtomicBool`) is set
  by `bd_cache_rs.run` and `bd_cache_rs.sync` (`_native._set_in_run`, through
  `_common.mark_inside_run`) just before they call user code. While it is set, `drain`,
  `manage.flush`, `tick`, `schedule_*` and `sync_use_*` raise `CacheError` (`PRECONDITION`,
  `details.reason: "inside_run"`): the run's lease and schedule are its runner's, and a verb that
  takes the lease or changes a row would race it or fence it. It is process state, not a
  handle's, because user code opens handles of its own inside the run, and the runner is a
  separate process whose whole life is that one call; a per-handle flag could not mark them.
  Which methods are refused is one field of the method table (`Spec::refuse_in_run`), checked
  against `Command::is_front_end` by a unit test.
- **The `ProjectLocks` set is bd-cache-sync's.** The handle opens through
  `bd_cache_sync::compose_tuned` (the one composition root, with the handle's
  `project_dir`, `call_timeout_s` and `miss_fill_timeout_ms` as its `ComposeTuning`), so the
  handles and every sync run of the process share bd-cache-sync's one process-wide
  `ProjectLocks`; this crate keeps no lock set and no schedule-store error text of its own
  (the `ScheduleTable` keeps the table's error with its source). `sc-bd-cli` keys the locks by
  the canonical path, so the `project_dir` given is checked (UTF-8, an existing directory) and
  used as written; `dunce` is dropped.
- **The front-end verbs run through `bd_cache_sync::execute`** (`FrontEnd::Python`), so this
  module and `bdc` give the same codes and details; only the suggested action is worded for
  Python. While such a call runs with the GIL released, the calling thread checks for a Python
  signal every 100 ms: a `KeyboardInterrupt` stops the run (a tick between steps, killing its
  hook; `sync_use_python` killing its script and restoring) and is raised when the call returns.

## Reply shapes

Versioned with the wheel (`bd_cache_rs.__version__`, now 0.2.0); these 0.1 shapes are
unchanged since wheel 0.1.0 (REQ-BDC-0812), and `tests/corpus/corpus_0_1.json` records each
variant. `ts` is an RFC 3339 UTC `str` with second
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

## The binding's API

`bd_cache_rs.Cache(repo_root, *, bd_binary="bd", project_dir=None, scopes=None,
cache_path=None, call_timeout_s=None, miss_fill_timeout_ms=None, max_attempts=None)` opens the
cache of the repository at `repo_root` (ADR-PY-0001): `.beads/bd-cache.toml` is read, and the
keywords override its keys; `bd` runs as `bd_binary` in `project_dir` (default `repo_root`).
The default file is `.bd-cache/<dolt_database>.bd-cache.sqlite3`; a file kept elsewhere is
opened by passing `cache_path` (ADR-BDC-0015; there is no search for it). Opening starts no
`bd`. A missing or wrong repository raises `CacheError` (`NO_DATABASE`, `WRONG_DATABASE`,
`SCHEMA_VERSION`); a `repo_root` that is not a directory, or an invalid keyword, is a
`ValueError` (ADR-PY-0002). The handle is a context manager; `close()` is idempotent and
`closed` says whether it ran. `path` is the cache file (`pathlib.Path`).

The 0.1 verbs (every argument and reply unchanged in 0.2, REQ-BDC-0812; timeouts in seconds):

| Verb | What it does |
|---|---|
| `get(bead, miss_timeout=None)` | the bead from the cache; a miss asks bd within `miss_timeout` (`0` asks nothing: `BD_UNAVAILABLE` at once) |
| `refresh(ids, timeout)` | one `bd list --id` for `ids`, stored with force, pending writes replayed; a list of views |
| `write(bead, op, args, precondition, idem_key, actor)` | journals a write (`close`, `update`, `dep_add`); `args` and `precondition` are dicts or JSON text; `{seq, duplicate}` |
| `list(phase, status=None, label=None)` | every row of the scope, filtered; `SCOPE_UNKNOWN` for a scope with no completed sync |
| `ready(phase, fresh_within=None)` | the ready beads of the scope: bd's answer when it comes within `fresh_within`, else the cached base |
| `sync(phase, timeout)` | syncs the scope from bd; an unreachable bd is the report's `outage` (ADR-BDC-0005) |
| `drain(start_gate_s, hard_s)` | applies journaled writes to bd under a start gate and a hard deadline |
| `entry(seq)`, `resolve(seq, note, actor)` | one journal entry; resolves an escalated one |
| `unannounced_escalations()`, `mark_announced(seq)` | the escalated entries not yet announced; marks one announced |
| `prune(phase)` | drops the scope's rows and sync state (`PRUNE_BLOCKED` while it has open writes) |
| `status()`, `open_work()` | the store's meta, journal counts, scopes, escalations, outage and stale rows; the pending and unannounced counts |
| `sync_alert(phase)`, `set_sync_alert(phase, cause)` | the alert stored for a scope; sets or clears it |

`phase` takes the 0.1 scope rule (ADR-BDC-0013): `x` and the legacy-prefixed `x` both name the
label scope of the legacy-prefixed label. The 0.2 namespaces `cache.beads` and `cache.manage`
and the phase-B methods take scope names as written; contract §10 maps each 0.1 verb to its 0.2
counterpart and lists every 0.1 → 0.2 difference.

Rules a caller follows:

- **No runner injection.** The binding runs `bd` itself, with deadlines and strict row
  parsing; it takes no `subprocess.run` stand-in. A caller that needs to observe, record or
  wrap `bd` (a test, a wrapper that sets an environment) passes a program path as `bd_binary`
  that runs `bd` or stands in for it (the test suite's recording fake bd does exactly that).
- **A detached `bdc cache flush` must log its output.** A hook that does not want to wait for a
  drain can start `bdc cache flush` as a detached process instead of calling `drain` in
  process; its envelope (on stdout) and its log (on stderr) then reach nobody, so it must
  redirect both to a log file of the caller's, or a failed flush and its escalations go
  unseen. Calling `drain(start_gate_s, hard_s)` in process returns the report directly.
- **One handle per process.** One process holds one connection per cache file: a second
  `Cache` on the same file in the same process is `STORE_IN_USE` until the first is closed
  (ADR-PY-0003); separate processes are fine. A hook and a sync script run in their own
  processes and open their own handle (`bd_cache_rs.run`, `bd_cache_rs.sync`).
- **The binding's own API.** The replies are this crate's shapes (below), versioned with the
  wheel; they do not follow any other implementation's.

## Known limits

- One process holds one connection per cache file (NFR-BDC-0007): a hook that
  opens a handle while the cron's handle is open in the same process gets
  `STORE_IN_USE`; separate processes are fine.
- Two writers must never drain the same journal at the same time: run one sync (the cache
  file's sync rows, or `tick()`), not a second drainer of the file beside it.
- Verbs on one handle run one at a time, whatever the thread (ADR-PY-0003).
