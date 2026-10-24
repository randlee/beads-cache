"""The Mapping table of `docs/bd-cache-py/architecture.md` covers every call atm-bd makes (REQ-PY-0009).

REQUIRED is the set of `bd_cache.py` names that the atm-bd hooks and
`bd_cache_cron.py` use at atm-core 05831df: `sanity_task_close_hook.py`,
`close_fields_backfill.py`, `close-vars.py`, `bd_cache_cron.py`, and the CLI
commands their messages tell people to run.
"""

import re
from pathlib import Path

import bd_cache_rs

DOC = Path(__file__).resolve().parents[3] / "docs" / "bd-cache-py" / "architecture.md"

REQUIRED = {
    "BdCache", "get", "cached", "bd_rows", "store", "view", "list", "write", "drain", "sync", "prune",
    "resolve", "status", "path", "unannounced_escalations", "mark_announced", "sync_alert",
    "set_sync_alert", "clear_sync_alert", "outage", "mark_outage_announced", "record_outage",
    "clear_outage", "has_open_work", "CacheError", "code", "detail", "fix", "extra", "as_json",
    "UNREACHABLE",
}


def mapping_rows():
    text = DOC.read_text(encoding="utf-8")
    section = text.split("## Mapping from `bd_cache.py`", 1)[1].split("\n## ", 1)[0]
    rows = []
    for line in section.splitlines():
        cells = [c.strip() for c in line.strip().strip("|").split(" | ")]
        if len(cells) == 3 and cells[0].startswith("`") and cells[0] != "`bd_cache.py`":
            rows.append(cells)
    return rows


def test_every_name_the_atm_bd_scripts_use_has_a_row():
    named = set()
    for first, _, _ in mapping_rows():
        named.update(re.findall(r"[A-Za-z_]+", " ".join(re.findall(r"`([^`]+)`", first))))
    missing = {name for name in REQUIRED if name not in named}
    assert not missing, f"no Mapping row names: {sorted(missing)}"


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
