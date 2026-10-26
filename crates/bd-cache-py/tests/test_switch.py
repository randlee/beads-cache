"""The switch to a consumer's Python sync and back (REQ-BDC-0724, 0725; acceptance 11).

The script is written with `bd_cache_rs.sync.main` and ingests one scope; it
runs under a real interpreter (a wrapper that only appends the module
directory to `sys.path`). With the Python row enabled, `tick()` is
PRECONDITION and no `bd list` runs.
"""

import os
import sys

import pytest

import bd_cache_rs

SCRIPT = r'''
import bd_cache_rs.sync

ROW = {"id": "p1", "title": "from the script", "priority": 2, "status": "open",
       "issue_type": "task", "created_at": "2026-10-23T09:00:00Z", "updated_at": "2026-10-23T09:00:00Z"}

def sync(cache, payload):
    cache.ingest_scope("mine", [ROW], [], revision=payload["run_id"])

if __name__ == "__main__":
    bd_cache_rs.sync.main(sync)
'''

FAILING = r'''
import bd_cache_rs.sync

def sync(cache, payload):
    raise RuntimeError("the source is down")

if __name__ == "__main__":
    bd_cache_rs.sync.main(sync)
'''


def raised(call, *args, **kwargs):
    with pytest.raises(bd_cache_rs.CacheError) as caught:
        call(*args, **kwargs)
    return caught.value


@pytest.fixture
def scripted(repo, tmp_path):
    modules = tmp_path / "modules"
    modules.mkdir()
    (modules / "consumer_sync.py").write_text(SCRIPT, encoding="utf-8")
    (modules / "consumer_failing.py").write_text(FAILING, encoding="utf-8")
    code = ("import runpy, sys; sys.path.append(%r); m = sys.argv[1]; sys.argv = [m] + sys.argv[2:]; "
            "runpy.run_module(m, run_name='__main__', alter_sys=True)") % str(modules)
    python = tmp_path / "python-wrapper"
    python.write_text(
        "#!/bin/sh\n"
        'if [ "$2" = -m ]; then mod="$3"; shift 3; exec "%s" -I -c "%s" "$mod" "$@"; fi\n'
        'exec "%s" "$@"\n' % (sys.executable, code, sys.executable),
        encoding="utf-8",
    )
    python.chmod(0o755)
    repo.config('scopes = ["x"]\n')
    cache = repo.open()
    os.chmod(cache.path, 0o600)
    cache.scopes_add("mine", all=True)
    cache.schedule_set("0 0 1 1 *")
    return cache, python


def test_use_python_runs_the_script_at_once_and_use_builtin_reverses_it(scripted, repo):
    cache, python = scripted
    reply = cache.sync_use_python(python=str(python), module="consumer_sync")
    assert reply["exit_status"] == 0 and reply["run_id"]
    assert cache.schedule_show()["mode"] == "python"
    assert [i["id"] for i in cache.beads.issues_list(scope="mine")["items"]] == ["p1"], "the next read sees it"
    calls = len(repo.bd.calls())
    e = raised(cache.tick)
    assert (e.code, e.details["reason"]) == ("BD_CACHE.PRECONDITION", "python_sync_enabled")
    assert len(repo.bd.calls()) == calls, "no bd process"
    cache.sync_use_builtin()
    assert cache.schedule_show()["mode"] == "builtin"
    assert cache.tick()["outcome"] == "ok"


def test_a_failing_script_is_rolled_back_with_tick_run_failed(scripted):
    cache, python = scripted
    e = raised(cache.sync_use_python, python=str(python), module="consumer_failing")
    assert e.code == "BD_CACHE.TICK_RUN_FAILED"
    assert (e.details["reason"], e.details["restored"]) == ("exit", True)
    assert "the source is down" in e.details["stderr"]
    show = cache.schedule_show()
    assert show["mode"] == "builtin" and show["rows"]["python"]["enabled"] is False


def test_an_interpreter_that_is_not_absolute_or_cannot_import_is_schedule_invalid(scripted, tmp_path):
    cache, _ = scripted
    e = raised(cache.sync_use_python, python="python3", module="consumer_sync")
    assert (e.code, e.details["reason"]) == ("BD_CACHE.SCHEDULE_INVALID", "not_absolute")
    bad = tmp_path / "noimport"
    bad.write_text("#!/bin/sh\nexit 1\n", encoding="utf-8")
    bad.chmod(0o755)
    e = raised(cache.sync_use_python, python=str(bad), module="consumer_sync")
    assert e.details["reason"] == "import_failed"
    assert cache.schedule_show()["mode"] == "builtin", "nothing changed"
