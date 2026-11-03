"""Product output remains UTF-8 even when the host subprocess locale is CP1252."""

from pathlib import Path
import os
import json
import subprocess
import tempfile
import time
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

import consumer_checks  # noqa: E402


class ProductEncodingTest(unittest.TestCase):
    def test_utf8_diagnostic_survives_windows_locale(self):
        diagnostic = "CFG.PARSE invalid JSON — Fix the JSON.\n"
        command = [sys.executable, "-I", "-c",
                   f"import os; os.write(2, {diagnostic.encode('utf-8')!r}); raise SystemExit(1)"]
        # Simulate Python's Windows locale decoder without changing process-global
        # locale or relying on a particular Python UTF-8 mode setting.
        with patch("subprocess._text_encoding", return_value="cp1252"):
            locale_result = subprocess.run(command, capture_output=True, text=True, timeout=10)
            self.assertNotEqual(locale_result.stderr, diagnostic)
            result = consumer_checks.run_product(command, timeout=10)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, "")
        self.assertEqual(result.stderr, diagnostic)
        self.assertTrue(result.stderr.startswith("CFG.PARSE ") and " — " in result.stderr
                        and len(result.stderr.splitlines()) == 1)

    def test_invalid_utf8_is_not_silently_replaced(self):
        command = [sys.executable, "-I", "-c", "import os; os.write(2, b'\\xff')"]
        with self.assertRaises(UnicodeDecodeError):
            consumer_checks.run_product(command, timeout=10)


class GraphIntroducerTest(unittest.TestCase):
    def test_delegates_canonical_introducer_rule_and_cli_exception_is_narrow(self):
        packages = [{"id": n, "name": n, "version": "0.1.1", "source": "registry+crates.io"}
                    for n in ("consumer", "sc-cron", "sc-command", "clap")]
        nodes = [{"id": p["id"], "features": [], "deps": []} for p in packages]
        def edge(source, target):
            nodes[source]["deps"].append({"pkg": packages[target]["id"], "dep_kinds": [{"kind": None}]})
        edge(0, 1); edge(1, 2); edge(0, 3)
        meta = {"packages": packages, "resolve": {"root": "consumer", "nodes": nodes}}
        self.assertEqual(consumer_checks.consumer_graph_errors(meta, "bd-cache-cli"), [])
        self.assertTrue(consumer_checks.consumer_graph_errors(meta, "bd-cache-py"))
        edge(0, 2)
        self.assertTrue(any("consumer -> sc-command" in e
                            for e in consumer_checks.consumer_graph_errors(meta, "bd-cache-cli")))
        packages[0]["name"] = "bd-cache-cli"
        self.assertEqual(consumer_checks.consumer_graph_errors(meta, "bd-cache-cli"), [])
        packages[3]["name"] = "bd-cache-service"
        self.assertTrue(any("bd-cache-service" in e
                            for e in consumer_checks.consumer_graph_errors(meta, "bd-cache-cli")))
        packages[3]["name"] = "clap"
        packages[0]["name"] = "bd-cache-sync"
        self.assertTrue(any("bd-cache-sync -> sc-command" in e
                            for e in consumer_checks.consumer_graph_errors(meta, "bd-cache-cli")))


class FixtureOwnershipTest(unittest.TestCase):
    def test_failed_parent_exit_still_terminates_descendants(self):
        self.separate_group_case(fail=True)

    def test_timeout_after_separate_group_ready_terminates_descendants(self):
        self.separate_group_case(fail=False)

    def separate_group_case(self, *, fail):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            scratch = root / "scratch"
            scratch.mkdir()
            record = root / "topology.json"
            command = [sys.executable, "-I", "-c", topology_program(root, fail)]
            sentinel = subprocess.Popen([sys.executable, "-I", "-c", "import time; time.sleep(60)"],
                                        start_new_session=os.name != "nt")
            original_tree = consumer_checks.FixtureTree
            original_wait = original_tree.wait
            original_remove = consumer_checks.shutil.rmtree
            original_kill = os.kill
            created, removed, signals = [], [], []

            def capture_tree(command):
                tree = original_tree(command)
                created.append(tree)
                return tree

            def wait_after_topology_ready(tree, timeout):
                until = time.monotonic() + 5
                while not record.exists() and time.monotonic() < until:
                    time.sleep(0.01)
                self.assertTrue(record.exists(), "bounded separate-group readiness handshake")
                data = json.loads(record.read_text())
                self.assertTrue(running(data["helper"]["pid"]))
                self.assertTrue(running(data["grandchild"]["pid"]))
                if os.name != "nt":
                    self.assertNotEqual(tree.sid, os.getsid(0))
                    self.assertNotEqual(tree.sid, os.getsid(sentinel.pid))
                    self.assertEqual(os.getsid(tree.child.pid), tree.sid)
                    for role in ("worker", "helper", "grandchild"):
                        self.assertEqual(data[role]["sid"], tree.sid)
                    self.assertEqual(data["worker"]["pgid"], tree.sid)
                    self.assertEqual(data["helper"]["pgid"], data["helper"]["pid"])
                    self.assertNotEqual(data["helper"]["pgid"], data["worker"]["pgid"])
                    self.assertEqual(data["grandchild"]["pgid"], data["helper"]["pgid"])
                    owned_sid = tree.sid
                    try:
                        tree.sid = os.getsid(0)
                        with patch.object(consumer_checks.os, "kill") as kill:
                            with self.assertRaisesRegex(RuntimeError, "owned session anchor"):
                                tree.close()
                            kill.assert_not_called()
                    finally:
                        tree.sid = owned_sid
                # The timeout begins after actual helper/grandchild readiness,
                # rather than hoping an arbitrary launch delay fits in 100ms.
                return original_wait(tree, timeout)

            def checked_kill(pid, signum):
                if os.name != "nt":
                    sid = os.getsid(pid)
                    self.assertEqual(sid, created[0].sid, "signal outside owned fixture session")
                    self.assertNotEqual(pid, sentinel.pid)
                    signals.append((pid, signum))
                return original_kill(pid, signum)

            def checked_remove(path, *args, **kwargs):
                if Path(path) == scratch:
                    data = json.loads(record.read_text())
                    for role in ("helper", "grandchild"):
                        self.assertFalse(running(data[role]["pid"]), f"{role} alive before scratch removal")
                    self.assertIsNone(created[0].child, "direct leader must be reaped last, before removal")
                    self.assertIsNone(sentinel.poll(), "unrelated session must survive cleanup")
                    removed.append(True)
                return original_remove(path, *args, **kwargs)

            try:
                with patch.object(consumer_checks, "FixtureTree", side_effect=capture_tree), \
                     patch.object(original_tree, "wait", wait_after_topology_ready), \
                     patch.object(consumer_checks.shutil, "rmtree", side_effect=checked_remove), \
                     patch.object(consumer_checks.os, "kill", side_effect=checked_kill):
                    if fail:
                        self.assertEqual(consumer_checks.run_fixture(command, timeout=10, scratch_root=scratch), 23)
                    else:
                        with self.assertRaises(subprocess.TimeoutExpired) as raised:
                            consumer_checks.run_fixture(command, timeout=0.1, scratch_root=scratch)
                        self.assertEqual(raised.exception.cmd, command)
                        self.assertEqual(raised.exception.timeout, 0.1)
                self.assertEqual(removed, [True])
                self.assertFalse(scratch.exists())
                self.assertIsNone(sentinel.poll())
                if os.name != "nt":
                    self.assertGreater(len(signals), 2)
                    self.assertEqual(signals[-1][0], created[0].sid, "anchor terminated last")
            finally:
                # Tests own the unrelated sentinel explicitly, outside fixture
                # cleanup. A failing assertion must not leave the sentinel alive.
                sentinel.kill()
                sentinel.wait(timeout=5)
                for tree in created:
                    if tree.child is not None:
                        tree.close()

    def test_watchdog_terminates_owned_child(self):
        command = [sys.executable, "-I", "-c",
                   "import sys,time; sys.stdin.buffer.readline(); time.sleep(60)"]
        with self.assertRaises(subprocess.TimeoutExpired):
            consumer_checks.run_fixture(command, timeout=0.1)



def topology_program(root, fail):
    grandchild_record = str(root / "grandchild.json")
    helper_record = str(root / "helper.json")
    record = str(root / "topology.json")
    identity = "{'pid': os.getpid(), 'pgid': os.getpgid(0), 'sid': os.getsid(0)} if os.name != 'nt' else {'pid': os.getpid()}"
    grandchild = f"""
import json, os, time
from pathlib import Path
p = Path({grandchild_record!r})
pending = p.with_suffix('.pending')
pending.write_text(json.dumps({identity}))
os.replace(pending, p)
time.sleep(60)
"""
    helper = f"""
import json, os, subprocess, sys, time
from pathlib import Path
if os.name != 'nt':
    os.setpgid(0, 0)
child = subprocess.Popen([sys.executable, '-I', '-c', {grandchild!r}])
p = Path({grandchild_record!r})
until = time.monotonic() + 5
while not p.exists() and time.monotonic() < until:
    time.sleep(0.01)
assert p.exists(), 'grandchild readiness deadline'
ready = Path({helper_record!r})
pending = ready.with_suffix('.pending')
pending.write_text(json.dumps({{'helper': {identity}, 'grandchild': json.loads(p.read_text())}}))
os.replace(pending, ready)
time.sleep(60)
"""
    return f"""
import json, os, subprocess, sys, time
from pathlib import Path
assert sys.stdin.buffer.readline() == b'owned-fixture-start\\n'
helper = subprocess.Popen([sys.executable, '-I', '-c', {helper!r}])
p = Path({helper_record!r})
until = time.monotonic() + 5
while not p.exists() and time.monotonic() < until:
    time.sleep(0.01)
assert p.exists(), 'helper readiness deadline'
data = json.loads(p.read_text())
data['worker'] = {identity}
ready = Path({record!r})
pending = ready.with_suffix('.pending')
pending.write_text(json.dumps(data))
os.replace(pending, ready)
if {fail!r}:
    print('fixture-worker-primary-diagnostic', file=sys.stderr)
    raise SystemExit(23)
time.sleep(60)
"""


def running(pid):
    if os.name == "nt":
        result = subprocess.check_output(["tasklist", "/FI", f"PID eq {pid}", "/NH", "/FO", "CSV"], text=True, timeout=5)
        return f'"{pid}"' in result
    result = subprocess.run(["/bin/ps", "-o", "stat=", "-p", str(pid)], capture_output=True, text=True, timeout=5)
    return bool(result.stdout.strip()) and not result.stdout.strip().startswith("Z")


if __name__ == "__main__":
    unittest.main()
