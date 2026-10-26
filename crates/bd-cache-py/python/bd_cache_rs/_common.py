"""What the two entry points share: opening the cache of a run (ADR-SYNC-0008)."""

import contextlib
import sys

from . import _native


def open_cache(cache_path, repo_root):
    """The ``Cache`` on ``cache_path`` in the repository at ``repo_root``.

    Both come from the runner (the hook request's ``repo_root`` and
    ``BD_CACHE_PATH``; the sync payload's ``repo_root`` and ``cache_path``),
    so ``.beads/bd-cache.toml`` is read from the repository the runner
    serves, whatever ``BEADS_DIR`` names; ``BEADS_DIR`` (set by the runner)
    names where ``metadata.json`` is read.
    """
    if not cache_path:
        raise RuntimeError("no cache file: the runner set neither BD_CACHE_PATH nor cache_path")
    if not repo_root:
        raise RuntimeError("no repository root: the runner's request carries no repo_root")
    return _native.Cache(repo_root, cache_path=cache_path)


def mark_inside_run():
    """From here on, drain, manage.flush, tick, schedule_* and sync_use_* raise PRECONDITION."""
    _native._set_in_run(True)


@contextlib.contextmanager
def output_to_stderr():
    """Sends what user code prints to stderr while it runs, so stdout holds
    only the runner's own answer; yields the real stdout."""
    real = sys.stdout
    with contextlib.redirect_stdout(sys.stderr):
        yield real
