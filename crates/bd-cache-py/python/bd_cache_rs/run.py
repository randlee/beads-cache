"""``python -I -m bd_cache_rs.run post_sync <module:function>``: the post-sync hook's runner.

bd-cache-sync runs it after each built-in sync (REQ-BDC-0723, 0727,
ADR-SYNC-0008), from a private temp cwd with a cleared environment, with one
``bd-cache.run.v1`` request on stdin. It opens its own ``Cache`` on
``BD_CACHE_PATH``, imports the function, calls ``func(cache, request)`` and
prints one JSON object::

    {"protocol": "bd-cache.run.v1", "ok": true, "result": ...}
    {"protocol": "bd-cache.run.v1", "ok": false, "error": {"type": ..., "message": ...}}

then exits 0, or 1 on error. The message is cut to 4 KiB and holds no
request data. Inside the call, ``drain``, ``manage.flush``, ``tick`` and
``schedule_*`` raise ``CacheError`` (``PRECONDITION``). The hook must be
idempotent: anything it does not mark is offered again on a later run.
"""

import importlib
import json
import os
import sys

from ._common import mark_inside_run, open_cache

PROTOCOL = "bd-cache.run.v1"
MAX_MESSAGE = 4096


def _answer(ok, **fields):
    print(json.dumps(dict({"protocol": PROTOCOL, "ok": ok}, **fields)))


def _function(target):
    module_name, sep, name = target.partition(":")
    if not sep or not module_name or not name:
        raise ValueError(f"{target!r} is not module:function")
    func = getattr(importlib.import_module(module_name), name)
    if not callable(func):
        raise TypeError(f"{target} is not callable")
    return func


def main(argv=None):
    """Runs the hook named on the command line; returns the exit status."""
    argv = sys.argv[1:] if argv is None else argv
    if len(argv) != 2 or argv[0] != "post_sync":
        print("usage: python -I -m bd_cache_rs.run post_sync <module:function>", file=sys.stderr)
        return 2
    try:
        request = json.load(sys.stdin)
        if request.get("protocol") != PROTOCOL or request.get("kind") != "post_sync":
            raise ValueError(f"not a {PROTOCOL} post_sync request")
        func = _function(argv[1])
        cache_path = os.environ.get("BD_CACHE_PATH") or request.get("cache_path")
        mark_inside_run()
        with open_cache(cache_path, os.environ.get("BEADS_DIR")) as cache:
            result = func(cache, request)
        _answer(True, result=result)
        return 0
    except Exception as e:  # noqa: BLE001 - every failure is the hook's answer
        _answer(False, error={"type": type(e).__name__, "message": str(e)[:MAX_MESSAGE]})
        return 1


if __name__ == "__main__":
    sys.exit(main())
