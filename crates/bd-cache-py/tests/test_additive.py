"""Wheel 0.2 is additive over 0.1 (REQ-BDC-0812, contract section 10).

The 0.1 verbs and their dicts stay as they were, 0.2 issues no `DeprecationWarning`, the
0.2 namespaces take millisecond timeouts and both scope-id spellings, `CacheError` gains
the contract's fields, and `CONTRACT_VERSION` is the contract's.
"""

import re
import warnings

import pytest

import bd_cache_rs
from conftest import Stopwatch

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
    # Causal first: `bd list` blocks on a gate this test releases only after the call returned.
    # A call that returns at all, with BD_UNAVAILABLE and not the bead, was cut off by a limit;
    # one that waited for bd would sit on the gate until its 60 s backstop and then return the
    # bead. The clock is only a very loose sanity bound on the unit (33x the 300 ms limit; read
    # as seconds the limit is capped at the 30 s call timeout), never a pass/fail margin.
    repo.bd.add({**repo.bd.get("b1"), "id": "late-1", "title": "late-1"})
    release = repo.bd.block("list", deadline=60)
    with Stopwatch() as watch:
        err = raised(cache.beads.issues_get, id="late-1", miss_timeout_ms=300)
    assert err.code == "BD_CACHE.BD_UNAVAILABLE" and err.retryable is True
    assert watch.elapsed < 10, f"300 ms waited {watch.elapsed:.1f} s: read as seconds?"
    assert not release.is_set(), "the gate was never released: the call was not waiting on bd's answer"
    assert release.started(), "bd was asked and still blocked when the call gave up"
    release.set()
    assert cache.beads.issues_get(id="late-1", miss_timeout_ms=20000)["id"] == "late-1"


def test_both_scope_spellings_and_the_0_1_list_return_the_same_rows(cache):
    bare = cache.beads.issues_list(scope="x")
    label = cache.beads.issues_list(scope="phase-x")
    old = cache.list("phase-x")
    assert ids(bare["items"]) == ids(label["items"]) == ids(old["rows"]) == ["b1", "b2", "s1"]
    assert cache.list("x")["rows"] == old["rows"]
    assert ids(cache.beads.ready(scope="x")["items"]) == ids(cache.beads.ready(scope="phase-x")["items"])
    assert cache.manage.refresh_scopes(scopes=["x"])["results"][0]["scope"] == "phase-x"
    assert cache.manage.refresh_scopes(scopes=["phase-x"])["results"][0]["scope"] == "phase-x"


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
