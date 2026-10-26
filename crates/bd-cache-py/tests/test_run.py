"""The post-sync hook end to end (REQ-BDC-0723, 0727, 0728; acceptance 11).

A real interpreter (this one, through a wrapper that only appends the test's
module directory to `sys.path`, since `-I` ignores `PYTHONPATH`) runs
`python -I -m bd_cache_rs.run post_sync hooks_mod:after_sync` after each
`Cache.tick()`, against the fake bd (which answers label filters: the `All`
scope's sync is pinned against the Rust fakes, `crates/bd-cache-sync/tests/
builtin_run.rs`). The hook marks one escalation and one failed scope; the next run offers the rest again, with `first_offered_at`
unchanged. A hostile `bd_cache_rs.py` in the project dir and in the module
dir is never imported.
"""

import json
import os
import stat
import sys

import pytest

import bd_cache_rs

HOOK = r'''
import json, os

def after_sync(cache, request):
    with open(os.environ["HOOK_LOG"], "a", encoding="utf-8") as log:
        log.write(json.dumps(request) + "\n")
    marked = 0
    for entry in request["escalated"]:
        if entry["seq"] == 1:
            cache.manage.journal_announce(seq=1)
            marked += 1
    for scope in request["scopes"]:
        if scope["result"] != "ok" and scope["scope"] == "every" and not scope["notified"]:
            cache.scope_announce(scope["scope"], scope["failing_since"])
            marked += 1
    return {"marked": marked}

def fails(cache, request):
    raise RuntimeError("the hook could not reach its channel")
'''

HOSTILE = "open(__file__ + '.imported', 'w').write('x')\nraise SystemExit('hostile module imported')\n"


def wrapper(tmp_path, module_dir):
    path = tmp_path / "python-wrapper"
    code = ("import runpy, sys; sys.path.append(%r); m = sys.argv[1]; sys.argv = [m] + sys.argv[2:]; "
            "runpy.run_module(m, run_name='__main__', alter_sys=True)") % str(module_dir)
    path.write_text(
        "#!/bin/sh\n"
        'if [ "$2" = -m ]; then mod="$3"; shift 3; exec "%s" -I -c "%s" "$mod" "$@"; fi\n'
        'exec "%s" "$@"\n' % (sys.executable, code, sys.executable),
        encoding="utf-8",
    )
    path.chmod(0o755)
    return path


@pytest.fixture
def hooked(repo, tmp_path, monkeypatch):
    modules = tmp_path / "modules"
    modules.mkdir()
    (modules / "hooks_mod.py").write_text(HOOK, encoding="utf-8")
    (modules / "bd_cache_rs.py").write_text(HOSTILE, encoding="utf-8")
    (repo.root / "bd_cache_rs.py").write_text(HOSTILE, encoding="utf-8")
    log = tmp_path / "hook.log"
    monkeypatch.setenv("HOOK_LOG", str(log))
    repo.config('scopes = ["x"]\n')
    cache = repo.open()
    cache.sync("x", 30)
    os.chmod(cache.path, 0o600)
    return cache, wrapper(tmp_path, modules), log, modules


def requests(log):
    return [json.loads(line) for line in log.read_text(encoding="utf-8").splitlines()]


def escalate(cache, repo, bead_id, key):
    cache.write(bead_id, "update", {"assignee": "dev-1"}, {"status": "open"}, key, "dev-1")
    repo.bd.edit(bead_id, status="in_progress")


def test_the_hook_gets_every_result_marks_some_and_the_rest_is_offered_again(hooked, repo):
    cache, python, log, modules = hooked
    cache.scopes_add("every", labels_all=["phase-x"])
    escalate(cache, repo, "b1", "k1")
    escalate(cache, repo, "b2", "k2")
    cache.schedule_set("0 0 1 1 *", post_sync="hooks_mod:after_sync", python=str(python), env=["HOOK_LOG"])
    first = cache.tick()
    assert first["hook"]["status"] == "ok", first["hook"]
    assert {s["scope"]: s["result"] for s in first["scopes"]} == {"every": "ok", "x": "ok"}
    req1 = requests(log)[0]
    assert req1["protocol"] == "bd-cache.run.v1" and req1["run_id"] == first["run_id"]
    assert [e["seq"] for e in req1["escalated"]] == [1, 2]
    assert first["hook"]["result"] == {"marked": 1}

    repo.bd.fail("list", "bd list exploded")
    second = cache.tick()
    req2 = requests(log)[1]
    assert [e["seq"] for e in req2["escalated"]] == [2], "the marked entry is not offered again"
    assert req2["escalated"][0]["first_offered_at"] == req1["escalated"][1]["first_offered_at"]
    failed = {s["scope"]: s for s in req2["scopes"]}
    assert failed["every"]["result"] == "failed" and failed["every"]["failing_since"]
    assert second["hook"]["result"] == {"marked": 1}, "the failed scope is marked"

    third = cache.tick()
    req3 = requests(log)[2]
    notified = {s["scope"]: s["notified"] for s in req3["scopes"]}
    assert notified == {"every": True, "x": False}
    assert [e["seq"] for e in req3["escalated"]] == [2]
    assert third["hook"]["status"] == "ok"
    for d in (modules, repo.root):
        assert not (d / "bd_cache_rs.py.imported").exists(), f"a shadowing module in {d} was imported"


def test_a_failing_hook_is_tick_run_failed_in_the_reply_and_the_run_is_ok(hooked):
    cache, python, log, _ = hooked
    cache.schedule_set("0 0 1 1 *", post_sync="hooks_mod:fails", python=str(python))
    reply = cache.tick()
    assert reply["outcome"] == "ok"
    error = reply["hook"]["error"]
    assert error["code"] == "BD_CACHE.TICK_RUN_FAILED" and error["details"]["reason"] == "exit"
    assert "could not reach its channel" in error["message"]
    assert cache.schedule_show()["hook"]["consecutive_failures"] == 1


def test_a_group_writable_cache_file_runs_no_hook(hooked):
    cache, python, log, _ = hooked
    cache.schedule_set("0 0 1 1 *", post_sync="hooks_mod:after_sync", python=str(python), env=["HOOK_LOG"])
    os.chmod(cache.path, 0o664)
    reply = cache.tick()
    assert reply["hook"]["error"]["details"]["reason"] == "untrusted"
    assert not log.exists(), "the hook never ran"
