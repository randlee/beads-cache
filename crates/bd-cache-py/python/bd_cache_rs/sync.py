"""``bd_cache_rs.sync.main(func)``: the harness of a consumer's Python sync script.

With the Python sync row enabled (``bdc cache sync use-python``,
REQ-BDC-0724, 0725), the script is the sync: sc-cron runs it as ``python -I
-m <module>`` with its hook protocol, one JSON object on stdin per event.
A script calls ``main(func)``:

- on ``cron.stop`` it answers ``{"action": "continue"}``;
- on ``cron.tick`` it opens ``Cache`` on the payload's cache file (the one
  the runner opened) in its ``repo_root`` and calls ``func(cache, payload)``,
  where ``payload`` carries ``cache_path``, ``beads_dir``, ``repo_root`` and
  the run's ``run_id``; the script fills the cache with
  ``cache.ingest_scope(scope, rows, blocked)``. What it prints goes to
  stderr. It exits 0 on return, and 1 with the exception's type and message
  on stderr.

Anything on stdin that is not one JSON object with an ``event`` is an error
line on stderr and exit 1, never a traceback.

Example::

    import bd_cache_rs.sync

    def sync(cache, payload):
        cache.ingest_scope("mine", rows_from_somewhere(), [])

    if __name__ == "__main__":
        bd_cache_rs.sync.main(sync)
"""

import json
import sys

from ._common import mark_inside_run, open_cache, output_to_stderr


def main(func, stdin=None, stdout=None):
    """Answers one sc-cron hook event for a sync script; exits the process."""
    stdin = sys.stdin if stdin is None else stdin
    stdout = sys.stdout if stdout is None else stdout
    try:
        request = json.load(stdin)
        if not isinstance(request, dict):
            raise ValueError("not a JSON object")
        event = request.get("event")
        payload = request.get("payload") or {}
        if not isinstance(payload, dict):
            raise ValueError("its payload is not a JSON object")
    except ValueError as e:
        print(f"bd_cache_rs.sync: stdin is not one JSON object: {e}", file=sys.stderr)
        sys.exit(1)
    if event == "cron.stop":
        stdout.write(json.dumps({"action": "continue"}) + "\n")
        stdout.flush()
        sys.exit(0)
    if event != "cron.tick":
        print(f"bd_cache_rs.sync: unknown event {event!r}", file=sys.stderr)
        sys.exit(1)
    try:
        mark_inside_run()
        with output_to_stderr():
            with open_cache(payload.get("cache_path"), payload.get("repo_root")) as cache:
                func(cache, payload)
    except Exception as e:  # noqa: BLE001 - the script's failure is its exit status
        print(f"{type(e).__name__}: {e}", file=sys.stderr)
        sys.exit(1)
    sys.exit(0)
