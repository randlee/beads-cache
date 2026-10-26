"""The Mapping table of `docs/bd-cache-py/architecture.md` covers every call atm-bd makes (REQ-PY-0009).

The names come from `atm_core_05831df_calls.json`, extracted from atm-core 05831df with file and
line (`atm_core_05831df_extract.py`, run with `gh`); nothing here is typed by hand.
"""

import json
import re
from pathlib import Path

import bd_cache_rs

HERE = Path(__file__).resolve().parent
DOC = HERE.parents[2] / "docs" / "bd-cache-py" / "architecture.md"
EVIDENCE = json.loads((HERE / "atm_core_05831df_calls.json").read_text(encoding="utf-8"))


def mapping_rows():
    text = DOC.read_text(encoding="utf-8")
    section = text.split("## Mapping from the baseline Python implementation", 1)[1].split("\n## ", 1)[0]
    rows = []
    for line in section.splitlines():
        cells = [c.strip() for c in line.strip().strip("|").split(" | ")]
        if len(cells) == 3 and cells[0].startswith("`") and cells[0] != "`bd_cache.py`":
            rows.append(cells)
    return rows


def test_the_evidence_is_from_the_pinned_commit_and_has_locations():
    assert EVIDENCE["ref"] == "05831dfe866d389feed138add3c29cb93cd6c82b"
    assert EVIDENCE["commands"] and EVIDENCE["uses"]
    for use in EVIDENCE["uses"]:
        assert use["file"] in EVIDENCE["clients"] and isinstance(use["line"], int) and use["line"] > 0


def test_every_name_the_atm_bd_scripts_use_has_a_row():
    named = set()
    for first, _, _ in mapping_rows():
        named.update(re.findall(r"[A-Za-z_]+", " ".join(re.findall(r"`([^`]+)`", first))))
    used = {entry.split(":", 1)[1] for entry in EVIDENCE["names"]}
    missing = sorted(used - named)
    assert not missing, f"no Mapping row names: {missing}; used at {EVIDENCE['ref'][:7]}"


def test_every_binding_the_table_names_exists():
    handle = {name for name in dir(bd_cache_rs.Cache) if not name.startswith("_")}
    module = {name for name in dir(bd_cache_rs) if not name.startswith("_")}
    rows = mapping_rows()
    assert len(rows) >= 25
    for first, _, binding in rows:
        for method in re.findall(r"`cache\.(\w+)\(", binding):
            assert method in handle, f"{first}: `cache.{method}(...)` is not a Cache method"
        for name in re.findall(r"`bd_cache_rs\.(\w+)", binding):
            assert name in module, f"{first}: `bd_cache_rs.{name}` does not exist"
        for attribute in re.findall(r"`cache\.(\w+)`", binding):
            assert attribute in handle, f"{first}: `cache.{attribute}` is not on Cache"
