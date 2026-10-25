"""The 0.1 `get` and `entry` dicts keep their key sets (REQ-BDC-0910, sprint b-1).

The core gained `GetReply.scope` and the journal fields `last_error_code`, `note` and
`changed`; none of them reaches a 0.1 dict. A resolved entry's `last_error` is still the
text bd_cache.py writes: the error, then ` | resolved by <actor>: <note>`.
"""

GET_KEYS = {"row", "scope_last_ok_at", "pending", "outage"}
ROW_KEYS = {"bead", "stale", "blocked"}
ENTRY_KEYS = {"seq", "idem_key", "phase", "bead", "op", "args", "precondition", "actor",
              "state", "attempts", "last_error", "announced_at", "created_at", "applied_at"}


def test_the_get_dict_keeps_its_0_1_key_set(cache):
    scoped = cache.get("b1")
    assert set(scoped) == GET_KEYS, scoped
    assert set(scoped["row"]) == ROW_KEYS
    cache.write("b2", "close", {}, None, "k1", "dev-1")
    pending = cache.get("b2")
    assert set(pending) == GET_KEYS and pending["pending"] == 1


def test_the_entry_dict_keeps_its_0_1_key_set_in_every_state(cache, repo):
    cache.write("b2", "update", {"assignee": "dev-1"}, {"status": "open"}, "k1", "dev-1")
    assert set(cache.entry(1)) == ENTRY_KEYS
    repo.bd.edit("b2", status="in_progress")
    assert len(cache.drain(10, 20)["escalated"]) == 1
    escalated = cache.entry(1)
    assert set(escalated) == ENTRY_KEYS
    cache.resolve(1, "set by hand", "lead-1")
    resolved = cache.entry(1)
    assert set(resolved) == ENTRY_KEYS
    assert resolved["last_error"] == escalated["last_error"] + " | resolved by lead-1: set by hand"
    assert cache.status()["escalated"] == [], "a resolved entry is no longer listed"
