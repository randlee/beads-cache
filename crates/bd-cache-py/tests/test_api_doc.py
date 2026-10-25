"""The "Verbs and construction" section of `docs/bd-cache-py/architecture.md` matches the handle (REQ-PY-0009)."""

import re
from pathlib import Path

import bd_cache_rs

DOC = Path(__file__).resolve().parents[3] / "docs" / "bd-cache-py" / "architecture.md"


def section():
    text = DOC.read_text(encoding="utf-8")
    return text.split("## Verbs and construction", 1)[1].split("\n## ", 1)[0]


def test_every_public_member_of_the_handle_is_described():
    members = {name for name in dir(bd_cache_rs.Cache) if not name.startswith("_")}
    described = set(re.findall(r"`cache\.(\w+)", section()))
    assert members - {"close", "closed"} <= described, sorted(members - described)


def test_every_cache_name_the_section_uses_exists():
    members = {name for name in dir(bd_cache_rs.Cache) if not name.startswith("_")}
    named = set(re.findall(r"`cache\.(\w+)", section()))
    assert named and named <= members, sorted(named - members)
    for name in re.findall(r"`bd_cache_rs\.(\w+)", section()):
        assert hasattr(bd_cache_rs, name), name
