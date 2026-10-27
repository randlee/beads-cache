"""Fixtures: a beads repository with a recording fake `bd`, and cache handles over it.

The fake `bd` (`fake_bd_main.py`) is run by the real `BdCli` as the `bd` program, so
every test drives the real composition: store, drain lock, argv builders, process
runner with deadlines and strict row parsing. Calls are recorded in a call log.
"""

import json
import os
import stat
import sys
import threading
import time
from datetime import datetime, timedelta, timezone
from pathlib import Path

import pytest

import bd_cache_rs

HERE = Path(__file__).resolve().parent
SCHEMA = (HERE / "fixtures" / "bd-schema-1.3.1.json").read_text(encoding="utf-8")
BD_VERSION = "1.3.1"
T0 = "2026-10-23T09:00:00Z"


def bead(bead_id, *, status="open", labels=("phase-x",), blocked_by=(), **extra):
    """A minimal valid bd row; `blocked_by` adds `blocks` edges to those beads."""
    row = {
        "id": bead_id,
        "title": bead_id,
        "status": status,
        "priority": 2,
        "issue_type": "task",
        "created_at": T0,
        "updated_at": T0,
        "labels": list(labels),
    }
    if blocked_by:
        row["dependencies"] = [
            {"issue_id": bead_id, "depends_on_id": other, "type": "blocks",
             "created_at": T0, "created_by": "fake", "metadata": "{}"}
            for other in blocked_by
        ]
    row.update(extra)
    return row


class FakeBd:
    """The test side of the fake `bd`: its world, its failure switches and its call log."""

    def __init__(self, directory):
        self.state_path = directory / "bd-state.json"
        self.calls_path = Path(str(self.state_path) + ".calls")
        self.binary = directory / "bd"
        self.state = {"beads": {}, "version": BD_VERSION, "schema": SCHEMA, "delay": {}, "down": False}
        self.calls_path.write_text("", encoding="utf-8")
        self.save()
        script = HERE / "fake_bd_main.py"
        # A shell wrapper, so the path of the interpreter never meets a shebang length limit.
        self.binary.write_text(
            f'#!/bin/sh\nexec "{sys.executable}" "{script}" "{self.state_path}" "$@"\n', encoding="utf-8")
        self.binary.chmod(self.binary.stat().st_mode | stat.S_IXUSR)

    def save(self):
        self.state_path.write_text(json.dumps(self.state), encoding="utf-8")

    def load(self):
        self.state = json.loads(self.state_path.read_text(encoding="utf-8"))

    def add(self, *beads):
        self.load()
        for row in beads:
            self.state["beads"][row["id"]] = row
        self.save()

    def edit(self, bead_id, *, updated_at=None, **fields):
        """Changes a bead as another writer would.

        `updated_at` is explicit when given; otherwise it is a counter-driven instant in 2099,
        later than any sync cursor and later than the bead's previous edit, so an incremental
        sync always lists the change and the newer-row guard always accepts it (no sleeping).
        """
        self.load()
        row = self.state["beads"][bead_id]
        row.update(fields)
        if updated_at is None:
            self.state["edits"] = self.state.get("edits", 0) + 1
            stamp = datetime(2099, 1, 1, tzinfo=timezone.utc) + timedelta(seconds=self.state["edits"])
            updated_at = stamp.strftime("%Y-%m-%dT%H:%M:%SZ")
        row["updated_at"] = updated_at
        self.save()

    def get(self, bead_id):
        self.load()
        return self.state["beads"][bead_id]

    def down(self, is_down=True):
        self.load()
        self.state["down"] = is_down
        self.save()

    def fail(self, command, message):
        """Makes `command` exit 1 with `message` (a reachable bd refusing the call)."""
        self.load()
        self.state.setdefault("fail", {})[command] = message
        self.save()

    def unfail(self, command):
        """Undoes `fail(command)`: `command` answers again."""
        self.load()
        self.state.get("fail", {}).pop(command, None)
        self.save()

    def delay(self, command, seconds):
        """Makes `command` (or `*` for all) answer after `seconds`."""
        self.load()
        self.state["delay"][command] = seconds
        self.save()

    def events(self):
        text = self.calls_path.read_text(encoding="utf-8")
        return [json.loads(line) for line in text.splitlines() if line]

    def calls(self):
        """The argv of every call started so far, in order."""
        return [e["argv"] for e in self.events() if e["event"] == "start"]

    def commands(self):
        return [argv[0] for argv in self.calls()]

    def spans(self):
        """(start, end, argv) of every call, in start order; `end` is None for a killed call.

        Times are `clock_gettime_ns(CLOCK_MONOTONIC)` readings written by the fake: one
        system-wide clock on Linux and macOS that never steps. (`time.monotonic_ns()` is not
        used: on macOS with Python 3.9 it is a per-process offset, not comparable across
        processes.)
        """
        ends = {e["pid"]: e["at"] for e in self.events() if e["event"] == "end"}
        return [(e["at"], ends.get(e["pid"]), e["argv"]) for e in self.events() if e["event"] == "start"]

    def block(self, command="*", deadline=20):
        """Makes `command` wait until the returned release file exists (or `deadline` seconds pass).

        The returned `Release` remembers how many calls had started when it was armed, so its
        `wait_started` sees only calls made after that: never a call an earlier step logged.
        """
        self.load()
        release = self.state_path.with_name(f"release-{command.strip('*') or 'all'}-{len(self.state.get('block', {}))}")
        self.state.setdefault("block", {})[command] = str(release)
        self.state["block_deadline"] = deadline
        self.save()
        return Release(release, self, command, baseline=len(self.calls()))


class Release:
    """The release file of a blocked command, and the calls made since it was armed."""

    def __init__(self, path, fake, command, baseline):
        self.path = path
        self.fake = fake
        self.command = command
        self.baseline = baseline

    def set(self):
        self.path.write_text("go", encoding="utf-8")

    def is_set(self):
        return self.path.exists()

    def started(self):
        """The argv of every call this release blocks that started after it was armed."""
        later = self.fake.calls()[self.baseline:]
        return [argv for argv in later if self.command == "*" or argv[0] == self.command]

    def wait_started(self, count=1, timeout=30):
        """Waits until `count` blocked calls have started since arming; returns their argv."""
        wait_until(lambda: len(self.started()) >= count, f"{count} blocked bd {self.command} call(s)", timeout)
        return self.started()


def wait_until(predicate, what, seconds=20):
    """Polls `predicate` against a monotonic deadline; a miss is a descriptive failure, not a hang."""
    end = time.monotonic() + seconds
    while not predicate():
        if time.monotonic() > end:
            raise AssertionError(f"timed out after {seconds}s waiting for {what}")
        time.sleep(0.002)


def daemon(target, *args):
    """A daemon thread: a test that fails must not leave a hung thread blocking exit."""
    return threading.Thread(target=target, args=args, daemon=True)


def join(thread, seconds=30):
    """Joins with a deadline; a thread that outlives it is a test failure, not a hang."""
    thread.join(seconds)
    assert not thread.is_alive(), f"thread {thread.name} still running after {seconds}s"


class Repo:
    """A beads repository: `.beads/metadata.json`, an optional config file and a fake `bd`."""

    def __init__(self, root, fake):
        self.root = root
        self.bd = fake
        self.opened = []

    def config(self, text):
        (self.root / ".beads" / "bd-cache.toml").write_text(text, encoding="utf-8")

    def open(self, **kwargs):
        kwargs.setdefault("bd_binary", str(self.bd.binary))
        cache = bd_cache_rs.Cache(str(self.root), **kwargs)
        self.opened.append(cache)
        return cache

    def reopen(self, cache, **kwargs):
        cache.close()
        return self.open(**kwargs)


@pytest.fixture(autouse=True)
def no_ambient_beads_dir(monkeypatch):
    """No test sees the `BEADS_DIR` of the shell that ran pytest (PE-FTQ-001).

    The store and every `bd` child read it, so an ambient value would point the cache at
    another repository's metadata. A test that needs one sets it in a child process.
    """
    monkeypatch.delenv("BEADS_DIR", raising=False)


@pytest.fixture
def repo(tmp_path):
    root = tmp_path / "repo"
    (root / ".beads").mkdir(parents=True)
    (root / ".beads" / "metadata.json").write_text(
        json.dumps({"dolt_database": "dev-db", "project_id": "p-1"}), encoding="utf-8")
    fake = FakeBd(tmp_path)
    fake.add(
        bead("b1"),
        bead("b2"),
        bead("s1", blocked_by=["b1"]),
    )
    fixture = Repo(root, fake)
    yield fixture
    for cache in fixture.opened:
        cache.close()


@pytest.fixture
def cache(repo):
    """A cache with phase `x` configured and synced once (bd up)."""
    repo.config('scopes = ["x"]\n')
    opened = repo.open()
    opened.sync("x", 30)
    return opened


class Stopwatch:
    def __enter__(self):
        self.start = time.monotonic()
        return self

    def __exit__(self, *exc):
        self.elapsed = time.monotonic() - self.start
        return False


@pytest.fixture
def stopwatch():
    return Stopwatch
