"""Tests for scripts/check_no_backend.py (NFR-BDC-0005, REQ-BDC-0006).

Each check has its planted negative: a `bd` or `dolt` on a temporary PATH, and a
MySQL or Dolt client crate in planted `cargo tree` text. No network, no cargo.

Run: python3 -I -m unittest discover -s scripts
"""

from __future__ import annotations

import os
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import check_no_backend  # noqa: E402

SCRIPT = Path(__file__).resolve().parent / "check_no_backend.py"

CLEAN_TREE = (
    "bd-cache v0.2.0 (/repo/crates/bd-cache)\n"
    "rusqlite v0.40.0\n"
    "libsqlite3-sys v0.36.0\n"
    "serde v1.0.228\n"
)


def run(*args: str, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    base = {k: v for k, v in os.environ.items() if k != "GITHUB_ACTIONS"}
    return subprocess.run([sys.executable, "-I", str(SCRIPT), *args], capture_output=True,
                          text=True, env={**base, **(env or {})})


class PathCheckTest(unittest.TestCase):
    def setUp(self) -> None:
        self.dir = Path(tempfile.mkdtemp(prefix="no-backend-"))
        self.addCleanup(shutil.rmtree, self.dir, ignore_errors=True)

    def plant(self, name: str) -> Path:
        tool = self.dir / (f"{name}.exe" if os.name == "nt" else name)
        tool.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
        tool.chmod(tool.stat().st_mode | stat.S_IXUSR)
        return tool

    def test_an_empty_path_dir_is_clean(self) -> None:
        self.assertEqual(check_no_backend.tools_on_path(str(self.dir)), [])
        result = run("path", env={"PATH": str(self.dir)})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("no bd or dolt on PATH", result.stdout)

    def test_a_planted_bd_or_dolt_fails(self) -> None:
        for name in ("bd", "dolt"):
            with self.subTest(name=name):
                tool = self.plant(name)
                found = check_no_backend.tools_on_path(str(self.dir))
                self.assertEqual([t for t, _ in found], [name])
                result = run("path", env={"PATH": str(self.dir)})
                self.assertEqual(result.returncode, 1)
                self.assertIn(f"{name} is on PATH", result.stderr)
                tool.unlink()

    def test_the_annotation_under_github_actions(self) -> None:
        self.plant("bd")
        result = run("path", env={"PATH": str(self.dir), "GITHUB_ACTIONS": "true"})
        self.assertEqual(result.returncode, 1)
        self.assertIn("::error::bd is on PATH", result.stdout)


class GraphCheckTest(unittest.TestCase):
    def tree_file(self, text: str) -> str:
        handle = tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False, encoding="utf-8")
        handle.write(text)
        handle.close()
        self.addCleanup(os.unlink, handle.name)
        return handle.name

    def test_a_clean_graph_passes(self) -> None:
        self.assertEqual(check_no_backend.client_crates(CLEAN_TREE), [])
        result = run("graph", "--tree", self.tree_file(CLEAN_TREE))
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_a_planted_client_crate_fails(self) -> None:
        for crate in ("mysql v25.0.1", "mysql_async v0.34.0", "mariadb-client v0.1.0",
                      "dolt-sql v0.1.0", "MySQL-Common v1.0.0"):
            with self.subTest(crate=crate):
                result = run("graph", "--tree", self.tree_file(CLEAN_TREE + crate + "\n"))
                self.assertEqual(result.returncode, 1)
                self.assertIn("a Dolt or MySQL client is in the dependency graph", result.stderr)

    def test_crates_that_only_mention_the_words_elsewhere_pass(self) -> None:
        tree = CLEAN_TREE + "sqlx-sqlite v0.8.0\nfeature-dolt-docs\nbeads-mysqlish\n"
        self.assertEqual(check_no_backend.client_crates(tree), [])


if __name__ == "__main__":
    unittest.main()
