"""A recording fake `bd` for the bd_cache_rs tests (stdlib only).

The Rust `BdCli` runs this script as the `bd` program, so the tests drive the
real composition: real argv builders, real process spawning with deadlines,
real strict row parsing. The script keeps its world in a JSON state file,
records every argv in a JSON-lines call log, and can be told (through the state
file) to be down or to answer late.

The call log (`<state>.calls`) gets a `start` line when a call begins and an `end` line
(with the exit status) when it finishes, so tests can see which calls overlapped. A call
the caller killed has no `end` line. The state file can make a command

- `delay`: answer after that many seconds;
- `block`: wait until the named release file exists, or `block_deadline` seconds pass
  (default 20), so a test decides when the call may finish instead of guessing a delay;
- `fail`: exit 1 with a message; or the whole fake `down`.

Each `start` line also records the child's `BEADS_DIR` (null when unset) and working
directory, so a test can check which beads directory `bd` was pointed at.

Two rules follow `sc-bd-cli`'s `FakeBd` (PE-ARCH-007), so the Python and Rust suites test
against one model of bd:

- `--set-metadata` values are coerced as `coerce_metadata`
  (`crates/sc-bd-cli/src/testing/fake_bd.rs`) does; the cases are the shared table
  `fixtures/metadata-coercion.json`;
- `bd blocked` and `bd ready` follow the P-6 recording (ADR-BEADS-0004,
  `crates/sc-beads/tests/fixtures/p6-blocked/`), as `sc-beads`' `FakeBackend` does:
  `blocks`, `conditional-blocks` and `waits-for` on an open bead block; `parent-child`
  blocks only when the parent is itself blocked; `related` and a closed blocker never do.

Usage: fake_bd_main.py STATE_JSON ARGV...
"""

import json
import math
import os
import sys
import time
from datetime import datetime, timezone

ALL_STATUSES = "open,in_progress,blocked,deferred,closed,pinned,hooked"
EXIT_PRECONDITION = 13
# The edge types that block like `blocks` (bd's `AffectsReadyWork` set, ADR-BEADS-0004).
BLOCKING = ("blocks", "conditional-blocks", "waits-for")
# serde_json's integer range: an i64, or a u64 above it. Outside it a JSON integer is an f64.
INT_MIN, INT_END = -(2 ** 63), 2 ** 64
# 2^63: every whole f64 below it in magnitude is an exact i64 (`whole_i64` in fake_bd.rs).
WHOLE_END = 2.0 ** 63


def now():
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def load(path):
    with open(path, encoding="utf-8") as handle:
        return json.load(handle)


def save(path, state):
    with open(path, "w", encoding="utf-8") as handle:
        json.dump(state, handle, indent=1, sort_keys=True)


def parse(argv, valued, switches=()):
    """Splits argv into positional words and {flag: [values]}; an unknown flag is a usage error."""
    positional, flags = [], {}
    i = 0
    while i < len(argv):
        word = argv[i]
        if word in switches:
            flags.setdefault(word, []).append("")
        elif word in valued:
            if i + 1 >= len(argv):
                usage(f"flag {word} needs a value")
            flags.setdefault(word, []).append(argv[i + 1])
            i += 1
        elif word.startswith("-") and len(word) > 1:
            usage(f"unknown flag {word}")
        else:
            positional.append(word)
        i += 1
    return positional, flags


def usage(message):
    print(f"Error: {message}", file=sys.stderr)
    sys.exit(2)


def fail(message, code=1):
    print(f"Error: {message}", file=sys.stderr)
    sys.exit(code)


def _refuse_constant(name):
    # serde_json has no NaN or Infinity; Python's json would accept them.
    raise ValueError(name)


def coerce_metadata(text):
    """What `bd --set-metadata key=TEXT` stores: `coerce_metadata` of sc-bd-cli's FakeBd.

    TEXT is parsed as JSON. A number, `true`/`false` or `null` is stored as that type, a
    whole number as an integer whatever its spelling (`5`, `5.0`, `1e3`, ` 7`); anything
    else, including JSON of another kind (`"5"`, `[1]`, `{"x":1}`), `True` and a number
    out of f64 range, is stored as the text it was given.
    """
    try:
        value = json.loads(text, parse_constant=_refuse_constant)
    except (ValueError, RecursionError):
        return text
    if value is None or isinstance(value, bool):
        return value
    if isinstance(value, int):
        return value if INT_MIN <= value < INT_END else float(value)
    if isinstance(value, float):
        if math.isinf(value):
            return text
        if value.is_integer() and abs(value) < WHOLE_END:
            return int(value)
        return value
    return text


def parse_time(text):
    """An RFC 3339 instant (`Z` or an offset), else None."""
    if not isinstance(text, str):
        return None
    try:
        at = datetime.fromisoformat(text[:-1] + "+00:00" if text.endswith("Z") else text)
    except ValueError:
        return None
    return at if at.tzinfo is not None else None


def blocked_map(state):
    """{id: blockers} of every blocked bead: `blocked_map` of sc-beads' FakeBackend.

    An open bead is blocked by each open target of its `BLOCKING` edges, and by its
    open `parent-child` parent when that parent is itself blocked. A closed bead is never
    blocked; a cycle stops at the bead already being visited. Blockers are in edge order,
    each once.
    """
    beads = state["beads"]
    memo, visiting = {}, set()

    def is_open(bead_id):
        target = beads.get(bead_id)
        return target is not None and target.get("status") != "closed"

    def blockers(bead_id):
        if bead_id in memo:
            return memo[bead_id]
        row = beads.get(bead_id)
        if row is None or row.get("status") == "closed" or bead_id in visiting:
            return []
        visiting.add(bead_id)
        found = []
        for dep in row.get("dependencies", []):
            other = dep["depends_on_id"]
            if dep["type"] in BLOCKING:
                hit = is_open(other)
            elif dep["type"] == "parent-child":
                hit = is_open(other) and bool(blockers(other))
            else:
                hit = False
            if hit and other not in found:
                found.append(other)
        visiting.discard(bead_id)
        memo[bead_id] = found
        return found

    for bead_id in sorted(beads):
        blockers(bead_id)
    return {bead_id: found for bead_id, found in memo.items() if found}


def render(state, bead):
    row = json.loads(json.dumps(bead))
    row["dependency_count"] = len(row.get("dependencies", []))
    row["dependent_count"] = sum(
        1
        for other in state["beads"].values()
        for dep in other.get("dependencies", [])
        if dep["depends_on_id"] == bead["id"]
    )
    row.setdefault("comment_count", 0)
    return row


def cmd_list(state, argv):
    _, flags = parse(argv, {"--id", "-l", "--status", "-n", "--updated-after"}, {"--json"})
    statuses = flags.get("--status", [ALL_STATUSES])[0].split(",")
    rows = []
    for bead in sorted(state["beads"].values(), key=lambda b: b["id"]):
        if "--id" in flags and bead["id"] not in flags["--id"][0].split(","):
            continue
        if "-l" in flags and flags["-l"][0] not in bead.get("labels", []):
            continue
        if bead["status"] not in statuses:
            continue
        if "--updated-after" in flags and not bead["updated_at"] > flags["--updated-after"][0]:
            continue
        rows.append(render(state, bead))
    print(json.dumps(rows))


def cmd_blocked(state, argv):
    _, flags = parse(argv, {"-l"}, {"--json"})
    blocked = blocked_map(state)
    rows = []
    for bead in sorted(state["beads"].values(), key=lambda b: b["id"]):
        found = blocked.get(bead["id"])
        if found and flags["-l"][0] in bead.get("labels", []):
            row = render(state, bead)
            row["blocked_by_count"] = len(found)
            row["blocked_by"] = found
            rows.append(row)
    print(json.dumps(rows))


def deferred(bead, at):
    """True while the bead's `defer_until` is in the future (an unparseable one is ignored)."""
    until = parse_time(bead.get("defer_until"))
    return until is not None and until > at


def cmd_ready(state, argv):
    _, flags = parse(argv, {"-l", "-n"}, {"--json"})
    blocked = blocked_map(state)
    at = datetime.now(timezone.utc)
    rows = [
        render(state, bead)
        for bead in sorted(state["beads"].values(), key=lambda b: b["id"])
        if flags["-l"][0] in bead.get("labels", []) and bead.get("status") == "open"
        and bead["id"] not in blocked and not deferred(bead, at)
    ]
    print(json.dumps(rows))


def target(state, positional, what):
    if not positional:
        usage(f"{what} needs a bead id")
    bead = state["beads"].get(positional[0])
    if bead is None:
        fail(f'no issue found matching "{positional[0]}"')
    return bead


def check_guards(bead, flags):
    if "--if-status" in flags and bead["status"] != flags["--if-status"][0]:
        fail("status guard failed", EXIT_PRECONDITION)
    if "--if-assignee" in flags and (bead.get("assignee") or "") != flags["--if-assignee"][0]:
        fail("assignee guard failed", EXIT_PRECONDITION)


def cmd_close(state, argv):
    positional, flags = parse(argv, {"--reason", "--actor"})
    bead = target(state, positional, "close")
    bead["status"] = "closed"
    bead["closed_at"] = bead["updated_at"] = now()
    if "--reason" in flags:
        bead["close_reason"] = flags["--reason"][0]
    print(f"closed {bead['id']}")


def cmd_update(state, argv):
    positional, flags = parse(
        argv,
        {"--status", "--assignee", "--add-label", "--remove-label", "--set-metadata",
         "--unset-metadata", "--append-notes", "--if-status", "--if-assignee", "--actor"},
    )
    bead = target(state, positional, "update")
    check_guards(bead, flags)
    if "--status" in flags:
        bead["status"] = flags["--status"][0]
    if "--assignee" in flags:
        if flags["--assignee"][0]:
            bead["assignee"] = flags["--assignee"][0]
        else:
            bead.pop("assignee", None)
    for label in flags.get("--add-label", []):
        if label not in bead.setdefault("labels", []):
            bead["labels"].append(label)
    for label in flags.get("--remove-label", []):
        bead["labels"] = [x for x in bead.get("labels", []) if x != label]
    for pair in flags.get("--set-metadata", []):
        key, _, value = pair.partition("=")
        bead.setdefault("metadata", {})[key] = coerce_metadata(value)
    for key in flags.get("--unset-metadata", []):
        bead.get("metadata", {}).pop(key, None)
    if "--append-notes" in flags:
        bead["notes"] = (bead.get("notes", "") + "\n" + flags["--append-notes"][0]).strip()
    bead["updated_at"] = now()
    print(f"updated {bead['id']}")


def cmd_dep(state, argv):
    if argv[:1] != ["add"]:
        usage("only `dep add` is supported")
    positional, flags = parse(argv[1:], {"--type", "--actor"})
    bead = target(state, positional, "dep add")
    if len(positional) < 2:
        usage("dep add needs two bead ids")
    if positional[1] not in state["beads"]:
        fail(f'no issue found matching "{positional[1]}"')
    bead.setdefault("dependencies", []).append({
        "issue_id": bead["id"],
        "depends_on_id": positional[1],
        "type": flags.get("--type", ["blocks"])[0],
        "created_at": now(),
        "created_by": flags.get("--actor", ["fake"])[0],
        "metadata": "{}",
    })
    bead["updated_at"] = now()
    print(f"added {bead['id']} -> {positional[1]}")


COMMANDS = {
    "list": cmd_list, "blocked": cmd_blocked, "ready": cmd_ready,
    "close": cmd_close, "update": cmd_update, "dep": cmd_dep,
}


# `at` is CLOCK_MONOTONIC in ns: system-wide, so calls of different processes compare. Not
# `time.monotonic_ns()`, which on macOS with Python 3.9 is a per-process offset. POSIX only.
def log(path, **event):
    with open(path + ".calls", "a", encoding="utf-8") as handle:
        handle.write(json.dumps({"pid": os.getpid(), "at": time.clock_gettime_ns(time.CLOCK_MONOTONIC), **event}) + "\n")


def wait_for(release, seconds):
    end = time.monotonic() + seconds
    while not os.path.exists(release) and time.monotonic() < end:
        time.sleep(0.005)


def run(path, argv):
    state = load(path)
    name = argv[0] if argv else ""
    block = state.get("block", {})
    release = block.get(name, block.get("*"))
    if release:
        wait_for(release, state.get("block_deadline", 20))
    delay = state.get("delay", {})
    time.sleep(delay.get(name, delay.get("*", 0)))
    if state.get("down"):
        fail("failed to open database: Dolt server unreachable at 127.0.0.1:1: dial tcp "
             "127.0.0.1:1: connect: connection refused")
    if name in state.get("fail", {}):
        fail(state["fail"][name])
    if name == "version":
        print(f"bd version {state['version']} (dev: fake)")
    elif name == "schema":
        sys.stdout.write(state["schema"])
    elif name in COMMANDS:
        # Re-read: a test may have edited the world while this call was blocked.
        state = load(path)
        COMMANDS[name](state, argv[1:])
        if name in ("close", "update", "dep"):
            save(path, state)
    else:
        usage(f"unknown command {name!r}")


def main():
    path, argv = sys.argv[1], sys.argv[2:]
    log(path, event="start", argv=argv, beads_dir=os.environ.get("BEADS_DIR"), cwd=os.getcwd())
    status = 0
    try:
        run(path, argv)
    except SystemExit as exit_:
        status = exit_.code if isinstance(exit_.code, int) else 1
        raise
    finally:
        sys.stdout.flush()
        log(path, event="end", argv=argv, status=status)


if __name__ == "__main__":
    main()
