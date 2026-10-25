"""The 0.2 namespaces are robust and honest (QA RCR-001, RCR-002).

- A request value that nests too deep, or contains itself, is a normal Python exception, never a
  crash of the interpreter. Each case runs in a subprocess, so a regression is a non-zero exit
  (a segmentation fault) and not a dead test run.
- Every member a method's docstring names is a member the decoder accepts.
- `cache.beads` shares the slot of the 0.1 verbs: one thread at a time, the GIL released while
  `bd` runs, `ClosedError` after `close()`.
"""

import re
import subprocess
import sys

import pytest

import bd_cache_rs
from conftest import daemon, join

SCRIPT = """
import sys
import bd_cache_rs

cache = bd_cache_rs.Cache(sys.argv[1], bd_binary=sys.argv[2], scopes=["x"])
kind = sys.argv[3]
if kind == "self_list":
    value = []
    value.append(value)
elif kind == "self_dict":
    value = {}
    value["k"] = value
elif kind == "deep_list":
    value = []
    for _ in range(5000):
        value = [value]
else:
    value = {}
    for _ in range(5000):
        value = {"k": value}
try:
    cache.beads.issues_list(scope="x", label=value)
except (ValueError, RecursionError) as error:
    print("raised", type(error).__name__)
    sys.exit(0)
print("returned or raised the wrong thing")
sys.exit(1)
"""


@pytest.mark.parametrize("kind", ["self_list", "self_dict", "deep_list", "deep_dict"])
def test_a_cyclic_or_very_deep_request_raises_and_does_not_crash_the_interpreter(kind, repo):
    done = subprocess.run(
        [sys.executable, "-c", SCRIPT, str(repo.root), str(repo.bd.binary), kind],
        capture_output=True, text=True, timeout=60)
    assert done.returncode == 0, f"{kind}: exit {done.returncode}\n{done.stdout}\n{done.stderr[-800:]}"
    assert done.stdout.startswith("raised "), done.stdout


def test_nesting_within_the_cap_is_still_accepted_by_the_converter(cache):
    nested = {"set": {"a": [[["deep"]]]}}
    err = None
    try:
        cache.beads.issues_update(id="b1", actor="a", idempotency_key="k",
                                  patch={"metadata": nested})
    except bd_cache_rs.CacheError as e:
        err = e
    # The decoder, not the converter, decides about the value.
    assert err is None or err.code.startswith("BD_CACHE."), err


def methods(namespace):
    return sorted(n for n in dir(namespace) if not n.startswith("_"))


def documented_members(method):
    """The identifiers in backticks after the command name in the first docstring line."""
    first = method.__doc__.strip().splitlines()[0]
    _, _, rest = first.partition(":")
    return [t for t in re.findall(r"`([^`]*)`", rest) if re.fullmatch(r"[a-z_]+", t)]


def rejected(call, member):
    """True when the decoder refuses `member` as unknown or as a `/v0` name it does not serve."""
    try:
        call(**{member: None})
    except bd_cache_rs.CacheError as e:
        if e.details.get("member") == member and e.message.startswith("unknown request member"):
            return True
        if e.code == "BD_CACHE.UNSUPPORTED" and e.details.get("name") == member:
            return True
    return False


def all_methods(cache):
    return [(ns, n) for ns in ("beads", "manage") for n in methods(getattr(cache, ns))]


def test_every_member_a_docstring_names_is_accepted_by_the_decoder(cache):
    checked = 0
    for ns, name in all_methods(cache):
        call = getattr(getattr(cache, ns), name)
        for member in documented_members(call):
            assert not rejected(call, member), f"cache.{ns}.{name} documents {member!r}, the decoder rejects it"
            checked += 1
    assert checked >= 40, checked


def test_the_probe_sees_a_member_the_decoder_rejects(cache):
    """The probe is not vacuous: the old docstring names `ids` and `from` are rejected."""
    assert rejected(cache.beads.dependencies_list, "ids")
    assert rejected(cache.beads.dependencies_add, "from")
    assert not rejected(cache.beads.dependencies_add, "edges")


def test_a_documented_member_set_is_complete_for_methods_with_members(cache):
    assert documented_members(cache.beads.dependencies_list) == ["issue_id", "type"]
    assert documented_members(cache.beads.dependencies_add) == ["actor", "idempotency_key", "edges"]
    assert documented_members(cache.manage.status) == []


def test_two_threads_use_one_handle_through_cache_beads(cache):
    errors, ids = [], []

    def worker():
        try:
            for _ in range(5):
                ids.append(cache.beads.issues_get(id="b1")["id"])
        except BaseException as error:
            errors.append(error)

    threads = [daemon(worker) for _ in range(2)]
    for t in threads:
        t.start()
    for t in threads:
        join(t)
    assert not errors and ids == ["b1"] * 10
    assert cache.get("b1")["row"]["bead"]["id"] == "b1", "the 0.1 verb shares the handle"


def test_cache_beads_releases_the_gil_while_bd_runs(cache, repo):
    repo.bd.add({**repo.bd.get("b1"), "id": "late-2", "title": "late-2"})
    release = repo.bd.block("list")
    seen = {}

    def release_once_the_call_runs():
        try:
            seen["argv"] = release.wait_started()[0]  # needs the GIL while the call is inside bd
        except BaseException as error:
            seen["error"] = error
        finally:
            release.set()

    helper = daemon(release_once_the_call_runs)
    helper.start()
    got = cache.beads.issues_get(id="late-2", miss_timeout_ms=15000)
    join(helper)
    assert "error" not in seen, seen
    assert got["id"] == "late-2" and seen["argv"][0] == "list"


def test_cache_beads_after_close_raises_closed_error_on_every_thread(cache):
    outcome = []
    cache.close()

    def other():
        try:
            cache.beads.context()
        except BaseException as error:
            outcome.append(type(error))

    t = daemon(other)
    t.start()
    join(t)
    assert outcome == [bd_cache_rs.ClosedError]
    with pytest.raises(bd_cache_rs.ClosedError):
        cache.manage.status()
