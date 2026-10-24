# bd-cache-rs

The bd-cache cache of a beads repository, in process from Python (maturin /
PyO3). One abi3 wheel per platform serves CPython 3.9 and later.

```python
import bd_cache_rs

cache = bd_cache_rs.Cache("/path/to/repo")           # reads .beads/bd-cache.toml
reply = cache.get("bk-12", miss_timeout=1.5)         # dict
print(reply["row"]["bead"]["status"])
try:
    cache.write("bk-12", "close", {"reason": "PASS"},
                {"status": "in_progress"}, "close:bk-12:1", "dev-1")
except bd_cache_rs.CacheError as e:
    print(e.code, e.fix, e.payload)
```

Reference: `docs/bd-cache-py/requirements.md` and `architecture.md` in the
repository, including the mapping from each `bd_cache.py` function.
