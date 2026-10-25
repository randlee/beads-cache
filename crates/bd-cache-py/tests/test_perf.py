"""In-process cached `get` through the binding: recorded, informational.

NFR-BDC-0001 (the cached-get budget) is sprint a-7's, measured in Rust. This
records what the same call costs from Python, including the reply conversion,
so atm-bd can compare it with `bd_cache.py`. The only assertion is causal (the
fake bd's call log is unchanged across the loop); the timings go to the pytest
report (`-rP` shows them) and are never asserted.
"""

import statistics
import time

RUNS = 2000


def test_cached_get_through_the_binding_is_recorded(repo, cache, record_property, capsys):
    cache.get("b1")  # warm: connection, statement cache
    calls = repo.bd.calls()
    samples = []
    for _ in range(RUNS):
        start = time.perf_counter()
        cache.get("b1")
        samples.append((time.perf_counter() - start) * 1e6)
    samples.sort()
    median = statistics.median(samples)
    p99 = samples[int(RUNS * 0.99)]
    record_property("cached_get_median_us", round(median, 1))
    record_property("cached_get_p99_us", round(p99, 1))
    with capsys.disabled():
        print(f"\ncached get via bd_cache_rs: median {median:.1f} us, p99 {p99:.1f} us over {RUNS} calls")
    assert repo.bd.calls() == calls, "a cached get must not call bd; timing is recorded, not asserted"
