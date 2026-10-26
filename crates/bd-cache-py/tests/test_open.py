"""Opening the cache: the repository config, explicit arguments, the handle's life cycle."""

import errno
import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

import bd_cache_rs


def test_the_config_file_supplies_scopes_and_the_cache_path(repo):
    repo.config('scopes = ["phase-x", "y"]\ncache_path = "state/cache.sqlite3"\nmax_attempts = 2\n')
    cache = repo.open()
    assert cache.path == repo.root / "state" / "cache.sqlite3" and (repo.root / "state").is_dir()
    assert cache.list("x")["rows"] == [] and cache.list("y")["rows"] == []


def test_the_default_cache_file_is_named_for_the_dolt_database(repo):
    cache = repo.open()
    assert cache.path == repo.root / ".bd-cache" / "dev-db.bd-cache.sqlite3"


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


def test_an_invalid_config_file_is_a_value_error_with_kind_and_path(repo):
    repo.config("surprise = 1\n")
    with pytest.raises(ValueError, match="bd-cache.toml") as caught:
        repo.open()
    assert caught.value.kind == "parse" and caught.value.path.name == "bd-cache.toml"


def test_an_invalid_scope_in_the_config_file_is_a_value_error_of_kind_scope(repo):
    repo.config('scopes = ["bad scope"]\n')
    with pytest.raises(ValueError) as caught:
        repo.open()
    assert caught.value.kind == "scope" and "bad scope" in str(caught.value)


@pytest.mark.skipif(os.name == "nt" or os.geteuid() == 0, reason="needs POSIX permissions, not root")
def test_an_unreadable_config_file_is_an_os_error_with_kind_and_path(repo):
    repo.config("scopes = []\n")
    path = repo.root / ".beads" / "bd-cache.toml"
    path.chmod(0)
    try:
        with pytest.raises(OSError) as caught:
            repo.open()
    finally:
        path.chmod(0o600)
    assert caught.value.kind == "read" and caught.value.path.name == "bd-cache.toml"
    # The OS error code and file come from the typed error, not from its text (RBP-F010).
    assert caught.value.errno == errno.EACCES, caught.value
    assert Path(caught.value.filename).name == "bd-cache.toml", caught.value


@pytest.mark.parametrize("keyword, value", [
    ("call_timeout_s", -1), ("call_timeout_s", -0.5), ("call_timeout_s", float("nan")),
    ("miss_fill_timeout_ms", -1), ("miss_fill_timeout_ms", float("inf")),
    ("max_attempts", -3), ("max_attempts", 0),
])
def test_signed_and_float_timeouts_and_attempts_are_validated(repo, keyword, value):
    with pytest.raises(ValueError, match=keyword):
        repo.open(**{keyword: value})
    assert repo.opened == [], "nothing was opened"


@pytest.mark.parametrize("keyword, value", [
    ("call_timeout_s", 0), ("call_timeout_s", 0.0), ("call_timeout_s", -0.0),
    ("miss_fill_timeout_ms", 0), ("miss_fill_timeout_ms", 0.0),
    # 1e-9 ms is 1e-12 s: below a nanosecond, so a zero duration.
    ("miss_fill_timeout_ms", 1e-9),
])
def test_a_zero_timeout_is_refused_as_the_config_file_refuses_it(repo, keyword, value):
    # PE-RSH-006 (S56): a configured timeout must be positive, from the file or an argument.
    with pytest.raises(ValueError, match=f"{keyword} must be a positive number of seconds"):
        repo.open(**{keyword: value})
    assert repo.opened == [], "nothing was opened"


def test_the_smallest_positive_timeouts_are_accepted(repo):
    cache = repo.open(call_timeout_s=0.001, miss_fill_timeout_ms=1)
    assert cache.closed is False


def test_float_timeouts_are_accepted_and_applied(repo):
    # The setup sync runs on a default-cap open, so a slow interpreter start cannot defer it;
    # only the blocked call below runs under the 0.5 s cap.
    default = repo.open()
    report = default.sync("x", 30)
    assert report["scope"] == "x" and report["stored"] > 0 and "outage" not in report, report
    cache = repo.reopen(default, call_timeout_s=0.5, miss_fill_timeout_ms=250.5, max_attempts=2)
    release = repo.bd.block("*")
    # The per-call cap (0.5 s) cuts a blocked call even though the caller allowed 30 s.
    with pytest.raises(bd_cache_rs.CacheError) as caught:
        cache.get("zz-9", miss_timeout=30)
    assert caught.value.code == "BD_CACHE.BD_UNAVAILABLE" and not release.is_set()


def test_an_invalid_scope_argument_is_a_value_error(repo):
    with pytest.raises(ValueError, match="scopes.*phase name"):
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
        json.dumps({"dolt_database": "dev-db", "project_id": "p-2"}), encoding="utf-8")
    with pytest.raises(bd_cache_rs.CacheError) as caught:
        repo.open()
    assert caught.value.code == "BD_CACHE.WRONG_DATABASE"
    assert path.name == "dev-db.bd-cache.sqlite3"


def test_a_second_open_of_one_file_in_a_process_is_refused_until_the_first_closes(repo):
    first = repo.open()
    with pytest.raises(bd_cache_rs.CacheError) as caught:
        repo.open()
    assert caught.value.code == "BD_CACHE.STORE_IN_USE"
    assert "close that handle, or reuse it" in caught.value.fix
    first.close()
    second = repo.open()
    assert second.closed is False


def test_close_ends_the_handle_and_is_idempotent(repo):
    cache = repo.open()
    assert cache.closed is False
    cache.close()
    cache.close()
    assert cache.closed is True
    with pytest.raises(bd_cache_rs.ClosedError, match="open a new bd_cache_rs.Cache") as caught:
        cache.status()
    assert isinstance(caught.value, RuntimeError)


def test_the_handle_is_a_context_manager(repo):
    with repo.open() as cache:
        assert cache.status()["meta"]["dolt_database"] == "dev-db"
    assert cache.closed is True


def test_a_missing_bd_is_an_outage_not_a_crash(repo):
    cache = repo.open(bd_binary=str(repo.root / "no-such-bd"))
    with pytest.raises(bd_cache_rs.CacheError) as caught:
        cache.get("b1", miss_timeout=2)
    assert caught.value.code == "BD_CACHE.BD_UNAVAILABLE"


def test_the_module_reports_the_wheel_version():
    assert bd_cache_rs.__version__.count(".") == 2


def test_a_repo_root_that_is_missing_is_an_argument_error_with_or_without_project_dir(repo, tmp_path):
    missing = str(tmp_path / "no-such-root")
    for kwargs in ({}, {"project_dir": str(repo.root)}):
        with pytest.raises(ValueError, match="repo_root.*does not resolve") as caught:
            bd_cache_rs.Cache(missing, bd_binary=str(repo.bd.binary), **kwargs)
        assert caught.value.kind == "argument", kwargs


def test_a_repo_root_that_is_a_file_is_an_argument_error_with_or_without_project_dir(repo, tmp_path):
    a_file = tmp_path / "a-file"
    a_file.write_text("x", encoding="utf-8")
    for kwargs in ({}, {"project_dir": str(repo.root)}):
        with pytest.raises(ValueError, match="repo_root.*is not a directory") as caught:
            bd_cache_rs.Cache(str(a_file), bd_binary=str(repo.bd.binary), **kwargs)
        assert caught.value.kind == "argument", kwargs


def _metadata(directory, database):
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "metadata.json").write_text(
        json.dumps({"dolt_database": database, "project_id": "p-1"}), encoding="utf-8")


# Run in a child process so the test sets `BEADS_DIR` and the working directory there,
# never in the pytest process. It prints the cache file and the error it met, as JSON.
_OPEN_AND_GET = """
import json, os, sys
import bd_cache_rs
repo_root, bd_binary, cwd, remove_cwd = sys.argv[1:5]
os.chdir(cwd)
if remove_cwd == "1":
    os.rmdir(cwd)
out = {}
try:
    cache = bd_cache_rs.Cache(repo_root, bd_binary=bd_binary, project_dir=repo_root, scopes=["x"])
    out["path"] = str(cache.path)
    out["status"] = cache.get("b1", miss_timeout=30)["row"]["bead"]["status"]
    cache.close()
except bd_cache_rs.CacheError as e:
    out["code"], out["detail"] = e.code, e.detail
print(json.dumps(out))
"""


def _open_in_child(repo, cwd, beads_dir, remove_cwd=False):
    env = dict(os.environ, BEADS_DIR=beads_dir)
    done = subprocess.run(
        [sys.executable, "-c", _OPEN_AND_GET, str(repo.root), str(repo.bd.binary), str(cwd),
         "1" if remove_cwd else "0"],
        env=env, capture_output=True, text=True, timeout=60, check=False)
    assert done.returncode == 0, done.stderr
    return json.loads(done.stdout)


def test_a_relative_beads_dir_names_one_directory_for_the_store_and_for_bd(repo, tmp_path):
    # PE-ARCH-001: the store resolves BEADS_DIR against the process's working directory;
    # bd runs in project_dir. Both must use the one absolute directory, so bd is given it.
    cwd = tmp_path / "elsewhere"
    _metadata(cwd / "rel", "from-cwd")
    _metadata(repo.root / "rel", "from-project-dir")  # what a relative value names for bd
    out = _open_in_child(repo, cwd, "rel")
    assert "code" not in out, out
    assert os.path.basename(out["path"]) == "from-cwd.bd-cache.sqlite3", "the store used <cwd>/rel"
    assert out["status"] == "open"
    calls = [e for e in repo.bd.events() if e["event"] == "start"]
    assert calls, "the get reached bd"
    want = os.path.realpath(cwd / "rel")
    for call in calls:
        seen = call["beads_dir"]
        assert seen is not None and os.path.isabs(seen), call
        assert os.path.realpath(os.path.join(call["cwd"], seen)) == want, call
        assert os.path.realpath(call["cwd"]) == os.path.realpath(repo.root), "bd ran in project_dir"


def test_an_unset_beads_dir_reaches_no_bd_child(repo):
    repo.config('scopes = ["x"]\n')
    repo.open().get("b1", miss_timeout=30)
    calls = [e for e in repo.bd.events() if e["event"] == "start"]
    assert calls and all(call["beads_dir"] is None for call in calls), calls


def test_a_beads_dir_that_cannot_be_made_absolute_is_no_database(repo, tmp_path):
    # The working directory is removed, so a relative BEADS_DIR has nothing to resolve against.
    gone = tmp_path / "removed-cwd"
    gone.mkdir()
    out = _open_in_child(repo, gone, "rel", remove_cwd=True)
    assert out.get("code") == "BD_CACHE.NO_DATABASE", out
    assert "cannot make BEADS_DIR rel absolute" in out["detail"], out
    assert not (repo.root / ".bd-cache").exists(), "no cache file was created"


def test_the_ambient_beads_dir_does_not_reach_a_test(repo):
    # PE-FTQ-001: conftest's autouse fixture removes it; run the suite with BEADS_DIR set
    # to a directory without metadata.json and this repository still opens.
    assert "BEADS_DIR" not in os.environ
    assert repo.open().status()["meta"]["dolt_database"] == "dev-db"
