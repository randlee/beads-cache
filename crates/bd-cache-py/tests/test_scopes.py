"""The scope registry through the PyO3 handle (ADR-BDC-0013, ADR-BDC-0015, ADR-PY-0006)."""

import pytest

import bd_cache_rs


def raised(call, *args, **kwargs):
    with pytest.raises(bd_cache_rs.CacheError) as caught:
        call(*args, **kwargs)
    return caught.value


def test_scopes_add_set_list_remove_and_list_is_the_cache_scopes_reply(cache):
    added = cache.scopes_add("bugs", labels_any=["bug"], status=["open"])
    assert added == {"scope": "bugs", "filter": added["filter"]}
    assert added["filter"]["labels_any"] == ["bug"]
    assert raised(cache.scopes_add, "bugs", all=True).code == "BD_CACHE.BAD_ARGS", "a second add"
    cache.scopes_set("bugs", all=True)
    listed = cache.scopes_list()
    assert listed == cache.manage.scopes(), "the alias's reply"
    bugs = [s for s in listed["scopes"] if s["scope"] == "bugs"][0]
    assert bugs["filter"]["all"] is True
    assert cache.scopes_remove("bugs")["removed"] is True
    assert all(s["scope"] != "bugs" for s in cache.scopes_list()["scopes"])
    assert raised(cache.scopes_set, scope="bugs", all=True).code == "BD_CACHE.SCOPE_UNKNOWN"


def test_a_configured_scope_cannot_be_changed_or_removed_through_the_registry(cache):
    assert raised(cache.scopes_remove, "x").code == "BD_CACHE.CONFIG_INVALID"
    assert raised(cache.scopes_set, "x", all=True).code == "BD_CACHE.CONFIG_INVALID"


def test_a_new_scope_argument_takes_the_name_as_written(cache):
    # Planted negative of the 0.1 rule: the prefixed spelling is another scope here.
    cache.scopes_add("phase-x", all=True)
    names = {s["scope"] for s in cache.scopes_list()["scopes"]}
    assert {"x", "phase-x"} <= names


def test_the_new_methods_are_bound_callables_and_dir_cache_is_still_the_0_1_set(cache):
    public = {n for n in dir(bd_cache_rs.Cache) if not n.startswith("_")}
    for name in ("tick", "schedule_set", "scopes_add", "ingest_scope", "sync_use_python"):
        assert name not in public
        assert callable(getattr(cache, name))
    with pytest.raises(AttributeError):
        cache.scopes_frobnicate
    with pytest.raises(TypeError):
        cache.scopes_remove("a", "b")
