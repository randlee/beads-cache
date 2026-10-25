"""Tests for scripts/lint_no_atm.py (ADR-BDC-0010).

Each test builds a temporary git repository with fixed file contents and runs
the lint in it, in a subprocess (exit code and output) or in process (the
pure scan). No timing, no network.

Run: python3 -m unittest discover -s scripts -p 'test_lint_no_atm.py'
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import lint_no_atm  # noqa: E402
from lint_no_atm import scan_text  # noqa: E402

SCRIPT = Path(__file__).resolve().parent / "lint_no_atm.py"

# Planted strings, one or more per rule.
PLANTED: dict[str, tuple[str, ...]] = {
    "atm-word": (
        "the tick sends an ATM message",
        "run `atm send lead@team`",
        "depends on atm-bd",
        "use atm_core::Team;",
        "export ATM_IDENTITY=cron",
        "read .atm.toml",
        "tests/atm_process.rs",
        "see atm-core",
    ),
    "tool-principal": ("sends as cron@tool", "from atm-bd@tool"),
    "atm-roles": ("tell the team-lead", "fallback_lead = \"x\"", "open a herdr pane"),
    "atm-bd-phase-file": (
        "reads phase-*.toml",
        "one phase-<x>.toml per phase",
        "fixture phase-bd.toml",
    ),
    "atm-bd-artifact": (
        "vendored bd_cache.py",
        "the bd_cache_cron script",
        "Source: BD-CACHE.md Cron module",
    ),
}

# Ordinary text that must never hit, including the generic words the cron's
# script extension point needs (ADR-BDC-0010).
NEGATIVES: tuple[str, ...] = (
    "format the output",
    "batman and robin",
    "the atmosphere is thin",
    "Atmospheric pressure; FORMAT; Batmobile",
    "a lead, a phase, a cron tick, a team",
    "phase-a scope and the phase-x label",
    "register a script; the hook is called; notify the entries",
    "the BD-CACHE v3 file at user_version 3",
    "import bd_cache_rs",
    "mail@toolbox.example",
    "lead-team and team leads",
)


def git(cwd: Path, *args: str) -> None:
    subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True)


class RepoCase(unittest.TestCase):
    """A fresh git repository per test."""

    def setUp(self) -> None:
        if shutil.which("git") is None:
            self.skipTest("git is not installed")
        self.root = Path(tempfile.mkdtemp(prefix="lint-no-atm-"))
        self.addCleanup(shutil.rmtree, self.root, ignore_errors=True)
        git(self.root, "init", "-q")

    def write(self, path: str, content: str | bytes) -> None:
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        if isinstance(content, bytes):
            target.write_bytes(content)
        else:
            target.write_text(content, encoding="utf-8")
        git(self.root, "add", "--", path)

    def run_lint(self, *args: str) -> subprocess.CompletedProcess[str]:
        env = {k: v for k, v in os.environ.items() if k != "GITHUB_ACTIONS"}
        return subprocess.run(
            [sys.executable, str(SCRIPT), *args],
            cwd=self.root,
            capture_output=True,
            text=True,
            env=env,
        )


class RuleTest(unittest.TestCase):
    def test_each_rule_hits_its_planted_strings(self) -> None:
        for rule, lines in PLANTED.items():
            for line in lines:
                with self.subTest(rule=rule, line=line):
                    rules = {h.rule for h in scan_text("src/x.rs", line)}
                    self.assertIn(rule, rules)

    def test_ordinary_words_do_not_hit(self) -> None:
        for line in NEGATIVES:
            with self.subTest(line=line):
                self.assertEqual(scan_text("src/x.rs", line), [])

    def test_rules_are_case_insensitive(self) -> None:
        for line in ("Atm Send", "CRON@TOOL", "Team-Lead", "HERDR", "Phase-BD.TOML"):
            with self.subTest(line=line):
                self.assertNotEqual(scan_text("src/x.rs", line), [])

    def test_hit_carries_path_line_and_text(self) -> None:
        hits = scan_text("a/b.md", "clean\n  atm send x  \nclean")
        self.assertEqual([(h.path, h.line, h.rule, h.text) for h in hits],
                         [("a/b.md", 2, "atm-word", "atm send x")])

    def test_the_lints_own_name_is_not_a_hit_but_the_rest_of_the_line_is(self) -> None:
        self.assertEqual(scan_text("ci.yml", "run: python3 scripts/lint_no_atm.py"), [])
        self.assertEqual(scan_text("ci.yml", "python3 -m unittest scripts/test_lint_no_atm.py"), [])
        hits = scan_text("ci.yml", "python3 scripts/lint_no_atm.py && atm send x")
        self.assertEqual([h.rule for h in hits], ["atm-word"])


class ExemptSectionTest(unittest.TestCase):
    DOC = (
        "# Architecture\n"
        "\n"
        "## ADR-BDC-0009: Something\n"
        "fine text\n"
        "## ADR-BDC-0010: No ATM or atm-bd concerns in beads-cache\n"
        "the cron sent atm send as cron@tool to the team-lead\n"
        "### a subsection stays inside\n"
        "phase-*.toml and herdr\n"
        "## ADR-BDC-0011: Later\n"
        "atm send leaks here\n"
    )

    def test_only_the_adr_section_of_architecture_md_is_skipped(self) -> None:
        hits = scan_text("docs/architecture.md", self.DOC)
        self.assertEqual([(h.line, h.rule) for h in hits], [(10, "atm-word")])

    def test_the_same_section_elsewhere_is_caught(self) -> None:
        hits = scan_text("docs/other.md", self.DOC)
        self.assertEqual(sorted({h.line for h in hits}), [5, 6, 8, 10])


class RepoTest(RepoCase):
    def test_clean_tree_exits_0(self) -> None:
        self.write("crates/x/src/lib.rs", "\n".join(NEGATIVES) + "\n")
        result = self.run_lint()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("no ATM concerns", result.stdout)

    def test_hits_exit_1_with_one_line_each(self) -> None:
        self.write("crates/x/src/lib.rs", "ok\n// atm send x\n")
        self.write("crates/x/tests/t.py", "SENDER = 'cron@tool'\n")
        result = self.run_lint()
        self.assertEqual(result.returncode, 1)
        self.assertEqual(
            result.stdout.splitlines(),
            ["crates/x/src/lib.rs:2: atm-word: // atm send x",
             "crates/x/tests/t.py:1: tool-principal: SENDER = 'cron@tool'"],
        )

    def test_untracked_files_are_not_scanned(self) -> None:
        self.write("crates/x/src/lib.rs", "clean\n")
        (self.root / "docs" / "untracked.md").parent.mkdir(parents=True, exist_ok=True)
        (self.root / "docs" / "untracked.md").write_text("atm send x\n", encoding="utf-8")
        self.assertEqual(self.run_lint().returncode, 0)

    def test_exempt_paths_are_skipped(self) -> None:
        text = "atm send as cron@tool to the team-lead\n"
        for path in ("scripts/lint_no_atm.py", "scripts/test_lint_no_atm.py",
                     "docs/team-protocol.md", "docs/plans/phase-b/sprint.md",
                     "docs/plans/deep/nested/x.toml"):
            self.write(path, text)
        result = self.run_lint()
        self.assertEqual(result.returncode, 0, result.stdout)

    def test_similar_paths_are_not_exempt(self) -> None:
        text = "atm send x\n"
        for path in ("docs/CHANGELOG.md", "docs/plans.md", "crates/plans/x.md",
                     "scripts/other_lint_no_atm.py.txt"):
            self.write(path, text)
        result = self.run_lint()
        self.assertEqual(result.returncode, 1)
        self.assertEqual(len(result.stdout.splitlines()), 4, result.stdout)

    def test_adr_section_is_exempt_in_architecture_md_only(self) -> None:
        self.write("docs/architecture.md", ExemptSectionTest.DOC)
        result = self.run_lint()
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout.splitlines(),
                         ["docs/architecture.md:10: atm-word: atm send leaks here"])

    def test_binary_files_are_skipped(self) -> None:
        self.write("crates/x/assets/blob.bin", b"\x00\x01atm send cron@tool\x00")
        self.write("crates/x/src/lib.rs", "clean\n")
        self.assertEqual(self.run_lint().returncode, 0)

    def test_count_prints_rule_and_file_totals(self) -> None:
        self.write("docs/a.md", "atm\natm\nteam-lead\n")
        self.write("docs/b.md", "cron@tool\n")
        result = self.run_lint("--count")
        self.assertEqual(result.returncode, 1)
        out = result.stdout
        self.assertNotIn("a.md:1:", out)
        self.assertRegex(out, r"\n\s+2\s+atm-word\n")
        self.assertRegex(out, r"\n\s+3\s+docs/a\.md\n")
        self.assertIn("total: 4 hits in 2 files", out)

    def test_github_actions_annotations(self) -> None:
        self.write("crates/x/src/lib.rs", "x\natm send\n")
        env = dict(os.environ, GITHUB_ACTIONS="true")
        result = subprocess.run([sys.executable, str(SCRIPT)], cwd=self.root,
                                capture_output=True, text=True, env=env)
        self.assertEqual(result.returncode, 1)
        self.assertIn("::error file=crates/x/src/lib.rs,line=2::atm-word:", result.stdout)

    def test_outside_a_git_tree_exits_2(self) -> None:
        bare = Path(tempfile.mkdtemp(prefix="lint-no-atm-nogit-"))
        self.addCleanup(shutil.rmtree, bare, ignore_errors=True)
        env = dict(os.environ, GIT_CEILING_DIRECTORIES=str(bare.parent))
        result = subprocess.run([sys.executable, str(SCRIPT)], cwd=bare,
                                capture_output=True, text=True, env=env)
        self.assertEqual(result.returncode, 2, result.stderr)


class ScopeTest(RepoCase):
    TEXT = "atm send as cron@tool to the team-lead\n"

    def test_out_of_scope_tooling_and_kit_files_are_not_hits(self) -> None:
        for path in (".claude/skills/team-lead/SKILL.md", ".claude/agents/qa.md",
                     ".cursor/skills/x/SKILL.md", ".codex/skills/x/SKILL.md",
                     ".agents/skills/x/SKILL.md", "CLAUDE.md", "AGENTS.md",
                     "CHANGELOG.md", "README.sc-publish.md", "install.py",
                     "release/publish-artifacts.toml", ".github/scripts/release_artifacts.py",
                     ".github/scripts/tests/test_release_artifacts.py",
                     ".github/actions/setup-renderer/action.yml",
                     ".github/workflows/release.yml", ".github/workflows/pypi-publish.yml",
                     ".github/workflows/prerelease-archive.yml",
                     "docs/getting-started.md", "docs/npm-channel-compatibility.md",
                     "docs/consumer-preflight-and-docli.md",
                     "docs/sprints/fix-homebrew-formula-bundled-paths-tojson.md",
                     "src/lib.rs", "Cargo.lock"):
            self.write(path, self.TEXT)
        result = self.run_lint()
        self.assertEqual(result.returncode, 0, result.stdout)

    def test_every_product_root_is_scanned(self) -> None:
        paths = ("crates/bd-cache-sync/src/atm.rs", "crates/bd-cache-py/pyproject.toml",
                 "crates/bd-cache-py/tests/test_x.py", "docs/requirements.md",
                 "docs/sprints/other.md", "scripts/prerelease_tag.py",
                 "scripts/bd-compat/lib.sh", "Cargo.toml", "README.md",
                 "rust-toolchain.toml", ".github/workflows/ci.yml",
                 ".github/workflows/python.yml", ".github/workflows/bd-compat.yml")
        for path in paths:
            self.write(path, "atm send x\n")
        result = self.run_lint()
        self.assertEqual(result.returncode, 1)
        self.assertEqual(sorted(line.split(":")[0] for line in result.stdout.splitlines()),
                         sorted(paths))


class ExemptionTableTest(unittest.TestCase):
    def test_exemptions_are_exactly_the_adr_list(self) -> None:
        self.assertEqual(lint_no_atm.EXEMPT_FILES,
                         ("scripts/lint_no_atm.py", "scripts/test_lint_no_atm.py", "docs/team-protocol.md"))
        self.assertEqual(lint_no_atm.EXEMPT_DIRS, ("docs/plans/",))
        self.assertEqual(lint_no_atm.EXEMPT_SECTIONS, (("docs/architecture.md", "## ADR-BDC-0010"),))


if __name__ == "__main__":
    unittest.main()
