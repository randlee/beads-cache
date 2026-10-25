#!/usr/bin/env python3
"""Fail on any ATM or atm-bd concern in the product (ADR-BDC-0010).

beads-cache is a cache, a CLI for configuration and management, and a PyO3
API. It sends no messages, reads no ATM or atm-bd configuration, and names no
ATM or atm-bd concept in its source, tests, scripts, workflows or docs. This
lint keeps it that way: it scans the git-tracked files (`git ls-files -z`)
under the product roots in SCOPE_DIRS and SCOPE_FILES, skipping binary files,
and reports each line that matches a rule below, case-insensitively, as
`path:line: <rule>: <text>`.

Out of scope by construction (dev tooling, not product): the agent tooling
(`.claude/**`, `.cursor/**`, `.codex/**`, `.agents/**`, `CLAUDE.md`,
`AGENTS.md`), which coordinates the repo's own agents over ATM; the sc-publish
kit, frozen upstream (`.github/scripts/**`, `.github/actions/**`, the
`*-publish.yml`, `release*.yml` and `prerelease-archive.yml` workflows,
`release/**`, `install.py`, `README.sc-publish.md` and the kit docs in
KIT_DOCS); `CHANGELOG.md` (a historical record); and `Cargo.lock` (generated,
with no hits).

Exemptions inside the scope are the fixed tuples below and nothing else: no
inline pragma, no allowlist file. Scope and exemptions change only through a
later ADR.

Run from anywhere inside the repository:

    python3 scripts/lint_no_atm.py           # one line per hit, exit 1 on any
    python3 scripts/lint_no_atm.py --count   # per-rule and per-file counts

Exit codes: 0 no hits, 1 hits, 2 the scan could not run (not a git tree).
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from collections import Counter
from dataclasses import dataclass
from pathlib import Path

# --- rules -------------------------------------------------------------------

# (rule name, pattern); every pattern is matched case-insensitively.
RULES: tuple[tuple[str, str], ...] = (
    # The word "atm" on its own or as a prefix of an identifier or path:
    # ATM, `atm send`, atm-bd, atm_core, ATM_IDENTITY, .atm.toml, atm_process.
    # Not a hit inside ordinary words (format, batman, atmosphere): a letter or
    # digit before it, or a letter after it, rules it out.
    ("atm-word", r"(?<![a-z0-9])atm(?![a-z])"),
    # An ATM principal on the `tool` team: cron@tool, atm-bd@tool.
    ("tool-principal", r"@tool\b"),
    # ATM team roles and its terminal multiplexer.
    ("atm-roles", r"\bteam-lead\b|\bfallback_lead\b|\bherdr\b"),
    # atm-bd's phase files (`.atm-bd/phase-*.toml`, `phase-<x>.toml`,
    # `phase-bd.toml`): a scope is configured in beads-cache's own
    # configuration, never discovered from a consumer's phase files.
    ("atm-bd-phase-file", r"\bphase-(?:\*|<[^>\s]+>|[a-z0-9_]+)\.toml\b"),
    # atm-bd's BD-CACHE baseline by file name: its implementation
    # (`bd_cache.py`), its cron script (`bd_cache_cron.py`) and its contract
    # (`BD-CACHE.md`). The shared store format name "BD-CACHE" alone is not a
    # hit; the consumer's files are.
    ("atm-bd-artifact", r"\bbd_cache\.py\b|\bbd_cache_cron\b|\bBD-CACHE\.md\b"),
)

COMPILED: tuple[tuple[str, re.Pattern[str]], ...] = tuple(
    (name, re.compile(pattern, re.IGNORECASE)) for name, pattern in RULES
)

# --- scope -------------------------------------------------------------------

# The product: every tracked file under these directories, and these files.
# The Python package config (crates/bd-cache-py/pyproject.toml) is under
# crates/. scripts/ holds only repo-owned scripts (the kit's live in
# .github/scripts/). The workflows are the repo's own; the kit's are not.
SCOPE_DIRS: tuple[str, ...] = ("crates/", "docs/", "scripts/")
SCOPE_FILES: tuple[str, ...] = (
    "Cargo.toml",
    "README.md",
    "rust-toolchain.toml",
    ".github/workflows/ci.yml",
    ".github/workflows/python.yml",
    ".github/workflows/bd-compat.yml",
)

# sc-publish kit docs installed under docs/ (kit install 21f8299, pin bump
# 2449acf, release b-5 9c31a82): kit files, frozen upstream, not product.
KIT_DOCS: tuple[str, ...] = (
    "docs/getting-started.md",
    "docs/npm-channel-compatibility.md",
    "docs/consumer-preflight-and-docli.md",
    "docs/sprints/fix-homebrew-formula-bundled-paths-tojson.md",
)

# --- exemptions --------------------------------------------------------------

# Whole files inside the scope: this lint and its tests, and the agent-team
# messaging protocol (how the repo's agents coordinate, not product behaviour).
EXEMPT_FILES: tuple[str, ...] = (
    "scripts/lint_no_atm.py",
    "scripts/test_lint_no_atm.py",
    "docs/team-protocol.md",
)

# Whole directories inside the scope: the phase plans are a historical record.
EXEMPT_DIRS: tuple[str, ...] = ("docs/plans/",)

# One section of one file: the ADR that states this boundary has to name what
# it forbids. The section runs from its heading to the next `## ` heading.
EXEMPT_SECTIONS: tuple[tuple[str, str], ...] = (
    ("docs/architecture.md", "## ADR-BDC-0010"),
)

# The lint's own file names, removed from a line before it is matched, so the
# CI step and the docs that run or cite this lint do not trip it. Only these
# exact names are removed; any other text on the line is still matched.
SELF_NAMES: tuple[str, ...] = ("test_lint_no_atm.py", "lint_no_atm.py")

# --- scan --------------------------------------------------------------------


@dataclass(frozen=True)
class Hit:
    path: str
    line: int
    rule: str
    text: str


def in_scope(path: str) -> bool:
    if path in KIT_DOCS:
        return False
    return path in SCOPE_FILES or any(path.startswith(d) for d in SCOPE_DIRS)


def is_exempt(path: str) -> bool:
    return path in EXEMPT_FILES or any(path.startswith(d) for d in EXEMPT_DIRS)


def is_binary(data: bytes) -> bool:
    return b"\0" in data[:8192]


def blank_exempt_sections(path: str, lines: list[str]) -> list[str]:
    """Blank the exempt section's lines, keeping line numbers."""
    headings = [h for p, h in EXEMPT_SECTIONS if p == path]
    if not headings:
        return lines
    out: list[str] = []
    inside = False
    for line in lines:
        if line.startswith("## "):
            inside = any(line.startswith(h) for h in headings)
        out.append("" if inside else line)
    return out


def scan_text(path: str, text: str) -> list[Hit]:
    hits: list[Hit] = []
    lines = blank_exempt_sections(path, text.splitlines())
    for number, line in enumerate(lines, start=1):
        probe = line
        for name in SELF_NAMES:
            probe = probe.replace(name, "")
        for rule, pattern in COMPILED:
            if pattern.search(probe):
                hits.append(Hit(path, number, rule, line.strip()))
    return hits


def tracked_files(root: Path) -> list[str]:
    out = subprocess.run(
        ["git", "ls-files", "-z"], cwd=root, check=True, capture_output=True
    ).stdout
    return [p for p in out.decode("utf-8", "surrogateescape").split("\0") if p]


def repo_root() -> Path:
    out = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"], check=True, capture_output=True, text=True
    ).stdout
    return Path(out.strip())


def scan_repo(root: Path) -> list[Hit]:
    hits: list[Hit] = []
    for path in tracked_files(root):
        if not in_scope(path) or is_exempt(path):
            continue
        try:
            data = (root / path).read_bytes()
        except (FileNotFoundError, IsADirectoryError):
            continue  # deleted in the work tree, or a submodule
        if is_binary(data):
            continue
        hits.extend(scan_text(path, data.decode("utf-8", "replace")))
    return hits


# --- report ------------------------------------------------------------------


def report(hits: list[Hit], count: bool) -> None:
    if count:
        print("per rule:")
        for rule, n in Counter(h.rule for h in hits).most_common():
            print(f"  {n:6d}  {rule}")
        print("per file:")
        for path, n in Counter(h.path for h in hits).most_common():
            print(f"  {n:6d}  {path}")
        print(f"total: {len(hits)} hits in {len({h.path for h in hits})} files")
    else:
        for h in hits:
            print(f"{h.path}:{h.line}: {h.rule}: {h.text}")
    if os.environ.get("GITHUB_ACTIONS"):
        for h in hits:
            print(f"::error file={h.path},line={h.line}::{h.rule}: ATM concern (ADR-BDC-0010)")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--count", action="store_true", help="print per-rule and per-file counts")
    args = parser.parse_args(argv)
    try:
        root = repo_root()
    except (subprocess.CalledProcessError, FileNotFoundError) as err:
        print(f"lint_no_atm: not a git work tree: {err}", file=sys.stderr)
        return 2
    hits = scan_repo(root)
    report(hits, args.count)
    if hits:
        print(f"{len(hits)} ATM concern(s) found (ADR-BDC-0010)", file=sys.stderr)
        return 1
    print("no ATM concerns (ADR-BDC-0010)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
