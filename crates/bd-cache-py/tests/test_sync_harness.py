"""`bd_cache_rs.sync.main(func)`: the harness of a consumer's sync script (REQ-BDC-0725)."""

import io
import json

import pytest

import bd_cache_rs
import bd_cache_rs.sync


def run(request, func):
    out = io.StringIO()
    with pytest.raises(SystemExit) as done:
        bd_cache_rs.sync.main(func, stdin=io.StringIO(json.dumps(request)), stdout=out)
    return done.value.code, out.getvalue()


def test_cron_stop_is_answered_continue_and_calls_nothing():
    called = []
    code, out = run({"protocol": "sc-cron.hook.v1", "event": "cron.stop", "payload": {}}, called.append)
    assert (code, json.loads(out), called) == (0, {"action": "continue"}, [])


def test_cron_tick_opens_the_payloads_cache_and_calls_func(cache, repo, capsys):
    path = cache.path
    cache.close()
    seen = []
    payload = {"cache_path": str(path), "beads_dir": str(repo.root / ".beads"), "run_id": "1-2-3"}
    try:
        code, _ = run({"event": "cron.tick", "payload": payload}, lambda c, p: seen.append((c.path, p["run_id"])))
    finally:
        bd_cache_rs._native._set_in_run(False)
    assert code == 0 and seen == [(path, "1-2-3")]


def test_a_func_that_raises_exits_1_with_its_type_and_message_on_stderr(cache, repo, capsys):
    path = cache.path
    cache.close()

    def boom(c, p):
        raise RuntimeError("no rows today")

    payload = {"cache_path": str(path), "beads_dir": str(repo.root / ".beads")}
    try:
        code, _ = run({"event": "cron.tick", "payload": payload}, boom)
    finally:
        bd_cache_rs._native._set_in_run(False)
    assert code == 1
    assert "RuntimeError: no rows today" in capsys.readouterr().err


def test_input_that_is_not_json_or_an_unknown_event_exits_1():
    with pytest.raises(SystemExit) as done:
        bd_cache_rs.sync.main(lambda c, p: None, stdin=io.StringIO("not json"), stdout=io.StringIO())
    assert done.value.code == 1
    assert run({"event": "cron.other"}, lambda c, p: None)[0] == 1
