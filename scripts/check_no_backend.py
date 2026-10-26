#!/usr/bin/env python3
"""The test environment never reaches the real backend (NFR-BDC-0005, REQ-BDC-0006).

Two checks, one per subcommand, each the one copy the CI steps call:

    python3 -I scripts/check_no_backend.py path    # no bd or dolt on PATH (NFR-BDC-0005)
    python3 -I scripts/check_no_backend.py graph   # no Dolt or MySQL client crate (REQ-BDC-0006)

`path` fails when `bd` or `dolt` resolves on `PATH` (`shutil.which`, so on Windows
`PATHEXT` applies): the unit tests run against fakes and must not find either.
`graph` runs `cargo tree` over the whole workspace (every edge kind, feature and
target) and fails on a crate whose name contains `dolt` or `mysql`, or starts with
`mariadb`: the backend is reached only through bd. `--tree FILE` reads the
`cargo tree --prefix none --format {p}` text from FILE instead (the unit tests).

Exit codes: 0 clean, 1 a hit (one `::error::` line each under GitHub Actions),
2 the check could not run.
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

TOOLS: tuple[str, ...] = ("bd", "dolt")

# A crate line of `cargo tree --prefix none --format '{p}'`: `name vX.Y.Z ...`.
CLIENT = re.compile(r"^([a-z0-9_-]*dolt[a-z0-9_-]*|[a-z0-9_-]*mysql[a-z0-9_-]*|mariadb[a-z0-9_-]*) v",
                    re.IGNORECASE)

CARGO_TREE: tuple[str, ...] = (
    "cargo", "tree", "--workspace", "-e", "normal,build,dev", "--all-features",
    "--target", "all", "--prefix", "none", "--format", "{p}",
)


def tools_on_path(path: str | None) -> list[tuple[str, str]]:
    """Each of TOOLS that resolves on `path` (a `PATH` value), with where."""
    found = []
    for tool in TOOLS:
        where = shutil.which(tool, path=path)
        if where is not None:
            found.append((tool, where))
    return found


def client_crates(tree: str) -> list[str]:
    """The Dolt or MySQL client crates in `cargo tree` text, each once, sorted."""
    return sorted({m.group(0)[:-2] for line in tree.splitlines() if (m := CLIENT.match(line.strip()))})


def error(message: str) -> None:
    if os.environ.get("GITHUB_ACTIONS"):
        print(f"::error::{message}")
    print(message, file=sys.stderr)


def check_path() -> int:
    found = tools_on_path(os.environ.get("PATH"))
    for tool, where in found:
        error(f"{tool} is on PATH ({where}): the tests must run without it (NFR-BDC-0005)")
    if found:
        return 1
    print("no bd or dolt on PATH")
    return 0


def check_graph(tree_file: str | None) -> int:
    if tree_file is not None:
        tree = Path(tree_file).read_text(encoding="utf-8")
    else:
        try:
            tree = subprocess.run(CARGO_TREE, check=True, capture_output=True, text=True).stdout
        except (OSError, subprocess.CalledProcessError) as err:
            print(f"check_no_backend: cargo tree could not run: {err}", file=sys.stderr)
            return 2
    hits = client_crates(tree)
    for crate in hits:
        error(f"a Dolt or MySQL client is in the dependency graph (REQ-BDC-0006): {crate}")
    if hits:
        return 1
    print("no Dolt or MySQL client in the workspace graph")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="check", required=True)
    sub.add_parser("path", help="no bd or dolt on PATH (NFR-BDC-0005)")
    graph = sub.add_parser("graph", help="no Dolt or MySQL client crate (REQ-BDC-0006)")
    graph.add_argument("--tree", metavar="FILE", help="read cargo tree text from FILE")
    args = parser.parse_args(argv)
    if args.check == "path":
        return check_path()
    return check_graph(args.tree)


if __name__ == "__main__":
    sys.exit(main())
