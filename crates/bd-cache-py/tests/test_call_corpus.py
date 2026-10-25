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


def test_the_contract_table_names_real_0_1_verbs_and_real_0_2_methods(cache):
    """Contract section 10: each 0.1 verb is on `Cache`, each 0.2 method is on its namespace."""
    section = CONTRACT.read_text(encoding="utf-8").split("## 10.", 1)[1].split("\n## ", 1)[0]
    handle = {name for name in dir(bd_cache_rs.Cache) if not name.startswith("_")}
    named = 0
    for line in section.splitlines():
        cells = [c.strip() for c in line.strip().strip("|").split(" | ")]
        if len(cells) < 2 or not cells[0].startswith("`"):
            continue
        for verb in re.findall(r"`(\w+)\(", cells[0]):
            assert verb in handle, f"{verb}: not a Cache member"
            named += 1
        for namespace, method in re.findall(r"`cache\.(beads|manage)\.(\w+)", cells[1]):
            assert hasattr(getattr(cache, namespace), method), f"cache.{namespace}.{method} does not exist"
    assert named >= 10
