"""Read paths: refresh with the overlay, entries in any state, on-demand scopes (REQ-BDC-0910)."""

import pytest

import bd_cache_rs
from conftest import bead


def test_refresh_returns_fresh_rows_with_the_overlay(cache, repo):
    cache.write("b2", "update", {"assignee": "dev-1"}, {"status": "open"}, "k1", "dev-1")
    repo.bd.edit("b2", title="renamed in bd")
    assert cache.get("b2")["row"]["bead"]["title"] == "b2", "the cache lags bd until a refresh"
    [row] = cache.refresh(["b2"], 5)
    assert row["bead"]["title"] == "renamed in bd", "fresh from bd"
    assert row["bead"]["assignee"] == "dev-1", "with the pending write replayed on top"
    assert row["stale"] is False
    stored = cache.get("b2")
    assert stored["row"]["bead"]["title"] == "renamed in bd", "stored with force"
    assert stored["pending"] == 1


def test_refresh_stores_a_row_older_than_the_cached_one(cache, repo):
    # `force` stores bd's answer even when its `updated_at` is not newer.
    repo.bd.edit("b1", title="older", updated_at="2020-01-01T00:00:00Z")
    [row] = cache.refresh(["b1"], 5)
    assert row["bead"]["title"] == "older"
    assert cache.get("b1")["row"]["bead"]["title"] == "older"


def test_refresh_of_a_row_bd_does_not_know_returns_nothing_for_it(cache):
    assert cache.refresh(["zz-9"], 5) == []


def test_entry_returns_an_entry_in_any_state(cache, repo):
    pending = cache.write("b1", "close", {"reason": "PASS"}, {"status": "open"}, "k-applied", "dev-1")["seq"]
    escalating = cache.write("b2", "update", {"assignee": "dev-1"}, {"status": "open"}, "k-esc", "dev-1")["seq"]
    assert cache.entry(pending)["state"] == "pending"
    repo.bd.edit("b2", status="in_progress")
    report = cache.drain(10, 20)
    assert report["applied"] == [pending] and [e["seq"] for e in report["escalated"]] == [escalating]
    applied = cache.entry(pending)
    assert applied["state"] == "applied" and applied["applied_at"].endswith("Z")
    escalated = cache.entry(escalating)
    assert escalated["state"] == "escalated" and "status" in escalated["last_error"]
    cache.resolve(escalating, "set by hand", "lead-1")
    assert cache.entry(escalating)["state"] == "resolved"
    assert cache.entry(pending)["state"] == "applied"


def test_sync_then_list_of_an_unconfigured_phase_works(repo):
    repo.bd.add(bead("y1", labels=["phase-y"]), bead("y2", labels=["phase-y"]))
    cache = repo.open()  # no .beads/bd-cache.toml: no configured scope
    with pytest.raises(bd_cache_rs.CacheError) as caught:
        cache.list("y")
    assert caught.value.code == "BD_CACHE.SCOPE_UNKNOWN" and caught.value.payload == {"known": []}
    report = cache.sync("y", 30)
    assert report["scope"] == "y" and report["stored"] == 2
    listed = cache.list("y")
    assert [r["bead"]["id"] for r in listed["rows"]] == ["y1", "y2"]
    assert listed["last_ok_at"].endswith("Z")
    assert cache.ready("y")["scope"] == "y"


def test_prune_deletes_a_synced_phase_after_the_journal_drained(cache):
    cache.write("b1", "close", {}, None, "k1", "dev-1")
    cache.drain(10, 20)
    report = cache.prune("x")
    assert report["beads_deleted"] == 3
    assert cache.status()["journal"] == {}, "the phase's applied entries go with it"


# The 0.1 bare-name rule (REQ-BDC-0812 as amended, ADR-BDC-0013, AC4): on a cache
# holding the phase-a scope stored as `x` (label `phase-x`), a 0.1 verb given `x` or
# `phase-x` reads it; the new interfaces take the name as written.


def test_a_0_1_verb_reads_the_phase_a_scope_by_its_bare_or_prefixed_name(cache):
    for name in ("x", "phase-x"):
        listed = cache.list(name)
        assert [r["bead"]["id"] for r in listed["rows"]] == ["b1", "b2", "s1"], name
        assert listed["scope"] == "x"
        assert cache.ready(name)["scope"] == "x"
    # The 0.2 namespace takes the name as written: `x` reads it, `phase-x` does not.
    assert [r["id"] for r in cache.beads.issues_list(scope="x")["items"]] == ["b1", "b2", "s1"]
    with pytest.raises(bd_cache_rs.CacheError) as caught:
        cache.beads.issues_list(scope="phase-x")
    assert caught.value.code == "BD_CACHE.SCOPE_UNKNOWN"


def test_a_0_1_read_of_an_unknown_name_is_scope_unknown_and_registers_nothing(repo):
    cache = repo.open()  # no configured scope
    for name in ("nope", "phase-nope"):
        with pytest.raises(bd_cache_rs.CacheError) as caught:
            cache.list(name)
        assert caught.value.code == "BD_CACHE.SCOPE_UNKNOWN", name
        with pytest.raises(bd_cache_rs.CacheError) as caught:
            cache.ready(name)
        assert caught.value.code == "BD_CACHE.SCOPE_UNKNOWN", name
    assert cache.status()["phases"] == [], "a read registers nothing"


def test_the_0_1_sync_of_a_prefixed_name_registers_the_phase_a_scope(repo):
    repo.bd.add(bead("y1", labels=["phase-y"]))
    cache = repo.open()
    report = cache.sync("phase-y", 30)
    assert report["scope"] == "y" and report["stored"] == 1
    assert [r["bead"]["id"] for r in cache.list("y")["rows"]] == ["y1"]
    assert [r["bead"]["id"] for r in cache.list("phase-y")["rows"]] == ["y1"]


def test_a_bare_0_1_name_never_means_a_configured_filter_scope_of_that_name(repo):
    # `lane` is a filter scope of the configuration, not the phase-a scope `lane`.
    repo.bd.add(bead("l1", labels=["lane"]), bead("p1", labels=["phase-lane"]))
    repo.config('[[scope]]\nname = "lane"\nlabels_all = ["lane"]\n')
    cache = repo.open()
    assert cache.manage.refresh_scopes(scopes=["lane"])["results"][0]["error"] is None
    assert [r["id"] for r in cache.beads.issues_list(scope="lane")["items"]] == ["l1"]
    for read in (cache.list, cache.ready):
        with pytest.raises(bd_cache_rs.CacheError) as caught:
            read("lane")
        assert caught.value.code == "BD_CACHE.SCOPE_UNKNOWN"
    with pytest.raises(bd_cache_rs.CacheError) as caught:
        cache.sync("lane", 30)
    assert caught.value.code == "BD_CACHE.SCOPE_UNKNOWN"
    assert [r["id"] for r in cache.beads.issues_list(scope="lane")["items"]] == ["l1"], "unchanged"


def test_the_0_1_prefix_rule_is_applied_once(repo):
    # `phase-phase-z` names the phase-a scope stored as `phase-z` (label
    # `phase-phase-z`); stripping twice would read `z`, another scope (B7Q1-012).
    repo.bd.add(bead("pz1", labels=["phase-phase-z"]), bead("z1", labels=["phase-z"]))
    cache = repo.open()
    report = cache.sync("phase-phase-z", 30)
    assert report["scope"] == "phase-z" and report["stored"] == 1
    listed = cache.list("phase-phase-z")
    assert listed["scope"] == "phase-z"
    assert [r["bead"]["id"] for r in listed["rows"]] == ["pz1"]
    assert cache.ready("phase-phase-z")["scope"] == "phase-z"
    with pytest.raises(bd_cache_rs.CacheError) as caught:
        cache.list("z")
    assert caught.value.code == "BD_CACHE.SCOPE_UNKNOWN", "z was never synced or configured"
    assert cache.prune("phase-phase-z")["scope"] == "phase-z"
