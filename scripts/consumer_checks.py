#!/usr/bin/env python3
"""Hermetic daemon/CLI acceptance against the existing stateful FakeBd binary.

No installed bd or Dolt is used. Build bdc/fake-bd/fake-python and the daemon,
then pass their absolute paths. The separate live scratch procedure is documented
under docs/development/runtime.md and never runs from this CI check.
"""
from __future__ import annotations
import argparse
import concurrent.futures
import http.client
import json
import os
from pathlib import Path
import signal
import socket
import sqlite3
import subprocess
import tempfile
import time


def run_product(command, **kwargs):
    """Capture the product UTF-8 protocol independently of the host locale."""
    return subprocess.run(command, capture_output=True, text=True, encoding="utf-8", **kwargs)


def wait_for(probe, seconds=15):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        value = probe()
        if value:
            return value
        time.sleep(0.025)
    raise AssertionError("bounded condition timed out")


def isolated_graph(packages):
    """Normal/build graph from independent Cargo roots; workspace union is irrelevant."""
    root = Path(__file__).resolve().parents[1]
    for package in packages:
        with tempfile.TemporaryDirectory(prefix="isolated-artifact-") as temp:
            consumer = Path(temp)
            (consumer / "src").mkdir()
            (consumer / "src/lib.rs").write_text("// independent consumer\n")
            (consumer / "Cargo.toml").write_text(
                '[workspace]\n[package]\nname="artifact-consumer"\nversion="0.0.0"\nedition="2024"\n[dependencies]\n'
                + f'{package} = {{path={json.dumps(str(root / "crates" / package))}}}\n')
            meta = json.loads(subprocess.check_output(["cargo", "metadata", "--format-version", "1", "--manifest-path", str(consumer / "Cargo.toml")], timeout=180))
            nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
            reachable, pending = set(), [meta["resolve"]["root"]]
            while pending:
                identity = pending.pop()
                if identity in reachable:
                    continue
                reachable.add(identity)
                for dep in nodes[identity]["deps"]:
                    if any(kind["kind"] != "dev" for kind in dep["dep_kinds"]):
                        pending.append(dep["pkg"])
            forbidden = {"sc-runtime", "sc-transport", "axum", "rmcp", "bd-cache-daemon", "bd-cache-service"}
            if package != "bd-cache-cli":
                forbidden |= {"clap", "clap_builder"}
            for pkg in meta["packages"]:
                if pkg["id"] not in reachable:
                    continue
                assert pkg["name"] not in forbidden, (package, pkg["name"])
                if pkg["name"] == "sc-command":
                    assert "server" not in nodes[pkg["id"]]["features"], package
                    assert pkg["version"] == "0.1.1" and pkg["source"].startswith("registry+"), pkg
            print(f"{package}: isolated normal/build graph PASS")


def main():
    parser = argparse.ArgumentParser(__doc__)
    parser.add_argument("--graph", action="store_true")
    parser.add_argument("--package", action="append", choices=("bd-cache-api", "bd-cache-py", "bd-cache-cli"))
    for name in ("bdc", "daemon", "fake-bd", "fake-python"):
        parser.add_argument("--" + name, type=Path)
    args = parser.parse_args()
    if args.graph:
        isolated_graph(args.package or ("bd-cache-api", "bd-cache-py", "bd-cache-cli"))
        return
    if any(getattr(args, name) is None for name in ("bdc", "daemon", "fake_bd", "fake_python")):
        parser.error("process checks require --bdc --daemon --fake-bd --fake-python")
    with tempfile.TemporaryDirectory(prefix="daemon-acceptance-") as temp:
        root = Path(temp)
        repo = root / "repo"
        (repo / ".beads").mkdir(parents=True)
        (repo / ".beads/metadata.json").write_text(json.dumps({"dolt_database": "beads", "project_id": "p-1"}))
        (repo / ".beads/bd-cache.toml").write_text('[[scope]]\nname="all"\nall=true\n')
        scenario = root / "scenario.json"
        scenario.write_text(json.dumps({"rows": [{"id": "b-1", "title": "Existing cached row", "status": "open", "priority": 2, "issue_type": "task", "created_at": "2026-10-23T10:00:00Z", "updated_at": "2026-10-23T10:00:00Z"}], "faults": {}}))
        env = {k: v for k, v in os.environ.items() if k in {"SYSTEMROOT", "WINDIR", "SYSTEMDRIVE", "TEMP", "TMP", "TMPDIR"}}
        env["PATH"] = os.pathsep.join((os.path.dirname(os.__file__), "/usr/bin", "/bin")) if os.name != "nt" else os.environ.get("SYSTEMROOT", r"C:\Windows") + r"\System32"
        env["FAKE_BD_SCENARIO"] = str(scenario)
        env["FAKE_PY_DIR"] = str(root)
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            port = sock.getsockname()[1]
        config = root / "daemon.json"
        values = {"endpoint": f"tcp:127.0.0.1:{port}", "instance_root": str(root / "instance"), "bd_binary": str(args.fake_bd), "shutdown_timeout_ms": 150, "logging": {"enableConsoleSink": False, "enableFileSink": True}}
        config.write_text(json.dumps(values))
        command = [str(args.daemon), "--repo", str(repo), "--config", str(config)]

        def bdc(*words, ok=True):
            result = run_product([str(args.bdc), "--repo", str(repo), "--bd", str(args.fake_bd), *words], env=env, timeout=15)
            data = json.loads(result.stdout)
            assert data["ok"] is ok, (words, result.returncode, data, result.stderr)
            return data

        def probe_http(path):
            conn = http.client.HTTPConnection("127.0.0.1", port, timeout=3)
            try:
                conn.request("GET", path)
                response = conn.getresponse()
                return response.status, json.loads(response.read())
            except (OSError, http.client.HTTPException):
                return None
            finally:
                conn.close()

        def spawn():
            return subprocess.Popen(command, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding="utf-8",
                creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if os.name == "nt" else 0)

        def stop(child):
            began = time.monotonic()
            child.send_signal(signal.CTRL_BREAK_EVENT if os.name == "nt" else signal.SIGTERM)
            stdout, stderr = child.communicate(timeout=34)
            elapsed = time.monotonic() - began
            assert child.returncode == 0 and not stdout, (child.returncode, stdout, stderr)
            #31s HTTP drain +2T +1s read +1s process scheduling tolerance.
            assert elapsed <= 31 + 0.3 + 1 + 1, elapsed
            return elapsed

        help_result = run_product([str(args.daemon), "--help"], env=env, timeout=10)
        assert help_result.returncode == 0 and help_result.stdout and not help_result.stderr
        usage_result = run_product([str(args.daemon), "--unknown"], env=env, timeout=10)
        assert usage_result.returncode == 2 and not usage_result.stdout and usage_result.stderr.startswith("CLI.USAGE ")
        # Every refusal keeps stdout empty and no store opens for config/policy/endpoint failures.
        for payload, code in (("{", "CFG.PARSE"), ('{"shutdown_timeout_ms":"secret-value"}', "CFG.DESERIALIZE"), (json.dumps({**values, "endpoint": "tcp:0.0.0.0:1"}), "DAEMON.LISTENER_POLICY"), (json.dumps({**values, "endpoint": "bad-endpoint"}), "RUNTIME.ENDPOINT_FAILED")):
            config.write_text(payload)
            result = run_product(command, env=env, timeout=10)
            assert result.returncode == 1 and not result.stdout, result
            assert result.stderr.startswith(code + " ") and " — " in result.stderr and len(result.stderr.splitlines()) == 1, result.stderr
            assert "secret-value" not in result.stderr
            assert not (repo / ".bd-cache").exists(), result.stderr
        config.unlink()
        result = run_product(command, env=env, timeout=10)
        assert result.returncode == 1 and result.stderr.startswith("CFG.READ ") and not result.stdout
        config.write_text(json.dumps(values))
        bdc("cache", "schedule", "set", "* * * * * *")
        db = repo / ".bd-cache/beads.bd-cache.sqlite3"
        with sqlite3.connect(db) as conn:
            assert conn.execute("pragma user_version").fetchone()[0] == 3
            before = conn.execute("select count(*) from sc_cron_schedule").fetchone()[0]
        bdc("cache", "refresh", "scopes")
        assert bdc("issues", "get", "b-1", "--miss-timeout-ms", "0")["data"]["title"] == "Existing cached row"
        initial_identity = (db.stat().st_dev, db.stat().st_ino)
        child = spawn()
        try:
            wait_for(lambda: probe_http("/health"))
            duplicate = run_product(command, env=env, timeout=10)
            assert duplicate.returncode == 1 and duplicate.stderr.startswith("RUNTIME.LOCK_HELD ") and not duplicate.stdout, duplicate
            wait_for(lambda: (reply := probe_http("/ready")) and reply[0] == 200)
            assert bdc("cache", "status")["data"]["store_schema_version"] == 3
            bdc("issues", "update", "b-1", "--status", "in_progress", "--actor", "acceptance", "--idempotency-key", "acceptance-update")
            assert bdc("issues", "get", "b-1", "--miss-timeout-ms", "0")["data"]["status"] == "in_progress"
            bdc("cache", "flush")
            # Backend is blocked after recorded outage; readiness still evaluates cached health.
            changed = json.loads(scenario.read_text()); changed["faults"] = {"unavailable": True}; scenario.write_text(json.dumps(changed))
            wait_for(lambda: (reply := probe_http("/ready")) and reply[0] == 503 and (reply[1].get("data") or {}).get("state") == "down")
            gate = root / "never-release"
            changed["faults"] = {"gate_file": str(gate)}; scenario.write_text(json.dumps(changed))
            with concurrent.futures.ThreadPoolExecutor(max_workers=12) as pool:
                replies = list(pool.map(lambda _: probe_http("/ready"), range(24)))
            assert all(r and r[0] == 503 for r in replies), replies
            assert any((r[1].get("data") or {}).get("state") == "down" for r in replies if r[1].get("data")), replies
            changed["faults"] = {}; scenario.write_text(json.dumps(changed))
            wait_for(lambda: (reply := probe_http("/ready")) and reply[0] == 200)
            assert probe_http("/health")[0] == 200
            first_stop = stop(child)
            child = None
            # Restart and schedule/identity preservation, with independent local reads.
            child = spawn(); wait_for(lambda: probe_http("/health")); wait_for(lambda: (r := probe_http("/ready")) and r[0] == 200)
            assert bdc("cache", "health")["data"]["state"] == "ok"
            assert (db.stat().st_dev, db.stat().st_ino) == initial_identity
            assert bdc("issues", "get", "b-1", "--miss-timeout-ms", "0")["data"]["status"] == "in_progress"
            with sqlite3.connect(db) as conn:
                assert conn.execute("pragma user_version").fetchone()[0] == 3
                assert conn.execute("select count(*) from sc_cron_schedule").fetchone()[0] == before
            second_stop = stop(child); child = None
            bdc("cache", "health")
            # Bind failure still executes application cleanup and permits immediate restart.
            with socket.socket() as occupied:
                occupied.bind(("127.0.0.1", port)); occupied.listen()
                result = run_product(command, env=env, timeout=10)
                assert result.returncode == 1 and result.stderr.startswith("RUNTIME.BIND_FAILED ") and not result.stdout, result
            child = spawn(); wait_for(lambda: probe_http("/health")); stop(child); child = None
            # Stop with a blocked hook process tree. The second owner must fail
            # during worker grace, and only succeed after both children are reaped.
            values["shutdown_timeout_ms"] = 1000
            config.write_text(json.dumps(values))
            bdc("cache", "schedule", "set", "* * * * * *", "--budget-ms", "30000", "--hook-reserve-ms", "20000", "--post-sync", "consumer.hooks:after", "--python", str(args.fake_python), "--env", "FAKE_PY_DIR")
            (root / "hook.mode").write_text("grandchild")
            child = spawn(); wait_for(lambda: probe_http("/health"))
            pids = [int(wait_for(lambda p=p: p.read_text().strip() if p.exists() else None)) for p in (root / "child.pid", root / "gc.pid")]
            began = time.monotonic()
            child.send_signal(signal.CTRL_BREAK_EVENT if os.name == "nt" else signal.SIGTERM)
            duplicate = run_product(command, env=env, timeout=10)
            assert duplicate.returncode == 1 and duplicate.stderr.startswith("RUNTIME.LOCK_HELD "), duplicate
            stdout, stderr = child.communicate(timeout=35)
            blocked_stop = time.monotonic() - began
            assert child.returncode == 0 and not stdout, (child.returncode, stdout, stderr)
            assert 0.8 <= blocked_stop <= 31 + 2 + 1 + 1, blocked_stop
            child = None
            def running(pid):
                if os.name == "nt":
                    output = subprocess.check_output(["tasklist", "/FI", f"PID eq {pid}", "/NH", "/FO", "CSV"], text=True, timeout=5)
                    return f'"{pid}"' in output
                result = subprocess.run(["/bin/ps", "-o", "stat=", "-p", str(pid)], capture_output=True, text=True, timeout=5)
                return bool(result.stdout.strip()) and not result.stdout.strip().startswith("Z")
            wait_for(lambda: all(not running(pid) for pid in pids), seconds=5)
            (root / "hook.mode").unlink()
            child = spawn(); wait_for(lambda: probe_http("/health")); stop(child); child = None
            print(json.dumps({"status": "PASS", "scheduled_sync": True, "outage_recovery": True, "readiness_without_backend": True, "stop_seconds": [first_stop, second_stop, blocked_stop], "blocked_hook_tree_reaped": True, "schema": 3, "duplicate_and_bind_cleanup": True}, sort_keys=True))
        except Exception:
            print("readiness at failure:", probe_http("/ready"))
            print("schedule at failure:", bdc("cache", "schedule", "show"))
            for log in (root / "instance").rglob("*.jsonl"):
                print(log.read_text()[-6000:])
            raise
        finally:
            if child is not None and child.poll() is None:
                child.kill(); child.communicate(timeout=10)


if __name__ == "__main__":
    main()
