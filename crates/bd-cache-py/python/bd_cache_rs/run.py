"""``python -I -m bd_cache_rs.run post_sync <module:function>``: the post-sync hook's runner.

bd-cache-sync runs it after each built-in sync (REQ-BDC-0723, 0727,
ADR-SYNC-0008), from a private temp cwd with a cleared environment, with one
``bd-cache.run.v1`` request on stdin. It opens its own ``Cache`` on
``BD_CACHE_PATH`` in the request's ``repo_root``, imports the function, calls
``func(cache, request)`` (what it prints goes to stderr, so stdout holds the
answer alone) and prints one JSON object::

    {"protocol": "bd-cache.run.v1", "ok": true, "result": ...}
    {"protocol": "bd-cache.run.v1", "ok": false, "error": {"type": ..., "message": ...}}

then exits 0, or 1 on error. The message is cut to 4 KiB and holds no
request data. Inside the call, ``drain``, ``manage.flush``, ``tick``,
``schedule_*`` and ``sync_use_*`` raise ``CacheError`` (``PRECONDITION``). The hook must be
idempotent: anything it does not mark is offered again on a later run.
"""

import importlib
import json
import os
import sys

from ._common import mark_inside_run, open_cache, output_to_stderr

PROTOCOL = "bd-cache.run.v1"
MAX_MESSAGE = 4096


def _answer(out, ok, **fields):
    out.write(json.dumps(dict({"protocol": PROTOCOL, "ok": ok}, **fields)) + "\n")
    out.flush()


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
    out = sys.stdout
    try:
        request = json.load(sys.stdin)
        if not isinstance(request, dict):
            raise ValueError(f"not a {PROTOCOL} post_sync request: not a JSON object")
        if request.get("protocol") != PROTOCOL or request.get("kind") != "post_sync":
            raise ValueError(f"not a {PROTOCOL} post_sync request")
        func = _function(argv[1])
        mark_inside_run()
        with output_to_stderr() as out:
            with open_cache(os.environ.get("BD_CACHE_PATH"), request.get("repo_root")) as cache:
                result = func(cache, request)
        _answer(out, True, result=result)
        return 0
    except Exception as e:  # noqa: BLE001 - every failure is the hook's answer
        _answer(out, False, error={"type": type(e).__name__, "message": str(e)[:MAX_MESSAGE]})
        return 1


if __name__ == "__main__":
    sys.exit(main())
