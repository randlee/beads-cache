"""`ingest_scope` through the PyO3 handle (REQ-BDC-0726, REQ-BDC-0007, 0008, 0502)."""

import pytest

import bd_cache_rs
from conftest import bead


def raised(call, *args, **kwargs):
    with pytest.raises(bd_cache_rs.CacheError) as caught:
        call(*args, **kwargs)
    return caught.value


def ids(cache, scope):
    return sorted(i["id"] for i in cache.beads.issues_list(scope=scope)["items"])


def beads_in(cache, scope):
    return [s for s in cache.scopes_list()["scopes"] if s["scope"] == scope][0]["beads"]


def test_a_snapshot_is_stored_and_the_next_read_sees_it(cache):
    cache.scopes_add("mine", all=True)
    done = cache.ingest_scope("mine", [bead("i1"), bead("i2")], [], revision="r1")
    assert (done["rows"], done["stored"], done["revision"]) == (2, 2, "r1")
    assert ids(cache, "mine") == ["i1", "i2"]


def test_one_invalid_row_stores_no_row_of_the_snapshot(cache):
    cache.scopes_add("mine", all=True)
    bad = dict(bead("i2"), surprise=1)
    e = raised(cache.ingest_scope, "mine", [bead("i1"), bad], [])
    assert e.code == "BD_CACHE.INGEST_INVALID" and e.details["index"] == 1
    assert beads_in(cache, "mine") == 0, "no row of the snapshot stored"


def test_an_older_row_never_replaces_a_newer_stored_one(cache):
    cache.scopes_add("mine", all=True)
    newer = dict(bead("i1"), title="newer", updated_at="2026-10-24T09:00:00Z")
    cache.ingest_scope("mine", [newer], [])
    older = dict(bead("i1"), title="older", updated_at="2026-10-23T09:00:00Z")
    report = cache.ingest_scope("mine", [older], [])
    assert report["skipped_newer"] == 1
    assert cache.get("i1")["row"]["bead"]["title"] == "newer"


def test_an_empty_snapshot_is_refused_unless_allowed_and_an_unknown_scope_is_refused(cache):
    cache.scopes_add("mine", all=True)
    assert raised(cache.ingest_scope, "mine", [], []).code == "BD_CACHE.INGEST_INVALID"
    assert cache.ingest_scope("mine", [], [], allow_empty=True)["rows"] == 0
    assert raised(cache.ingest_scope, "nobody", [bead("i1")], []).code == "BD_CACHE.SCOPE_UNKNOWN"
