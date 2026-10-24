"""Opening the cache: the repository config, explicit arguments, the handle's life cycle."""

import json

import pytest

import bd_cache_rs


def test_the_config_file_supplies_scopes_and_the_cache_path(repo):
    repo.config('scopes = ["phase-x", "y"]\ncache_path = "state/cache.sqlite3"\nmax_attempts = 2\n')
    cache = repo.open()
    assert cache.path == repo.root / "state" / "cache.sqlite3" and (repo.root / "state").is_dir()
    assert cache.list("x")["rows"] == [] and cache.list("y")["rows"] == []


def test_the_default_cache_file_is_named_for_the_dolt_database(repo):
    cache = repo.open()
    assert cache.path == repo.root / ".bd-cache" / "atm-dev.bd-cache.sqlite3"


def test_explicit_arguments_override_the_config_file(repo):
    repo.config('scopes = ["x"]\n')
    cache = repo.open(scopes=["z"], cache_path="other.sqlite3", max_attempts=1)
    assert cache.path == repo.root / "other.sqlite3"
    assert cache.list("z")["rows"] == []
    with pytest.raises(bd_cache_rs.CacheError) as caught:
        cache.list("x")
    assert caught.value.payload == {"known": ["z"]}
    cache.write("b1", "close", {}, None, "k1", "dev-1")


def test_max_attempts_from_the_arguments_decides_when_a_failing_write_escalates(repo):
    repo.config('scopes = ["x"]\n')
    patient = repo.open()
    patient.sync("x", 30)
    repo.bd.fail("update", "constraint failed")
    patient.write("b1", "update", {"assignee": "dev-1"}, None, "k1", "dev-1")
    report = patient.drain(10, 20)
    assert report["retrying"] == [1] and report["escalated"] == [], "the default allows five attempts"
    assert patient.entry(1)["attempts"] == 1

    impatient = repo.reopen(patient, max_attempts=1)
    impatient.write("b2", "update", {"assignee": "dev-1"}, None, "k2", "dev-1")
    report = impatient.drain(10, 20)
    escalated = sorted(e["seq"] for e in report["escalated"])
    assert escalated == [1, 2], "one attempt is all it gets: the earlier entry, at its second, and the new one"


def test_an_invalid_config_file_is_a_value_error_naming_the_file(repo):
    repo.config("surprise = 1\n")
    with pytest.raises(ValueError, match="bd-cache.toml"):
        repo.open()


def test_an_invalid_scope_argument_is_a_value_error(repo):
    with pytest.raises(ValueError, match="scopes"):
        repo.open(scopes=["bad scope"])


def test_a_repository_without_beads_metadata_is_no_database(tmp_path):
    (tmp_path / ".beads").mkdir()
    with pytest.raises(bd_cache_rs.CacheError) as caught:
        bd_cache_rs.Cache(str(tmp_path))
    assert caught.value.code == "BD_CACHE.NO_DATABASE"
    assert "BEADS_DIR" in caught.value.fix


def test_a_cache_file_of_another_project_is_refused(repo):
    cache = repo.open()
    path = cache.path
    cache.close()
    (repo.root / ".beads" / "metadata.json").write_text(
        json.dumps({"dolt_database": "atm-dev", "project_id": "p-2"}), encoding="utf-8")
    with pytest.raises(bd_cache_rs.CacheError) as caught:
        repo.open()
    assert caught.value.code == "BD_CACHE.WRONG_DATABASE"
    assert path.name == "atm-dev.bd-cache.sqlite3"


def test_a_second_open_of_one_file_in_a_process_is_refused_until_the_first_closes(repo):
    first = repo.open()
    with pytest.raises(bd_cache_rs.CacheError) as caught:
        repo.open()
    assert caught.value.code == "BD_CACHE.STORE_FAILED"
    first.close()
    second = repo.open()
    assert second.closed is False


def test_close_ends_the_handle_and_is_idempotent(repo):
    cache = repo.open()
    assert cache.closed is False
    cache.close()
    cache.close()
    assert cache.closed is True
    with pytest.raises(RuntimeError, match="closed"):
        cache.status()


def test_the_handle_is_a_context_manager(repo):
    with repo.open() as cache:
        assert cache.status()["meta"]["dolt_database"] == "atm-dev"
    assert cache.closed is True


def test_a_missing_bd_is_an_outage_not_a_crash(repo):
    cache = repo.open(bd_binary=str(repo.root / "no-such-bd"))
    with pytest.raises(bd_cache_rs.CacheError) as caught:
        cache.get("b1", miss_timeout=2)
    assert caught.value.code == "BD_CACHE.BD_UNAVAILABLE"


def test_the_module_reports_the_wheel_version():
    assert bd_cache_rs.__version__.count(".") == 2
