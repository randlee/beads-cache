"""Drives the vendored bd_cache.py for bd-cache-store's compatibility tests (sprint a-4).

Not vendored: written for these tests. Run with uv from tests/compat.rs:

    uv run --no-project --with 'pydantic>=2,<3' python driver.py <command> <root> <db> [args...]

Commands (each prints one JSON object on success, exits nonzero on failure):

    write    <root> <db>         sync phase `ct` from the recorded bd-1.3.1 rows, journal two
                                 writes, set a sync alert and an outage (a Python-written v3 file)
    rebuild  <root> <db>         run the backend check against a changed `bd schema` (forced rebuild)
    hold     <root> <db>         take bd_cache.py's drain lock, print {"held": ...}, wait for stdin EOF
    escalate <root> <db> <seq> <reason>
                                 escalate a journal entry as bd_cache.py's drain does (`_mark`)
    cli      <root> <db> args..  run bd_cache.py's own CLI (`get`, `status`, `prune`, ...)

No bd process runs: the backend calls go to the deterministic runner below.
"""
import json
import subprocess
import sys
import types
from pathlib import Path

HERE = Path(__file__).resolve().parent
FIXTURES = HERE / "fixtures" / "bd-1.3.1"
CLOCK = 1792749600.0  # 2026-10-23T10:00:00Z

if sys.platform == "win32":
    # bd_cache.py imports fcntl (Unix only) for its drain lock alone; the
    # commands run here on Windows never take that lock.
    sys.modules.setdefault("fcntl", types.ModuleType("fcntl"))

sys.path.insert(0, str(HERE))
import bd_cache  # noqa: E402  (vendored, atm-core 05831df)


class Runner:
    """A bd that answers version, schema and list from the recorded fixtures."""

    def __init__(self, schema):
        self.schema = schema
        self.rows = json.loads((FIXTURES / "list-rows.json").read_text(encoding="utf-8"))

    def __call__(self, command, **kwargs):
        verb = command[1]
        if verb == "version":
            out = "bd version 1.3.1 (compat driver)\n"
        elif verb == "schema":
            out = self.schema
        elif verb == "list":
            out = json.dumps(self.rows)
        else:
            raise AssertionError(f"the compat driver runs no writes: {command}")
        return subprocess.CompletedProcess(command, 0, out, "")


def pinned_schema():
    return (FIXTURES / "bd-schema.json").read_text(encoding="utf-8")


def cache(root, db, schema):
    return bd_cache.BdCache(root, runner=Runner(schema), clock=lambda: CLOCK, db_path=db)


def write(root, db):
    c = cache(root, db, pinned_schema())
    synced = c.sync("ct")
    first = c.write("ct-3", "update", {"add_labels": ["reviewed"]}, idem_key="py-1", actor="dev")
    second = c.write("ct-1", "close", {"reason": "done"}, idem_key="py-2", actor="dev",
                     precondition={"status": "in_progress"})
    c.set_sync_alert("ct", "BD_CACHE.SYNC_FAILED: boom")
    c.record_outage(bd_cache.CacheError("BD_CACHE.BD_UNAVAILABLE", "refused", "retry"))
    return {"ok": True, "stored": synced["stored"], "seqs": [first["seq"], second["seq"]]}


def rebuild(root, db):
    report = cache(root, db, '{"schema_version": "changed"}').check_backend()
    return {"ok": True, "rebuilt": report["rebuilt"]}


def hold(root, db):
    c = cache(root, db, pinned_schema())
    with c.drain_lock() as held:
        print(json.dumps({"ok": True, "held": held}), flush=True)
        sys.stdin.read()
    return None


def escalate(root, db, seq, reason):
    c = cache(root, db, pinned_schema())
    with c.transaction():
        c._mark(int(seq), "escalated", reason, 1)
    return {"ok": True, "seq": int(seq)}


def main(argv):
    command, root, db, rest = argv[0], argv[1], argv[2], argv[3:]
    if command == "cli":
        return bd_cache.main(["--root", root, "--db", db, *rest])
    if command == "escalate":
        out = escalate(root, db, *rest)
    else:
        out = {"write": write, "rebuild": rebuild, "hold": hold}[command](root, db)
    if out is not None:
        print(json.dumps(out, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
