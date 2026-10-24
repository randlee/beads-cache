"""Threading per ADR-CORE-0001 and ADR-PY-0003: a closable, mutex-guarded handle that releases the GIL.

The GIL, close and lock tests are causal. The fake `bd` blocks until a release file exists, and a
Python thread creates that file only after observing something that can happen only if the GIL
was free (a ticker's progress, `closed` turning true). If the GIL were held, the release never
comes and the verb fails at its own deadline; there is no duration ratio to tune.
"""

import threading
import time

import pytest

import bd_cache_rs
from conftest import daemon, join, wait_until


def test_a_python_thread_runs_while_a_verb_waits_on_bd_and_releases_it(cache, repo):
    release = repo.bd.block("list")
    seen = {}

    def release_once_the_verbs_own_call_runs():
        try:
            # Needs the GIL while the verb is blocked inside bd; sees only calls made after arming.
            seen["argv"] = release.wait_started()[0]
        except BaseException as error:
            seen["error"] = error
        finally:
            release.set()

    helper = daemon(release_once_the_verbs_own_call_runs)
    helper.start()
    rows = cache.refresh(["b1"], 10)  # a held GIL would never let the helper release bd
    join(helper)
    assert "error" not in seen, seen
    assert seen["argv"][:3] == ["list", "--id", "b1"], "the helper saw this verb's own bd call"
    assert [r["bead"]["id"] for r in rows] == ["b1"]


def test_any_thread_can_use_the_handle_and_none_panics(cache):
    outcome = []

    def other_thread():
        try:
            outcome.append(cache.status()["meta"]["dolt_database"])
            outcome.append(cache.write("b1", "close", {}, None, "k-thread", "dev-1")["seq"])
        except BaseException as error:  # any failure here is a defect, including a panic
            outcome.append(error)

    thread = daemon(other_thread)
    thread.start()
    join(thread)
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

    threads = [daemon(worker, n) for n in range(4)]
    for t in threads:
        t.start()
    for t in threads:
        join(t)
    assert errors == []
    assert sorted(seqs) == list(range(1, 41))


def test_close_while_a_verb_is_in_flight(cache, repo):
    release = repo.bd.block("list")
    result = {}
    ticks = [0]
    stop = threading.Event()
    seen = {}

    def verb():
        try:
            result["rows"] = cache.refresh(["b1"], 10)
        except BaseException as error:
            result["error"] = error

    def ticker():
        while not stop.is_set():
            ticks[0] += 1
            time.sleep(0.005)

    def releaser():
        try:
            # `closed` turns true as close() begins, and close() then waits for the verb.
            wait_until(lambda: cache.closed, "close() to begin")
            base = ticks[0]
            # The ticker can only advance if close() released the GIL while it waits.
            wait_until(lambda: ticks[0] >= base + 5, "the ticker to advance during close()")
            seen["closed_while_in_flight"] = cache.closed
        except BaseException as error:
            seen["error"] = error
        finally:
            release.set()

    in_flight, tick, rel = daemon(verb), daemon(ticker), daemon(releaser)
    in_flight.start()
    argv = release.wait_started()[0]  # the verb's own call holds the lock inside bd
    assert argv[:3] == ["list", "--id", "b1"]
    tick.start()
    rel.start()
    cache.close()  # returns only after the verb finished
    join(in_flight)
    join(rel)
    stop.set()
    join(tick)

    assert "error" not in seen, seen
    assert "error" not in result, result
    assert [r["bead"]["id"] for r in result["rows"]] == ["b1"], "the in-flight verb finished normally"
    assert seen["closed_while_in_flight"] is True
    assert release.is_set() and cache.closed is True
    with pytest.raises(bd_cache_rs.ClosedError, match="open a new bd_cache_rs.Cache"):
        cache.get("b1")


def test_a_handle_can_be_closed_and_dropped_on_another_thread(repo):
    cache = repo.open()
    done = []

    def other_thread():
        cache.close()
        done.append(cache.closed)

    thread = daemon(other_thread)
    thread.start()
    join(thread)
    assert done == [True]
    repo.opened.remove(cache)
    del cache
    assert repo.open().closed is False, "the file is free again"


def test_a_handle_dropped_on_another_thread_does_not_panic(repo):
    holder = [repo.open()]
    repo.opened.clear()

    def other_thread():
        holder.clear()  # the last reference dies here

    thread = daemon(other_thread)
    thread.start()
    join(thread)
    assert repo.open().closed is False


def test_each_thread_can_own_a_cache_over_its_own_file(repo):
    results = []

    def work(n):
        cache = bd_cache_rs.Cache(
            str(repo.root), bd_binary=str(repo.bd.binary), cache_path=f"thread-{n}.sqlite3", scopes=["x"])
        results.append((n, cache.sync("x", 30)["stored"]))
        cache.close()

    threads = [daemon(work, n) for n in range(3)]
    for t in threads:
        t.start()
    for t in threads:
        join(t)
    assert sorted(results) == [(0, 3), (1, 3), (2, 3)]


def test_handles_on_different_cache_files_never_overlap_their_bd_calls(repo):
    # Two handles, one project directory: bd on embedded Dolt must not run twice at once, so the
    # handles share one process-wide lock set. Each fake bd call logs its start and its end.
    repo.bd.delay("*", 0.2)
    first = repo.open(cache_path="first.sqlite3")
    second = repo.open(cache_path="second.sqlite3")
    barrier = threading.Barrier(2)
    errors = []

    def work(cache, bead):
        try:
            barrier.wait(30)
            for _ in range(3):
                cache.refresh([bead], 20)
        except BaseException as error:
            errors.append(error)

    threads = [daemon(work, first, "b1"), daemon(work, second, "b2")]
    for t in threads:
        t.start()
    for t in threads:
        join(t)
    assert errors == []
    spans = sorted(repo.bd.spans())
    assert len(spans) == 6
    for (_, end, argv), (start, _, _) in zip(spans, spans[1:]):
        assert end is not None and end <= start, f"bd calls overlapped: {argv} ended {end}, next started {start}"


def test_spellings_of_one_project_directory_share_one_lock(repo):
    # `root`, `root/.` and a symlink to it are one directory. The lock key must be the same
    # string for all of them, or their `bd` calls would overlap.
    repo.bd.delay("*", 0.2)
    link = repo.root.parent / "repo-link"
    try:
        link.symlink_to(repo.root, target_is_directory=True)
    except (OSError, NotImplementedError):
        pytest.skip("symlinks are not available here")
    spellings = [str(repo.root), str(repo.root) + "/.", str(link)]
    caches = [repo.open(cache_path=f"c{n}.sqlite3", project_dir=d) for n, d in enumerate(spellings)]
    barrier = threading.Barrier(3)
    errors = []

    def work(cache):
        try:
            barrier.wait(30)
            for _ in range(2):
                cache.refresh(["b1"], 20)
        except BaseException as error:
            errors.append(error)

    threads = [daemon(work, c) for c in caches]
    for t in threads:
        t.start()
    for t in threads:
        join(t)
    assert errors == []
    spans = sorted(repo.bd.spans())
    assert len(spans) == 6
    for (_, end, argv), (start, _, _) in zip(spans, spans[1:]):
        assert end is not None and end <= start, f"bd calls overlapped: {argv} ended {end}, next started {start}"


def test_a_project_directory_that_does_not_resolve_is_refused(repo):
    with pytest.raises(ValueError, match="does not resolve") as caught:
        repo.open(project_dir=str(repo.root / "no-such-dir"))
    assert caught.value.kind == "argument"
