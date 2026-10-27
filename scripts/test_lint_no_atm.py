"""Tests for scripts/lint_no_atm.py (ADR-BDC-0010, ADR-BDC-0012).

Each test builds a temporary git repository with fixed file contents and runs
the lint in it, in a subprocess (exit code and output) or in process (the
pure scan). No timing, no network.

Run: python3 -I -m unittest discover -s scripts
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
REPO = Path(__file__).resolve().parent.parent

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
    "scope-prefix": (
        'format!("phase-{}", x)',
        'if name.starts_with("phase-") {',
        "prefix = 'phase-'",
        'scope = f"phase-{x}"',
        'label = "phase-%s" % name',
        'label="phase-$name"',
        'label="phase-${name}"',
        'let label = "phase-\\(name)"',
    ),
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
        # A `test_` prefix does not hide the name (an underscore is a word character).
        "ported cases of test_bd_cache_cron.py keep their names",
        "see tests/test_bd_cache.py",
    ),
    "consumer-schema-file": (
        "models in bead_schema.py",
        "required at close (close_fields.py)",
        "run close_fields_backfill",
        "tests/test_close_fields.py",
    ),
    "maintainer-handle": (
        "reviewed by fenix",
        "fenix's input on S81",
        '"created_by": "fenix",',
        "ping @fenix",
        "see fenix-review.txt and fenix_notes",
        "FENIX",
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
    # The consumer-file rules do not hit prose or lookalike words.
    "a bead schema package and the fields set at close",
    "closed_fields and enclose_fields and bead_schemas.json",
    "the bd_cache package and bd_cache.pyi",
    "mail@toolbox.example",
    "lead-team and team leads",
    # L2: the frozen column name and its default, and phase names in prose.
    "phase TEXT NOT NULL DEFAULT 'phase'",
    "SELECT phase FROM scopes WHERE phase = ?1",
    "row.phase = \"x\"",
    "the phase-b plan and the phase-a scope",
    "a \"phase\" label",
    # L5: neutral words around cron and tool.
    "the cron job uses a tool",
    "cron_tools_dir and mycron-tool and cron-toolkit",
    # ADR-BDC-0012: ordinary words that contain the maintainer handle.
    "a phoenix rises; Phoenix, AZ",
    "afenix and fenixia and fenixes",
    "the maintainer of the first consumer",
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


class NoSectionExemptionTest(unittest.TestCase):
    """ADR-BDC-0012: the ADR-BDC-0010 section of docs/architecture.md is scanned like any text."""

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

    def test_the_adr_section_of_architecture_md_is_caught(self) -> None:
        hits = scan_text("docs/architecture.md", self.DOC)
        self.assertEqual(sorted({h.line for h in hits}), [5, 6, 8, 10])

    def test_architecture_md_and_any_other_doc_are_scanned_alike(self) -> None:
        self.assertEqual(
            [(h.line, h.rule) for h in scan_text("docs/architecture.md", self.DOC)],
            [(h.line, h.rule) for h in scan_text("docs/other.md", self.DOC)],
        )

    def test_the_restated_section_passes(self) -> None:
        doc = ("## ADR-BDC-0010: No consumer-specific concerns in beads-cache\n"
               "the first consumer's name is a prohibited word; its maintainer handle is one too\n")
        self.assertEqual(scan_text("docs/architecture.md", doc), [])


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
        text = "atm send as cron@tool to the team-lead, reviewed by fenix\n"
        for path in ("scripts/lint_no_atm.py", "scripts/test_lint_no_atm.py",
                     "docs/team-protocol.md"):
            self.write(path, text)
        result = self.run_lint()
        self.assertEqual(result.returncode, 0, result.stdout)

    def test_a_prohibited_word_under_docs_plans_fails(self) -> None:
        # ADR-BDC-0012: `docs/plans/` is no longer exempt (the planted negative of b-9 AC-1).
        self.write("docs/plans/phase-b/sprint.md", "the sprint removes the atm sender\n")
        self.write("docs/plans/phase-b/anchors/anchors.md", "said by fenix\n")
        self.write("docs/plans/deep/nested/x.toml", "clean = true\n")
        result = self.run_lint()
        self.assertEqual(result.returncode, 1)
        self.assertEqual(
            result.stdout.splitlines(),
            ["docs/plans/phase-b/anchors/anchors.md:1: maintainer-handle: said by fenix",
             "docs/plans/phase-b/sprint.md:1: atm-word: the sprint removes the atm sender"],
        )

    def test_the_maintainer_handle_in_a_fixture_fails(self) -> None:
        # ADR-BDC-0012, ANC-001: the planted negative of b-9 AC-1.
        self.write("crates/sc-beads/tests/fixtures/bd-1.3.1/list-rows.json",
                   '[{"id": "a", "created_by": "recorder"},\n {"id": "b", "created_by": "fenix"}]\n')
        result = self.run_lint()
        self.assertEqual(result.returncode, 1)
        self.assertEqual(
            result.stdout.splitlines(),
            ['crates/sc-beads/tests/fixtures/bd-1.3.1/list-rows.json:2: maintainer-handle: '
             '{"id": "b", "created_by": "fenix"}]'],
        )

    def test_similar_paths_are_not_exempt(self) -> None:
        text = "atm send x\n"
        for path in ("docs/CHANGELOG.md", "docs/plans.md", "crates/plans/x.md",
                     "scripts/other_lint_no_atm.py.txt"):
            self.write(path, text)
        result = self.run_lint()
        self.assertEqual(result.returncode, 1)
        self.assertEqual(len(result.stdout.splitlines()), 4, result.stdout)

    def test_the_adr_section_of_architecture_md_is_not_exempt(self) -> None:
        self.write("docs/architecture.md", NoSectionExemptionTest.DOC)
        result = self.run_lint()
        self.assertEqual(result.returncode, 1)
        self.assertEqual(sorted({int(line.split(":")[1]) for line in result.stdout.splitlines()}),
                         [5, 6, 8, 10])

    def test_a_known_binary_is_skipped(self) -> None:
        self.write("crates/x/assets/logo.png", b"\x89PNG\x00\x01atm send cron@tool\x00")
        self.write("crates/x/src/lib.rs", "clean\n")
        self.assertEqual(self.run_lint().returncode, 0)

    def test_text_with_a_nul_byte_and_no_known_extension_is_unscannable(self) -> None:
        # RSH-006: a NUL byte no longer hides a file; only KNOWN_BINARY extensions are skipped.
        self.write("crates/x/assets/blob.bin", b"\x00\x01atm send cron@tool\x00")
        result = self.run_lint()
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout.splitlines(),
                         ["crates/x/assets/blob.bin:0: unscannable: crates/x/assets/blob.bin"])

    def test_utf16_text_is_decoded_and_scanned(self) -> None:
        # RSH-006: UTF-16 text has NUL bytes but a byte-order mark; it is read, not skipped.
        self.write("docs/notes.txt", "clean\natm send x\n".encode("utf-16"))
        result = self.run_lint()
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout.splitlines(), ["docs/notes.txt:2: atm-word: atm send x"])

    @unittest.skipIf(os.name == "nt", "symlinks need privileges on Windows")
    def test_a_tracked_symlink_is_scanned_as_its_target_text(self) -> None:
        # RSH-006: a dangling link and a link to a directory are reported, not skipped.
        (self.root / "docs").mkdir(parents=True, exist_ok=True)
        (self.root / "docs" / "dangling").symlink_to("atm-notes/missing.md")
        (self.root / "crates").mkdir(parents=True, exist_ok=True)
        (self.root / "crates" / "dir-link").symlink_to(".")
        git(self.root, "add", "--", "docs/dangling", "crates/dir-link")
        result = self.run_lint()
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout.splitlines(), ["docs/dangling:1: atm-word: atm-notes/missing.md"])

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

    def test_declared_out_of_scope_files_are_not_hits(self) -> None:
        for path in (".claude/skills/team-lead/SKILL.md", ".claude/agents/qa.md",
                     ".cursor/skills/x/SKILL.md", ".codex/skills/x/SKILL.md",
                     ".agents/skills/x/SKILL.md", ".sc/plan-anchor/settings.yaml",
                     ".beads/issues.jsonl", ".beads/config.yaml",
                     "CHANGELOG.md", "README.sc-publish.md", "install.py",
                     "release/publish-artifacts.toml", ".github/scripts/release_artifacts.py",
                     ".github/scripts/tests/test_release_artifacts.py",
                     ".github/actions/setup-renderer/action.yml",
                     ".github/workflows/release.yml", ".github/workflows/pypi-publish.yml",
                     ".github/workflows/release-candidate.yml",
                     ".github/workflows/prerelease-archive.yml",
                     "docs/getting-started.md", "docs/npm-channel-compatibility.md",
                     "docs/consumer-preflight-and-docli.md",
                     "docs/sprints/fix-homebrew-formula-bundled-paths-tojson.md",
                     "Cargo.lock"):
            self.write(path, self.TEXT)
        result = self.run_lint()
        self.assertEqual(result.returncode, 0, result.stdout)

    def test_a_path_not_declared_is_scanned_wherever_it_is(self) -> None:
        # ADR-BDC-0012 Amendment 2 (B9Q1-013): the scope is inverted, so a new or
        # misspelled top-level directory, a root file and a lookalike of a declared
        # entry are scanned, never skipped silently.
        paths = ("src/lib.rs", "newdir/notes.md", "doc/plan/x.md", ".github/CODEOWNERS",
                 ".github/workflows/other.yml", "LICENSE", ".gitattributes",
                 "CLAUDE.md", ".beadsx/data.jsonl", "release.md", "install.py.bak",
                 "docs/getting-started.md.orig")
        for path in paths:
            self.write(path, "atm send x\n")
        result = self.run_lint()
        self.assertEqual(result.returncode, 1)
        self.assertEqual(sorted(line.split(":")[0] for line in result.stdout.splitlines()),
                         sorted(paths))

    def test_every_product_root_is_scanned(self) -> None:
        paths = ("crates/bd-cache-sync/src/tick.rs", "crates/bd-cache-py/pyproject.toml",
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


class ScopePrefixTest(unittest.TestCase):
    COMPAT = "crates/bd-cache-model/src/compat.rs"
    DECLARATION = 'pub const LEGACY_SCOPE_PREFIX: &str = "phase-";'

    def test_hits(self) -> None:
        for line in ('format!("phase-{}", x)', '"phase-"', 'f"phase-{x}"', "'phase-'",
                     'FORMAT!("PHASE-{}", X)', '"phase-%d" % n', '"phase-$x"',
                     '"phase-${x}"', '"phase-\\(x)"', "sprintf('phase-%s', $x)"):
            with self.subTest(line=line):
                self.assertEqual([h.rule for h in scan_text("src/x.rs", line)], ["scope-prefix"])

    def test_the_one_legacy_declaration_is_not_a_hit(self) -> None:
        # ADR-BDC-0012: the compatibility constant, with or without indentation.
        for line in (self.DECLARATION, f"    {self.DECLARATION}  "):
            with self.subTest(line=line):
                self.assertEqual(scan_text(self.COMPAT, line), [])

    def test_a_second_copy_of_the_declaration_hits(self) -> None:
        # RBQA-F011 (B9Q1-033): the narrowing spares the one declaration, once per file.
        text = f"{self.DECLARATION}\nfn f() {{}}\n    {self.DECLARATION}\n"
        hits = scan_text(self.COMPAT, text)
        self.assertEqual([(h.line, h.rule) for h in hits], [(3, "scope-prefix")])
        self.assertEqual(scan_text(self.COMPAT, f"{self.DECLARATION}\n"), [])

    def test_any_other_legacy_literal_hits(self) -> None:
        # ADR-BDC-0012: the planted negative of b-9 AC-1, a second literal of the prefix.
        cases = (
            (self.COMPAT, 'pub const OTHER: &str = "phase-";'),
            (self.COMPAT, 'let label = format!("phase-{}", id);'),
            (self.COMPAT, f'{self.DECLARATION} // and "phase-" again'),
            ("crates/bd-cache-model/src/scope.rs", self.DECLARATION),
            ("crates/bd-cache/src/sync.rs", 'if name.starts_with("phase-") {'),
        )
        for path, line in cases:
            with self.subTest(path=path, line=line):
                self.assertEqual([h.rule for h in scan_text(path, line)], ["scope-prefix"])

    def test_the_declaration_with_more_text_on_its_line_is_matched_as_usual(self) -> None:
        hits = scan_text(self.COMPAT, f"{self.DECLARATION} // atm")
        self.assertEqual(sorted(h.rule for h in hits), ["atm-word", "scope-prefix"])

    def test_non_hits(self) -> None:
        for line in ("phase TEXT NOT NULL DEFAULT 'phase'", "let phase = row.get(\"phase\")?;",
                     "SELECT phase FROM scopes", "the phase-b sprints", "scope phase-b is synced",
                     "50% of phase-a", "a phase-(x) note", "the cost is $5 per phase-b run",
                     "subphase-{x}", "multiphase-%s"):
            with self.subTest(line=line):
                self.assertEqual(scan_text("src/x.rs", line), [])


class ToolPrincipalTest(unittest.TestCase):
    def test_cron_variants_hit_case_insensitively(self) -> None:
        for line in ("cron@tool", "CRON@TOOL", "const SENDER: &str = cron_tool;", "Cron_Tool",
                     "the cron-tool principal", "CRON-TOOL"):
            with self.subTest(line=line):
                self.assertIn("tool-principal", {h.rule for h in scan_text("src/x.rs", line)})

    def test_cron_variants_need_whole_words(self) -> None:
        for line in ("cron_tools", "mycron_tool", "cron-toolkit", "cron tool", "cron::tool"):
            with self.subTest(line=line):
                self.assertEqual(scan_text("src/x.rs", line), [])


class PathScanTest(RepoCase):
    def test_path_hits_are_reported_at_line_0(self) -> None:
        self.write("crates/x/tests/atm_process.rs", "clean\n")
        self.write("crates/x/tests/fixtures/.atm-bd/phases.toml", "clean\n")
        self.write("crates/x/src/format.rs", "clean\n")
        result = self.run_lint()
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertEqual(
            sorted(result.stdout.splitlines()),
            ["crates/x/tests/atm_process.rs:0: atm-word: crates/x/tests/atm_process.rs",
             "crates/x/tests/fixtures/.atm-bd/phases.toml:0: atm-word: "
             "crates/x/tests/fixtures/.atm-bd/phases.toml"],
        )

    def test_neutral_path_is_not_a_hit(self) -> None:
        self.write("crates/x/src/format.rs", "clean\n")
        self.assertEqual(lint_no_atm.scan_path("crates/x/src/format.rs"), [])
        self.assertEqual(self.run_lint().returncode, 0)

    def test_path_and_content_hits_are_both_reported(self) -> None:
        self.write("docs/atm.md", "line one\natm send x\n")
        result = self.run_lint()
        self.assertEqual(result.stdout.splitlines(),
                         ["docs/atm.md:0: atm-word: docs/atm.md",
                          "docs/atm.md:2: atm-word: atm send x"])

    def test_exempt_and_out_of_scope_paths_are_not_path_hits(self) -> None:
        for path in (".claude/atm.md", ".beads/atm.jsonl",
                     "scripts/lint_no_atm.py", "scripts/test_lint_no_atm.py"):
            self.write(path, "clean\n")
        self.assertEqual(self.run_lint().returncode, 0, self.run_lint().stdout)

    def test_a_plans_path_is_a_path_hit(self) -> None:
        self.write("docs/plans/atm_notes.md", "clean\n")
        self.write("docs/plans/fenix-review.md", "clean\n")
        result = self.run_lint()
        self.assertEqual(result.returncode, 1)
        self.assertEqual(sorted(result.stdout.splitlines()),
                         ["docs/plans/atm_notes.md:0: atm-word: docs/plans/atm_notes.md",
                          "docs/plans/fenix-review.md:0: maintainer-handle: docs/plans/fenix-review.md"])

    def test_path_hit_annotation_has_no_line(self) -> None:
        self.write("crates/x/src/atm.rs", "clean\n")
        env = dict(os.environ, GITHUB_ACTIONS="true")
        result = subprocess.run([sys.executable, str(SCRIPT)], cwd=self.root,
                                capture_output=True, text=True, env=env)
        self.assertIn("::error file=crates/x/src/atm.rs::atm-word:", result.stdout)


class PathsOptionTest(RepoCase):
    def setUp(self) -> None:
        super().setUp()
        self.write("crates/a/src/lib.rs", "atm send a\n")
        self.write("crates/ab/src/lib.rs", "atm send ab\n")
        self.write("docs/b.md", "atm send b\n")
        self.write("crates/clean/src/lib.rs", "clean\n")

    def files(self, result: subprocess.CompletedProcess[str]) -> list[str]:
        return sorted({line.split(":")[0] for line in result.stdout.splitlines()})

    def test_limits_to_the_prefix(self) -> None:
        result = self.run_lint("--paths", "crates/a")
        self.assertEqual(result.returncode, 1)
        self.assertEqual(self.files(result), ["crates/a/src/lib.rs"])

    def test_trailing_slash_and_dot_slash_are_accepted(self) -> None:
        for prefix in ("crates/a/", "./crates/a"):
            with self.subTest(prefix=prefix):
                self.assertEqual(self.files(self.run_lint("--paths", prefix)),
                                 ["crates/a/src/lib.rs"])

    def test_several_prefixes_and_an_exact_file(self) -> None:
        result = self.run_lint("--paths", "crates/ab", "docs/b.md")
        self.assertEqual(result.returncode, 1)
        self.assertEqual(self.files(result), ["crates/ab/src/lib.rs", "docs/b.md"])

    def test_clean_prefix_exits_0(self) -> None:
        result = self.run_lint("--paths", "crates/clean")
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn("no ATM concerns", result.stdout)

    def test_unmatched_prefix_exits_0(self) -> None:
        self.assertEqual(self.run_lint("--paths", "crates/none").returncode, 0)

    def test_does_not_widen_scope_or_lift_exemptions(self) -> None:
        self.write(".claude/x.md", "atm send x\n")
        self.write("docs/team-protocol.md", "atm send x\n")
        for prefix in (".claude", "docs/team-protocol.md"):
            with self.subTest(prefix=prefix):
                self.assertEqual(self.run_lint("--paths", prefix).returncode, 0)

    def test_docs_plans_is_scanned_through_paths_too(self) -> None:
        self.write("docs/plans/p.md", "atm send x\n")
        self.assertEqual(self.run_lint("--paths", "docs/plans").returncode, 1)

    def test_count_with_paths(self) -> None:
        result = self.run_lint("--count", "--paths", "docs")
        self.assertEqual(result.returncode, 1)
        self.assertIn("total: 1 hits in 1 files", result.stdout)

    def test_paths_needs_a_value(self) -> None:
        self.assertEqual(self.run_lint("--paths").returncode, 2)


class ExemptionTableTest(unittest.TestCase):
    def test_exemptions_are_exactly_the_adr_list(self) -> None:
        # ADR-BDC-0012: the lint, its tests and the team protocol; no directory, no section.
        self.assertEqual(lint_no_atm.EXEMPT_FILES,
                         ("scripts/lint_no_atm.py", "scripts/test_lint_no_atm.py", "docs/team-protocol.md"))
        self.assertFalse(hasattr(lint_no_atm, "EXEMPT_DIRS"))
        self.assertFalse(hasattr(lint_no_atm, "EXEMPT_SECTIONS"))

    def test_the_one_narrowed_line_is_the_legacy_declaration(self) -> None:
        self.assertEqual(lint_no_atm.NARROWED, (
            ("scope-prefix", "crates/bd-cache-model/src/compat.rs",
             'pub const LEGACY_SCOPE_PREFIX: &str = "phase-";'),
        ))

    def test_the_narrowed_line_is_the_one_in_the_tree_exactly_once(self) -> None:
        # The declaration the rule is narrowed for is the line the compatibility module holds,
        # once (RBQA-F011), so a rename there makes the whole-tree run fail rather than pass
        # silently, and a second copy is a hit.
        compat = REPO / "crates/bd-cache-model/src/compat.rs"
        if not compat.is_file():
            self.skipTest("not run inside the beads-cache tree")
        lines = [line.strip() for line in compat.read_text(encoding="utf-8").splitlines()]
        self.assertEqual(lines.count(lint_no_atm.NARROWED[0][2]), 1)


class TreeClassificationTest(unittest.TestCase):
    """ADR-BDC-0012 Amendment 2 (B9Q1-013): every tracked path is scanned or declared out of scope."""

    def tracked(self) -> list[str]:
        if shutil.which("git") is None or not (REPO / "crates/bd-cache-model/src/compat.rs").is_file():
            self.skipTest("not run inside the beads-cache tree")
        return lint_no_atm.tracked_files(REPO)

    def test_every_tracked_path_is_scanned_or_declared_with_a_reason(self) -> None:
        reasons = {reason for _, reason in lint_no_atm.OUT_OF_SCOPE}
        for path in self.tracked():
            with self.subTest(path=path):
                reason = lint_no_atm.out_of_scope_reason(path)
                self.assertTrue(lint_no_atm.in_scope(path) or reason in reasons, path)
                self.assertEqual(lint_no_atm.in_scope(path), reason is None, path)

    def test_every_declared_entry_matches_a_tracked_path(self) -> None:
        # A declaration that matches nothing is stale: it would hide a file added later.
        tracked = self.tracked()
        for pattern, _ in lint_no_atm.OUT_OF_SCOPE:
            with self.subTest(pattern=pattern):
                self.assertTrue(any(lint_no_atm.matches(pattern, p) for p in tracked), pattern)

    def test_the_product_roots_are_scanned(self) -> None:
        tracked = set(self.tracked())
        for path in ("Cargo.toml", "README.md", "rust-toolchain.toml", "LICENSE",
                     ".github/workflows/ci.yml", ".github/workflows/python.yml",
                     ".github/workflows/bd-compat.yml", "crates/bd-cache/src/lib.rs",
                     "docs/architecture.md", "docs/plans/phase-b/anchors/anchors.json",
                     "scripts/lint_no_atm.py"):
            with self.subTest(path=path):
                self.assertIn(path, tracked)
                self.assertTrue(lint_no_atm.in_scope(path), path)


if __name__ == "__main__":
    unittest.main()
