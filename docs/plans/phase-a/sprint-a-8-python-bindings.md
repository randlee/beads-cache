---
id: a-8
phase: a
status: planned
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
  owns: [REQ-BDC-0910]
  constrained_by: [REQ-BDC-0001, REQ-BDC-0002, REQ-BDC-0802, REQ-BDC-0803, REQ-BDC-0903, REQ-BDC-0907, REQ-BDC-0908, REQ-BDC-0909, REQ-BDC-0911, REQ-BDC-0912, NFR-BDC-0002]
adrs: [ADR-BDC-0003, ADR-BDC-0005, ADR-CORE-0001]
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

1. `bd_cache_rs` Python module (name final at dispatch) over the concrete
   composition `Cache<SqliteStore, FlockDrainLock, BdCli>`, opened from the
   repository (`.beads/bd-cache.toml` when present, else explicit arguments),
   with the verbs atm-bd uses (REQ-BDC-0910): `get(bead, miss_timeout=None)`,
   `refresh(ids, timeout)`, `write(bead, op, args, precondition, idem_key,
   actor)`, `list(phase, status=None, label=None)`, `ready(phase, fresh_within=None)`,
   `sync(phase, timeout)`, `drain(start_gate_s, hard_s)`, `entry(seq)`,
   `resolve(seq, note, actor)`, `prune(phase)`, `status()`.
2. Replies are the Rust reply types converted to Python dicts with the same
   field names as their JSON (ADR-BDC-0003); `CacheError` becomes one Python
   exception class carrying `code`, `detail`, `fix`, `reason` and the payload
   (REQ-BDC-0802, 0803).
3. Threading per ADR-CORE-0001: the handle is a `#[pyclass(unsendable)]`
   object owning one connection; calls release the GIL while they wait on
   `bd`.
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
| An in-process cached `get` through the binding takes under 1 ms median (no process start) | req:REQ-BDC-0910 |
| The wheel builds and the pytest suite passes in `python.yml` | req:REQ-BDC-0910 |

## This sprint does not close

Switching atm-bd to the bindings (an atm-core change, done with the one-step
cutover in the phase plan); PyPI publishing; the `bdc` binary.

## Required validation

`python.yml` green; workspace CI green.
