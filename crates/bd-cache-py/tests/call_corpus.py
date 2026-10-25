"""The 0.1 call corpus: every 0.1 verb driven against the fake `bd`, recorded as data (REQ-BDC-0812).

`build(repo)` runs a fixed script of calls on a cache over a `conftest.Repo` and records, for each
call, what was called, the shape of the reply (or the error it raised) and the `bd` argv the fake
saw. `call_corpus_0_1.json` is that record, committed. `test_call_corpus.py` rebuilds it on every
run and compares, so a change to a 0.1 reply, an error or an argv is a diff in review, and a 0.1
verb with no case fails the run. Regenerate after an intended change:

    BDC_WRITE_CORPUS=1 pytest tests/test_call_corpus.py

Nothing here is typed by hand except the script: the replies come from the wheel and the argv from
the fake. Values that change between runs (instants, paths, floats) are recorded by kind.
"""

import os
import re

import bd_cache_rs

TIMESTAMP = re.compile(r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d")


def shape(value):
    """The reply with its volatile values replaced by their kind; everything else verbatim."""
    if isinstance(value, dict):
        return {key: shape(value[key]) for key in sorted(value)}
    if isinstance(value, (list, tuple)):
        return [shape(item) for item in value]
    if isinstance(value, (set, frozenset)):
        return sorted(shape(item) for item in value)
    if value is None or isinstance(value, (bool, int)):
        return value
    if isinstance(value, float):
        return "<float>"
    if isinstance(value, str):
        return "<timestamp>" if TIMESTAMP.match(value) else value
    if isinstance(value, os.PathLike):
        return "<path>"
    return f"<{type(value).__name__}>"


def error_shape(err):
    """What a caller can read from a `CacheError`: its code, text kinds, exit code and payload."""
    return {
        "type": type(err).__name__,
        "code": err.code,
        "fix": err.fix,
        "reason": err.reason,
        "exit_code": err.exit_code,
        "payload": shape(err.payload),
        "detail_is_text": isinstance(err.detail, str) and bool(err.detail),
        "body_keys": sorted(err.body),
    }


class Recorder:
    """Runs calls against one cache and keeps the record."""

    def __init__(self, repo, cache):
        self.repo = repo
        self.cache = cache
        self.entries = []

    def run(self, verb, case, *args, **kwargs):
        """Calls `cache.<verb>(*args, **kwargs)`; records the reply or the error; returns the reply."""
        rendered = ", ".join([repr(a) for a in args] + [f"{k}={v!r}" for k, v in kwargs.items()])
        entry = {"verb": verb, "case": case, "call": f"{verb}({rendered})"}
        before = len(self.repo.bd.calls())
        reply = None
        try:
            reply = getattr(self.cache, verb)(*args, **kwargs)
            entry["reply"] = shape(reply)
        except bd_cache_rs.CacheError as err:
            entry["raises"] = error_shape(err)
        except (ValueError, TypeError) as err:
            entry["raises"] = {"type": type(err).__name__}
        entry["bd_calls"] = [
            ["<timestamp>" if TIMESTAMP.match(word) else word for word in argv]
            for argv in self.repo.bd.calls()[before:]
        ]
        self.entries.append(entry)
        return reply

    def attribute(self, name, case):
        """Records a non-call member of the handle (`path`, `closed`)."""
        self.entries.append({"verb": name, "case": case, "call": name, "reply": shape(getattr(self.cache, name)),
                             "bd_calls": []})


def build(repo):
    """The corpus: a fixed script over the `repo` fixture's world (beads b1, b2, s1 blocked by b1)."""
    repo.config('scopes = ["x"]\n')
    cache = repo.open()
    rec = Recorder(repo, cache)
    rec.run("sync", "first sync of a configured scope", "x", 30)

    # Reads.
    rec.attribute("path", "the cache file")
    rec.attribute("closed", "an open handle")
    rec.run("get", "a cached row, no bd call", "b1")
    rec.run("get", "a miss with no time to ask bd", "zz-9", miss_timeout=0)
    rec.run("get", "a bead in neither the cache nor bd", "zz-9", miss_timeout=5)
    repo.bd.add({**repo.bd.get("b1"), "id": "o1", "title": "o1", "labels": []})
    rec.run("get", "a miss filled from bd", "o1", miss_timeout=5)
    rec.run("refresh", "two ids in one bd call", ["b1", "b2"], 5)
    repo.bd.edit("b2", labels=["phase-x", "stage:dev"])
    rec.run("sync", "an incremental sync lists from the cursor", "x", 30)
    rec.run("list", "every row of the scope", "x")
    rec.run("list", "by status", "x", status="closed")
    rec.run("list", "by label, label spelling of the scope", "phase-x", None, "stage:dev")
    rec.run("list", "an unknown scope", "nope")
    rec.run("ready", "the cached ready base, no bd call", "x")
    rec.run("ready", "bd's answer when it comes in time", "x", fresh_within=10)

    # Writes and the journal.
    rec.run("write", "a close, journaled", "b1", "close", {"reason": "PASS"}, {"status": "open"}, "k-close", "dev-1")
    rec.run("write", "the same key and op again", "b1", "close", {"reason": "PASS"}, {"status": "open"}, "k-close", "dev-1")
    rec.run("write", "the same key with another op", "b1", "close", {"reason": "other"}, None, "k-close", "dev-1")
    rec.run("write", "an update with JSON text for args", "b2", "update", '{"add_labels": ["a"]}', None, "k-update", "dev-1")
    rec.run("write", "an edge", "s1", "dep_add", {"depends_on": "b2", "type": "blocks"}, None, "k-dep", "dev-1")
    rec.run("write", "an unknown op", "b1", "frobnicate", {}, None, "k-bad-op", "dev-1")
    rec.run("write", "arguments the op does not take", "b1", "update", {}, None, "k-bad-args", "dev-1")
    rec.run("write", "a precondition the cached row fails", "b2", "close", {}, {"status": "closed"}, "k-pre", "dev-1")
    rec.run("entry", "a pending entry", 1)
    rec.run("entry", "an unknown seq", 99)
    rec.run("open_work", "pending writes")
    rec.run("get", "a pending write replayed into the view", "b1")

    # Draining, escalation and what a caller does with it.
    rec.run("drain", "applies in order", 10, 20)
    rec.run("write", "an update the bead will refuse", "b2", "update", {"assignee": "dev-1"}, {"status": "open"}, "k-esc", "dev-1")
    repo.bd.edit("b2", status="in_progress")
    rec.run("drain", "escalates a failed precondition", 10, 20)
    rec.run("unannounced_escalations", "one escalated entry")
    rec.run("open_work", "an unannounced escalation")
    rec.run("mark_announced", "the escalation", 4)
    rec.run("mark_announced", "an unknown seq", 99)
    rec.run("unannounced_escalations", "none after marking")
    rec.run("entry", "an escalated entry", 4)
    rec.run("resolve", "an escalated entry", 4, "set by hand", "lead-1")
    rec.run("entry", "a resolved entry keeps the note in last_error", 4)
    rec.run("write", "a close left pending", "b2", "close", {}, None, "k-late", "dev-1")
    rec.run("drain", "a spent start gate defers without calling bd", 0, 0)

    # Alerts.
    rec.run("sync_alert", "none set", "x")
    rec.run("set_sync_alert", "set", "x", "BD_CACHE.BD_CONTRACT: bad row")
    rec.run("sync_alert", "read back, label spelling", "phase-x")
    rec.run("set_sync_alert", "cleared with None", "x", None)

    # The backend goes away and comes back.
    repo.bd.down()
    rec.run("sync", "bd unreachable reports an outage", "x", 30)
    rec.run("get", "a miss while bd is down", "zz-9", miss_timeout=5)
    rec.run("status", "an outage is recorded")
    repo.bd.down(False)
    rec.run("sync", "bd back", "x", 30)

    rec.run("status", "the end state")
    rec.run("prune", "refused while a write is pending", "x")
    rec.run("drain", "the last pending write", 10, 20)
    rec.run("prune", "a scope with nothing open", "x")
    rec.run("list", "after the prune", "x")
    cache.close()
    rec.attribute("closed", "a closed handle")
    return {
        "unreachable_codes": sorted(bd_cache_rs.UNREACHABLE),
        "verbs": sorted({entry["verb"] for entry in rec.entries}),
        "calls": rec.entries,
    }
