"""What the two entry points share: opening the cache of a run (ADR-SYNC-0008)."""

import os

from . import _native


def open_cache(cache_path, beads_dir):
    """The ``Cache`` on ``cache_path``, in the repository whose beads directory is ``beads_dir``.

    The repository root is the beads directory's parent; ``BEADS_DIR`` (set by
    the runner) names where ``metadata.json`` is read.
    """
    beads_dir = beads_dir or os.environ.get("BEADS_DIR")
    if not beads_dir:
        raise RuntimeError("no beads directory: the payload and BEADS_DIR are both empty")
    root = os.path.dirname(os.path.abspath(beads_dir))
    return _native.Cache(root, cache_path=cache_path)


def mark_inside_run():
    """From here on, drain, manage.flush, tick and schedule_* raise PRECONDITION."""
    _native._set_in_run(True)
