"""``bd_cache_rs.sync.main(func)``: the harness of a consumer's Python sync script.

With the Python sync row enabled (``bdc cache sync use-python``,
REQ-BDC-0724, 0725), the script is the sync: sc-cron runs it as ``python -I
-m <module>`` with its hook protocol, one JSON object on stdin per event.
A script calls ``main(func)``:

- on ``cron.stop`` it answers ``{"action": "continue"}``;
- on ``cron.tick`` it opens ``Cache`` on the payload's cache file and calls
  ``func(cache, payload)``, where ``payload`` carries ``cache_path``,
  ``beads_dir`` and the run's ``run_id``; the script fills the cache with
  ``cache.ingest_scope(scope, rows, blocked)``. It exits 0 on return, and 1
  with the exception's type and message on stderr.

Example::

    import bd_cache_rs.sync

    def sync(cache, payload):
        cache.ingest_scope("mine", rows_from_somewhere(), [])

    if __name__ == "__main__":
        bd_cache_rs.sync.main(sync)
"""

import json
import sys

from ._common import mark_inside_run, open_cache


def main(func, stdin=None, stdout=None):
    """Answers one sc-cron hook event for a sync script; exits the process."""
    stdin = sys.stdin if stdin is None else stdin
    stdout = sys.stdout if stdout is None else stdout
    try:
        request = json.load(stdin)
    except ValueError as e:
        print(f"bd_cache_rs.sync: stdin is not one JSON object: {e}", file=sys.stderr)
        sys.exit(1)
    event = request.get("event")
    if event == "cron.stop":
        stdout.write(json.dumps({"action": "continue"}) + "\n")
        stdout.flush()
        sys.exit(0)
    if event != "cron.tick":
        print(f"bd_cache_rs.sync: unknown event {event!r}", file=sys.stderr)
        sys.exit(1)
    payload = request.get("payload") or {}
    try:
        mark_inside_run()
        with open_cache(payload.get("cache_path"), payload.get("beads_dir")) as cache:
            func(cache, payload)
    except Exception as e:  # noqa: BLE001 - the script's failure is its exit status
        print(f"{type(e).__name__}: {e}", file=sys.stderr)
        sys.exit(1)
    sys.exit(0)
