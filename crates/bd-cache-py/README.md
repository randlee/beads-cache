# bd-cache-rs

Python bindings for bd-cache, the local SQLite read/write cache of a beads
repository. Built with maturin and PyO3. The distribution is `bd-cache-rs`
and the module is `bd_cache_rs`. One abi3 wheel per platform serves CPython
3.9 and later.

The module wraps the real composition: the SQLite store, the `flock` drain
lock and the `bd` CLI backend. Reads come from the cache file and start no
`bd` process. Writes are validated, checked against their precondition and
journaled; `drain` applies them through `bd` later.

## Example

```python
import bd_cache_rs

# Reads .beads/metadata.json and .beads/bd-cache.toml; opening starts no bd process.
with bd_cache_rs.Cache("/path/to/repo", scopes=["x"]) as cache:
    cache.sync("x", 30)                        # refresh phase-x from bd, 30 s budget

    reply = cache.get("bk-12", miss_timeout=1.5)
    print(reply["row"]["bead"]["status"], reply["row"]["stale"])

    try:
        w = cache.write("bk-12", "close", {"reason": "PASS"},
                        {"status": "open"}, "close:bk-12:1", "dev-1")
        print("journaled as seq", w["seq"])
    except bd_cache_rs.CacheError as e:
        print(e.code, e.fix, e.payload)        # for example BD_CACHE.PRECONDITION

    ready = cache.ready("x", fresh_within=0.5) # bd's answer within 0.5 s, else the cached one
    print([v["bead"]["id"] for v in ready["ready"]], ready["basis"]["kind"])

    report = cache.drain(25, 28)               # start gate and hard deadline, in seconds
    print(report["applied"], report["escalated"])
```

`Cache(repo_root, *, bd_binary="bd", project_dir=None, scopes=None,
cache_path=None, call_timeout_s=None, miss_fill_timeout_ms=None,
max_attempts=None)`: each keyword that is not `None` overrides the same key
of `.beads/bd-cache.toml`. A scope is a phase name: `"x"` selects the beads
labelled `phase-x`.

The verbs are `get`, `refresh`, `write`, `list`, `ready`, `sync`, `drain`,
`entry`, `resolve`, `prune` and `status`, plus the announcement state
`unannounced_escalations`, `mark_announced`, `sync_alert`, `set_sync_alert`
and `open_work`. Replies are builtin `dict` / `list` values whose shapes are
versioned with the wheel (`bd_cache_rs.__version__`).

Errors:

- `bd_cache_rs.CacheError` carries every `BD_CACHE.*` code, with the
  attributes `code`, `detail`, `fix`, `reason`, `payload`, `body` and
  `exit_code`.
- A bad argument (an invalid bead id, phase, timeout or op) is `ValueError`.
- A verb on a closed handle is `bd_cache_rs.ClosedError`, a `RuntimeError`.
- `bd_cache_rs.UNREACHABLE` is the frozenset of codes that mean the backend
  could not be reached.

Threading: the handle may be shared between threads. Verbs on one handle run
one at a time, and the GIL is released while a verb waits on `bd` or SQLite.

## Install

Once it is published to PyPI:

```sh
pip install bd-cache-rs
```

Until then, build it from the repository. This needs a Rust
toolchain (rustup installs the pinned 1.98.1 from `rust-toolchain.toml`);
maturin is fetched by pip as the build backend:

```sh
pip install "git+https://github.com/randlee/beads-cache@integrate/phase-a#subdirectory=crates/bd-cache-py"
```

## Known limits

- One process holds one connection per cache file. A second handle on the
  same file in the same process is `BD_CACHE.STORE_FAILED` until the first is
  closed; separate processes are fine.
- Never let two implementations drain the same journal at the same time. Stop
  the Python `bd_cache.py` cron before this module's `drain` runs.
- Wheels are built and tested on macOS arm64 and Linux x86_64 only. The Rust
  crate's own tests also run on Windows.

## Links

- Requirements: [`docs/bd-cache-py/requirements.md`](https://github.com/randlee/beads-cache/blob/develop/docs/bd-cache-py/requirements.md)
- Architecture, reply shapes and the mapping from each `bd_cache.py` call:
  [`docs/bd-cache-py/architecture.md`](https://github.com/randlee/beads-cache/blob/develop/docs/bd-cache-py/architecture.md)

License: MIT.
