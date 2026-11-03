"""Local fixture coverage for REQ-BDC-1003's template-upkeep boundary.

No bd, Dolt, network, application build, setup recipe or lint installer runs.
Opt in to the real pinned renderer with RUNTIME_TEMPLATE_SMOKE_SOURCE=<Git repo>.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import runtime_templates as rt  # noqa: E402


class FixtureTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="runtime-upkeep-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def tree(self, name: str, files: dict[str, str]) -> Path:
        root = self.root / name
        root.mkdir()
        for name, text in files.items():
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
        return root

    def mapping(self, rows: list[tuple[str, str]], version: object = 1) -> Path:
        path = self.root / "adoption.toml"
        path.write_text(f"version = {json.dumps(version)}\npaths = [\n" + "".join(
            f"{{ generated = {json.dumps(source)}, adopted = {json.dumps(dest)} }},\n"
            for source, dest in rows
        ) + "]\n", encoding="utf-8")
        return path


class ComparisonTest(FixtureTest):
    def setUp(self) -> None:
        super().setUp()
        self.old = self.tree("old", {"modified": "old", "deleted": "gone", "same": "same",
                                     "unmapped-deleted": "gone", ".config/data": "before"})
        self.new = self.tree("new", {"modified": "new", "added": "new", "same": "same",
                                     "unmapped-added": "new", ".config/data": "after"})

    def test_every_change_including_mapped_deletion_and_new_adoption_is_reported(self) -> None:
        mapping = self.mapping([("modified", "app/modified"), ("deleted", "app/deleted"),
                                ("added", "app/added")])
        self.assertEqual(rt.compare(self.old, self.new, mapping), {
            "changed_paths": 6,
            "changes": [
                {"generated": ".config/data", "change": "modified", "adopted": None, "ownership": "unadopted"},
                {"generated": "added", "change": "added", "adopted": "app/added", "ownership": "adopted"},
                {"generated": "deleted", "change": "deleted", "adopted": "app/deleted", "ownership": "adopted"},
                {"generated": "modified", "change": "modified", "adopted": "app/modified", "ownership": "adopted"},
                {"generated": "unmapped-added", "change": "added", "adopted": None, "ownership": "unadopted"},
                {"generated": "unmapped-deleted", "change": "deleted", "adopted": None, "ownership": "unadopted"},
            ],
        })

    def test_no_mapping_reports_all_changes_unadopted(self) -> None:
        result = rt.compare(self.old, self.new)
        self.assertEqual(result["changed_paths"], 6)
        self.assertTrue(all(row["ownership"] == "unadopted" and row["adopted"] is None
                            for row in result["changes"]))
        self.assertEqual(rt.compare(self.old, self.old)["changes"], [])

    def test_stale_duplicate_directory_and_unsupported_mappings_fail(self) -> None:
        cases = [([("missing", "app/missing")], 1),
                 ([("modified", "app/a"), ("modified", "app/b")], 1),
                 ([("modified", "app/a"), ("added", "app/a")], 1),
                 ([(".config", "app/config")], 1),
                 ([("modified", "app/a")], 2),
                 ([("modified", "app/a")], True)]
        for rows, version in cases:
            with self.subTest(rows=rows, version=version), self.assertRaises(rt.TemplateError):
                rt.compare(self.old, self.new, self.mapping(rows, version))

    def test_invalid_paths_on_either_side_are_rejected(self) -> None:
        for invalid in ("", "/absolute", "../escape", "a/../b", "a//b", "./a", "a/",
                        "a\\b", "C:/a", "*.rs", "a?", "a[0]"):
            for rows in ([(invalid, "app/a")], [("modified", invalid)]):
                with self.subTest(rows=rows), self.assertRaises(rt.TemplateError):
                    rt.compare(self.old, self.new, self.mapping(rows))

    def test_malformed_mapping_shape_fails(self) -> None:
        path = self.root / "adoption.toml"
        for text in ('version = 1\npaths = "bad"\n',
                     'version = 1\n[[paths]]\ngenerated = "modified"\n',
                     'version = 1\npaths = []\nextra = 1\n'):
            path.write_text(text, encoding="utf-8")
            with self.subTest(text=text), self.assertRaises(rt.TemplateError):
                rt.compare(self.old, self.new, path)

    @unittest.skipIf(os.name == "nt", "POSIX permission/link fixture")
    def test_executable_changes_are_reported_and_links_are_rejected(self) -> None:
        before = self.tree("mode-before", {"script": "same"})
        after = self.tree("mode-after", {"script": "same"})
        (after / "script").chmod(0o755)
        self.assertEqual(rt.compare(before, after)["changes"][0]["change"], "modified")
        (after / "link").symlink_to(before, target_is_directory=True)
        with self.assertRaises(rt.TemplateError):
            rt.compare(before, after)


class GenerationTest(FixtureTest):
    def setUp(self) -> None:
        super().setUp()
        self.source = self.tree("source", {"wizard/answers.schema.json": json.dumps({
            "$schema": "https://json-schema.org/draft/2020-12/schema", "type": "object",
            "additionalProperties": False, "required": ["project_name", "db", "mcp"],
            "properties": {"project_name": {"type": "string"},
                           "db": {"enum": ["none", "sqlite"]}, "mcp": {"type": "boolean"},
                           "licence": {"type": "string", "minLength": 1}},
        }), "template/marker": "committed source"})
        self.output = self.root / "baseline"
        self.answers = self.root / "answers.json"
        self.answers.write_bytes((rt.ROOT / "runtime/answers.json").read_bytes())
        self.pin = rt.ROOT / "runtime/pin.toml"
        self.calls = []

    def fake_run(self, argv: list[str], cwd: Path) -> str:
        self.calls.append((argv, cwd))
        if argv[:3] == ["cargo", "generate", "--version"]:
            return "cargo generate-generate 0.25.1"
        if argv[-1] == "--version":
            return "sc-compose 1.7.0"
        if argv[:2] == ["cargo", "generate"]:
            self.assertIn("--allow-commands", argv)
            self.assertIn("--no-workspace", argv)
            self.assertEqual(argv[argv.index("--name") + 1], "bd-cache")
            rendered = Path(argv[argv.index("--destination") + 1])
            self.assertEqual(list(rendered.iterdir()), [])
            self.assertNotEqual(rendered, self.output)
            self.assertFalse(self.output.exists())
            values = Path(argv[argv.index("--template-values-file") + 1]).read_text()
            self.assertEqual(rt.tomllib.loads(values), {"values": {"db": "none", "mcp": False, "licence": "MIT"}})
            (rendered / "Cargo.toml").write_text('[workspace.dependencies]\nsc-runtime = "0.1.1"\n')
            for name in ("AGENTS.md", "CLAUDE.md"):
                (rendered / f"{name}.j2").write_text("agent template")
            return ""
        if argv[1] == "render":
            # Compose writes timestamped diagnostics under its cwd. Keep those
            # outside the baseline instead of suppressing changes in diff.
            self.assertNotEqual(cwd, Path(argv[argv.index("--root") + 1]))
            log = cwd / ".sc-compose/logs/fixture.jsonl"
            log.parent.mkdir(parents=True, exist_ok=True)
            log.write_text("timestamped tool diagnostic")
            Path(argv[argv.index("--output") + 1]).write_text("rendered agent document")
        else:
            self.assertEqual(argv, ["cargo", "fmt", "--all"])
        return ""

    def generate(self) -> dict:
        with patch.object(rt, "export_source", return_value=self.source), patch.object(rt, "run", side_effect=self.fake_run):
            return rt.generate(self.pin, self.answers, self.output)

    def test_scratch_generation_supplies_values_and_renders_docs_without_recipes(self) -> None:
        result = self.generate()
        self.assertEqual(result["files"], 3)
        self.assertTrue(result["registry_dependencies"])
        self.assertFalse((self.output / "AGENTS.md.j2").exists())
        self.assertEqual(len(self.calls), 6)
        self.assertTrue(all(argv[0] != "just" for argv, _ in self.calls))

    def test_existing_file_directory_and_dangling_link_refused_before_generation(self) -> None:
        candidates = [self.source, self.answers]
        if os.name != "nt":
            link = self.root / "dangling"
            link.symlink_to(self.root / "missing")
            candidates.append(link)
        for path in candidates:
            with self.subTest(path=path), patch.object(rt, "export_source") as export:
                with self.assertRaisesRegex(rt.TemplateError, "already exists"):
                    rt.generate(self.pin, self.answers, path)
                export.assert_not_called()

    def test_application_destination_is_refused(self) -> None:
        with self.assertRaisesRegex(rt.TemplateError, "temporary storage"):
            rt.generate(self.pin, self.answers, rt.ROOT / "absent-runtime-test-output")

    def test_generator_and_document_failures_preserve_original_tree_and_diagnostics(self) -> None:
        original = self.tree("application", {"source.rs": "original", ".untracked": "keep"})
        for failed_step in ("generate", "render"):
            before = rt.file_tree(self.root)
            real_fake = self.fake_run

            def failing(argv: list[str], cwd: Path) -> str:
                if argv[1] == failed_step and argv[-1] != "--version":
                    # Model a generator that wrote partial scratch output before failure.
                    (cwd / "partial").write_text("incomplete")
                    raise rt.TemplateError("upstream exited 42: planted upstream diagnostic")
                return real_fake(argv, cwd)

            with patch.object(rt, "export_source", return_value=self.source), patch.object(rt, "run", side_effect=failing):
                with self.assertRaisesRegex(rt.TemplateError, "42: planted upstream diagnostic"):
                    rt.generate(self.pin, self.answers, self.output)
            self.assertEqual(rt.file_tree(self.root), before)
            self.assertEqual((original / "source.rs").read_text(), "original")
            self.assertFalse(self.output.exists())

    def test_upstream_process_failure_retains_stdout_stderr_and_status(self) -> None:
        child = subprocess.CompletedProcess(["renderer"], 42, "upstream stdout\n", "upstream stderr\n")
        with patch.object(rt.subprocess, "run", return_value=child):
            with self.assertRaisesRegex(rt.TemplateError, "exited 42") as failure:
                rt.run(["renderer"], self.root)
        self.assertIn("upstream stdout", str(failure.exception))
        self.assertIn("upstream stderr", str(failure.exception))

    def test_invalid_answers_fail_before_any_generator_or_tool_call(self) -> None:
        expected = json.loads(self.answers.read_text())
        for changed in ({**expected, "db": "both"}, {**expected, "mcp": "false"},
                        {**expected, "project_name": "other"}, {**expected, "extra": True},
                        {key: value for key, value in expected.items() if key != "licence"}):
            self.answers.write_text(json.dumps(changed))
            with patch.object(rt, "export_source", return_value=self.source), patch.object(rt, "run") as run:
                with self.assertRaises(rt.TemplateError):
                    rt.generate(self.pin, self.answers, self.output)
                run.assert_not_called()

    def test_wrong_tool_version_fails(self) -> None:
        with patch.object(rt, "run", return_value="cargo generate-generate 0.99.0"):
            with self.assertRaisesRegex(rt.TemplateError, "must be 0.25.1"):
                rt.require_tool(["cargo", "generate"], "0.25.1", self.root)

    def test_copy_failure_removes_only_new_output(self) -> None:
        with patch.object(rt, "export_source", return_value=self.source), patch.object(rt, "run", side_effect=self.fake_run):
            with patch.object(rt.shutil, "copytree", side_effect=OSError("copy failed")):
                with self.assertRaisesRegex(OSError, "copy failed"):
                    rt.generate(self.pin, self.answers, self.output)
        self.assertFalse(self.output.exists())

    def test_output_created_during_generation_is_preserved(self) -> None:
        original_fake = self.fake_run

        def race(argv: list[str], cwd: Path) -> str:
            result = original_fake(argv, cwd)
            if argv[1] == "render" and "CLAUDE.md.j2" in " ".join(argv):
                self.output.mkdir()
                (self.output / "keep").write_text("concurrent owner's file")
            return result

        with patch.object(rt, "export_source", return_value=self.source), patch.object(rt, "run", side_effect=race):
            with self.assertRaises(FileExistsError):
                rt.generate(self.pin, self.answers, self.output)
        self.assertEqual((self.output / "keep").read_text(), "concurrent owner's file")

    def test_patched_and_checkout_framework_dependencies_fail_baseline_check(self) -> None:
        self.generate()
        manifest = self.output / "Cargo.toml"
        for text in ('[patch.crates-io]\nsc-runtime = { path = "/checkout" }\n',
                     '[workspace.dependencies]\nsc-runtime = { path = "/checkout", version = "0.1.1" }\n',
                     '[dependencies]\nalias = { package = "sc-runtime", git = "https://invalid" }\n'):
            manifest.write_text(text)
            with self.subTest(text=text), self.assertRaises(rt.TemplateError):
                rt.check_baseline(self.output)


class GitSourceTest(FixtureTest):
    def test_tag_sha_is_verified_and_dirty_checkout_is_never_used(self) -> None:
        source = self.tree("git-source", {"template/file": "committed", "wizard/answers.schema.json": "{}"})
        rt.run(["git", "init", "--quiet"], source)
        rt.run(["git", "add", "."], source)
        rt.run(["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "fixture"], source)
        rt.run(["git", "tag", "fixture-v1"], source)
        commit = rt.run(["git", "rev-parse", "HEAD"], source)
        (source / "template/file").write_text("uncommitted checkout corruption")
        pin = {"tag": "fixture-v1", "commit": commit}
        scratch = self.root / "export"
        scratch.mkdir()
        exported = rt.export_source(pin, source, scratch)
        self.assertEqual((exported / "template/file").read_text(), "committed")
        with self.assertRaisesRegex(rt.TemplateError, "resolves to"):
            rt.export_source({**pin, "commit": "0" * 40}, source, scratch)

    def test_recorded_pin_requires_full_sha(self) -> None:
        pin = rt.load_pin(rt.ROOT / "runtime/pin.toml")
        path = self.root / "bad-pin.toml"
        path.write_text((rt.ROOT / "runtime/pin.toml").read_text().replace(pin["commit"], pin["commit"][:8]))
        with self.assertRaisesRegex(rt.TemplateError, "full lowercase"):
            rt.load_pin(path)


@unittest.skipUnless(os.environ.get("RUNTIME_TEMPLATE_SMOKE_SOURCE"), "explicit real pinned generation smoke")
class RealPinnedSmoke(FixtureTest):
    def test_real_pinned_generation(self) -> None:
        result = rt.generate(rt.ROOT / "runtime/pin.toml", rt.ROOT / "runtime/answers.json",
                             self.root / "pristine", Path(os.environ["RUNTIME_TEMPLATE_SMOKE_SOURCE"]))
        self.assertGreater(result["files"], 20)
        self.assertTrue(result["registry_dependencies"])
        self.assertTrue(result["agent_documents"])
        self.assertFalse((self.root / "pristine/crates/store-sqlite").exists())
        self.assertFalse((self.root / "pristine/crates/daemon/src/mcp.rs").exists())
        self.assertFalse((self.root / "pristine/.sc-compose").exists())
        rt.generate(rt.ROOT / "runtime/pin.toml", rt.ROOT / "runtime/answers.json",
                    self.root / "repeat", Path(os.environ["RUNTIME_TEMPLATE_SMOKE_SOURCE"]))
        self.assertEqual(rt.compare(self.root / "pristine", self.root / "repeat"),
                         {"changes": [], "changed_paths": 0})


if __name__ == "__main__":
    unittest.main()
