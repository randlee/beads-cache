"""Wheel 0.2 is additive over 0.1 (REQ-BDC-0812, contract section 10).

The 0.1 verbs and their dicts stay as they were, 0.2 issues no `DeprecationWarning`, the
0.2 namespaces take millisecond timeouts and a scope name as written (a legacy-prefixed
name there is another, unknown scope: `SCOPE_UNKNOWN`, ADR-BDC-0013), `CacheError` gains
the contract's fields, and `CONTRACT_VERSION` is the contract's.
"""

import re
import warnings
from pathlib import Path

import pytest

import bd_cache_rs

CONTRACT = Path(__file__).resolve().parents[3] / "docs" / "bd-cache-cli" / "contract.md"

VERBS_01 = {"get", "refresh", "write", "list", "ready", "sync", "drain", "entry", "resolve", "prune",
            "status", "unannounced_escalations", "mark_announced", "sync_alert", "set_sync_alert",
            "open_work"}
LIST_KEYS = {"scope", "last_ok_at", "rows", "outage"}
GET_KEYS = {"row", "scope_last_ok_at", "pending", "outage"}
NEW_ERROR_FIELDS = ("kind", "message", "details", "suggested_action", "retryable")
OLD_ERROR_FIELDS = ("code", "detail", "fix", "reason", "payload", "body", "exit_code")


def raised(call, *args, **kwargs):
    with pytest.raises(bd_cache_rs.CacheError) as caught:
        call(*args, **kwargs)
    return caught.value


def ids(rows):
    return sorted(r["bead"]["id"] if "bead" in r else r["id"] for r in rows)


def test_the_0_1_handle_has_exactly_the_0_1_public_names_and_the_namespaces_still_resolve(cache):
    public = {name for name in dir(bd_cache_rs.Cache) if not name.startswith("_")}
    assert public == VERBS_01 | {"path", "closed", "close"}
    assert type(cache.beads).__name__ == "Beads" and type(cache.manage).__name__ == "Manage"
    with pytest.raises(AttributeError):
        cache.nothing


def test_0_1_dicts_keep_their_key_sets_next_to_the_0_2_calls(cache):
    before = (set(cache.get("b1")), set(cache.list("phase-x")))
    cache.beads.issues_get(id="b1")
    cache.beads.issues_list(scope="x")
    cache.manage.status()
    after = (set(cache.get("b1")), set(cache.list("phase-x")))
    assert before == after == (GET_KEYS, LIST_KEYS)
    assert set(cache.get("b1")["row"]) == {"bead", "stale", "blocked"}


def test_no_verb_old_or_new_issues_a_deprecation_warning(cache):
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        cache.get("b1")
        cache.list("x")
        cache.ready("x")
        cache.status()
        cache.open_work()
        cache.beads.issues_get(id="b1")
        cache.beads.issues_list(scope="x")
        cache.beads.ready(scope="x")
        cache.beads.dependencies_list(issue_id=["s1"])
        cache.beads.context()
        cache.manage.status()
        cache.manage.health()
        cache.manage.scopes()
        cache.manage.journal_list()


def test_a_zero_miss_timeout_is_not_cached_and_retryable_while_0_1_keeps_its_behaviour(cache, repo):
    calls = len(repo.bd.calls())
    err = raised(cache.beads.issues_get, id="zz-9", miss_timeout_ms=0)
    assert (err.code, err.kind, err.retryable) == ("BD_CACHE.NOT_CACHED", "unavailable", True)
    assert len(repo.bd.calls()) == calls, "bd was not asked"
    old = raised(cache.get, "zz-9", miss_timeout=0.0)
    assert old.code == "BD_CACHE.BD_UNAVAILABLE", "0.1 get(miss_timeout=0.0) is unchanged"
    assert len(repo.bd.calls()) == calls


def test_miss_timeout_ms_is_milliseconds(cache, repo):
    """Causal (NFR-BDC-0005): bd is gated, so only the 300 ms timeout can end the first call.

    Read as seconds the call would wait for the gate's own deadline and then return the row,
    not raise; no wall-clock bound is asserted.
    """
    repo.bd.add({**repo.bd.get("b1"), "id": "late-1", "title": "late-1"})
    release = repo.bd.block("list", deadline=10)
    err = raised(cache.beads.issues_get, id="late-1", miss_timeout_ms=300)
    assert release.started(), "the miss reached bd"
    assert not release.is_set(), "the call gave up while bd was still gated: the timeout cut it"
    assert err.code == "BD_CACHE.BD_UNAVAILABLE" and err.retryable is True
    release.set()
    assert cache.beads.issues_get(id="late-1", miss_timeout_ms=20000)["id"] == "late-1"


def test_the_0_1_list_takes_both_spellings_and_the_0_2_namespaces_the_name_as_written(cache):
    # ADR-BDC-0013, REQ-BDC-0812 as amended: the legacy prefix is read on the 0.1
    # paths only; a 0.2 scope argument is a scope name as written.
    bare = cache.beads.issues_list(scope="x")
    old = cache.list("phase-x")
    assert ids(bare["items"]) == ids(old["rows"]) == ["b1", "b2", "s1"]
    assert cache.list("x")["rows"] == old["rows"]
    assert raised(cache.beads.issues_list, scope="phase-x").code == "BD_CACHE.SCOPE_UNKNOWN"
    assert raised(cache.beads.ready, scope="phase-x").code == "BD_CACHE.SCOPE_UNKNOWN"
    assert ids(cache.beads.ready(scope="x")["items"]) == ids(cache.ready("phase-x")["ready"])
    assert cache.manage.refresh_scopes(scopes=["x"])["results"][0]["scope"] == "x"


def test_an_unknown_scope_is_scope_unknown_for_both_spellings(cache):
    for scope in ("nope", "phase-nope"):
        assert raised(cache.beads.issues_list, scope=scope).code == "BD_CACHE.SCOPE_UNKNOWN"


def test_contract_version_is_the_one_context_reports(cache):
    assert re.fullmatch(r"\d+\.\d+\.\d+", bd_cache_rs.CONTRACT_VERSION)
    assert cache.beads.context()["x_cache"]["contract_version"] == bd_cache_rs.CONTRACT_VERSION
    assert cache.manage.status()["contract_version"] == bd_cache_rs.CONTRACT_VERSION
    assert bd_cache_rs.__version__ == "0.2.0"


def test_cache_error_carries_the_contract_fields_from_the_error_table(cache):
    err = raised(cache.beads.issues_get, id="b1", bogus=1)
    assert (err.kind, err.code, err.retryable) == ("invalid_input", "BD_CACHE.BAD_ARGS", False)
    assert err.details["member"] == "bogus" and isinstance(err.message, str) and err.message
    assert isinstance(err.suggested_action, str)
    assert err.body["code"] == err.code and err.exit_code == 3
    unavailable = raised(cache.beads.issues_get, id="zz-9", miss_timeout_ms=0)
    assert unavailable.retryable is True and unavailable.details["bead"] == "zz-9"
    conflict = raised(cache.beads.issues_update, id="b1", actor="a", idempotency_key="k",
                      patch={"assignee": "x"}, expected_status="closed")
    assert (conflict.kind, conflict.retryable) == ("conflict", False)
    assert conflict.details["unmet"]["status"] == {"expected": "closed", "actual": "open"}


def test_a_0_1_error_gains_the_new_fields_and_keeps_the_old_ones(cache):
    err = raised(cache.get, "zz-9", miss_timeout=0)
    assert err.code == "BD_CACHE.BD_UNAVAILABLE"
    assert (err.kind, err.retryable) == ("unavailable", True)
    assert err.message == err.detail and err.suggested_action == err.fix
    for field in OLD_ERROR_FIELDS + NEW_ERROR_FIELDS:
        assert hasattr(err, field), field


def test_a_python_type_mistake_is_a_type_error_not_a_cache_error(cache):
    with pytest.raises(TypeError):
        cache.beads.issues_get("b1")
    with pytest.raises(TypeError):
        cache.beads.issues_get(id={"b1"})
    with pytest.raises(TypeError):
        cache.beads.issues_get(id="b1", miss_timeout_ms=object())
    # A JSON type the decoder refuses is the decoder's code, as for the CLI.
    assert raised(cache.beads.issues_get, id=5).code == "BD_CACHE.BAD_ARGS"


def test_cache_tick_is_not_exposed(cache):
    assert not hasattr(cache.manage, "tick") and not hasattr(cache.manage, "cache_tick")
    assert not hasattr(cache.beads, "tick")


def test_namespaces_on_a_closed_handle_raise_closed_error(cache):
    beads = cache.beads
    cache.close()
    with pytest.raises(bd_cache_rs.ClosedError):
        beads.context()
    with pytest.raises(bd_cache_rs.ClosedError):
        cache.manage.status()


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
    """Contract section 10 (b-9 deliverable 4): every backtick span of the verb table is a 0.1 verb,
    a 0.2 method or a listed literal.

    Both cells of every row are read, the em-dash rows too. A 0.2 name is `cache.<ns>.<method>`,
    `<ns>.<method>`, or a bare method of the namespace named earlier in the cell.
    """
    section = CONTRACT.read_text(encoding="utf-8").split("## 10. 0.1 → 0.2 API differences", 1)[1]
    section = section.split("\n## ", 1)[0]
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


def test_the_contract_names_the_sync_methods_the_handle_has(cache):
    """Contract section 10: the sync's verbs it lists are the handle's methods (ADR-PY-0006)."""
    section = CONTRACT.read_text(encoding="utf-8").split("## 10. 0.1 → 0.2 API differences", 1)[1]
    listed = section.split("the sync's verbs are methods of the handle itself", 1)[1]
    listed = listed.split("`cache cron run`", 1)[0]
    names = re.findall(r"`(\w+)(?:\(\))?`", listed)
    assert len(names) == 14, names
    missing = [name for name in names if not callable(getattr(cache, name, None))]
    assert not missing, f"listed in contract section 10 but not on the handle: {missing}"
