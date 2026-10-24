"""Threading per ADR-CORE-0001: an unsendable handle that releases the GIL while it waits on bd."""

import threading
import time

import bd_cache_rs


def test_other_python_threads_run_while_a_verb_waits_on_bd(cache, repo):
    repo.bd.delay("*", 1.5)
    ticks = []
    stop = threading.Event()

    def ticker():
        while not stop.is_set():
            ticks.append(time.monotonic())
            time.sleep(0.01)

    thread = threading.Thread(target=ticker)
    thread.start()
    try:
        started = time.monotonic()
        cache.refresh(["b1"], 10)  # one bd call that answers after 1.5 s
        elapsed = time.monotonic() - started
    finally:
        stop.set()
        thread.join()
    assert elapsed >= 1.4
    during = [t for t in ticks if started <= t <= started + elapsed]
    assert len(during) >= 50, f"only {len(during)} ticks of a 10 ms ticker in {elapsed:.2f}s: the GIL was held"


def test_the_handle_is_unsendable(cache):
    outcome = []

    def other_thread():
        try:
            cache.status()
        except BaseException as error:  # PyO3 raises for a handle used off its thread
            outcome.append(error)

    thread = threading.Thread(target=other_thread)
    thread.start()
    thread.join()
    assert len(outcome) == 1
    assert "unsendable" in str(outcome[0])
    assert cache.status()["meta"]["dolt_database"] == "atm-dev", "the owning thread is unaffected"


def test_each_thread_can_own_a_cache_over_its_own_file(repo):
    # One connection per cache file per process: threads use different files (cache_path).
    results = []

    def work(n):
        cache = bd_cache_rs.Cache(
            str(repo.root), bd_binary=str(repo.bd.binary), cache_path=f"thread-{n}.sqlite3", scopes=["x"])
        results.append((n, cache.sync("x", 30)["stored"]))
        cache.close()

    threads = [threading.Thread(target=work, args=(n,)) for n in range(3)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    assert sorted(results) == [(0, 3), (1, 3), (2, 3)]
