"""Typed errors raise `CacheError` with code, detail, fix, reason and payload (REQ-BDC-0802, 0803, 0910)."""

import pytest

import bd_cache_rs


def raised(call, *args, **kwargs):
    with pytest.raises(bd_cache_rs.CacheError) as caught:
        call(*args, **kwargs)
    return caught.value


def test_a_precondition_error_carries_unmet_with_expected_and_actual(cache, repo):
    repo.bd.edit("b2", status="in_progress")
    cache.refresh(["b2"], 5)
    err = raised(cache.write, "b2", "close", {}, {"status": "open", "assignee": ""}, "k1", "dev-1")
    assert err.code == "BD_CACHE.PRECONDITION"
    assert err.fix == "re-read the bead and decide again"
    assert err.reason == "precondition_failed"
    assert "b2" in err.detail and str(err) == f"{err.code}: {err.detail}"
    assert err.payload == {"unmet": {"status": {"expected": "open", "actual": "in_progress"}}}
    assert err.exit_code == 3
    assert err.body == {"ok": False, "code": err.code, "detail": err.detail, "fix": err.fix,
                        "reason": "precondition_failed", **err.payload}
    assert cache.status()["journal"] == {}, "nothing was journaled"


def test_the_error_is_an_exception_class_of_the_module():
    assert issubclass(bd_cache_rs.CacheError, Exception)
    assert bd_cache_rs.CacheError.__module__ == "bd_cache_rs"


def test_an_unknown_op_is_a_value_error_and_bad_args_are_typed(cache):
    with pytest.raises(ValueError, match="op.*close, update, dep_add"):
        cache.write("b1", "frobnicate", {}, None, "k1", "dev-1")
    assert cache.status()["journal"] == {}
    bad_args = raised(cache.write, "b1", "update", {}, None, "k2", "dev-1")
    assert bad_args.code == "BD_CACHE.BAD_ARGS" and bad_args.reason is None and bad_args.payload == {}
    not_json = raised(cache.write, "b1", "close", "{nope", None, "k3", "dev-1")
    assert not_json.code == "BD_CACHE.BAD_ARGS"


def test_a_bad_precondition_is_typed(cache):
    err = raised(cache.write, "b1", "close", {}, {"colour": "red"}, "k1", "dev-1")
    assert err.code == "BD_CACHE.BAD_PRECONDITION"
    assert "colour" in err.detail


def test_a_reused_idempotency_key_carries_the_holding_seq(cache):
    cache.write("b1", "close", {"reason": "a"}, None, "k1", "dev-1")
    err = raised(cache.write, "b1", "close", {"reason": "b"}, None, "k1", "dev-1")
    assert err.code == "BD_CACHE.IDEM_CONFLICT" and err.payload == {"seq": 1}


def test_a_bead_in_neither_cache_nor_bd_is_not_found(cache):
    err = raised(cache.get, "zz-9", miss_timeout=5)
    assert err.code == "BD_CACHE.NOT_FOUND" and err.fix == "check the bead id"


def test_an_unknown_scope_lists_the_known_ones(cache):
    err = raised(cache.list, "never")
    assert err.code == "BD_CACHE.SCOPE_UNKNOWN" and err.payload == {"known": ["x"]}
    assert raised(cache.ready, "never").code == "BD_CACHE.SCOPE_UNKNOWN"


def test_resolve_refuses_an_entry_that_is_not_escalated(cache):
    cache.write("b1", "close", {}, None, "k1", "dev-1")
    assert raised(cache.resolve, 1, "note", "lead-1").code == "BD_CACHE.NOT_ESCALATED"
    assert raised(cache.resolve, 99, "note", "lead-1").code == "BD_CACHE.NOT_ESCALATED"


def test_entry_and_mark_announced_refuse_an_unknown_seq(cache):
    assert raised(cache.entry, 99).code == "BD_CACHE.NOT_FOUND"
    assert raised(cache.mark_announced, 99).code == "BD_CACHE.NOT_FOUND"


def test_prune_is_blocked_by_unapplied_writes(cache):
    cache.write("b1", "close", {}, None, "k1", "dev-1")
    err = raised(cache.prune, "x")
    assert err.code == "BD_CACHE.PRUNE_BLOCKED" and "drain" in err.fix


def test_an_unreachable_backend_is_bd_unavailable_with_its_source(cache, repo):
    repo.bd.down()
    err = raised(cache.get, "zz-9", miss_timeout=5)
    assert err.code == "BD_CACHE.BD_UNAVAILABLE"
    assert err.payload["source"]["code"]
    assert "unreachable" in err.payload["source"]["detail"]


def test_a_sync_against_an_unreachable_backend_reports_the_outage(cache, repo):
    repo.bd.down()
    report = cache.sync("x", 5)
    assert report["outage"]["last_error"].startswith("BD_CACHE.BD_UNAVAILABLE")
    assert cache.status()["outage"]["since"].endswith("Z")


def test_a_failed_sync_is_typed_with_the_phase_and_cause(cache, repo):
    repo.bd.edit("b1", priority="high")
    err = raised(cache.sync, "x", 5)
    assert err.code == "BD_CACHE.SYNC_FAILED"
    assert err.payload["phase"] == "x" and err.payload["cause"] == "BD_CACHE.BD_CONTRACT"
    assert err.fix == "retried by the next sync; see cause"
    assert cache.status()["phases"][0]["last_error"]


def test_a_row_that_breaks_the_strict_model_is_bd_contract(cache, repo):
    repo.bd.edit("b1", priority="high")
    err = raised(cache.refresh, ["b1"], 5)
    assert err.code == "BD_CACHE.BD_CONTRACT"
    assert err.payload["field"]


def test_arguments_that_are_not_valid_values_raise_value_error(cache):
    with pytest.raises(ValueError, match="bead"):
        cache.get("has space")
    with pytest.raises(ValueError, match="actor"):
        cache.write("b1", "close", {}, None, "k1", "  ")
    with pytest.raises(ValueError, match="idem_key"):
        cache.write("b1", "close", {}, None, "", "dev-1")
    with pytest.raises(ValueError, match="phase"):
        cache.list("bad phase!")
    with pytest.raises(ValueError, match="timeout"):
        cache.sync("x", -1)
    with pytest.raises(ValueError, match="fresh_within"):
        cache.ready("x", fresh_within=float("nan"))
    with pytest.raises(ValueError, match="miss_timeout"):
        cache.get("b1", miss_timeout=-0.5)
    with pytest.raises(ValueError, match="bead"):
        cache.refresh(["b1", "a,b"], 5)
    with pytest.raises(TypeError):
        cache.refresh("b1", 5)


def test_unreachable_is_the_set_of_codes_that_mean_bd_did_not_answer(cache):
    assert bd_cache_rs.UNREACHABLE == frozenset({
        "BD_CACHE.BD_UNAVAILABLE", "BD_CACHE.BD_VERSION_FAILED",
        "BD_CACHE.BD_SCHEMA_FAILED", "BD_CACHE.BD_LIST_FAILED"})
    assert raised(cache.get, "zz-9", miss_timeout=0).code in bd_cache_rs.UNREACHABLE


def test_a_zero_miss_timeout_is_a_cached_only_get(cache, repo):
    n = len(repo.bd.calls())
    assert cache.get("b1", miss_timeout=0)["row"]["bead"]["id"] == "b1"
    assert raised(cache.get, "zz-9", miss_timeout=0).code == "BD_CACHE.BD_UNAVAILABLE"
    assert len(repo.bd.calls()) == n, "neither a hit nor a zero-time miss calls bd"
