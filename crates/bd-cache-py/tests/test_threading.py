"""Threading per ADR-CORE-0001 and ADR-PY-0003: a mutex-guarded handle that releases the GIL while it waits."""

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
    # A held GIL freezes the ticker for the whole call (a gap of about `elapsed`);
    # scheduling jitter on a busy runner is a small fraction of that. The signal is the
    # longest gap between ticks across the call, not a tick count.
    marks = [started] + [t for t in ticks if started <= t <= started + elapsed] + [started + elapsed]
    longest = max(b - a for a, b in zip(marks, marks[1:]))
    assert longest < elapsed / 2, f"the ticker stalled {longest:.2f}s of a {elapsed:.2f}s call: the GIL was held"


def test_any_thread_can_use_the_handle_and_none_panics(cache):
    outcome = []

    def other_thread():
        try:
            outcome.append(cache.status()["meta"]["dolt_database"])
            outcome.append(cache.write("b1", "close", {}, None, "k-thread", "dev-1")["seq"])
        except BaseException as error:  # any failure here is a defect, including a panic
            outcome.append(error)

    thread = threading.Thread(target=other_thread)
    thread.start()
    thread.join()
    assert outcome == ["atm-dev", 1]
    assert cache.entry(1)["idem_key"] == "k-thread", "the owning thread sees what the other wrote"


def test_verbs_from_several_threads_run_one_at_a_time_without_error(cache):
    errors = []
    seqs = []

    def worker(n):
        try:
            for i in range(10):
                seqs.append(cache.write("b1", "update", {"add_labels": [f"l{n}-{i}"]}, None,
                                        f"k-{n}-{i}", "dev-1")["seq"])
                cache.get("b1")
                cache.status()
        except BaseException as error:
            errors.append(error)

    threads = [threading.Thread(target=worker, args=(n,)) for n in range(4)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    assert errors == []
    assert sorted(seqs) == list(range(1, 41))


def test_a_handle_can_be_closed_and_dropped_on_another_thread(repo):
    cache = repo.open()
    done = []

    def other_thread():
        cache.close()
        done.append(cache.closed)

    thread = threading.Thread(target=other_thread)
    thread.start()
    thread.join()
    assert done == [True]
    repo.opened.remove(cache)
    del cache
    assert repo.open().closed is False, "the file is free again"


def test_a_handle_dropped_on_another_thread_does_not_panic(repo):
    holder = [repo.open()]
    repo.opened.clear()

    def other_thread():
        holder.clear()  # the last reference dies here

    thread = threading.Thread(target=other_thread)
    thread.start()
    thread.join()
    assert repo.open().closed is False


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
