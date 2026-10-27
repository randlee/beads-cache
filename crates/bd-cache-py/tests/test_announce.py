"""The announcement state a caller keeps through its own sender (REQ-BDC-0910)."""


def tick(cache, sender):
    """What a caller does with the escalations: send each once, then mark it."""
    sent = []
    for entry in cache.unannounced_escalations():
        if sender(entry):
            cache.mark_announced(entry["seq"])
            sent.append(entry["seq"])
    return sent


def test_an_escalated_entry_is_announced_once_then_marked(cache, repo):
    cache.write("b2", "update", {"assignee": "dev-1"}, {"status": "open"}, "k1", "dev-1")
    repo.bd.edit("b2", status="in_progress")
    report = cache.drain(10, 20)
    assert [e["seq"] for e in report["escalated"]] == [1]
    assert cache.open_work() == {"pending": 0, "escalated_unannounced": 1}

    messages = []
    assert tick(cache, lambda e: messages.append((e["seq"], e["bead"], e["actor"], e["last_error"])) or True) == [1]
    assert tick(cache, lambda e: messages.append(e) or True) == [], "marked, not re-sent"
    assert len(messages) == 1 and messages[0][:3] == (1, "b2", "dev-1")
    assert "precondition" in messages[0][3]

    assert cache.entry(1)["announced_at"].endswith("Z")
    assert cache.unannounced_escalations() == []
    assert cache.open_work() == {"pending": 0, "escalated_unannounced": 0}
    assert cache.status()["escalated"][0]["seq"] == 1, "still listed until resolved"


def test_an_undelivered_escalation_stays_unannounced_and_is_retried(cache, repo):
    cache.write("b2", "update", {"assignee": "dev-1"}, {"status": "open"}, "k1", "dev-1")
    repo.bd.edit("b2", status="in_progress")
    cache.drain(10, 20)
    assert tick(cache, lambda e: False) == []
    assert [e["seq"] for e in cache.unannounced_escalations()] == [1]
    assert cache.open_work()["escalated_unannounced"] == 1
    assert tick(cache, lambda e: True) == [1]


def test_the_sync_alert_announces_a_failure_cause_once(cache, repo):
    cause = "BD_CACHE.SYNC_FAILED: row b1 breaks the strict model"
    sent = []

    def sync_tick():
        if cache.sync_alert("x") != cause:
            sent.append(cause)
            cache.set_sync_alert("x", cause)

    sync_tick()
    sync_tick()
    assert sent == [cause]
    cache.set_sync_alert("x", None)  # a passing sync clears it
    sync_tick()
    assert sent == [cause, cause]
