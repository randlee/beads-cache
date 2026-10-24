"""Every verb of the module, driven against the real composition (REQ-BDC-0910).

Each test checks the fields of the reply it gets and, where the verb reaches
`bd`, the argv the recording fake `bd` saw.
"""

import time

import pytest

import bd_cache_rs

ALL = "open,in_progress,blocked,deferred,closed,pinned,hooked"


def ids(rows):
    return [row["bead"]["id"] for row in rows]


def test_get_serves_a_cached_row_without_calling_bd(cache, repo):
    before = len(repo.bd.calls())
    reply = cache.get("b1")
    assert set(reply) == {"row", "scope_last_ok_at", "pending", "outage"}
    assert set(reply["row"]) == {"bead", "stale", "blocked"}
    assert reply["row"]["bead"]["id"] == "b1"
    assert reply["row"]["bead"]["status"] == "open"
    assert reply["row"]["stale"] is False
    assert reply["row"]["blocked"] == []
    assert reply["pending"] == 0
    assert reply["outage"] is None
    assert reply["scope_last_ok_at"].endswith("Z")
    assert len(repo.bd.calls()) == before


def test_get_miss_reads_bd_once_then_serves_from_the_cache(cache, repo):
    repo.bd.add({"id": "o1", "title": "o1", "status": "open", "priority": 2, "issue_type": "task",
                 "created_at": "2026-10-23T09:00:00Z", "updated_at": "2026-10-23T09:00:00Z"})
    first = cache.get("o1", miss_timeout=5)
    assert first["row"]["bead"]["id"] == "o1"
    assert repo.bd.calls()[-1] == ["list", "--id", "o1", "--status", ALL, "-n", "0", "--json"]
    n = len(repo.bd.calls())
    cache.get("o1")
    assert len(repo.bd.calls()) == n


def test_get_replays_pending_writes_into_the_view(cache):
    cache.write("b2", "update", {"assignee": "dev-1"}, {"status": "open"}, "k-assign", "dev-1")
    reply = cache.get("b2")
    assert reply["pending"] == 1
    assert reply["row"]["bead"]["assignee"] == "dev-1"


def test_refresh_reads_bd_with_one_call_for_all_ids(cache, repo):
    rows = cache.refresh(["b1", "b2"], 5)
    assert ids(rows) == ["b1", "b2"]
    assert set(rows[0]) == {"bead", "stale", "blocked"}
    assert repo.bd.calls()[-1] == ["list", "--id", "b1,b2", "--status", ALL, "-n", "0", "--json"]


def test_write_returns_the_seq_and_deduplicates_on_the_key(cache):
    first = cache.write("b1", "close", {"reason": "PASS"}, {"status": "open"}, "k-close", "dev-1")
    assert first == {"seq": 1, "duplicate": False}
    again = cache.write("b1", "close", {"reason": "PASS"}, {"status": "open"}, "k-close", "dev-1")
    assert again == {"seq": 1, "duplicate": True}
    other = cache.write("b2", "close", "{}", None, "k-other", "dev-1")
    assert other == {"seq": 2, "duplicate": False}


def test_write_accepts_json_text_dicts_and_keyword_arguments(cache):
    text = cache.write("b1", "update", '{"add_labels": ["a"]}', '{"status": "open"}', "k-text", "dev-1")
    keyword = cache.write(
        bead="b2", op="update", args={"add_labels": ["b"]}, precondition={"status": "open"},
        idem_key="k-kw", actor="dev-1")
    assert (text["seq"], keyword["seq"]) == (1, 2)
    assert cache.entry(1)["args"] == {"add_labels": ["a"]}
    assert cache.entry(2)["precondition"] == {"status": "open"}


def test_list_filters_by_status_and_label(cache, repo):
    repo.bd.edit("b2", labels=["phase-x", "stage:dev"])
    cache.sync("x", 30)
    everything = cache.list("x")
    assert set(everything) == {"scope", "last_ok_at", "rows", "outage"}
    assert everything["scope"] == "x"
    assert ids(everything["rows"]) == ["b1", "b2", "s1"]
    assert ids(cache.list("x", status="open")["rows"]) == ["b1", "b2", "s1"]
    assert ids(cache.list("x", status="closed")["rows"]) == []
    assert ids(cache.list("x", label="stage:dev")["rows"]) == ["b2"]
    assert ids(cache.list("phase-x", None, "stage:dev")["rows"]) == ["b2"]
    blocked = {row["bead"]["id"]: row["blocked"] for row in everything["rows"]}
    assert blocked == {"b1": [], "b2": [], "s1": ["b1"]}


def test_ready_lists_the_unblocked_beads_from_the_cached_base(cache, repo):
    before = len(repo.bd.calls())
    reply = cache.ready("x")
    assert set(reply) == {"scope", "last_ok_at", "basis", "locally_adjusted", "ready",
                          "read_fallback", "missing_rows"}
    assert ids(reply["ready"]) == ["b1", "b2"]
    assert reply["basis"]["kind"] == "cached" and reply["basis"]["stale"] is False
    assert reply["locally_adjusted"] is False and reply["read_fallback"] is None
    assert reply["missing_rows"] == []
    assert len(repo.bd.calls()) == before, "no limit, no bd call"


def test_ready_with_a_limit_uses_bds_answer_when_it_comes_in_time(cache, repo):
    repo.bd.edit("b1", status="closed")
    reply = cache.ready("x", fresh_within=10)
    assert reply["basis"]["kind"] == "fresh" and reply["read_fallback"] is None
    assert ids(reply["ready"]) == ["b2", "s1"]
    assert repo.bd.commands()[-2:] == ["blocked", "ready"]


def test_sync_reports_what_it_stored(cache, repo):
    time.sleep(1.1)  # bd's `updated_at` has second resolution; the cursor is the last sync's start
    repo.bd.edit("b2", title="renamed")
    report = cache.sync("x", 30)
    assert set(report) == {"scope", "rebuilt", "model_current", "stored", "blocked", "deferred", "outage"}
    assert report["scope"] == "x" and report["rebuilt"] is False and report["model_current"] is True
    assert report["stored"] == 1 and report["blocked"] == 1
    assert report["deferred"] is False and report["outage"] is None
    assert "--updated-after" in repo.bd.calls()[-3], "an incremental sync lists from the cursor"
    assert cache.get("b2")["row"]["bead"]["title"] == "renamed"


def test_drain_applies_pending_writes_in_order(cache, repo):
    cache.write("b1", "update", {"assignee": "dev-1"}, {"status": "open"}, "k1", "dev-1")
    cache.write("b1", "close", {"reason": "PASS"}, {"status": "open"}, "k2", "dev-1")
    report = cache.drain(10, 20)
    assert set(report) == {"busy", "applied", "retrying", "escalated", "deferred",
                           "deferred_in_flight", "outage"}
    assert report["applied"] == [1, 2] and report["busy"] is False
    assert report["retrying"] == report["escalated"] == report["deferred"] == []
    assert report["deferred_in_flight"] == [] and report["outage"] is None
    writes = [argv for argv in repo.bd.calls() if argv[0] in ("update", "close")]
    assert writes == [
        ["update", "b1", "--assignee", "dev-1", "--if-status", "open", "--actor", "dev-1"],
        ["close", "b1", "--reason", "PASS", "--actor", "dev-1"],
    ]
    assert repo.bd.get("b1")["status"] == "closed"
    assert cache.open_work() == {"pending": 0, "escalated_unannounced": 0}


def test_drain_with_a_spent_start_gate_defers_without_calling_bd(cache, repo):
    cache.write("b1", "close", {}, None, "k1", "dev-1")
    n = len(repo.bd.calls())
    report = cache.drain(0, 0)
    assert report["deferred"] == [1] and report["applied"] == []
    assert len(repo.bd.calls()) == n
    assert cache.entry(1)["state"] == "pending" and cache.entry(1)["attempts"] == 0


def test_drain_start_gate_and_hard_deadline_are_separate_bounds(cache, repo):
    cache.write("b1", "close", {}, None, "k1", "dev-1")
    n = len(repo.bd.calls())
    # The gate is spent, the hard deadline is not: nothing starts.
    report = cache.drain(0, 20)
    assert report["deferred"] == [1] and report["deferred_in_flight"] == []
    assert len(repo.bd.calls()) == n
    # The gate is open, a slow bd runs into the hard deadline: the call is cut, not an attempt.
    repo.bd.delay("*", 3)
    report = cache.drain(20, 0.5)
    assert report["applied"] == [] and report["escalated"] == []
    assert cache.entry(1)["attempts"] == 0


def test_sync_stops_at_its_timeout_when_bd_answers_late(cache, repo, stopwatch):
    repo.bd.delay("*", 5)
    with stopwatch() as watch:
        report = cache.sync("x", 0.4)
    assert watch.elapsed < 2.0, f"sync took {watch.elapsed:.2f}s with a 0.4 s timeout"
    assert report["deferred"] or report["outage"], report


def test_entry_returns_every_field_of_a_journal_entry(cache):
    cache.write("b2", "update", {"status": "in_progress"}, {"assignee": ""}, "k-fields", "dev-1")
    entry = cache.entry(1)
    assert set(entry) == {"seq", "idem_key", "phase", "bead", "op", "args", "precondition", "actor",
                          "state", "attempts", "last_error", "announced_at", "created_at", "applied_at"}
    assert entry["seq"] == 1 and entry["idem_key"] == "k-fields" and entry["phase"] == "x"
    assert entry["bead"] == "b2" and entry["op"] == "update" and entry["actor"] == "dev-1"
    assert entry["args"] == {"status": "in_progress"} and entry["precondition"] == {"assignee": ""}
    assert entry["state"] == "pending" and entry["attempts"] == 0
    assert entry["last_error"] is None and entry["announced_at"] is None and entry["applied_at"] is None
    assert entry["created_at"].endswith("Z")


def test_resolve_closes_an_escalated_entry_and_keeps_the_note(cache, repo):
    cache.write("b2", "update", {"assignee": "dev-1"}, {"status": "open"}, "k1", "dev-1")
    repo.bd.edit("b2", status="in_progress")
    assert len(cache.drain(10, 20)["escalated"]) == 1
    assert cache.resolve(1, "set by hand", "lead-1") is None
    entry = cache.entry(1)
    assert entry["state"] == "resolved"
    assert "set by hand" in entry["last_error"] and "lead-1" in entry["last_error"]


def test_prune_removes_the_phase_once_nothing_is_open(cache):
    report = cache.prune("x")
    assert report["scope"] == "x" and report["beads_deleted"] == 3
    assert set(report) == {"scope", "beads_deleted", "out_of_scope_swept"}
    assert cache.list("x")["rows"] == []


def test_status_reports_meta_journal_phases_escalations_and_outage(cache):
    cache.write("b1", "close", {}, None, "k1", "dev-1")
    status = cache.status()
    assert set(status) == {"meta", "journal", "phases", "escalated", "outage", "stale"}
    assert status["meta"]["dolt_database"] == "atm-dev" and status["meta"]["bd_version"] == "1.3.1"
    assert status["journal"] == {"x:pending": 1}
    assert [p["phase"] for p in status["phases"]] == ["x"] and status["phases"][0]["beads"] == 3
    assert status["escalated"] == [] and status["outage"] is None and status["stale"] == []


def test_the_announcement_state_round_trips(cache):
    assert cache.sync_alert("x") is None
    assert cache.set_sync_alert("x", "BD_CACHE.BD_CONTRACT: bad row") is None
    assert cache.sync_alert("x") == "BD_CACHE.BD_CONTRACT: bad row"
    assert cache.sync_alert("phase-x") == "BD_CACHE.BD_CONTRACT: bad row"
    cache.set_sync_alert("x", None)
    assert cache.sync_alert("x") is None
    assert cache.unannounced_escalations() == []
    assert cache.open_work() == {"pending": 0, "escalated_unannounced": 0}
    cache.write("b1", "close", {}, None, "k1", "dev-1")
    assert cache.open_work() == {"pending": 1, "escalated_unannounced": 0}


def test_the_verbs_are_exactly_the_documented_set():
    verbs = {"get", "refresh", "write", "list", "ready", "sync", "drain", "entry", "resolve", "prune",
             "status", "unannounced_escalations", "mark_announced", "sync_alert", "set_sync_alert",
             "open_work"}
    public = {name for name in dir(bd_cache_rs.Cache) if not name.startswith("_")}
    assert public == verbs | {"path", "closed", "close"}
