"""`bdc` end to end with a real interpreter (REQ-BDC-0720, 0723, 0724, 0725; acceptance 11).

The `bdc` binary runs against the `fake-bd` binary (both built from this
workspace: `BD_CACHE_BDC` and `BD_CACHE_FAKE_BD` name them; CI's python
workflow builds them beside the wheel). The post-sync hook and the sync
script are real Python (this interpreter, with the installed wheel), run
with `-I` through a wrapper that passes the runner's flags unchanged:

- `bdc cache tick` syncs a label scope and an `All` scope, and a label scope
  bd refuses; the hook receives every result, marks the failed scope, and the
  next run shows it notified;
- `bdc cache sync use-python` with a script written with
  `bd_cache_rs.sync.main` that ingests one scope and exits 0; the next read
  sees its row; `bdc cache cron run --once` runs it again;
- a script that exits 1 is rolled back with `TICK_RUN_FAILED`.

Skipped when either binary is not given.
"""

import json
import os
import subprocess
import sys
import time
from pathlib import Path

import pytest

BDC = os.environ.get("BD_CACHE_BDC")
FAKE_BD = os.environ.get("BD_CACHE_FAKE_BD")

pytestmark = [
    pytest.mark.skipif(not (BDC and FAKE_BD), reason="BD_CACHE_BDC and BD_CACHE_FAKE_BD name the binaries"),
    pytest.mark.skipif(sys.platform == "win32", reason="the stub wrapper is a shell script"),
]

T0 = "2026-10-23T09:00:00Z"

HOOK = r'''
import json, os

def after_sync(cache, request):
    marked = []
    for scope in request["scopes"]:
        if scope["result"] == "failed" and not scope["notified"]:
            cache.scope_announce(scope["scope"], scope["failing_since"])
            marked.append(scope["scope"])
    with open(os.environ["HOOK_LOG"], "a", encoding="utf-8") as log:
        log.write(json.dumps({"request": request, "marked": marked}) + "\n")
    return {"marked": marked}
'''

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


def row(bead_id, labels):
    return {"id": bead_id, "title": bead_id, "status": "open", "priority": 2, "issue_type": "task",
            "created_at": T0, "updated_at": T0, "labels": labels}


class World:
    def __init__(self, tmp_path):
        self.root = tmp_path / "repo"
        beads = self.root / ".beads"
        beads.mkdir(parents=True)
        (beads / "metadata.json").write_text(json.dumps({"dolt_database": "beads", "project_id": "p-1"}))
        self.scenario = tmp_path / "scenario.json"
        self.scenario.write_text(json.dumps({
            "now": "2026-10-23T10:00:00Z",
            "rows": [row("b1", ["area"]), row("b2", ["other"]), row("b3", ["broken"])],
            "faults": {"fail_labels": ["broken"]},
        }))
        self.log = tmp_path / "hook.log"
        modules = tmp_path / "modules"
        modules.mkdir()
        (modules / "consumer_hooks.py").write_text(HOOK, encoding="utf-8")
        (modules / "consumer_sync.py").write_text(SCRIPT, encoding="utf-8")
        (modules / "consumer_failing.py").write_text(FAILING, encoding="utf-8")
        code = ("import runpy, sys; sys.path.append(%r); m = sys.argv[1]; sys.argv = [m] + sys.argv[2:]; "
                "runpy.run_module(m, run_name='__main__', alter_sys=True)") % str(modules)
        self.python = tmp_path / "python-wrapper"
        self.python.write_text(
            "#!/bin/sh\n"
            'case " $* " in *" -m "*) ;; *) exec "%s" "$@" ;; esac\n'
            'flags=""\n'
            'while [ "$1" != -m ]; do flags="$flags $1"; shift; done\n'
            'mod="$2"; shift 2\n'
            'exec "%s" $flags -c "%s" "$mod" "$@"\n' % (sys.executable, sys.executable, code),
            encoding="utf-8",
        )
        self.python.chmod(0o755)

    def bdc(self, *args):
        env = {"PATH": "/usr/bin:/bin", "FAKE_BD_SCENARIO": str(self.scenario), "HOOK_LOG": str(self.log),
               "TMPDIR": os.environ.get("TMPDIR", "/tmp")}
        out = subprocess.run([BDC, "--repo", str(self.root), "--bd", FAKE_BD, *args],
                             env=env, capture_output=True, timeout=120)
        return out.returncode, json.loads(out.stdout)

    def hook_calls(self):
        return [json.loads(line) for line in self.log.read_text(encoding="utf-8").splitlines()]


@pytest.fixture
def world(tmp_path):
    w = World(tmp_path)
    for args in (("cache", "scopes", "add", "lbl", "--labels-all", "area"),
                 ("cache", "scopes", "add", "every", "--all"),
                 ("cache", "scopes", "add", "broken", "--labels-all", "broken"),
                 ("cache", "scopes", "add", "mine", "--all")):
        code, v = w.bdc(*args)
        assert code == 0, v
    cache_file = next((w.root / ".bd-cache").glob("*.sqlite3"))
    os.chmod(cache_file, 0o600)
    return w


def test_bdc_tick_syncs_a_label_and_an_all_scope_and_the_hook_marks_a_failed_scope(world):
    code, v = world.bdc("cache", "schedule", "set", "0 0 1 1 *", "--post-sync", "consumer_hooks:after_sync",
                        "--python", str(world.python), "--env", "HOOK_LOG")
    assert code == 0, v
    code, first = world.bdc("cache", "tick")
    assert code == 0, first
    results = {s["scope"]: s["result"] for s in first["data"]["scopes"]}
    assert results["lbl"] == "ok" and results["every"] == "ok" and results["broken"] == "failed", results
    assert first["data"]["hook"]["status"] == "ok", first["data"]["hook"]
    call = world.hook_calls()[0]
    seen = {s["scope"]: s for s in call["request"]["scopes"]}
    assert set(seen) >= {"lbl", "every", "broken"}
    assert seen["broken"]["error"]["outage"] is False
    assert call["marked"] == ["broken"]
    code, _ = world.bdc("cache", "tick")
    second = world.hook_calls()[1]
    assert {s["scope"]: s["notified"] for s in second["request"]["scopes"]}["broken"] is True
    assert second["marked"] == [], "a marked failure is not offered as unmarked again"


def test_bdc_use_python_runs_the_script_then_cron_run_once_reruns_it_and_a_failure_rolls_back(world):
    assert world.bdc("cache", "schedule", "set", "* * * * * *")[0] == 0
    code, v = world.bdc("cache", "sync", "use-python", "--python", str(world.python), "--module", "consumer_sync")
    assert code == 0, v
    assert v["data"]["exit_status"] == 0 and v["data"]["run_id"]
    code, show = world.bdc("cache", "schedule", "show")
    assert show["data"]["mode"] == "python"
    assert show["data"]["rows"]["python"]["run_id"] == v["data"]["run_id"]
    code, items = world.bdc("issues", "list", "--scope", "mine")
    assert [i["id"] for i in items["data"]["items"]] == ["p1"], items
    time.sleep(1.1)
    code, once = world.bdc("cache", "cron", "run", "--once")
    assert code == 0, once
    assert [r["row"] for r in once["data"]["ran"]] == ["bd-cache.sync.python"]
    code, show = world.bdc("cache", "schedule", "show")
    assert show["data"]["rows"]["python"]["run_id"] != v["data"]["run_id"], "a new run"

    assert world.bdc("cache", "sync", "use-builtin")[0] == 0
    code, e = world.bdc("cache", "sync", "use-python", "--python", str(world.python), "--module", "consumer_failing")
    assert (code, e["error"]["code"]) == (3, "BD_CACHE.TICK_RUN_FAILED"), e
    assert (e["error"]["details"]["reason"], e["error"]["details"]["restored"]) == ("exit", True)
    assert "the source is down" in e["error"]["details"]["stderr"]
    assert world.bdc("cache", "schedule", "show")[1]["data"]["mode"] == "builtin", "rolled back"
