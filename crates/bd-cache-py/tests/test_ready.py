"""`ready` through the binding: our own writes at once, and the bounded fresh answer (REQ-BDC-0911, 0912)."""

from conftest import bead


def ready_ids(reply):
    return [row["bead"]["id"] for row in reply["ready"]]


def sanity_world(repo):
    """The `atm task open` world: a sanity bead blocked by the dev bead."""
    repo.config('scopes = ["x"]\n')
    repo.bd.add(
        bead("dev-1", status="in_progress", assignee="dev-1"),
        bead("san-1", blocked_by=["dev-1"]),
    )
    cache = repo.open()
    cache.sync("x", 30)
    return cache


def test_task_open_path_ready_after_a_dev_close_with_the_backend_unreachable(repo):
    cache = sanity_world(repo)
    assert "san-1" not in ready_ids(cache.ready("x"))
    assert cache.get("san-1")["row"]["blocked"] == ["dev-1"]

    repo.bd.down()
    calls = len(repo.bd.calls())
    write = cache.write("dev-1", "close", {"reason": "PASS at abc123"}, {"status": "in_progress"},
                        "close:dev-1:1", "dev-1")
    assert write["duplicate"] is False

    reply = cache.ready("x")
    assert "san-1" in ready_ids(reply)
    assert reply["locally_adjusted"] is True and reply["read_fallback"] is None
    assert cache.get("san-1")["row"]["blocked"] == []
    assert len(repo.bd.calls()) == calls, "no bd call: the answer comes from the base and the journal"
    assert cache.status()["outage"] is None, "the backend was never asked, so no outage was recorded"


def test_the_journal_lands_when_bd_returns_and_fresh_ready_agrees(repo):
    cache = sanity_world(repo)
    repo.bd.down()
    cache.write("dev-1", "close", {"reason": "PASS"}, {"status": "in_progress"}, "close:dev-1:1", "dev-1")
    down = cache.drain(10, 20)
    assert down["outage"] is not None and down["applied"] == []
    assert cache.entry(1)["attempts"] == 0, "an outage counts no attempt"

    repo.bd.down(False)
    up = cache.drain(10, 20)
    assert up["applied"] == [1] and up["outage"] is None
    fresh = cache.ready("x", fresh_within=10)
    assert fresh["basis"]["kind"] == "fresh" and "san-1" in ready_ids(fresh)


def test_ready_with_a_limit_returns_the_cached_adjusted_answer_when_bd_answers_late(repo, stopwatch):
    cache = sanity_world(repo)
    cache.write("dev-1", "close", {"reason": "PASS"}, {"status": "in_progress"}, "close:dev-1:1", "dev-1")
    repo.bd.delay("*", 5)

    with stopwatch() as watch:
        reply = cache.ready("x", fresh_within=0.5)

    assert watch.elapsed < 0.5 + 0.5, f"returned after {watch.elapsed:.2f}s, the limit was 0.5s"
    assert reply["basis"]["kind"] == "cached"
    assert reply["read_fallback"].startswith("BD_CACHE.BD_UNAVAILABLE")
    assert "san-1" in ready_ids(reply), "the cached answer, adjusted by the journaled close"
    assert reply["locally_adjusted"] is True


def test_ready_with_a_limit_and_bd_down_falls_back_and_records_the_outage(repo):
    cache = sanity_world(repo)
    repo.bd.down()
    reply = cache.ready("x", fresh_within=2)
    assert reply["basis"]["kind"] == "cached" and reply["read_fallback"]
    assert cache.status()["outage"]["last_error"].startswith("BD_CACHE.BD_UNAVAILABLE")
