"""Fixtures: a beads repository with a recording fake `bd`, and cache handles over it.

The fake `bd` (`fake_bd_main.py`) is run by the real `BdCli` as the `bd` program, so
every test drives the real composition: store, drain lock, argv builders, process
runner with deadlines and strict row parsing. Calls are recorded in a call log.
"""

import json
import os
import stat
import sys
import time
from datetime import datetime, timezone
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

    def edit(self, bead_id, **fields):
        """Changes a bead as another writer would, stamping a later `updated_at`."""
        self.load()
        row = self.state["beads"][bead_id]
        row.update(fields)
        row["updated_at"] = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
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

    def delay(self, command, seconds):
        """Makes `command` (or `*` for all) answer after `seconds`."""
        self.load()
        self.state["delay"][command] = seconds
        self.save()

    def calls(self):
        """The argv of every call so far, in order."""
        text = self.calls_path.read_text(encoding="utf-8")
        return [json.loads(line)["argv"] for line in text.splitlines() if line]

    def commands(self):
        return [argv[0] for argv in self.calls()]


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


@pytest.fixture
def repo(tmp_path):
    root = tmp_path / "repo"
    (root / ".beads").mkdir(parents=True)
    (root / ".beads" / "metadata.json").write_text(
        json.dumps({"dolt_database": "atm-dev", "project_id": "p-1"}), encoding="utf-8")
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
