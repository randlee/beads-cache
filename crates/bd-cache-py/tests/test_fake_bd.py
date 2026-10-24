"""The fake `bd` answers as sc-bd-cli's `FakeBd` does (PE-ARCH-007).

Two rules are checked against shared data, not against this file's own expectations:
`--set-metadata` coercion against `fixtures/metadata-coercion.json` (generated from
`coerce_metadata` in `crates/sc-bd-cli/src/testing/fake_bd.rs`), and `bd blocked` /
`bd ready` against the P-6 recording of bd itself (`crates/sc-beads/tests/fixtures/p6-blocked/`,
ADR-BEADS-0004).
"""

import json
import subprocess

import pytest

from conftest import HERE, FakeBd, bead
from fake_bd_main import coerce_metadata

COERCION = json.loads((HERE / "fixtures" / "metadata-coercion.json").read_text(encoding="utf-8"))["cases"]
P6 = HERE.parent.parent / "sc-beads" / "tests" / "fixtures" / "p6-blocked"
P6_STATES = ("1-initial", "2-x-closed", "3-x-reopened")


def typed(value):
    """JSON text, so 5 and 5.0 and True and 1 are told apart."""
    return json.dumps(value, sort_keys=True)


def run(fake, *argv):
    done = subprocess.run([str(fake.binary), *argv], capture_output=True, text=True, timeout=60, check=False)
    assert done.returncode == 0, done.stderr
    return done.stdout


def test_the_coercion_table_has_the_cases_bd_disagrees_with_a_naive_parser_on():
    given = {case["given"] for case in COERCION}
    assert {"5.0", "1e3", "null", " 7", "True", '"5"', "[1]", "NaN", "1e400"} <= given


@pytest.mark.parametrize("case", COERCION, ids=lambda case: repr(case["given"]))
def test_metadata_coercion_matches_the_shared_table(case):
    assert typed(coerce_metadata(case["given"])) == typed(case["stored"])


def test_set_metadata_through_the_fake_stores_every_value_of_the_table(tmp_path):
    fake = FakeBd(tmp_path)
    fake.add(bead("b1"))
    argv = ["update", "b1", "--actor", "t"]
    for i, case in enumerate(COERCION):
        argv += ["--set-metadata", f"k{i}={case['given']}"]
    run(fake, *argv)
    stored = fake.get("b1")["metadata"]
    for i, case in enumerate(COERCION):
        assert typed(stored[f"k{i}"]) == typed(case["stored"]), case


def blocked_and_ready(fake, label):
    blocked = {row["id"]: row["blocked_by"] for row in json.loads(run(fake, "blocked", "-l", label, "--json"))}
    for row in json.loads(run(fake, "blocked", "-l", label, "--json")):
        assert row["blocked_by_count"] == len(row["blocked_by"])
    ready = {row["id"] for row in json.loads(run(fake, "ready", "-l", label, "-n", "0", "--json"))}
    return blocked, ready


@pytest.mark.parametrize("state", P6_STATES)
def test_blocked_and_ready_match_the_p6_recording(tmp_path, state):
    fake = FakeBd(tmp_path)
    fake.add(*json.loads((P6 / f"{state}-list.json").read_text(encoding="utf-8")))
    blocked, ready = blocked_and_ready(fake, "phase-p6")
    recorded_blocked = json.loads((P6 / f"{state}-blocked.json").read_text(encoding="utf-8"))
    recorded_ready = json.loads((P6 / f"{state}-ready.json").read_text(encoding="utf-8"))
    assert blocked == {row["id"]: row["blocked_by"] for row in recorded_blocked}
    assert ready == {row["id"] for row in recorded_ready}


def edge(kind, other):
    return {"issue_id": "", "depends_on_id": other, "type": kind,
            "created_at": "2026-10-23T09:00:00Z", "created_by": "fake", "metadata": "{}"}


def test_conditional_blocks_and_waits_for_block_like_blocks_and_related_does_not(tmp_path):
    # Unprobed edge types err towards "not ready" (ADR-BEADS-0004, REQ-BDC-0904).
    fake = FakeBd(tmp_path)
    fake.add(
        bead("a"),
        bead("c", dependencies=[edge("conditional-blocks", "a")]),
        bead("w", dependencies=[edge("waits-for", "a")]),
        bead("r", dependencies=[edge("related", "a")]),
    )
    blocked, ready = blocked_and_ready(fake, "phase-x")
    assert blocked == {"c": ["a"], "w": ["a"]}
    assert ready == {"a", "r"}
    fake.edit("a", status="closed")
    blocked, ready = blocked_and_ready(fake, "phase-x")
    assert blocked == {} and ready == {"c", "w", "r"}


def test_a_child_is_blocked_only_through_a_blocked_parent_at_any_depth(tmp_path):
    fake = FakeBd(tmp_path)
    fake.add(
        bead("x", labels=()),
        bead("p", blocked_by=["x"]),
        bead("c", dependencies=[edge("parent-child", "p")]),
        bead("g", dependencies=[edge("parent-child", "c")]),
        bead("q"),
        bead("d", dependencies=[edge("parent-child", "q")]),
    )
    blocked, ready = blocked_and_ready(fake, "phase-x")
    assert blocked == {"p": ["x"], "c": ["p"], "g": ["c"]}
    assert ready == {"q", "d"}
    fake.edit("x", status="closed")
    blocked, ready = blocked_and_ready(fake, "phase-x")
    assert blocked == {} and ready == {"p", "c", "g", "q", "d"}


def test_a_dependency_cycle_ends_and_a_deferred_bead_is_not_ready(tmp_path):
    fake = FakeBd(tmp_path)
    fake.add(
        bead("m", dependencies=[edge("parent-child", "n")]),
        bead("n", dependencies=[edge("parent-child", "m")]),
        bead("later", defer_until="2999-01-01T00:00:00Z"),
        bead("past", defer_until="2000-01-01T00:00:00Z"),
    )
    blocked, ready = blocked_and_ready(fake, "phase-x")
    assert blocked == {}
    assert ready == {"m", "n", "past"}
