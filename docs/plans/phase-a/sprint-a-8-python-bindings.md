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
  constrained_by: [REQ-BDC-0001, REQ-BDC-0002, REQ-BDC-0101, REQ-BDC-0102, REQ-BDC-0104, REQ-BDC-0202, REQ-BDC-0203, REQ-BDC-0302, REQ-BDC-0402, REQ-BDC-0406, REQ-BDC-0601, REQ-BDC-0602, REQ-BDC-0603, REQ-BDC-0604, REQ-BDC-0704, REQ-BDC-0705, REQ-BDC-0802, REQ-BDC-0803, REQ-BDC-0804, REQ-BDC-0806, REQ-BDC-0903, REQ-BDC-0904, REQ-BDC-0907, REQ-BDC-0908, REQ-BDC-0909, REQ-BDC-0911, REQ-BDC-0912, NFR-BDC-0001, NFR-BDC-0004, NFR-BDC-0005, NFR-BDC-0006, NFR-BDC-0007]
adrs: [ADR-BDC-0001, ADR-BDC-0002, ADR-BDC-0003, ADR-BDC-0004, ADR-BDC-0005, ADR-BDC-0007, ADR-MODEL-0002, ADR-CORE-0001, ADR-STORE-0003, ADR-PY-0001, ADR-PY-0002, ADR-PY-0003, ADR-PY-0004]
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
   2026-10-24, PR #11):* the handle is guarded by a closable mutex slot, not
   `unsendable` (ADR-CORE-0001's own-mutex option, ADR-PY-0003), so a handle
   used or dropped on another thread never panics in PyO3; `close()`,
   `closed` and `__exit__` never hold the GIL while they wait.
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
# also: path, closed, close(), with-statement; module: CacheError, ClosedError,
# UNREACHABLE (frozenset), __version__
```

Reply shapes: `docs/bd-cache-py/architecture.md`, Reply shapes.
`CacheError(Exception)` carries `code`, `detail`, `fix`, `reason`, `payload`,
`body`, `exit_code`. Bad arguments are `ValueError`; a closed handle is
``ClosedError` (a `RuntimeError`).

## Requirement scope notes

- REQ-BDC-0910 is owned here; REQ-PY-0001..0010 are this crate's
  (`docs/bd-cache-py/requirements.md`, Approved: lead ruling R1, bc-lead, 2026-10-24).
- REQ-BDC-0806 (the CLI drop-in) is not closed; REQ-BDC-0911 and 0912 are
  a-6's behaviour, exercised here through the binding. REQ-BDC-0402,
  0601, 0101, 0102, 0104, 0202, 0203, 0302, 0406, 0602, 0603, 0604, 0704, 0705,
  0904, NFR-BDC-0004 and NFR-BDC-0007 are core and store behaviour reached
  through the verbs (the verb tests check the reply fields they define).
  REQ-BDC-0804 (a file written by either implementation is read by the other)
  is a-4's; the bindings open the shared file and the mapping table keeps
  atm-bd's file path usable through `cache_path`. REQ-BDC-0806 is deferred
  (not closed here). NFR-BDC-0006 (live tests on the testbed only) applies
  as: no test here runs a live `bd`; all use the fake program. NFR-BDC-0001 is a-7's; the cached
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
  PyO3 when a handle is touched or freed off its thread, so the handle owns a
  closable mutex slot (`src/slot.rs`; ADR-PY-0003, REQ-PY-0006; lead ruling on
  PR #11). `close()`, `__exit__` and `closed` hold no GIL while they wait.
- **Additions beyond the verb list:** `Cache.path`, `closed`, `close()`, the
  `with` statement, `UNREACHABLE`, `ClosedError` and `__version__`.
- **Cache file default** is `.bd-cache/<db>.bd-cache.sqlite3` (decided
  2026-10-24); atm-bd keeps `.atm-bd/` by setting `cache_path`.
- **Tests use a fake `bd` program** (`tests/fake_bd_main.py`) so the wheel
  under test is the shipped one. Time-dependent tests are causal (the fake
  blocks until a Python thread creates a release file); durations are loose
  sanity bounds, except the `ready` limit, which asserts the limit plus a
  stated 0.4 s margin and cites the core test that proves the bound
  (`crates/bd-cache/tests/read_ready.rs`,
  `bounded_fresh_ready_falls_back_to_the_cached_answer_at_the_limit_without_an_outage`).
- **Workspace dependencies.** `bd-cache-store` and `sc-bd-cli` are in the root
  `[workspace.dependencies]` (lead ruling R2, bc-lead, 2026-10-24; recorded in
  phase-a-plan's Shared files table); the crate uses `workspace = true`.
- **`list` status filter** is not validated: the model has no closed set of
  statuses (REQ-PY-0005).
- **ci.yml** is unchanged: it excludes `bd-cache-py`; `python.yml` runs the
  crate's clippy and `cargo test` on macOS, Linux and Windows (R4).

## Evidence: planted bugs (NFR-BDC-0005)

Run on commit `42f6ef46b8b144d1c9df4e129cbb7c3527d8ea98`, the last commit
that changes code (later commits change documents only). Each mutation was
applied to the working tree, the wheel rebuilt, the whole pytest suite and
`cargo test -p bd-cache-py --lib` run, and the file restored with
`git checkout`:

```text
cd crates/bd-cache-py && maturin develop -q && python -m pytest -q tests && cargo test -p bd-cache-py --lib
```

| Planted bug | File | First failing test; failures |
|---|---|---|
| typed error drops its payload (no `unmet`) | `src/error.rs` | `test_a_precondition_error_carries_unmet_with_expected_and_actual`; 9 |
| `ready` ignores `fresh_within` | `src/handle.rs` | `test_the_journal_lands_when_bd_returns_and_fresh_ready_agrees`; 5 |
| `mark_announced` does nothing | `src/handle.rs` | `test_an_escalated_entry_is_announced_once_then_marked`; 2 |
| GIL held during a verb | `src/handle.rs` | `test_ready_with_a_limit_uses_bd_when_it_answers_after_a_release_inside_the_limit`; 3 |
| write precondition dropped | `src/handle.rs` | `test_an_escalated_entry_is_announced_once_then_marked`; 9 |
| scopes override ignored | `src/compose.rs` | `test_explicit_arguments_override_the_config_file`; 1 |
| negative timeouts accepted | `src/convert.rs` | `test_arguments_that_are_not_valid_values_raise_value_error`; 5 |
| `close()` does nothing | `src/handle.rs` | `test_max_attempts_from_the_arguments_decides_when_a_failing_write_escalates`; 7 |
| `__exit__` does not close | `src/handle.rs` | `test_the_handle_is_a_context_manager`; 1 |
| `sync` ignores its timeout | `src/handle.rs` | `test_sync_stops_at_its_timeout_when_bd_is_blocked`; 1 |
| `reason` not carried | `src/error.rs` | `test_a_precondition_error_carries_unmet_with_expected_and_actual`; 1 |
| `get` ignores `miss_timeout` | `src/handle.rs` | `test_unreachable_is_the_set_of_codes_that_mean_bd_did_not_answer`; 2 |
| `UNREACHABLE` lacks BD_LIST_FAILED | `src/error.rs` | `test_unreachable_is_the_set_of_codes_that_mean_bd_did_not_answer`; 2 |
| drain start gate and hard deadline swapped | `src/handle.rs` | `test_drain_hard_deadline_cuts_a_blocked_call_and_counts_no_attempt`; 2 |
| `write` swaps idempotency key and actor | `src/handle.rs` | `test_an_escalated_entry_is_announced_once_then_marked`; 9 |
| one `ProjectLocks` per handle | `src/compose.rs` | `test_handles_on_different_cache_files_never_overlap_their_bd_calls`; 2 |
| `op` not validated in the binding | `src/handle.rs` | `test_an_unknown_op_is_a_value_error_and_bad_args_are_typed`; 1 |
| `max_attempts` of 0 accepted | `src/handle.rs` | `test_signed_and_float_timeouts_and_attempts_are_validated[max_attempts-0]`; 1 |
| `closed` set only after the in-flight verb ends | `src/slot.rs` | `test_close_while_a_verb_is_in_flight`; 2 |
| poisoned slot keeps its connection | `src/slot.rs` | `a_panicking_verb_drops_the_value_clears_the_poison_and_closes`; 1 |
| `fix` treated as an envelope key (left out of the attribute) | `src/error.rs` | `test_a_precondition_error_carries_unmet_with_expected_and_actual`; 7 |

Every bug fails at least one test and none by a compile error; the tree was
clean after the run. Failures count pytest and Rust tests together.

## Validation

Local, macOS arm64, on `42f6ef4` (documents after it): `cargo test --workspace
--all-features`: 653 passed; pytest: 88 passed.

| Gate (with `set -o pipefail`) | Result |
|---|---|
| `cargo fmt --all --check` | pass |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pass |
| `cargo clippy --workspace --lib -- -D warnings` | pass |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | pass |
| `cargo test --workspace --all-features` | pass |
| pytest (`maturin develop`, 88 tests) | pass |

CI on the last code-changing commit `42f6ef4`: `python.yml` run
https://github.com/randlee/beads-cache/actions/runs/40085690694 (rust job on
macos-14, ubuntu-latest and windows-latest, wheels, pytest on Python 3.9, 3.11
and 3.13 on macos-14 and ubuntu-latest: all success) and `ci.yml` run
https://github.com/randlee/beads-cache/actions/runs/40085690769 (success).
The docs-only head `27bf211` is also green (python 40095045711, ci 40095045761).

## Fix round 1 (QA-1 at b75b5f3)

Lead rulings R1-R8 (bc-lead, 2026-10-24) applied: REQ-PY and ADR-PY approved
(R1); workspace dependencies (R2); serde and serde_json recorded (R3); Windows
in the `python.yml` rust job (R4); project-plan row `in-review` (R5);
REQ-QA-010 closed by lead evidence (R6); `op` validated in the binding (R7);
atm-core call evidence in `crates/bd-cache-py/tests/atm_core_05831df_calls.json`,
extracted with `atm_core_05831df_extract.py` (R8).

## Hand-off notes for the lead

1. **Status.** `in-review`; the project-plan row says `in-review` too. Set both
   to `complete` at merge.
2. **Publishing** is untouched: no token was read or used.
