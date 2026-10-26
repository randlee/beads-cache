"""`Cache.tick()` (REQ-BDC-0720, ADR-PY-0006): the built-in row's run, on the handle's connection."""

import asyncio
import threading

import pytest

import bd_cache_rs


def test_tick_runs_the_built_in_sync_and_frees_the_lease(cache, repo):
    reply = cache.tick()
    assert reply["outcome"] == "ok" and reply["skipped"] is None
    assert [s["scope"] for s in reply["scopes"]] == ["x"]
    assert reply["scopes"][0]["result"] == "ok"
    assert cache.schedule_show()["lease"] is None


def test_tick_from_an_asyncio_loop_thread_returns_without_nesting_a_runtime(cache):
    async def main():
        return cache.tick()

    assert asyncio.run(main())["outcome"] == "ok"


def test_other_threads_keep_running_while_a_tick_runs(cache):
    beat = []
    stop = threading.Event()

    def heartbeat():
        while not stop.is_set():
            beat.append(1)
            stop.wait(0.001)

    t = threading.Thread(target=heartbeat, daemon=True)
    t.start()
    cache.tick()
    stop.set()
    t.join(5)
    assert beat, "the GIL was released"


def test_an_unreachable_bd_is_an_outage_result_and_reads_still_answer(cache, repo):
    repo.bd.down()
    reply = cache.tick()
    assert reply["scopes"][0]["result"] == "outage"
    assert reply["scopes"][0]["error"]["outage"] is True
    assert cache.get("b1")["row"]["bead"]["id"] == "b1", "cached reads answer"
