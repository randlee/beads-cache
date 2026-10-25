# bd-cache-py Requirements

Status: Approved (lead ruling, bc-lead, 2026-10-24). Crate-local requirements (domain code `PY`)
refining [`../requirements.md`](../requirements.md). Architecture:
[`architecture.md`](architecture.md). Created by sprint a-8.

Tests are under `crates/bd-cache-py/tests/` (pytest, run by
`.github/workflows/python.yml` against the built wheel). Every test drives the
real composition (SQLite store, flock drain lock, `BdCli`) with a recording
fake `bd` program (`tests/fake_bd_main.py`), which logs every argv.

## REQ-PY-0001: The module and its handle

**Status:** Approved (lead ruling, bc-lead, 2026-10-24); amended by lead rulings S38/S42 (bc-lead, 2026-10-24, fix-a-7-12): the `config` error kind is reserved (nothing produces it while `ConfigLoadError` has no further variants)

`bd_cache_rs.Cache(repo_root, *, bd_binary="bd", project_dir=None, scopes=None,
cache_path=None, call_timeout_s=None, miss_fill_timeout_ms=None,
max_attempts=None)` opens the cache of a repository over
`Cache<SqliteStore, FlockDrainLock, BdCli>`. Settings come from
`.beads/bd-cache.toml` through `load_repo_config`; each keyword argument that
is not `None` overrides the file's key (`call_timeout_s` and `miss_fill_timeout_ms` take int or float and `max_attempts` an int; a negative, zero or non-finite timeout, or fewer than 1 attempt, is `ValueError`; zero is refused as the config file loader refuses it, lead ruling S56). Opening starts no `bd` process. `$BEADS_DIR` is read once and made absolute against the process's working directory; that one directory locates the store and is the `BEADS_DIR` of every `bd` child (removed there when unset), so a relative value cannot name one directory for the store and another for `bd`, which runs in `project_dir` (PE-ARCH-001); a value that cannot be made absolute is `BD_CACHE.NO_DATABASE`, before the config file is read. A
second open of the same cache file in one process is `BD_CACHE.STORE_IN_USE`
until the first handle is closed (`close()`, or leaving a `with` block). A
repository without readable `.beads/metadata.json` is `BD_CACHE.NO_DATABASE`;
an invalid config file or scope is `ValueError` with attributes `kind` (`parse`, `scope`, `argument`, `config`) and `path` (`config` is reserved; not produced while ConfigLoadError has no further variants (S38/S42); `path` may be `None`); an unreadable config file is `OSError` with `kind` `read` and `path`; a `repo_root` or `project_dir` that is not valid UTF-8, or that does not resolve to a directory, is refused as a `ValueError` (never converted lossily); the directory `bd` runs in is canonicalized, so every spelling of one project shares one `bd` lock.

**Refines:** REQ-BDC-0910, REQ-BDC-0101, REQ-BDC-0104
**Acceptance evidence:** `test_open.py` (`test_a_repo_root_that_is_missing_*`, `test_a_repo_root_that_is_a_file_*`, `test_a_zero_timeout_is_refused_*`, `test_a_relative_beads_dir_names_one_directory_for_the_store_and_for_bd`, `test_a_beads_dir_that_cannot_be_made_absolute_is_no_database`); Rust `compose::tests` (`the_beads_dir_override_locates_the_store`, `an_unresolvable_beads_dir_is_no_database_before_the_config_is_read`); for the reserved `config` kind (S38/S42): the exhaustive `From<ConfigLoadError>` match in `crates/bd-cache-py/src/compose.rs` (compile-time) and the kind-text test `the_kind_texts_are_exactly_parse_scope_argument_config_and_read`.

## REQ-PY-0002: The verbs atm-bd uses

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

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

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Every reply is the `bd-cache-model` reply type serialized and converted to
`dict`, `list`, `str`, `int`, `float`, `bool` and `None` (timestamps are
RFC 3339 `str`). The shapes are the tables in `architecture.md`, Reply shapes;
they change only with a new wheel version (`bd_cache_rs.__version__`). Where
they differ from `bd_cache.py`'s JSON, the Mapping table records it. This does
not close REQ-BDC-0806 (the CLI drop-in).

**Refines:** REQ-BDC-0910
**Acceptance evidence:** the field-set assertions of `test_verbs.py`.

## REQ-PY-0004: One exception class carries the typed error

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

A typed error raises `bd_cache_rs.CacheError` (a subclass of `Exception`)
with the attributes `code` (`BD_CACHE.*`), `detail`, `fix`, `reason` (the
`/v0` refusal reason of `PRECONDITION`, else `None`), `payload` (the code's
extra fields: `unmet`, `seq`, `known`, `bead`, `field`, `phase`, `cause`,
`source`), `body` (the whole BD-CACHE JSON error object) and `exit_code` (3).
`PRECONDITION` carries `unmet` with `expected` and `actual` per key.
`bd_cache_rs.UNREACHABLE` is the frozenset of codes that mean `bd` did not
answer (the four `BdUnavailable`, `BdVersionFailed`, `BdSchemaFailed` and
`BdListFailed` variants; a Rust test checks the set against the model).
An error is always a `CacheError`; if building its attributes fails, that
failure is chained as its `__cause__`.

**Refines:** REQ-BDC-0802, REQ-BDC-0803, REQ-BDC-0910
**Acceptance evidence:** `test_errors.py`; Rust `error::tests`
(`unreachable_codes_are_exactly_the_four_backend_did_not_answer_variants`,
`fixed_keys_match_the_model_envelope_for_every_variant`).

## REQ-PY-0005: A bad argument is a Python error, not a cache error

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

A value that is not a valid bead id, actor, idempotency key, phase name, `op`
(not one of `bd_cache_model::OPS`: `close`, `update`, `dep_add`) or timeout
(negative, NaN, infinite) raises `ValueError` naming the argument; a list
expected where a `str` was given raises `TypeError`. A verb on a closed handle
raises `bd_cache_rs.ClosedError`, a `RuntimeError` subclass whose message says
to open a new `Cache`. None of them touches the store. `list`'s `status` must be
one of bd's seven statuses (`sc_bd_cli::argv::ALL_STATUSES`: `open`,
`in_progress`, `blocked`, `deferred`, `closed`, `pinned`, `hooked`); any other
text is a `ValueError` naming them, so a typo cannot read as "no rows".

**Refines:** REQ-BDC-0910
**Acceptance evidence:** `test_arguments_that_are_not_valid_values_raise_value_error`,
`test_an_unknown_op_is_a_value_error_and_bad_args_are_typed`,
`test_a_status_filter_must_be_a_bd_status`,
`test_close_ends_the_handle_and_is_idempotent`.

## REQ-PY-0006: Threading

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

The handle keeps the `Cache` behind a mutex (ADR-CORE-0001: a `Cache` is
`!Sync`, so a shared owner guards it). Any Python thread may call a verb,
close the handle or drop it; verbs run one at a time, and no cross-thread use
panics. Each verb, `close()`, `__exit__` and `closed` release the GIL or take no
lock: `closed` is an atomic flag, true as soon as `close()` begins; `close()`
waits for the in-flight verb with the GIL released and drops the connection
there. A verb that panics is caught at the panic: the connection is dropped and
the handle closed before the call returns (so `closed` is already true and the
cache file can be opened again), and that call and every later one raise
`ClosedError`; no `PanicException` reaches Python. Other Python
threads continue while a verb waits on `bd` or SQLite.

**Refines:** REQ-BDC-0910, ADR-CORE-0001
**Acceptance evidence:** `test_a_python_thread_runs_while_a_verb_waits_on_bd_and_releases_it`,
`test_close_while_a_verb_is_in_flight`,
`test_handles_on_different_cache_files_never_overlap_their_bd_calls`,
`test_spellings_of_one_project_directory_share_one_lock`, Rust `slot::tests`,
`test_any_thread_can_use_the_handle_and_none_panics`,
`test_verbs_from_several_threads_run_one_at_a_time_without_error`,
`test_a_handle_can_be_closed_and_dropped_on_another_thread`,
`test_a_handle_dropped_on_another_thread_does_not_panic`.

## REQ-PY-0007: Ready through the binding

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

After a dev close journaled through `write`, `ready` for its sanity bead is
true with the backend unreachable and no `bd` call (the `atm task open` path).
`ready(phase, fresh_within=s)` against a `bd` that answers late returns the
cached, journal-adjusted answer with `read_fallback` set, within `s` seconds.

**Refines:** REQ-BDC-0911, REQ-BDC-0912
**Acceptance evidence:** `test_ready.py`.

## REQ-PY-0008: Announcement state for an external sender

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`unannounced_escalations`, `mark_announced`, `sync_alert`, `set_sync_alert` and
`open_work` let atm-bd's own cron announce through its own sender: an escalated
entry is announced once (marked, not re-sent), an undelivered one stays
unannounced, and a sync-failure cause is announced once per cause.

**Refines:** REQ-BDC-0604, REQ-BDC-0704, REQ-BDC-0705
**Acceptance evidence:** `test_announce.py`.

## REQ-PY-0009: The mapping covers every call atm-bd makes

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`architecture.md`, Mapping, lists every `bd_cache.py` name that the atm-bd
hooks and `bd_cache_cron.py` call at atm-core `05831df`, with the binding that
replaces it or the reason none is needed. Every binding named in the table
exists on the handle.

**Refines:** REQ-BDC-0910
**Acceptance evidence:** `test_mapping_doc.py`.

## REQ-PY-0010: Packaging

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

The crate builds with maturin into one abi3 wheel per platform (`abi3-py39`,
distribution `bd-cache-rs`, module `bd_cache_rs`). `.github/workflows/python.yml`
builds it on macOS arm64 and Linux x86_64, runs the pytest suite against the
installed wheel on Python 3.9, 3.11 and 3.13, and runs the crate's clippy and
tests on macOS, Linux and Windows (lead ruling R4: the Rust crate must compile
and pass its tests on Windows like every other crate; the wheel and pytest stay
macOS and Linux because the atm-bd hooks run on POSIX only). Nothing in phase a publishes the wheel; the release step uses the
`pypi` and `testpypi` environments later.

**Refines:** REQ-BDC-0910
**Acceptance evidence:** the `python.yml` run cited in the sprint doc.

## REQ-PY-0011: The `cache.beads` and `cache.manage` namespaces

**Status:** Approved (lead ruling, bc-lead, 2026-10-25); created by sprint b-4

Since wheel 0.2 the handle has two namespaces, `cache.beads` (surface (a),
contract section 5) and `cache.manage` (surface (b), section 6), one method per
command of `bd-cache-api`'s `Command` except `cache tick`. A method's name is the
command path joined with `_`, without the `cache_` prefix of surface (b):
`issues_get`, `issues_list`, `ready`, `dependencies_list`, `issues_update`,
`issues_close`, `dependencies_add`, `context` on `cache.beads`; `status`, `health`,
`scopes`, `refresh_scopes`, `refresh_issues`, `flush`, `evict`, `journal_get`,
`journal_list`, `journal_resolve`, `journal_announce` on `cache.manage`. A method takes
the command's request as keyword arguments only, builds it through
`bd_cache_api::decode` and nothing else, runs `bd_cache_api::execute` with the GIL
released and returns exactly the CLI's `data` as builtin Python objects. The binding
validates nothing: an unknown member, a missing `idempotency_key`, a `/v0` name the
cache does not serve and every other refusal has the decoder's code. Timeouts are
milliseconds (`miss_timeout_ms`, `timeout_ms`, `start_gate_ms`, `deadline_ms`). Every
method, old and new, accepts a bare (`bk`) or label (`phase-bk`) scope id. The
namespaces are resolved by `Cache.__getattr__`, so `dir(Cache)` keeps exactly the 0.1
names. A Python-level type mistake (a positional argument, a value with no JSON form)
is `TypeError`; a non-finite float, an integer beyond 64 bits, or a value nested deeper than 64 levels (which includes a container that contains itself) is `ValueError`, never a crash (`test_api_robustness.py`, each case in a subprocess).

**Refines:** REQ-BDC-0807, REQ-BDC-0808, REQ-BDC-0809
**Acceptance evidence:** `test_api_fixtures.py` (each shared fixture of
`crates/bd-cache-api/tests/fixtures`, replayed through the namespaces: same `data` or
same `CacheError.code`); `test_additive.py` (`test_miss_timeout_ms_is_milliseconds`,
`test_both_scope_spellings_and_the_0_1_list_return_the_same_rows`,
`test_a_python_type_mistake_is_a_type_error_not_a_cache_error`,
`test_cache_tick_is_not_exposed`); Rust `api::tests` (method names follow the command
paths and cover every command).

## REQ-PY-0012: `CacheError` carries the contract's error fields

**Status:** Approved (lead ruling, bc-lead, 2026-10-25); created by sprint b-4

Every `CacheError`, from a 0.1 verb or a 0.2 method, has the attributes `kind`
(`invalid_input`, `not_found`, `unavailable`, `conflict` or `internal`), `message`,
`details` (a dict), `suggested_action` (`str` or `None`) and `retryable` (`bool`), taken
from `ApiError` and the error table of contract section 4. The 0.1 attributes (`code`,
`detail`, `fix`, `reason`, `payload`, `body`, `exit_code`) are unchanged for a 0.1
error; for a 0.2 error `detail` is the message, `fix` the suggested action (`""` when
there is none), `reason` is `None`, `payload` is `{}` and `body` is the error object.

**Refines:** REQ-BDC-0803, REQ-BDC-0812
**Acceptance evidence:** `test_additive.py`
(`test_cache_error_carries_the_contract_fields_from_the_error_table`,
`test_a_0_1_error_gains_the_new_fields_and_keeps_the_old_ones`,
`test_a_zero_miss_timeout_is_not_cached_and_retryable_while_0_1_keeps_its_behaviour`).

## REQ-PY-0013: 0.2 is additive; `CONTRACT_VERSION`

**Status:** Approved (lead ruling, bc-lead, 2026-10-25); created by sprint b-4

Every 0.1 verb, its arguments, its units (seconds) and its dict key sets are unchanged,
and 0.2 issues no `DeprecationWarning` (the suite runs under `-W
error::DeprecationWarning`). `bd_cache_rs.CONTRACT_VERSION` is `bd_cache_api::CONTRACT_VERSION`,
the contract version `context` and `cache status` report; wheel 0.2.x implements
contract 1.0.x.

**Refines:** REQ-BDC-0811, REQ-BDC-0812
**Acceptance evidence:** the unedited 0.1 test files (`test_verbs.py`, `test_key_sets.py`,
`test_mapping_doc.py`, ...); `test_additive.py`
(`test_0_1_dicts_keep_their_key_sets_next_to_the_0_2_calls`,
`test_no_verb_old_or_new_issues_a_deprecation_warning`,
`test_contract_version_is_the_one_context_reports`).

## REQ-PY-0014: One version for the workspace and the wheel

**Status:** Approved (lead ruling S61, bc-lead, 2026-10-25); created by sprint b-4

`[workspace.package].version`, every inter-crate path-dependency `version` in the root
`Cargo.toml`, `Cargo.lock` and `crates/bd-cache-py/pyproject.toml` `[project].version`
are 0.2.0 and change together; the kit's `verify-version-lockstep` fails if the
`pyproject.toml` version differs from the workspace's.

**Refines:** REQ-BDC-0812, REQ-PY-0010
**Acceptance evidence:** `release_artifacts.py verify-version` and
`verify-version-lockstep` (sprint b-4 record); `test_additive.py` asserts
`__version__ == "0.2.0"`.
