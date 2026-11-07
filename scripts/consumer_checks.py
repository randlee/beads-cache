#!/usr/bin/env python3
"""Hermetic daemon/CLI acceptance against the existing stateful FakeBd binary.

No installed bd or Dolt is used. Build bdc/fake-bd/fake-python and the daemon,
then pass their absolute paths. The separate live scratch procedure is documented
under docs/development/runtime.md and never runs from this CI check.
"""
from __future__ import annotations
import argparse
import importlib.util
import concurrent.futures
import contextlib
import errno
import http.client
import json
import os
from pathlib import Path
import signal
import shutil
import socket
import sqlite3
import subprocess
import sys
import tempfile
import time


def run_product(command, **kwargs):
    """Capture the product UTF-8 protocol independently of the host locale."""
    result = subprocess.run(command, capture_output=True, **kwargs)
    # Windows subprocess reader threads cannot propagate decoder exceptions to
    # communicate(). Own strict decoding on this caller thread instead.
    result.stdout = result.stdout.decode("utf-8")
    result.stderr = result.stderr.decode("utf-8")
    return result


def wait_for(probe, seconds=15):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        value = probe()
        if value:
            return value
        time.sleep(0.025)
    raise AssertionError("bounded condition timed out")


def child_bound_endpoint(child, log, offset):
    """Observe this child's public listener event, never a freed reservation."""
    assert child.poll() is None, ("daemon exited before bound observation", child.returncode)
    try:
        with log.open("rb") as stream:
            stream.seek(offset)
            data = stream.read(1024 * 1024 + 1)
    except FileNotFoundError:
        return None
    assert len(data) <= 1024 * 1024, "bounded startup log observation"
    for line in data.splitlines(keepends=True):
        if not line.endswith(b"\n"):
            continue
        event = json.loads(line)
        if (event.get("service") == "bd-cache"
                and event.get("action") == "daemon.listener_bound"
                and event.get("identity", {}).get("pid") == child.pid):
            endpoint = event["fields"]["endpoint"]
            assert endpoint.startswith("tcp:127.0.0.1:"), endpoint
            port = int(endpoint.removeprefix("tcp:127.0.0.1:"))
            assert 0 < port <= 65535, endpoint
            assert child.poll() is None, "bound owner exited"
            return port
    return None


def consumer_graph_errors(meta, package):
    """Delegate provenance and introducer policy to the canonical consumer gate."""
    path = Path(__file__).resolve().parents[1] / "crates/bd-cache-sync/tests/check_consumer_graph.py"
    spec = importlib.util.spec_from_file_location("canonical_consumer_graph", path)
    graph = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(graph)
    errors = graph.violations(meta)
    if package == "bd-cache-cli":
        # The existing CLI owns its direct command-contract dependency. This
        # exception does not permit sync (below Python too) to introduce it.
        errors = [e for e in errors if e not in {
            "isolated consumer contains clap", "isolated consumer contains clap_builder",
            "bd-cache-cli -> sc-command: only an approved same-source sc-cron may introduce the contract/logging exception"}]
    packages = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    reached, pending = set(), [meta["resolve"]["root"]]
    while pending:
        identity = pending.pop()
        if identity in reached:
            continue
        reached.add(identity)
        if packages[identity]["name"] in {"bd-cache-service", "bd-cache-daemon"}:
            errors.append("isolated consumer contains " + packages[identity]["name"])
        pending.extend(edge["pkg"] for edge in nodes[identity]["deps"]
                       if any(k["kind"] in (None, "build") for k in edge["dep_kinds"]))
    return errors


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
            errors = consumer_graph_errors(meta, package)
            assert not errors, "\n".join(errors)
            print(f"{package}: isolated normal/build graph PASS")


# A POSIX session leader remains alive and unreaped after the worker reports its
# result. This anchors the SID until every hook PGID in that session is stopped.
_SESSION_OWNER = r'''
import json
import os
from pathlib import Path
import subprocess
import sys
import traceback

status = Path(sys.argv[1])
if sys.stdin.buffer.readline() != b"owned-fixture-start\n":
    raise SystemExit("session ownership handshake missing")
try:
    worker = subprocess.Popen(sys.argv[2:], stdin=subprocess.PIPE)
    worker.stdin.write(b"owned-fixture-start\n")
    worker.stdin.close()
    code = worker.wait()
except BaseException:
    traceback.print_exc()
    code = 1
pending = status.with_suffix(".pending")
pending.write_text(json.dumps(code), encoding="utf-8")
os.replace(pending, status)
# The supervisor keeps this pipe open and reaps this leader only after cleanup.
sys.stdin.buffer.read()
'''


def remaining(deadline):
    left = deadline - time.monotonic()
    if left <= 0:
        raise TimeoutError("owned fixture session cleanup exceeded 10s")
    return left


class FixtureTree:
    """Own all fixture descendants, even after their direct parent has exited."""
    def __init__(self, command):
        self.job = None
        self.child = None
        self.control = None
        self.command = command
        self.sid = None
        if os.name == "nt":
            import ctypes
            from ctypes import wintypes
            self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
            self.kernel.CreateJobObjectW.restype = wintypes.HANDLE
            self.kernel.CreateJobObjectW.argtypes = (ctypes.c_void_p, wintypes.LPCWSTR)
            self.kernel.AssignProcessToJobObject.argtypes = (wintypes.HANDLE, wintypes.HANDLE)
            self.kernel.TerminateJobObject.argtypes = (wintypes.HANDLE, wintypes.UINT)
            self.kernel.QueryInformationJobObject.argtypes = (wintypes.HANDLE, ctypes.c_int,
                                                              ctypes.c_void_p, wintypes.DWORD, ctypes.c_void_p)
            self.kernel.CloseHandle.argtypes = (wintypes.HANDLE,)
            self.kernel.SetInformationJobObject.argtypes = (wintypes.HANDLE, ctypes.c_int,
                                                            ctypes.c_void_p, wintypes.DWORD)
            class BasicLimits(ctypes.Structure):
                _fields_ = [("process_time", ctypes.c_int64), ("job_time", ctypes.c_int64),
                            ("flags", wintypes.DWORD), ("minimum_working_set", ctypes.c_size_t),
                            ("maximum_working_set", ctypes.c_size_t), ("active_limit", wintypes.DWORD),
                            ("affinity", ctypes.c_size_t), ("priority", wintypes.DWORD),
                            ("scheduling_class", wintypes.DWORD)]
            class ExtendedLimits(ctypes.Structure):
                _fields_ = [("basic", BasicLimits), ("io_counters", ctypes.c_uint64 * 6),
                            ("process_memory", ctypes.c_size_t), ("job_memory", ctypes.c_size_t),
                            ("peak_process_memory", ctypes.c_size_t), ("peak_job_memory", ctypes.c_size_t)]
            self.job = self.kernel.CreateJobObjectW(None, None)
            if not self.job:
                raise ctypes.WinError(ctypes.get_last_error())
            limits = ExtendedLimits()
            limits.basic.flags = 0x2000  # JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            if not self.kernel.SetInformationJobObject(self.job, 9, ctypes.byref(limits), ctypes.sizeof(limits)):
                failure = ctypes.WinError(ctypes.get_last_error())
                self.kernel.CloseHandle(self.job)
                self.job = None
                raise failure
        try:
            if os.name != "nt":
                self.control = tempfile.TemporaryDirectory(prefix="fixture-owner-")
                self.status = Path(self.control.name) / "worker-status.json"
                command = [sys.executable, "-I", "-c", _SESSION_OWNER, str(self.status), *command]
            self.child = subprocess.Popen(command, stdin=subprocess.PIPE,
                                          start_new_session=os.name != "nt")
            if os.name != "nt":
                self.sid = self.child.pid
                if os.getsid(self.child.pid) != self.sid or self.sid == os.getsid(0):
                    raise RuntimeError("fixture did not establish a private session")
            if self.job and not self.kernel.AssignProcessToJobObject(self.job, int(self.child._handle)):
                import ctypes
                failure = ctypes.WinError(ctypes.get_last_error())
                self.child.kill()
                raise failure
            # Child has not launched any descendants before assignment/session
            # ownership. No process discovery race or PID-file dependence.
            self.child.stdin.write(b"owned-fixture-start\n")
            self.child.stdin.flush()
            if os.name == "nt":
                self.child.stdin.close()
        except BaseException:
            self.close()
            raise

    def close(self):
        if self.child is None:
            if self.job:
                self.kernel.CloseHandle(self.job)
                self.job = None
            return
        if self.job:
            if not self.kernel.TerminateJobObject(self.job, 1):
                import ctypes
                raise ctypes.WinError(ctypes.get_last_error())
            import ctypes
            class Accounting(ctypes.Structure):
                _fields_ = [(name, ctypes.c_int64) for name in (
                    "user_time", "kernel_time", "period_user_time", "period_kernel_time")] + [
                    (name, ctypes.c_uint32) for name in (
                    "page_faults", "total_processes", "active_processes", "terminated_processes")]
            accounting = Accounting()
            until = time.monotonic() + 10
            while True:
                if not self.kernel.QueryInformationJobObject(self.job, 1, ctypes.byref(accounting),
                                                              ctypes.sizeof(accounting), None):
                    raise ctypes.WinError(ctypes.get_last_error())
                if accounting.active_processes == 0:
                    break
                if time.monotonic() >= until:
                    raise RuntimeError("owned fixture job did not reap all descendants in 10s")
                time.sleep(0.01)
            self.kernel.CloseHandle(self.job)
            self.job = None
        else:
            self.close_session(time.monotonic() + 10)
            self.child = None
            self.control.cleanup()
            self.control = None
            return
        self.child.wait(timeout=10)
        self.child = None

    def wait(self, timeout):
        if os.name == "nt":
            return self.child.wait(timeout=timeout)
        deadline = time.monotonic() + timeout
        while True:
            if self.status.exists():
                return json.loads(self.status.read_text(encoding="utf-8"))
            if time.monotonic() >= deadline:
                raise subprocess.TimeoutExpired(self.command, timeout)
            # Do not poll/wait the leader here: its unreaped identity anchors
            # the session even if the keeper itself exits unexpectedly.
            time.sleep(max(0, min(0.01, deadline - time.monotonic())))

    def session_members(self, deadline):
        snapshot = subprocess.run(["/bin/ps", "-axo", "pid=,stat="],
                                  capture_output=True, text=True,
                                  timeout=min(5, remaining(deadline)), check=True)
        members = []
        for line in snapshot.stdout.splitlines():
            remaining(deadline)
            parts = line.split()
            if len(parts) != 2 or parts[1].startswith("Z"):
                continue
            pid = int(parts[0])
            if pid == self.child.pid:
                continue
            try:
                if os.getsid(pid) == self.sid:
                    members.append(pid)
            except (ProcessLookupError, PermissionError):
                continue
        return members

    def signal_member(self, pid, signum, deadline):
        remaining(deadline)
        # A group can change or be reparented during cleanup. Signal only an
        # individually reverified member of this anchored SID, never a name,
        # arbitrary descendant PID, the caller session or a reused foreign PGID.
        if self.sid == os.getsid(0) or pid == self.child.pid:
            raise RuntimeError("refusing unsafe fixture session signal")
        try:
            if os.getsid(pid) == self.sid:
                os.kill(pid, signum)
        except ProcessLookupError:
            pass

    def close_session(self, deadline):
        if self.sid == os.getsid(0) or os.getsid(self.child.pid) != self.sid:
            raise RuntimeError("refusing cleanup without the owned session anchor")
        # Freeze the anchor too: it must not launch a worker between the last
        # empty inventory and its own final termination.
        os.kill(self.child.pid, signal.SIGSTOP)
        while members := self.session_members(deadline):
            # Freeze known members before rescanning, so normal forking cannot
            # keep outrunning cleanup; new hook PGIDs remain inside the SID.
            for pid in members:
                self.signal_member(pid, signal.SIGSTOP, deadline)
            for pid in self.session_members(deadline):
                self.signal_member(pid, signal.SIGKILL, deadline)
            time.sleep(min(0.01, remaining(deadline)))
        # Reap the session leader last. Until here, its PID/SID cannot be reused
        # for an unrelated session even when the acceptance worker has exited.
        if os.getsid(self.child.pid) != self.sid:
            raise RuntimeError("fixture session anchor changed during cleanup")
        self.child.kill()
        self.child.wait(timeout=remaining(deadline))
        self.child.stdin.close()


def run_fixture(command, timeout=300, *, scratch_root=None):
    tree = FixtureTree(command)
    primary = None
    code = None
    try:
        code = tree.wait(timeout=timeout)
        return code
    except BaseException as error:
        primary = error
        raise
    finally:
        closed = False
        try:
            tree.close()
            closed = True
        except BaseException as cleanup_error:
            print(f"owned fixture cleanup failed: {cleanup_error!r}", file=sys.stderr)
            if primary is None and not code:
                raise
        finally:
            if scratch_root is not None:
                if closed:
                    try:
                        shutil.rmtree(scratch_root)
                    except BaseException as scratch_error:
                        print(f"fixture scratch cleanup failed: {scratch_error!r}", file=sys.stderr)
                        if primary is None and not code:
                            raise
                else:
                    print(f"fixture scratch retained after cleanup failure: {scratch_root}", file=sys.stderr)

def main():
    parser = argparse.ArgumentParser(__doc__)
    parser.add_argument("--graph", action="store_true")
    parser.add_argument("--fixture-root", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("--package", action="append", choices=("bd-cache-api", "bd-cache-py", "bd-cache-cli"))
    for name in ("bdc", "daemon", "fake-bd", "fake-python"):
        parser.add_argument("--" + name, type=Path)
    args = parser.parse_args()
    if args.graph:
        isolated_graph(args.package or ("bd-cache-api", "bd-cache-py", "bd-cache-cli"))
        return
    if any(getattr(args, name) is None for name in ("bdc", "daemon", "fake_bd", "fake_python")):
        parser.error("process checks require --bdc --daemon --fake-bd --fake-python")
    # The supervisor removes scratch only after terminating/reaping the fixture
    # tree; worker exception cleanup must not race surviving helper processes.
    with contextlib.nullcontext(args.fixture_root) as temp:
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
        port = None
        config = root / "daemon.json"
        values = {"endpoint": "tcp:127.0.0.1:0", "instance_root": str(root / "instance"), "bd_binary": str(args.fake_bd), "shutdown_timeout_ms": 150, "logging": {"enableConsoleSink": False, "enableFileSink": True}}
        config.write_text(json.dumps(values))
        command = [str(args.daemon), "--repo", str(repo), "--config", str(config)]

        def bdc(*words, ok=True):
            result = run_product([str(args.bdc), "--repo", str(repo), "--bd", str(args.fake_bd), *words], env=env, timeout=15)
            data = json.loads(result.stdout)
            assert data["ok"] is ok, (words, result.returncode, data, result.stderr)
            return data

        def probe_http(path):
            assert child is not None and child.poll() is None, "HTTP requires the observed live child"
            conn = http.client.HTTPConnection("127.0.0.1", port, timeout=3)
            try:
                conn.request("GET", path)
                response = conn.getresponse()
                reply = response.status, json.loads(response.read())
                assert child.poll() is None, "HTTP owner exited during probe"
                return reply
            except (OSError, http.client.HTTPException):
                return None
            finally:
                conn.close()

        def spawn():
            nonlocal port
            log = root / "instance/logs/bd-cache.log.jsonl"
            offset = log.stat().st_size if log.exists() else 0
            owned = subprocess.Popen(command, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding="utf-8",
                creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if os.name == "nt" else 0)
            try:
                port = wait_for(lambda: child_bound_endpoint(owned, log, offset))
                # The callback is before serving. Check continuous ownership,
                # then separately require the actual child's health/readiness.
                with socket.socket() as competitor:
                    try:
                        competitor.bind(("127.0.0.1", port))
                    except OSError as error:
                        assert error.errno == errno.EADDRINUSE, error
                    else:
                        raise AssertionError("reported child listener was not continuously owned")
                duplicate = run_product(command, env=env, timeout=10)
                assert duplicate.returncode == 1 and duplicate.stderr.startswith("RUNTIME.LOCK_HELD ") and not duplicate.stdout, duplicate
                assert owned.poll() is None, "instance owner exited during competing controls"
                return owned
            except BaseException:
                if owned.poll() is None:
                    owned.kill()
                stdout, stderr = owned.communicate(timeout=10)
                print("owned daemon startup failed:", owned.returncode, stdout, stderr, file=sys.stderr)
                raise

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
                occupied.bind(("127.0.0.1", 0)); occupied.listen()
                # This explicit endpoint is deliberately occupied throughout
                # child startup; it is never represented as a reservation.
                config.write_text(json.dumps({**values, "endpoint": f"tcp:127.0.0.1:{occupied.getsockname()[1]}"}))
                result = run_product(command, env=env, timeout=10)
                assert result.returncode == 1 and result.stderr.startswith("RUNTIME.BIND_FAILED ") and not result.stdout, result
            config.write_text(json.dumps(values))
            child = spawn(); wait_for(lambda: probe_http("/health")); stop(child); child = None
            # Stop and reap a real blocked hook tree. Deterministic ownership
            # through cleanup is covered by the bounded readiness HTTP fixture;
            # a fresh process launch cannot be ordered inside finite worker grace.
            values["shutdown_timeout_ms"] = 1000
            config.write_text(json.dumps(values))
            bdc("cache", "schedule", "set", "* * * * * *", "--budget-ms", "30000", "--hook-reserve-ms", "20000", "--post-sync", "consumer.hooks:after", "--python", str(args.fake_python), "--env", "FAKE_PY_DIR")
            (root / "hook.mode").write_text("grandchild")
            child = spawn(); wait_for(lambda: probe_http("/health"))
            pids = [int(wait_for(lambda p=p: p.read_text().strip() if p.exists() else None)) for p in (root / "child.pid", root / "gc.pid")]
            began = time.monotonic()
            child.send_signal(signal.CTRL_BREAK_EVENT if os.name == "nt" else signal.SIGTERM)
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
            if child is not None and child.poll() is None:
                print("readiness at failure:", probe_http("/ready"))
            print("schedule at failure:", bdc("cache", "schedule", "show"))
            for log in (root / "instance").rglob("*.jsonl"):
                print(log.read_text()[-6000:])
            raise
        finally:
            if child is not None and child.poll() is None:
                child.kill(); child.communicate(timeout=10)


if __name__ == "__main__":
    if "--owned-fixture-child" in sys.argv:
        sys.argv.remove("--owned-fixture-child")
        if sys.stdin.buffer.readline() != b"owned-fixture-start\n":
            raise SystemExit("fixture ownership handshake missing")
        main()
    elif "--graph" in sys.argv:
        main()
    else:
        fixture_root = tempfile.mkdtemp(prefix="daemon-acceptance-")
        code = run_fixture([sys.executable, "-I", str(Path(__file__).resolve()),
                            "--owned-fixture-child", "--fixture-root", fixture_root, *sys.argv[1:]],
                           scratch_root=fixture_root)
        raise SystemExit(code)
