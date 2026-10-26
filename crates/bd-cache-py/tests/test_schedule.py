"""`schedule_*` through the PyO3 handle (REQ-BDC-0720, 0731, ADR-PY-0006)."""

import os
import stat
import sys

import pytest

import bd_cache_rs


def raised(call, *args, **kwargs):
    with pytest.raises(bd_cache_rs.CacheError) as caught:
        call(*args, **kwargs)
    return caught.value


def test_schedule_set_show_enable_disable_clear_round_trip(cache):
    written = cache.schedule_set("*/30 * * * * *", budget_ms=20000)
    assert written["enabled"] is True and written["timeout_ms"] == 30000
    show = cache.schedule_show()
    assert show["mode"] == "builtin" and show["rows"]["builtin"]["budget_ms"] == 20000
    cache.schedule_disable()
    cache.schedule_set("0 * * * * *")
    assert cache.schedule_show()["rows"]["builtin"]["enabled"] is False, "kept disabled"
    assert "both_disabled" in cache.schedule_show()["flags"]
    cache.schedule_enable()
    assert cache.schedule_clear()["cleared"] is True
    assert cache.schedule_show()["mode"] == "none"


def test_each_schedule_set_refusal_is_schedule_invalid_naming_its_field(cache, tmp_path):
    fake = tmp_path / "python"
    fake.write_text("#!/bin/sh\n[ \"$2\" = -c ] && exit 1\nexit 0\n", encoding="utf-8")
    fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
    cases = [
        (dict(cron="no"), "cron"),
        (dict(cron="* * * * *", budget_ms=10), "budget_ms"),
        (dict(cron="* * * * *", post_sync="m:f"), "python"),
        (dict(cron="* * * * *", post_sync="m:f", python="python3"), "python"),
        (dict(cron="* * * * *", post_sync="m", python=sys.executable), "post_sync"),
        (dict(cron="* * * * *", post_sync="m:f", python=str(fake)), "python"),
    ]
    for kwargs, field in cases:
        e = raised(cache.schedule_set, **kwargs)
        assert (e.code, e.details["field"]) == ("BD_CACHE.SCHEDULE_INVALID", field), kwargs
    assert cache.schedule_show()["rows"]["builtin"] is None, "nothing written"


def test_schedule_methods_raise_precondition_inside_a_run(cache):
    bd_cache_rs._native._set_in_run(True)
    try:
        for call in (cache.schedule_show, cache.tick, lambda: cache.drain(1, 2), cache.manage.flush):
            e = raised(call)
            assert (e.code, e.details["reason"]) == ("BD_CACHE.PRECONDITION", "inside_run")
    finally:
        bd_cache_rs._native._set_in_run(False)
    assert cache.schedule_show()["mode"] == "none", "planted negative: outside a run it works"
