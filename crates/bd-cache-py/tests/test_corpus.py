"""The neutral 0.1 call corpus is current and covers every 0.1 verb (REQ-BDC-0812, ADR-PY-0006).

`corpus/record.py` drives the 0.1 verbs against the fake `bd`; `corpus/corpus_0_1.json` is the
committed record of that run. Rebuilding it here means a changed reply, error or `bd` argv is a
diff, and a verb added to the handle without a case fails. The corpus replaces the vendored call
log REQ-PY-0009 read (superseded): it is generated in this repository and names no consumer.
"""

import json
import os
from pathlib import Path

import bd_cache_rs
from conftest import bead
from corpus.record import BARE_NAME_CASE, LEGACY_LABEL_Y, build

HERE = Path(__file__).resolve().parent
CORPUS = HERE / "corpus" / "corpus_0_1.json"
VERBS_01 = {"get", "refresh", "write", "list", "ready", "sync", "drain", "entry", "resolve", "prune",
            "status", "unannounced_escalations", "mark_announced", "sync_alert", "set_sync_alert",
            "open_work"}


def render(corpus):
    return json.dumps(corpus, indent=1) + "\n"


def committed():
    return json.loads(CORPUS.read_text(encoding="utf-8"))


def test_the_committed_corpus_is_what_the_fake_bd_run_produces(repo):
    built = render(build(repo))
    if os.environ.get("BDC_WRITE_CORPUS"):
        CORPUS.write_text(built, encoding="utf-8")
    assert built == CORPUS.read_text(encoding="utf-8"), (
        "the 0.1 replies changed; if intended, regenerate with BDC_WRITE_CORPUS=1 pytest tests/test_corpus.py")


def test_every_0_1_verb_has_a_success_case_and_the_error_paths_a_failing_one():
    corpus = committed()
    members = {name for name in dir(bd_cache_rs.Cache) if not name.startswith("_")}
    members -= {"close"}  # the call that ends the corpus; its effect is the `closed` case
    assert members == VERBS_01 | {"path", "closed"}, "the 0.1 handle changed"
    assert set(corpus["verbs"]) == members, "a handle member with no case, or a case with no member"
    succeeded = {c["verb"] for c in corpus["calls"] if "reply" in c}
    assert succeeded == members, f"no successful call recorded for {sorted(members - succeeded)}"
    failing = {c["verb"] for c in corpus["calls"] if "raises" in c}
    assert {"get", "write", "list", "entry", "mark_announced", "prune"} <= failing


def test_the_corpus_records_the_bd_argv_of_the_verbs_that_reach_bd():
    corpus = committed()
    reached = {c["verb"] for c in corpus["calls"] if c["bd_calls"]}
    assert {"get", "refresh", "sync", "drain", "ready"} <= reached
    drained = [c for c in corpus["calls"] if c["verb"] == "drain" and c["bd_calls"]]
    assert any(argv[0] in ("update", "close") for c in drained for argv in c["bd_calls"])
    no_bd = [c for c in corpus["calls"] if c["case"] == "a cached row, no bd call"]
    assert no_bd and no_bd[0]["bd_calls"] == [], "a cached read starts no bd (REQ-BDC-0001)"


def test_the_corpus_records_the_error_fields_and_the_unreachable_codes():
    corpus = committed()
    errors = [c["raises"] for c in corpus["calls"] if c.get("raises", {}).get("code")]
    assert errors and all(e["detail_is_text"] and e["fix"] and "payload" in e for e in errors)
    assert {"BD_CACHE.PRECONDITION", "BD_CACHE.IDEM_CONFLICT", "BD_CACHE.SCOPE_UNKNOWN",
            "BD_CACHE.NOT_FOUND", "BD_CACHE.PRUNE_BLOCKED"} <= {e["code"] for e in errors}
    assert corpus["unreachable_codes"] == sorted(bd_cache_rs.UNREACHABLE)
    assert any(c.get("raises", {}).get("type") == "ValueError" for c in corpus["calls"]), "an unknown op"


def bare_name_entry(corpus):
    (entry,) = [c for c in corpus["calls"] if c["case"] == BARE_NAME_CASE]
    return entry


def test_the_bare_name_case_syncs_the_legacy_prefixed_label():
    entry = bare_name_entry(committed())
    assert entry["call"] == "sync('y', 30)"
    assert entry["reply"]["scope"] == "y" and entry["reply"]["stored"] == 1, entry
    listed = [argv for argv in entry["bd_calls"] if argv[0] == "list"]
    assert listed and listed[0][1:3] == ["-l", LEGACY_LABEL_Y], entry


def test_without_the_bare_name_rule_the_case_does_not_match_the_corpus(repo):
    """Planted negative (REQ-BDC-0812 as amended): the corpus fails if the 0.1 rule is dropped.

    Without the rule a bare `y` would be the scope named as written, which the cache registers on
    demand as the label scope of the label `y` (ADR-BDC-0013). The 0.2 namespace takes names
    that way, so the same case run through it is the case as it would run with the rule dropped:
    its record differs from the committed one in the label bd is asked for and the rows stored.
    """
    repo.config('scopes = ["x"]\n')
    cache = repo.open()
    repo.bd.add(bead("y1", labels=(LEGACY_LABEL_Y,)))
    before = len(repo.bd.calls())
    result = cache.manage.refresh_scopes(scopes=["y"])["results"][0]
    without_rule = {
        "scope": result["scope"],
        "stored": result["report"]["stored"],
        "bd_calls": [argv for argv in repo.bd.calls()[before:] if argv[0] == "list"],
    }
    recorded = bare_name_entry(committed())
    with_rule = {
        "scope": recorded["reply"]["scope"],
        "stored": recorded["reply"]["stored"],
        "bd_calls": [argv for argv in recorded["bd_calls"] if argv[0] == "list"],
    }
    assert without_rule["scope"] == with_rule["scope"] == "y"
    assert without_rule != with_rule, "the corpus cannot tell the rule from its absence"
    assert without_rule["stored"] == 0 and without_rule["bd_calls"][0][1:3] == ["-l", "y"], without_rule
    # The control is the corpus test above: the 0.1 verb on the same world matches the record.
