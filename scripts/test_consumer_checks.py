"""Product output remains UTF-8 even when the host subprocess locale is CP1252."""

from pathlib import Path
import os
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
        with tempfile.TemporaryDirectory() as temporary:
            pids = Path(temporary) / "pids"
            grandchild = "import time; time.sleep(60)"
            helper = ("import subprocess, sys, time; from pathlib import Path; "
                      f"child=subprocess.Popen([sys.executable, '-I', '-c', {grandchild!r}]); "
                      f"Path({str(pids)!r}).write_text(str(child.pid)); time.sleep(60)")
            parent = ("import subprocess, sys, time; from pathlib import Path; "
                      "assert sys.stdin.buffer.readline() == b'owned-fixture-start\\n'; "
                      f"child=subprocess.Popen([sys.executable, '-I', '-c', {helper!r}]); "
                      f"p=Path({str(pids)!r}); until=time.monotonic()+5; "
                      "exec('while not p.exists() and time.monotonic()<until: time.sleep(0.01)'); "
                      "assert p.exists(); p.write_text(str(child.pid)+' '+p.read_text()); "
                      "raise SystemExit(23)")
            self.assertEqual(consumer_checks.run_fixture([sys.executable, "-I", "-c", parent], timeout=10), 23)
            for pid in map(int, pids.read_text().split()):
                until = time.monotonic() + 5
                while running(pid) and time.monotonic() < until:
                    time.sleep(0.01)
                self.assertFalse(running(pid), f"helper {pid} survived failed fixture")

    def test_watchdog_terminates_owned_child(self):
        command = [sys.executable, "-I", "-c",
                   "import sys,time; sys.stdin.buffer.readline(); time.sleep(60)"]
        with self.assertRaises(subprocess.TimeoutExpired):
            consumer_checks.run_fixture(command, timeout=0.1)


def running(pid):
    if os.name == "nt":
        result = subprocess.check_output(["tasklist", "/FI", f"PID eq {pid}", "/NH", "/FO", "CSV"], text=True, timeout=5)
        return f'"{pid}"' in result
    result = subprocess.run(["/bin/ps", "-o", "stat=", "-p", str(pid)], capture_output=True, text=True, timeout=5)
    return bool(result.stdout.strip()) and not result.stdout.strip().startswith("Z")


if __name__ == "__main__":
    unittest.main()
