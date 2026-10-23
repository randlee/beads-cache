#!/usr/bin/env python3
"""Find TODO comments in repo files and emit structured rows."""

from __future__ import annotations

import argparse
import os
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

_ALLOWED_SUFFIXES = {
    ".rs",
}
# Directories pruned anywhere in the tree: VCS metadata, build output,
# vendored dependencies, and tool caches. Everything else is scanned.
_IGNORED_DIRS = {
    ".beads",
    ".cargo",
    ".git",
    ".hg",
    ".jj",
    ".svn",
    ".venv",
    ".worktrees",
    "__pycache__",
    "node_modules",
    "target",
    "third_party",
    "vendor",
    "venv",
}
_TODO_BODY_RE = re.compile(
    r"TODO(?:\((?P<tag>[^)]+)\))?(?:(?::\s*)|(?:\s+))(?P<text>[^\n\r]*)"
)
_COMMENT_PATTERNS = {
    ".rs": re.compile(r"//(?P<body>.*TODO(?:\(|:|\s).*)$"),
}


@dataclass(frozen=True)
class TodoRow:
    path: str
    line: int
    tag: str
    text: str

    def render(self) -> str:
        return f"{self.path}:{self.line}:{self.tag}:{self.text}"


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Recursively find TODO comments in repo Rust source files."
    )
    parser.add_argument(
        "root",
        nargs="?",
        default=".",
        help="repo root to scan (default: current directory)",
    )
    return parser.parse_args(argv)


def iter_candidate_files(root: Path) -> Iterable[Path]:
    candidates: list[Path] = []
    for dirpath, dirnames, filenames in os.walk(root):
        # Prune ignored directories in place so os.walk never descends into them.
        dirnames[:] = [name for name in dirnames if name not in _IGNORED_DIRS]
        for filename in filenames:
            path = Path(dirpath) / filename
            if path.suffix not in _ALLOWED_SUFFIXES:
                continue
            if not path.is_file():
                continue
            candidates.append(path)
    yield from sorted(candidates)


def extract_rows(root: Path) -> list[TodoRow]:
    rows: list[TodoRow] = []
    for path in iter_candidate_files(root):
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except UnicodeDecodeError:
            continue
        rel_path = path.relative_to(root).as_posix()
        line_pattern = _COMMENT_PATTERNS[path.suffix]
        for line_number, line in enumerate(lines, start=1):
            comment_match = line_pattern.search(line)
            if not comment_match:
                continue
            match = _TODO_BODY_RE.search(comment_match.group("body"))
            if not match:
                continue
            tag = (match.group("tag") or "untagged").strip() or "untagged"
            text = match.group("text").strip() or "<empty>"
            rows.append(
                TodoRow(
                    path=rel_path,
                    line=line_number,
                    tag=tag,
                    text=text,
                )
            )
    return rows


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    root = Path(args.root).resolve()
    for row in extract_rows(root):
        print(row.render())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
