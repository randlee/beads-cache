"""`ready` through the binding: our own writes at once, and the bounded fresh answer (REQ-BDC-0911, 0912)."""

from conftest import bead, daemon, join


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


LIMIT = 0.5


def test_ready_with_a_limit_returns_the_cached_adjusted_answer_while_bd_is_still_blocked(repo, stopwatch):
    # The core proves the bound itself with a fake clock
    # (crates/bd-cache/tests/read_ready.rs,
    # `bounded_fresh_ready_falls_back_to_the_cached_answer_at_the_limit_without_an_outage`).
    # Here the proof is causal: bd is blocked for up to 20 s and nothing releases it, yet the
    # reply arrives, so the binding did not wait for bd. The time bound is a loose sanity check.
    cache = sanity_world(repo)
    cache.write("dev-1", "close", {"reason": "PASS"}, {"status": "in_progress"}, "close:dev-1:1", "dev-1")
    release = repo.bd.block("*")

    with stopwatch() as watch:
        reply = cache.ready("x", fresh_within=LIMIT)

    assert not release.is_set(), "the reply came while bd was still blocked, so it did not wait for bd"
    # That bd was asked at all is the core's to prove (`bounded_fresh_ready_*` in
    # crates/bd-cache/tests/read_ready.rs); a process-start race is not asserted here.
    assert watch.elapsed < 10, f"returned after {watch.elapsed:.2f}s; bd was blocked for up to 20 s"
    assert reply["basis"]["kind"] == "cached"
    assert reply["read_fallback"].startswith("BD_CACHE.BD_UNAVAILABLE")
    assert "san-1" in ready_ids(reply), "the cached answer, adjusted by the journaled close"
    assert reply["locally_adjusted"] is True


def test_ready_with_a_limit_uses_bd_when_it_answers_after_a_release_inside_the_limit(repo):
    cache = sanity_world(repo)
    release = repo.bd.block("blocked")
    repo.bd.edit("dev-1", status="closed")
    seen = {}

    def release_when_bd_is_asked():
        try:
            seen["argv"] = release.wait_started()[0]
        except BaseException as error:
            seen["error"] = error
        finally:
            release.set()

    helper = daemon(release_when_bd_is_asked)
    helper.start()
    reply = cache.ready("x", fresh_within=10)
    join(helper)
    assert "error" not in seen, seen
    assert seen["argv"][0] == "blocked", "the helper saw this verb's own call"
    assert reply["basis"]["kind"] == "fresh" and reply["read_fallback"] is None
    assert "san-1" in ready_ids(reply)


def test_ready_with_a_limit_and_bd_down_falls_back_and_records_the_outage(repo):
    cache = sanity_world(repo)
    repo.bd.down()
    # A down bd fails at once, so a 10 s budget is always met by the failure rather than by the
    # clock: the fallback and the outage record do not depend on how fast this machine is.
    reply = cache.ready("x", fresh_within=10)
    assert reply["basis"]["kind"] == "cached" and reply["read_fallback"]
    assert cache.status()["outage"]["last_error"].startswith("BD_CACHE.BD_UNAVAILABLE")
