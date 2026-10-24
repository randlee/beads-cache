---
id: a-8
phase: a
status: in-review
closure_type: integration
target_boundary: bd-cache-py and its composition
vertical_rationale: "The bd-cache-py boundary does not exist yet: this sprint creates it and composes the real store, sc-bd-cli and core behind it, so atm-bd can call the cache in process without sc-runtime."
track: python
wave: 3
branch: sprint/a-8-python-bindings
worktree: sprint/a-8-python-bindings
recommended_agent: rust-developer
recommended_model: sonnet
must_follow: [a-2, a-4, a-5, a-6]
parallel_safe: [a-7]
requirements:
  owns: [REQ-BDC-0910, REQ-PY-0001, REQ-PY-0002, REQ-PY-0003, REQ-PY-0004, REQ-PY-0005, REQ-PY-0006, REQ-PY-0007, REQ-PY-0008, REQ-PY-0009, REQ-PY-0010]
  constrained_by: [REQ-BDC-0001, REQ-BDC-0002, REQ-BDC-0101, REQ-BDC-0104, REQ-BDC-0402, REQ-BDC-0601, REQ-BDC-0802, REQ-BDC-0803, REQ-BDC-0903, REQ-BDC-0907, REQ-BDC-0908, REQ-BDC-0909, REQ-BDC-0911, REQ-BDC-0912, NFR-BDC-0001, NFR-BDC-0004, NFR-BDC-0005, NFR-BDC-0007]
adrs: [ADR-BDC-0001, ADR-BDC-0002, ADR-BDC-0003, ADR-BDC-0005, ADR-BDC-0007, ADR-MODEL-0002, ADR-CORE-0001, ADR-STORE-0003, ADR-PY-0001, ADR-PY-0002, ADR-PY-0003, ADR-PY-0004]
owned_paths:
  - "crates/bd-cache-py/**"
  - ".github/workflows/python.yml"
  - "docs/bd-cache-py/**"
---

# Sprint a-8 — Python bindings (maturin / PyO3)

## Outcome and closure

A `bd-cache-py` crate, built with maturin and PyO3, that exposes the cache
in process to Python. atm-bd imports `bd_cache.py` today; this gives it a
module it can switch to without waiting for sc-runtime, because atm-bd
already has its own scheduler (ATM cron) and calls the cache synchronously.
Closes when a Python test suite drives every exposed verb against the real
composition and the wheel builds in CI.

## must_follow rationale

Composes `BdCli` (a-2), `SqliteStore` (a-4) and the write and read paths of
`Cache` (a-5, a-6). It does not need `bd-cache-sync` or `sc-cron` (a-3, a-7):
atm-bd's own `bd_cache_cron.py` keeps scheduling and calls `drain` and
`sync` through these bindings.

## Deliverables

1. `bd_cache_rs` Python module (name final: `bd_cache_rs`) over the concrete
   composition `Cache<SqliteStore, FlockDrainLock, BdCli>`, opened from the
   repository through a-4's `load_repo_config` (or explicit arguments),
   with the verbs atm-bd uses (REQ-BDC-0910): `get(bead, miss_timeout=None)`,
   `refresh(ids, timeout)`, `write(bead, op, args, precondition, idem_key,
   actor)`, `list(phase, status=None, label=None)`, `ready(phase, fresh_within=None)`,
   `sync(phase, timeout)`, `drain(start_gate_s, hard_s)`, `entry(seq)`,
   `resolve(seq, note, actor)`, `prune(phase)`, `status()`, and the
   announcement state atm-bd's `bd_cache_cron.py` needs to keep announcing
   through its own sender: `unannounced_escalations()`, `mark_announced(seq)`,
   `sync_alert(phase)`, `set_sync_alert(phase, cause)`, `open_work()`.
2. Replies are the Rust reply types converted to Python dicts. Their shapes
   are documented in `docs/bd-cache-py/architecture.md` and versioned with
   the wheel; where they differ from `bd_cache.py`'s JSON, the mapping table
   records it. This does not close REQ-BDC-0806 (the CLI drop-in); `CacheError` becomes one Python
   exception class carrying `code`, `detail`, `fix`, `reason` and the payload
   (REQ-BDC-0802, 0803).
3. Threading per ADR-CORE-0001: the handle owns one connection; calls
   release the GIL while they wait on `bd`. *Amended (lead ruling,
   2026-10-24, PR #11):* the handle is `Mutex`-guarded, not `unsendable`
   (ADR-CORE-0001's own-mutex option, ADR-PY-0003), so a handle used or
   dropped on another thread never panics in PyO3.
4. Packaging: maturin `pyproject.toml` in the crate, abi3 wheels for the
   Python versions atm-bd runs, built in `.github/workflows/python.yml`
   (macOS arm64 and Linux x86_64 at least). Publishing to PyPI is a later
   release step (`pypi` / `testpypi` environments).
5. `docs/bd-cache-py/requirements.md` and `architecture.md`, including the
   mapping from each `bd_cache.py` function atm-bd calls today to its
   binding.

## Acceptance criteria

| Criterion | Root |
|---|---|
| A pytest suite drives every verb in deliverable 1 against the real composition with a recording `bd` invoker, and checks each reply's fields (REQ-BDC-0910) | req:REQ-BDC-0910 |
| A typed error raises the exception with `code`, `fix` and payload; `PRECONDITION` carries `unmet` | req:REQ-BDC-0910 |
| `refresh` returns fresh rows with the overlay; `entry(seq)` returns an entry in any state; `sync` then `list` of an unconfigured phase works | req:REQ-BDC-0910 |
| The mapping table in `docs/bd-cache-py/architecture.md` covers every `bd_cache.py` function the atm-bd hooks and `bd_cache_cron.py` call at atm-core 05831df | req:REQ-BDC-0910 |
| The `task open` path: after a dev close journaled through the binding, `ready` for its sanity bead is true with the backend unreachable (REQ-BDC-0911) | req:REQ-BDC-0910 |
| An escalated entry is announced once through the binding's announcement verbs (marked, not re-sent) | req:REQ-BDC-0910 |
| `ready(phase, fresh_within=0.5)` against a recording `bd` that answers late returns the cached adjusted answer with `read_fallback` set, within the limit | req:REQ-BDC-0912 |
| In-process cached `get` through the binding is recorded (informational; NFR-BDC-0001 is a-7's) | req:REQ-BDC-0910 |
| The wheel builds and the pytest suite passes in `python.yml` | req:REQ-BDC-0910 |

## Public contract (Python)

Normative; matches `crates/bd-cache-py` exactly. Times are seconds (`float`).

```python
import bd_cache_rs
bd_cache_rs.Cache(repo_root, *, bd_binary="bd", project_dir=None, scopes=None,
                  cache_path=None, call_timeout_s=None,
                  miss_fill_timeout_ms=None, max_attempts=None)
# verbs
get(bead, miss_timeout=None); refresh(ids, timeout)
write(bead, op, args, precondition, idem_key, actor)
list(phase, status=None, label=None); ready(phase, fresh_within=None)
sync(phase, timeout); drain(start_gate_s, hard_s)
entry(seq); resolve(seq, note, actor); prune(phase); status()
unannounced_escalations(); mark_announced(seq)
sync_alert(phase); set_sync_alert(phase, cause); open_work()
# also: path, closed, close(), with-statement; module: CacheError,
# UNREACHABLE (frozenset), __version__
```

Reply shapes: `docs/bd-cache-py/architecture.md`, Reply shapes.
`CacheError(Exception)` carries `code`, `detail`, `fix`, `reason`, `payload`,
`body`, `exit_code`. Bad arguments are `ValueError`; a closed handle is
`RuntimeError`.

## Requirement scope notes

- REQ-BDC-0910 is owned here; REQ-PY-0001..0010 are this crate's
  (`docs/bd-cache-py/requirements.md`, Draft: lead ruling needed).
- REQ-BDC-0806 (the CLI drop-in) is not closed; REQ-BDC-0911 and 0912 are
  a-6's behaviour, exercised here through the binding. REQ-BDC-0402,
  0601, 0101, 0104, NFR-BDC-0004 and NFR-BDC-0007 are core and store
  behaviour reached through the verbs. NFR-BDC-0001 is a-7's; the cached
  `get` number below is informational.

## Acceptance evidence

| Criterion | Evidence (`crates/bd-cache-py/tests/`) |
|---|---|
| every verb, fields checked, recording `bd` | `test_verbs.py` (one test per verb; argv asserted), `test_reads.py` |
| typed error with `code`, `fix`, payload; `PRECONDITION` carries `unmet` | `test_errors.py::test_a_precondition_error_carries_unmet_with_expected_and_actual` and the other `test_errors.py` cases |
| `refresh` fresh rows with overlay; `entry(seq)` any state; sync then list of an unconfigured phase | `test_reads.py::test_refresh_returns_fresh_rows_with_the_overlay`, `test_entry_returns_an_entry_in_any_state`, `test_sync_then_list_of_an_unconfigured_phase_works` |
| mapping table covers every call at atm-core 05831df | `docs/bd-cache-py/architecture.md`, Mapping; `test_mapping_doc.py` |
| `task open` path with the backend unreachable | `test_ready.py::test_task_open_path_ready_after_a_dev_close_with_the_backend_unreachable` (no `bd` call logged) |
| escalated entry announced once | `test_announce.py::test_an_escalated_entry_is_announced_once_then_marked` |
| `ready(fresh_within=0.5)` with a late `bd` | `test_ready.py::test_ready_with_a_limit_returns_the_cached_adjusted_answer_when_bd_answers_late` (under 1.0 s, `read_fallback` set) |
| cached `get` recorded | `test_perf.py`: macOS arm64 release build, local, Python 3.14: median 17.5 us, p99 about 20 us over 2000 calls. CI (run below, `-rP`): macos-14 median 22.7-23.0 us, ubuntu-latest median 46.6-47.2 us |
| wheel builds, suite passes in `python.yml` | CI run below |

## This sprint does not close

Switching atm-bd to the bindings (an atm-core change, done with the one-step
cutover in the phase plan); PyPI publishing; the `bdc` binary.

## Required validation

`python.yml` green; workspace CI green.

## Interpretations and deviations

- **Threading.** The sprint doc said `unsendable`. CI showed that panics in
  PyO3 when a handle is touched or freed off its thread, so the handle is
  `Mutex`-guarded (lead ruling on PR #11; ADR-PY-0003, REQ-PY-0006).
- **Additions beyond the verb list:** `Cache.path`, `closed`, `close()`, the
  `with` statement, `UNREACHABLE` and `__version__`. `path` replaces
  `status['db']`; `UNREACHABLE` is `bd_cache.py`'s set of that name, which
  the cron uses; `close()` is needed because a process holds one connection
  per cache file (NFR-BDC-0007).
- **Cache file default** is `.bd-cache/<db>.bd-cache.sqlite3` (decided
  2026-10-24); atm-bd keeps `.atm-bd/` by setting `cache_path`.
- **Tests use a fake `bd` program** (`tests/fake_bd_main.py`), not a Rust
  invoker, so the wheel under test is the shipped one and the real process
  runner, deadlines and strict row parsing are exercised.
- **Workspace dependencies.** `bd-cache-store` and `sc-bd-cli` are not in
  `[workspace.dependencies]`; the crate names them as `path` dependencies in
  its own `Cargo.toml` (no shared file edited). The other dependencies are
  `workspace = true`.
- **ci.yml** is unchanged: it excludes `bd-cache-py` as a-1 set up;
  `python.yml` runs the crate's clippy and `cargo test` itself.

## Evidence: planted bugs (NFR-BDC-0005)

Run on commit `f554cc10294f913cd5e837721068b9f481c3ef46`, the last commit
that changes code (later commits change documents only). Each mutation was
applied to the working tree, the wheel rebuilt, the whole pytest suite run
(`cd crates/bd-cache-py && maturin develop -q && python -m pytest -q tests`),
and the file restored with `git checkout`:

| Planted bug | File | Compile error | Failing tests (first; count) |
|---|---|---|---|
| typed error drops its payload | `src/error.rs` | no | `test_a_precondition_error_carries_unmet_with_expected_and_actual`; 8 |
| `ready` ignores `fresh_within` | `src/handle.rs` | no | `test_ready_with_a_limit_returns_the_cached_adjusted_answer_when_bd_answers_late`; 4 |
| `mark_announced` does nothing | `src/handle.rs` | no | `test_an_escalated_entry_is_announced_once_then_marked`; 2 |
| GIL held during a verb | `src/handle.rs` | no | `test_other_python_threads_run_while_a_verb_waits_on_bd`; 1 |
| write precondition dropped | `src/handle.rs` | no | `test_a_precondition_error_carries_unmet_with_expected_and_actual`; 9 |
| scopes override ignored | `src/compose.rs` | no | `test_explicit_arguments_override_the_config_file`; 1 |
| negative timeouts accepted | `src/convert.rs` | no | `test_arguments_that_are_not_valid_values_raise_value_error`; 1 |
| `close()` keeps the connection | `src/handle.rs` | no | `test_close_ends_the_handle_and_is_idempotent`; 6 |
| `sync` ignores its timeout | `src/handle.rs` | no | `test_sync_stops_at_its_timeout_when_bd_answers_late`; 1 |
| `reason` not carried | `src/error.rs` | no | `test_a_precondition_error_carries_unmet_with_expected_and_actual`; 1 |
| `get` ignores `miss_timeout` | `src/handle.rs` | no | `test_a_zero_miss_timeout_is_a_cached_only_get`; 2 |
| `UNREACHABLE` lacks `BD_LIST_FAILED` | `src/error.rs` | no | `test_unreachable_is_the_set_of_codes_that_mean_bd_did_not_answer`; 1 |
| drain start gate and hard deadline swapped | `src/handle.rs` | no | `test_drain_start_gate_and_hard_deadline_are_separate_bounds`; 1 |
| `write` swaps idempotency key and actor | `src/handle.rs` | no | `test_an_escalated_entry_is_announced_once_then_marked`; 9 |

Every bug fails at least one test and none by a compile error (the first
`close()` mutation did not compile and was replaced by the one listed); the
tree was clean after the run. The mutations are the `old`/`new` pairs of one
script; the escalation, precondition and `ready` rules themselves are
a-1/a-5/a-6's and carry their planted bugs there.

## Validation

Local, macOS arm64, on `f554cc1` (docs after it): `cargo test --workspace
--all-features`: 644 passed; pytest: 72 passed.

| Gate | Result |
|---|---|
| `cargo fmt --all --check` | pass |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pass |
| `cargo clippy --workspace --lib -- -D warnings` | pass |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | pass |
| `cargo test --workspace --all-features` | pass |
| pytest (`maturin develop`, 72 tests) | pass |

CI on the last code-changing commit `f554cc1`: `python.yml` run
https://github.com/randlee/beads-cache/actions/runs/40083265242 (clippy and
test, wheel, pytest on Python 3.9, 3.11 and 3.13 on macos-14 and
ubuntu-latest: all success) and `ci.yml` run
https://github.com/randlee/beads-cache/actions/runs/40083265243 (success).
CI run 40082767056 on `fee1893` failed (a tick-count threading test and the
unsendable panic); both are fixed in `f554cc1`.

## Hand-off notes for the lead

1. **Draft ids for a ruling:** REQ-PY-0001..0010 and ADR-PY-0001..0004.
2. **Status.** Set to `in-review` per the wave-2 hand-off; the dispatch gate
   text says `complete`, to be set at merge.
3. **`docs/project-plan.md`** already lists a-8 (status `planned`); not an
   owned path, so unchanged.
4. **Publishing** is untouched: no token was read or used.
