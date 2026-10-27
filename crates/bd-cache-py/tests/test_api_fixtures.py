"""Replays the shared `bd-cache-api` fixtures through `cache.beads` / `cache.manage` (REQ-BDC-0807).

The fixtures (`crates/bd-cache-api/tests/fixtures/*.json`, format in their README) are
the ones `bd-cache-api` runs over `FakeBackend` and `FakeStore`. Here the same worlds are
built on the real composition: the fake `bd` program, the SQLite store and the flock drain
lock. Each step must give the same `data`, or raise a `CacheError` with the same `code`
(and every other member the fixture states), as `bd-cache-api` does.

Two things differ between the worlds, and the replay maps both, nothing else:

- The Rust worlds run on a manual clock fixed at `2026-10-23T10:00:00Z`; here the clock is
  the system's. An expected instant equal to that start must be a millisecond RFC 3339
  timestamp between the moment the fixture started and the moment the step ended.
- The fixtures put the repository at `/repo`; here it is a temporary directory. An
  expected string starting with `/repo` is matched with that prefix replaced.
"""

import json
import re
import time
from datetime import datetime, timezone
from pathlib import Path

import pytest

import bd_cache_rs
from conftest import HERE, bead

FIXTURES = HERE.parents[1] / "bd-cache-api" / "tests" / "fixtures"
FILES = sorted(FIXTURES.glob("*.json"))
START = "2026-10-23T10:00:00.000Z"
MISSING = object()  # an absent key, distinct from every JSON value
TIMESTAMP = re.compile(r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{3}Z$")


def test_every_fixture_file_is_replayed():
    assert [f.name for f in FILES] == [
        "decode_refusals.json", "idempotency_key_required.json", "issues_list_filters.json",
        "reads.json", "writes.json"], "add the new fixture file to this list and replay it"


def world(repo, spec):
    """The fixture's beads and edges in the fake `bd`, the repository's identity, and a synced cache."""
    (repo.root / ".beads" / "metadata.json").write_text(
        json.dumps({"dolt_database": "beads", "project_id": "proj-1"}), encoding="utf-8")
    rows = {}
    for fields in spec.get("beads", []):
        fields = dict(fields)
        bead_id = fields.pop("id")
        fields.setdefault("labels", ["phase-x"])
        rows[bead_id] = bead(bead_id, **fields)
    for source, target, kind in spec.get("edges", []):
        rows[source].setdefault("dependencies", []).append(
            {"issue_id": source, "depends_on_id": target, "type": kind,
             "created_at": rows[source]["created_at"], "created_by": "fake", "metadata": "{}"})
    repo.bd.load()
    repo.bd.state["beads"] = rows
    repo.bd.save()
    repo.config('scopes = ["x"]\n')
    cache = repo.open()
    for scope in spec.get("sync", []):
        cache.sync(scope, 30)
    return cache


def run(cache, command, request):
    """One step: the envelope's `{ok, data}` or `{ok, error}`, as `bd-cache-api`'s runner builds it."""
    words = command.split(" ")
    namespace, name = (cache.manage, "_".join(words[1:])) if words[0] == "cache" else (cache.beads, "_".join(words))
    try:
        return {"ok": True, "data": getattr(namespace, name)(**request)}
    except bd_cache_rs.CacheError as e:
        return {"ok": False, "error": {
            "kind": e.kind, "code": e.code, "message": e.message, "details": e.details,
            "suggested_action": e.suggested_action}}


def instant(text):
    return datetime.strptime(text, "%Y-%m-%dT%H:%M:%S.%fZ").replace(tzinfo=timezone.utc).timestamp()


def mismatches(path, want, got, window, root, out):
    """Appends the differences between the pattern `want` and `got`; the matching rules of `common/mod.rs`."""
    missing = MISSING
    if want == "$absent":
        if got is not missing:
            out.append(f"{path}: expected absent, got {got!r}")
    elif got is missing:
        out.append(f"{path}: missing, expected {want!r}")
    elif want == "$any":
        pass
    elif isinstance(want, dict) and isinstance(got, dict):
        for key, value in want.items():
            mismatches(f"{path}.{key}", value, got.get(key, missing), window, root, out)
    elif isinstance(want, list) and isinstance(got, list):
        if len(want) != len(got):
            out.append(f"{path}: expected {len(want)} items, got {len(got)}: {got!r}")
        else:
            for i, (w, g) in enumerate(zip(want, got)):
                mismatches(f"{path}[{i}]", w, g, window, root, out)
    elif want == START:
        ok = isinstance(got, str) and TIMESTAMP.match(got) and window[0] - 1 <= instant(got) <= window[1] + 1
        if not ok:
            out.append(f"{path}: expected a timestamp within {window}, got {got!r}")
    elif isinstance(want, str) and want.startswith("/repo"):
        if got != root + want[len("/repo"):]:
            out.append(f"{path}: expected {root + want[len('/repo'):]!r}, got {got!r}")
    elif want != got or type(want) is not type(got):
        out.append(f"{path}: expected {want!r}, got {got!r}")


@pytest.mark.parametrize("path", FILES, ids=lambda p: p.stem)
def test_the_fixture_gives_the_same_data_or_code_as_bd_cache_api(path, repo):
    fixture = json.loads(path.read_text(encoding="utf-8"))
    began = time.time()
    cache = world(repo, fixture.get("world", {}))
    root = str(repo.root.resolve())
    failures = []
    assert fixture["steps"], path.name
    for i, step in enumerate(fixture["steps"]):
        got = run(cache, step["command"], step["request"])
        found = []
        mismatches("", step["expect"], got, (began, time.time()), root, found)
        failures += [f"step {i} ({step.get('name', '')}){m}\n  got: {json.dumps(got)[:600]}" for m in found]
    assert not failures, f"{path.name}:\n" + "\n".join(failures)
