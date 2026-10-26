#!/usr/bin/env python3
"""Fail on any ATM or atm-bd concern in the product (ADR-BDC-0010, ADR-BDC-0012).

beads-cache is a cache, a CLI for configuration and management, and a PyO3
API. It sends no messages, reads no ATM or atm-bd configuration, and names no
ATM or atm-bd concept in its source, tests, scripts, workflows or docs. This
lint keeps it that way.

Scope: the git-tracked files (`git ls-files -z`) under SCOPE_DIRS
(`crates/`, `docs/`, `scripts/`) and the files in SCOPE_FILES (`Cargo.toml`,
`README.md`, `rust-toolchain.toml` and the workflows `ci.yml`, `python.yml`,
`bd-compat.yml`), minus the kit docs in KIT_DOCS. For each in-scope,
non-exempt file it matches every rule below, case-insensitively, against

- the file's tracked path, reported as `path:0: <rule>: <path>`; and
- each line of its contents (binary files skipped), reported as
  `path:line: <rule>: <text>`.

`--paths PREFIX...` narrows the scan to the in-scope files equal to or under
those repo-relative prefixes, so a sprint can lint its own fence; it never
widens the scope or lifts an exemption.

The only exemptions inside the scope are the lint and its tests and
`docs/team-protocol.md` (EXEMPT_FILES, ADR-BDC-0012). The plans, the anchor
record and every ADR are scanned like any other doc and name a prohibited word
only as "a prohibited word". One rule is narrowed for one line: `scope-prefix`
does not match the one `LEGACY_SCOPE_PREFIX` declaration of `bd-cache-model`'s
compatibility module (NARROWED, ADR-BDC-0012); the same text in any other
file, and any other line of that file, still hits. No inline pragma, no
allowlist file. Scope, exemptions and narrowed lines change only through a
later ADR.

The RULES below are the one place the prohibited words are written (ADR-BDC-0012).

Out of scope by construction (dev tooling, not product): the agent tooling
(`.claude/**`, `.cursor/**`, `.codex/**`, `.agents/**`, `CLAUDE.md`,
`AGENTS.md`), which coordinates the repo's own agents over ATM; the sc-publish
kit, frozen upstream (`.github/scripts/**`, `.github/actions/**`, the
`*-publish.yml`, `release*.yml` and `prerelease-archive.yml` workflows,
`release/**`, `install.py`, `README.sc-publish.md` and the kit docs in
KIT_DOCS); `CHANGELOG.md` (a historical record); and `Cargo.lock` (generated,
with no hits).

Strings built at runtime (joined from parts, read from input) cannot be caught
here; review guards against them.

Run from anywhere inside the repository:

    python3 scripts/lint_no_atm.py                    # one line per hit, exit 1 on any
    python3 scripts/lint_no_atm.py --count            # per-rule and per-file counts
    python3 scripts/lint_no_atm.py --paths crates/x/  # only the files under crates/x/

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
    # An ATM principal on the `tool` team: cron@tool, atm-bd@tool, and the
    # cron principal spelled as an identifier: cron_tool, cron-tool.
    ("tool-principal", r"@tool\b|(?<![a-z0-9])cron[@_-]tool(?![a-z0-9])"),
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
    # A hard-coded scope prefix: a `"phase-"` / `'phase-'` string literal or a
    # `phase-{` format string (`format!("phase-{}", x)`, `f"phase-{x}"`).
    # Scopes are configured, never derived from a consumer's phase naming.
    # Not a hit: the column `phase`, `DEFAULT 'phase'`, `phase-b` in prose.
    ("scope-prefix", r"[\"']phase-[\"']|(?<![a-z0-9_])phase-\{"),
    # The first consumer's maintainer handle (ADR-BDC-0012, anchor ANC-001), as
    # a word: on its own, in a path, or before `'s`, `@`, `-` or `_`. Not a hit
    # inside an ordinary word (phoenix): a letter or digit before it, or a
    # letter after it, rules it out.
    ("maintainer-handle", r"(?<![a-z0-9])fenix(?![a-z])"),
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

# The one line a rule is narrowed for (ADR-BDC-0012): (rule, path, the line
# with its surrounding whitespace removed). The 0.1 bare-name rule of
# ADR-BDC-0013 writes the legacy scope prefix once, as this constant; every use
# refers to it. Any other line, in this file or another, is matched as usual.
NARROWED: tuple[tuple[str, str, str], ...] = (
    (
        "scope-prefix",
        "crates/bd-cache-model/src/compat.rs",
        'pub const LEGACY_SCOPE_PREFIX: &str = "phase-";',
    ),
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
    return path in EXEMPT_FILES


def is_binary(data: bytes) -> bool:
    return b"\0" in data[:8192]


def is_narrowed(rule: str, path: str, line: str) -> bool:
    """True for the one line NARROWED takes out of `rule`."""
    return (rule, path, line.strip()) in NARROWED


def scan_text(path: str, text: str) -> list[Hit]:
    hits: list[Hit] = []
    for number, line in enumerate(text.splitlines(), start=1):
        probe = line
        for name in SELF_NAMES:
            probe = probe.replace(name, "")
        for rule, pattern in COMPILED:
            if pattern.search(probe) and not is_narrowed(rule, path, line):
                hits.append(Hit(path, number, rule, line.strip()))
    return hits


def scan_path(path: str) -> list[Hit]:
    """Match the rules against a tracked path itself, reported at line 0."""
    probe = path
    for name in SELF_NAMES:
        probe = probe.replace(name, "")
    return [Hit(path, 0, rule, path) for rule, pattern in COMPILED if pattern.search(probe)]


def normalize_prefix(prefix: str) -> str:
    prefix = prefix.replace(os.sep, "/")
    while prefix.startswith("./"):
        prefix = prefix[2:]
    return prefix.rstrip("/")


def under_prefixes(path: str, prefixes: list[str]) -> bool:
    """True when `path` equals a prefix or lies in the directory it names."""
    for prefix in prefixes:
        if prefix in ("", "."):
            return True
        if path == prefix or path.startswith(prefix + "/"):
            return True
    return False


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


def scan_repo(root: Path, prefixes: list[str] | None = None) -> list[Hit]:
    hits: list[Hit] = []
    wanted = [normalize_prefix(p) for p in prefixes] if prefixes else None
    for path in tracked_files(root):
        if not in_scope(path) or is_exempt(path):
            continue
        if wanted is not None and not under_prefixes(path, wanted):
            continue
        hits.extend(scan_path(path))
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
            where = f"file={h.path},line={h.line}" if h.line else f"file={h.path}"
            print(f"::error {where}::{h.rule}: ATM concern (ADR-BDC-0010)")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--count", action="store_true", help="print per-rule and per-file counts")
    parser.add_argument(
        "--paths",
        nargs="+",
        metavar="PREFIX",
        help="scan only the in-scope tracked files equal to or under these repo-relative prefixes",
    )
    args = parser.parse_args(argv)
    try:
        root = repo_root()
    except (subprocess.CalledProcessError, FileNotFoundError) as err:
        print(f"lint_no_atm: not a git work tree: {err}", file=sys.stderr)
        return 2
    hits = scan_repo(root, args.paths)
    report(hits, args.count)
    if hits:
        print(f"{len(hits)} ATM concern(s) found (ADR-BDC-0010)", file=sys.stderr)
        return 1
    print("no ATM concerns (ADR-BDC-0010)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
