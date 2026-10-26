"""The post-sync hook end to end (REQ-BDC-0723, 0727, 0728; acceptance 11).

A real interpreter (this one, through a wrapper that passes the runner's
flags, `-I` included, through unchanged and only appends the test's module
directory to the end of `sys.path`) runs `python -I -m bd_cache_rs.run
post_sync hooks_mod:after_sync` after each `Cache.tick()`, against the fake
bd (which answers label filters: the `All` scope's sync is in the `bdc` end
to end test, `test_cli_e2e.py`, and the Rust fakes). The hook marks one
escalation and one failed scope; the next run offers the rest again, with
`first_offered_at` unchanged. A hostile `bd_cache_rs.py` on the child's
`PYTHONPATH` (where only a missing `-I` would import it) and in the project
dir is never imported.
"""

import json
import os
import signal
import stat
import subprocess
import sys
import threading
import time

import pytest

import bd_cache_rs

HOOK = r'''
import json, os

def after_sync(cache, request):
    print("a chatty hook prints to stdout")
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

def refused(cache, request):
    """Each verb a run may not call, with the reason it is refused for."""
    out = {}
    for name in ("sync_use_builtin", "sync_use_python", "tick", "schedule_show"):
        try:
            getattr(cache, name)()
            out[name] = "ran"
        except Exception as e:
            out[name] = getattr(e, "details", {}).get("reason", type(e).__name__)
    out["scopes"] = [s["scope"] for s in cache.scopes_list()["scopes"]]
    with open(os.environ["HOOK_LOG"], "a", encoding="utf-8") as log:
        log.write(json.dumps(out) + "\n")
    return out

def hangs(cache, request):
    import time
    time.sleep(120)
'''

HOSTILE = "open(__file__ + '.imported', 'w').write('x')\nraise SystemExit('hostile module imported')\n"


def wrapper(tmp_path, module_dir):
    """An interpreter that runs `-m <module>` with the caller's flags as given
    (so `-I` is the runner's, never the wrapper's) and the module directory
    appended to `sys.path`; anything else is passed to Python unchanged."""
    path = tmp_path / "python-wrapper"
    code = ("import runpy, sys; sys.path.append(%r); m = sys.argv[1]; sys.argv = [m] + sys.argv[2:]; "
            "runpy.run_module(m, run_name='__main__', alter_sys=True)") % str(module_dir)
    path.write_text(
        "#!/bin/sh\n"
        'case " $* " in *" -m "*) ;; *) exec "%s" "$@" ;; esac\n'
        'flags=""\n'
        'while [ "$1" != -m ]; do flags="$flags $1"; shift; done\n'
        'mod="$2"; shift 2\n'
        'exec "%s" $flags -c "%s" "$mod" "$@"\n' % (sys.executable, sys.executable, code),
        encoding="utf-8",
    )
    path.chmod(0o755)
    return path


@pytest.fixture
def hooked(repo, tmp_path, monkeypatch):
    modules = tmp_path / "modules"
    modules.mkdir()
    (modules / "hooks_mod.py").write_text(HOOK, encoding="utf-8")
    hostile = tmp_path / "hostile"
    hostile.mkdir()
    (hostile / "bd_cache_rs.py").write_text(HOSTILE, encoding="utf-8")
    (repo.root / "bd_cache_rs.py").write_text(HOSTILE, encoding="utf-8")
    log = tmp_path / "hook.log"
    monkeypatch.setenv("HOOK_LOG", str(log))
    # Passed to the hook with --env: only `-I` keeps it from shadowing the package.
    monkeypatch.setenv("PYTHONPATH", str(hostile))
    repo.config('scopes = ["x"]\n')
    cache = repo.open()
    cache.sync("x", 30)
    os.chmod(cache.path, 0o600)
    return cache, wrapper(tmp_path, modules), log, hostile


def requests(log):
    return [json.loads(line) for line in log.read_text(encoding="utf-8").splitlines()]


def escalate(cache, repo, bead_id, key):
    cache.write(bead_id, "update", {"assignee": "dev-1"}, {"status": "open"}, key, "dev-1")
    repo.bd.edit(bead_id, status="in_progress")


def test_the_hook_gets_every_result_marks_some_and_the_rest_is_offered_again(hooked, repo):
    cache, python, log, hostile = hooked
    cache.scopes_add("every", labels_all=["phase-x"])
    escalate(cache, repo, "b1", "k1")
    escalate(cache, repo, "b2", "k2")
    cache.schedule_set("0 0 1 1 *", post_sync="hooks_mod:after_sync", python=str(python),
                       env=["HOOK_LOG", "PYTHONPATH"])
    first = cache.tick()
    assert first["hook"]["status"] == "ok", first["hook"]
    assert {s["scope"]: s["result"] for s in first["scopes"]} == {"every": "ok", "x": "ok"}
    req1 = requests(log)[0]
    assert req1["protocol"] == "bd-cache.run.v1" and req1["run_id"] == first["run_id"]
    assert req1["repo_root"] == str(repo.root)
    assert [e["seq"] for e in req1["escalated"]] == [1, 2]
    assert first["hook"]["result"] == {"marked": 1}

    repo.bd.fail("list", "bd list exploded")
    second = cache.tick()
    req2 = requests(log)[1]
    assert [e["seq"] for e in req2["escalated"]] == [2], "the marked entry is not offered again"
    assert req2["escalated"][0]["first_offered_at"] == req1["escalated"][1]["first_offered_at"]
    failed = {s["scope"]: s for s in req2["scopes"]}
    assert failed["every"]["result"] == "failed" and failed["every"]["failing_since"]
    error = failed["every"]["error"]
    assert set(error) == {"code", "message", "outage"} and error["outage"] is False, error
    assert second["hook"]["result"] == {"marked": 1}, "the failed scope is marked"

    third = cache.tick()
    req3 = requests(log)[2]
    notified = {s["scope"]: s["notified"] for s in req3["scopes"]}
    assert notified == {"every": True, "x": False}
    assert [e["seq"] for e in req3["escalated"]] == [2]
    assert third["hook"]["status"] == "ok"
    for d in (hostile, repo.root):
        assert not (d / "bd_cache_rs.py.imported").exists(), f"a shadowing module in {d} was imported"


def test_the_hostile_module_is_imported_only_without_minus_i(hooked, tmp_path):
    """Planted negative of the -I check: the same wrapper run without -I, on
    the same PYTHONPATH, imports the hostile module."""
    _, python, _, hostile = hooked
    env = {"PATH": os.environ.get("PATH", ""), "PYTHONPATH": str(hostile)}
    subprocess.run([str(python), "-m", "bd_cache_rs.run", "post_sync", "x:y"], input=b"{}",
                   env=env, cwd=tmp_path, capture_output=True, timeout=60)
    assert (hostile / "bd_cache_rs.py.imported").exists(), "without -I the hostile module is imported"


def test_a_run_refuses_the_lease_and_row_verbs_and_opens_the_runners_repository(hooked, repo):
    """Inside a hook, `sync_use_builtin`, `sync_use_python`, `tick` and
    `schedule_*` raise PRECONDITION `inside_run` (B8Q1-025); the hook's
    cache is the runner's repository, its configured scope included
    (B8Q1-037). Planted negative: outside a run `schedule_show` works."""
    cache, python, log, _ = hooked
    cache.schedule_set("0 0 1 1 *", post_sync="hooks_mod:refused", python=str(python), env=["HOOK_LOG"])
    reply = cache.tick()
    assert reply["hook"]["status"] == "ok", reply["hook"]
    seen = requests(log)[-1]
    for verb in ("sync_use_builtin", "sync_use_python", "tick", "schedule_show"):
        assert seen[verb] == "inside_run", seen
    assert "x" in seen["scopes"], seen
    assert cache.schedule_show()["mode"] == "builtin"


def test_a_ctrl_c_during_a_tick_stops_it_and_is_raised(hooked):
    """B8Q1-058: a KeyboardInterrupt while `tick()` waits on a hook kills the
    hook's tree, frees the lease and is raised when the call returns, long
    before the hook's deadline. Planted negative: the hook's reserve is 20 s."""
    cache, python, _, _ = hooked
    cache.schedule_set("0 0 1 1 *", budget_ms=30000, hook_reserve_ms=20000,
                       post_sync="hooks_mod:hangs", python=str(python))
    timer = threading.Timer(1.5, lambda: os.kill(os.getpid(), signal.SIGINT))
    started = time.monotonic()
    timer.start()
    try:
        with pytest.raises(KeyboardInterrupt):
            cache.tick()
    finally:
        timer.cancel()
    assert time.monotonic() - started < 15
    assert cache.schedule_show()["lease"] is None


def test_the_hook_opens_the_repository_of_a_beads_dir_elsewhere(repo, tmp_path, monkeypatch):
    """B8Q1-037: with BEADS_DIR outside the repository, the hook still opens
    the runner's repository (its `repo_root`), so it sees the configured
    scopes. Planted negative: the beads directory's parent has no config."""
    elsewhere = tmp_path / "elsewhere" / "beads"
    elsewhere.mkdir(parents=True)
    (elsewhere / "metadata.json").write_text((repo.root / ".beads" / "metadata.json").read_text())
    monkeypatch.setenv("BEADS_DIR", str(elsewhere))
    modules = tmp_path / "modules"
    modules.mkdir()
    (modules / "hooks_mod.py").write_text(HOOK, encoding="utf-8")
    log = tmp_path / "hook.log"
    monkeypatch.setenv("HOOK_LOG", str(log))
    repo.config('scopes = ["x"]\n')
    cache = repo.open()
    os.chmod(cache.path, 0o600)
    cache.schedule_set("0 0 1 1 *", post_sync="hooks_mod:refused", python=str(wrapper(tmp_path, modules)),
                       env=["HOOK_LOG"])
    assert cache.tick()["hook"]["status"] == "ok"
    assert "x" in requests(log)[-1]["scopes"]
    assert not (elsewhere.parent / ".beads").exists()


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
