# bd-cache-py Requirements

Status: Draft (sprint a-8, awaiting lead ruling). Crate-local requirements (domain code `PY`)
refining [`../requirements.md`](../requirements.md). Architecture:
[`architecture.md`](architecture.md). Created by sprint a-8.

Tests are under `crates/bd-cache-py/tests/` (pytest, run by
`.github/workflows/python.yml` against the built wheel). Every test drives the
real composition (SQLite store, flock drain lock, `BdCli`) with a recording
fake `bd` program (`tests/fake_bd_main.py`), which logs every argv.

## REQ-PY-0001: The module and its handle

**Status:** Draft

`bd_cache_rs.Cache(repo_root, *, bd_binary="bd", project_dir=None, scopes=None,
cache_path=None, call_timeout_s=None, miss_fill_timeout_ms=None,
max_attempts=None)` opens the cache of a repository over
`Cache<SqliteStore, FlockDrainLock, BdCli>`. Settings come from
`.beads/bd-cache.toml` through `load_repo_config`; each keyword argument that
is not `None` overrides the file's key. Opening starts no `bd` process. A
second open of the same cache file in one process is `BD_CACHE.STORE_FAILED`
until the first handle is closed (`close()`, or leaving a `with` block). A
repository without readable `.beads/metadata.json` is `BD_CACHE.NO_DATABASE`;
an invalid config file or scope is `ValueError`.

**Refines:** REQ-BDC-0910, REQ-BDC-0101, REQ-BDC-0104
**Acceptance evidence:** `test_open.py`.

## REQ-PY-0002: The verbs atm-bd uses

**Status:** Draft

The handle has exactly these verbs: `get(bead, miss_timeout=None)`,
`refresh(ids, timeout)`, `write(bead, op, args, precondition, idem_key,
actor)`, `list(phase, status=None, label=None)`, `ready(phase,
fresh_within=None)`, `sync(phase, timeout)`, `drain(start_gate_s, hard_s)`,
`entry(seq)`, `resolve(seq, note, actor)`, `prune(phase)`, `status()`,
`unannounced_escalations()`, `mark_announced(seq)`, `sync_alert(phase)`,
`set_sync_alert(phase, cause)` and `open_work()`, plus the `path` and `closed`
properties and `close()`. Times are seconds as `float`. `phase` is a phase
name with or without the `phase-` prefix; `args` and `precondition` are JSON
text or any object `json.dumps` accepts (`precondition` may be `None`). A verb
adds no cache rule: it calls the `Cache` method of the same name.

**Refines:** REQ-BDC-0910, REQ-BDC-0907, REQ-BDC-0908, REQ-BDC-0909
**Acceptance evidence:** `test_verbs.py` (one test per verb, every reply field
checked, the argv `bd` saw), `test_reads.py`.

## REQ-PY-0003: Replies are builtin Python objects with shapes versioned with the wheel

**Status:** Draft

Every reply is the `bd-cache-model` reply type serialized and converted to
`dict`, `list`, `str`, `int`, `float`, `bool` and `None` (timestamps are
RFC 3339 `str`). The shapes are the tables in `architecture.md`, Reply shapes;
they change only with a new wheel version (`bd_cache_rs.__version__`). Where
they differ from `bd_cache.py`'s JSON, the Mapping table records it. This does
not close REQ-BDC-0806 (the CLI drop-in).

**Refines:** REQ-BDC-0910
**Acceptance evidence:** the field-set assertions of `test_verbs.py`.

## REQ-PY-0004: One exception class carries the typed error

**Status:** Draft

A typed error raises `bd_cache_rs.CacheError` (a subclass of `Exception`)
with the attributes `code` (`BD_CACHE.*`), `detail`, `fix`, `reason` (the
`/v0` refusal reason of `PRECONDITION`, else `None`), `payload` (the code's
extra fields: `unmet`, `seq`, `known`, `bead`, `field`, `phase`, `cause`,
`source`), `body` (the whole BD-CACHE JSON error object) and `exit_code` (3).
`PRECONDITION` carries `unmet` with `expected` and `actual` per key.
`bd_cache_rs.UNREACHABLE` is the frozenset of codes that mean `bd` did not
answer.

**Refines:** REQ-BDC-0802, REQ-BDC-0803, REQ-BDC-0910
**Acceptance evidence:** `test_errors.py`.

## REQ-PY-0005: A bad argument is a Python error, not a cache error

**Status:** Draft

A value that is not a valid bead id, actor, idempotency key, phase name or
timeout (negative, NaN, infinite) raises `ValueError` naming the argument; a
list expected where a `str` was given raises `TypeError`; a verb on a closed
handle raises `RuntimeError`. None of them touches the store.

**Refines:** REQ-BDC-0910
**Acceptance evidence:** `test_arguments_that_are_not_valid_values_raise_value_error`,
`test_close_ends_the_handle_and_is_idempotent`.

## REQ-PY-0006: Threading

**Status:** Draft

The handle is `unsendable`: it owns one store connection and is used only from
the thread that created it (another thread gets PyO3's `PanicException`, which
names the class as unsendable). Each verb releases the GIL while it runs, so
other Python threads continue while a verb waits on `bd` or SQLite.

**Refines:** REQ-BDC-0910, ADR-CORE-0001
**Acceptance evidence:** `test_other_python_threads_run_while_a_verb_waits_on_bd`,
`test_the_handle_is_unsendable`, `test_each_thread_can_own_a_cache_over_its_own_file`.

## REQ-PY-0007: Ready through the binding

**Status:** Draft

After a dev close journaled through `write`, `ready` for its sanity bead is
true with the backend unreachable and no `bd` call (the `atm task open` path).
`ready(phase, fresh_within=s)` against a `bd` that answers late returns the
cached, journal-adjusted answer with `read_fallback` set, within `s` seconds.

**Refines:** REQ-BDC-0911, REQ-BDC-0912
**Acceptance evidence:** `test_ready.py`.

## REQ-PY-0008: Announcement state for an external sender

**Status:** Draft

`unannounced_escalations`, `mark_announced`, `sync_alert`, `set_sync_alert` and
`open_work` let atm-bd's own cron announce through its own sender: an escalated
entry is announced once (marked, not re-sent), an undelivered one stays
unannounced, and a sync-failure cause is announced once per cause.

**Refines:** REQ-BDC-0910, REQ-BDC-0601
**Acceptance evidence:** `test_announce.py`.

## REQ-PY-0009: The mapping covers every call atm-bd makes

**Status:** Draft

`architecture.md`, Mapping, lists every `bd_cache.py` name that the atm-bd
hooks and `bd_cache_cron.py` call at atm-core `05831df`, with the binding that
replaces it or the reason none is needed. Every binding named in the table
exists on the handle.

**Refines:** REQ-BDC-0910
**Acceptance evidence:** `test_mapping_doc.py`.

## REQ-PY-0010: Packaging

**Status:** Draft

The crate builds with maturin into one abi3 wheel per platform (`abi3-py39`,
distribution `bd-cache-rs`, module `bd_cache_rs`). `.github/workflows/python.yml`
builds it on macOS arm64 and Linux x86_64, runs the pytest suite against the
installed wheel on Python 3.9, 3.11 and 3.13, and runs the crate's clippy and
tests. Nothing in phase a publishes the wheel; the release step uses the
`pypi` and `testpypi` environments later.

**Refines:** REQ-BDC-0910
**Acceptance evidence:** the `python.yml` run cited in the sprint doc.
