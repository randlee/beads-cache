"""The 0.1 call corpus is current and covers every 0.1 verb (REQ-BDC-0812, REQ-BDC-0910).

`call_corpus.py` drives the verbs against the fake `bd`; `call_corpus_0_1.json` is the committed
record of that run. Rebuilding it here means a changed reply, error or `bd` argv is a diff, and a
verb added to the handle without a case fails.
"""

import json
import os
import re
from pathlib import Path

import bd_cache_rs
from call_corpus import build

HERE = Path(__file__).resolve().parent
CORPUS = HERE / "call_corpus_0_1.json"
CONTRACT = HERE.parents[2] / "docs" / "bd-cache-cli" / "contract.md"
HANDLE_MEMBERS = {"path", "closed"}


def render(corpus):
    return json.dumps(corpus, indent=1) + "\n"


def test_the_committed_corpus_is_what_the_fake_bd_run_produces(repo):
    built = render(build(repo))
    if os.environ.get("BDC_WRITE_CORPUS"):
        CORPUS.write_text(built, encoding="utf-8")
    assert built == CORPUS.read_text(encoding="utf-8"), (
        "the 0.1 replies changed; if intended, regenerate with BDC_WRITE_CORPUS=1 pytest tests/test_call_corpus.py")


def test_every_0_1_verb_has_a_success_case_and_every_error_path_a_failing_one():
    corpus = json.loads(CORPUS.read_text(encoding="utf-8"))
    verbs = {name for name in dir(bd_cache_rs.Cache) if not name.startswith("_")}
    verbs -= {"close"}  # the call that ends the corpus; its effect is the `closed` case
    assert set(corpus["verbs"]) == verbs, "a handle member with no case, or a case with no member"
    succeeded = {c["verb"] for c in corpus["calls"] if "reply" in c}
    assert succeeded == verbs, f"no successful call recorded for {sorted(verbs - succeeded)}"
    failing = {c["verb"] for c in corpus["calls"] if "raises" in c}
    assert {"get", "write", "list", "entry", "mark_announced"} <= failing


def test_the_corpus_records_the_bd_argv_of_the_verbs_that_reach_bd():
    corpus = json.loads(CORPUS.read_text(encoding="utf-8"))
    reached = {c["verb"] for c in corpus["calls"] if c["bd_calls"]}
    assert {"get", "refresh", "sync", "drain", "ready"} <= reached
    drained = [c for c in corpus["calls"] if c["verb"] == "drain" and c["bd_calls"]]
    assert any(argv[0] in ("update", "close") for c in drained for argv in c["bd_calls"])


def test_the_corpus_records_the_error_fields_and_the_unreachable_codes():
    corpus = json.loads(CORPUS.read_text(encoding="utf-8"))
    errors = [c["raises"] for c in corpus["calls"] if c.get("raises", {}).get("code")]
    assert errors and all(e["detail_is_text"] and e["fix"] and "payload" in e for e in errors)
    assert {"BD_CACHE.PRECONDITION", "BD_CACHE.IDEM_CONFLICT", "BD_CACHE.SCOPE_UNKNOWN",
            "BD_CACHE.NOT_FOUND"} <= {e["code"] for e in errors}
    assert corpus["unreachable_codes"] == sorted(bd_cache_rs.UNREACHABLE)
    assert any(c.get("raises", {}).get("type") == "ValueError" for c in corpus["calls"]), "an unknown op"


# Spans in the section 10 table that name neither a 0.1 verb nor a 0.2 method: `None` (what the 0.1
# `resolve` returns) and the `CacheError` row, which lists the error class and the members it gains.
# Listed here so any other unknown span fails.
TABLE_LITERALS = {"None", "CacheError", "kind", "message", "details", "suggested_action", "retryable"}
NAMESPACES = ("beads", "manage")
# Every name the table must be seen to check: the rows a reader could think the test skips.
MUST_BE_CHECKED = {
    "dependencies_list", "context", "issues_close", "dependencies_add", "journal_list",
    "journal_announce", "health", "scopes", "sync_alert", "set_sync_alert", "issues_update",
    "journal_get", "journal_resolve", "refresh_scopes", "flush", "evict",
}
SPAN = re.compile(r"^(\w+(?:\.\w+)*)\s*(?:\(.*\))?$")


def contract_table_rows(section):
    """The cells of each body row of the section 10 verb table (header and rule excluded)."""
    lines = section.splitlines()
    start = next(i for i, line in enumerate(lines) if line.startswith("| 0.1 verb"))
    rows = []
    for line in lines[start + 2:]:  # past the header and the `|---|` rule
        if not line.startswith("|"):
            break
        rows.append([c.strip() for c in re.split(r"(?<!\\)\|", line.strip()[1:-1])])
    return rows


def test_the_contract_table_names_real_0_1_verbs_and_real_0_2_methods(cache):
    """Contract section 10: every backtick span of the table is a 0.1 verb, a 0.2 method or a listed literal.

    Both cells of every row are read, the em-dash rows too. A 0.2 name is `cache.<ns>.<method>`,
    `<ns>.<method>`, or a bare method of the namespace named earlier in the cell.
    """
    section = CONTRACT.read_text(encoding="utf-8").split("## 10.", 1)[1].split("\n## ", 1)[0]
    handle = {name for name in dir(bd_cache_rs.Cache) if not name.startswith("_")}
    rows = contract_table_rows(section)
    assert len(rows) >= 13, f"the table was not read: {len(rows)} rows"
    checked = set()
    for cells in rows:
        assert len(cells) == 3, f"not a three-cell row: {cells}"
        for span in re.findall(r"`([^`]*)`", cells[0]):
            shape = SPAN.match(span)
            assert shape, f"{span!r}: not a name or call in the 0.1 cell"
            name = shape.group(1)
            assert name in handle or name in TABLE_LITERALS, f"{name}: not a Cache member"
            checked.add(name)
        namespace = None
        for span in re.findall(r"`([^`]*)`", cells[1]):
            shape = SPAN.match(span)
            assert shape, f"{span!r}: not a name or call in the 0.2 cell"
            parts = shape.group(1).split(".")
            if parts[0] == "cache" and len(parts) == 3:
                parts = parts[1:]
            assert len(parts) <= 2, f"{span}: more than namespace.method"
            if len(parts) == 2:
                namespace, method = parts
                assert namespace in NAMESPACES, f"{span}: {namespace} is not a namespace"
            elif namespace is not None:
                method = parts[0]
            else:
                assert parts[0] in TABLE_LITERALS, f"{span}: neither a 0.2 method nor a listed literal"
                checked.add(parts[0])
                continue
            assert hasattr(getattr(cache, namespace), method), f"cache.{namespace}.{method} does not exist"
            checked.add(method)
    assert MUST_BE_CHECKED <= checked, f"never read: {sorted(MUST_BE_CHECKED - checked)}"
