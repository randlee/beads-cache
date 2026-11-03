"""Local fixture coverage for REQ-BDC-1003's template-upkeep boundary.

No bd, Dolt, network, application build, setup recipe or lint installer runs.
Opt in to the real pinned renderer with RUNTIME_TEMPLATE_SMOKE_SOURCE=<Git repo>.
"""

from __future__ import annotations

import json
import io
import os
import queue
import threading
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout, redirect_stderr
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


class GenerationFixture(FixtureTest):
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


class GenerationTest(GenerationFixture):
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
        for failed_step, code in (("generate", "UPSTREAM_FAILED"), ("render", "UPSTREAM_FAILED"),
                                  ("generate", "UPSTREAM_TIMEOUT")):
            before = rt.file_tree(self.root)
            real_fake = self.fake_run

            def failing(argv: list[str], cwd: Path) -> str:
                if argv[1] == failed_step and argv[-1] != "--version":
                    # Model a generator that wrote partial scratch output before failure.
                    (cwd / "partial").write_text("incomplete")
                    raise rt.TemplateError("upstream exited 42: planted upstream diagnostic", code=code,
                                           cause={"exit_status": 42, "stdout": "partial output",
                                                  "stderr": "planted upstream diagnostic"})
                return real_fake(argv, cwd)

            with patch.object(rt, "export_source", return_value=self.source), patch.object(rt, "run", side_effect=failing):
                with self.assertRaisesRegex(rt.TemplateError, "42: planted upstream diagnostic") as failure:
                    rt.generate(self.pin, self.answers, self.output)
                self.assertEqual(failure.exception.code, f"TEMPLATE.{code}")
            self.assertEqual(rt.file_tree(self.root), before)
            self.assertEqual((original / "source.rs").read_text(), "original")
            self.assertFalse(self.output.exists())

    def test_upstream_process_failure_retains_stdout_stderr_and_status(self) -> None:
        with self.assertRaisesRegex(rt.TemplateError, "exited 42") as failure:
            rt.run([sys.executable, "-I", "-c",
                    "import sys; print('upstream stdout'); print('upstream stderr',file=sys.stderr); sys.exit(42)"], self.root)
        self.assertEqual(failure.exception.code, "TEMPLATE.UPSTREAM_FAILED")
        self.assertEqual(failure.exception.cause["exit_status"], 42)
        self.assertEqual(failure.exception.cause["stdout"], "upstream stdout\n")
        self.assertEqual(failure.exception.cause["stderr"], "upstream stderr\n")

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


class ErrorContractTest(GenerationFixture):
    """CLI envelopes use real fixture validation, not a substitute schema."""

    def invoke(self, *args: str) -> tuple[int, dict]:
        stdout, stderr = io.StringIO(), io.StringIO()
        with redirect_stdout(stdout), redirect_stderr(stderr):
            status = rt.main(list(args))
        self.assertEqual(stderr.getvalue(), "")
        envelope = json.loads(stdout.getvalue())
        self.assertEqual(set(envelope), {"ok", "data", "error"})
        if status:
            self.assertFalse(envelope["ok"])
            self.assertIsNone(envelope["data"])
            self.assertEqual(set(envelope["error"]), {"code", "message", "cause", "recovery"})
            self.assertTrue(envelope["error"]["recovery"])
            self.assertIsInstance(envelope["error"]["cause"], dict)
        return status, envelope

    def test_python_minimum_and_capability_are_actionable_before_tools_or_output(self) -> None:
        for version in ((3, 10, 14), (3, 11, 0), (3, 11, 3)):
            with self.subTest(version=version), patch.object(rt.sys, "version_info", version), patch.object(rt, "run") as run:
                status, envelope = self.invoke("generate", "--output", str(self.output))
                self.assertEqual(status, 1)
                self.assertEqual(envelope["error"]["code"], "TEMPLATE.PYTHON_UNSUPPORTED")
                self.assertEqual(envelope["error"]["cause"]["minimum"], [3, 11, 4])
                run.assert_not_called()
                self.assertFalse(self.output.exists())
        with patch.object(rt.sys, "version_info", (3, 11, 4)):
            rt.require_python()
        with patch.object(rt.tarfile, "data_filter", None):
            self.assertEqual(self.invoke("check", str(self.root))[1]["error"]["code"],
                             "TEMPLATE.PYTHON_UNSUPPORTED")
        with patch.object(rt.tarfile.TarFile, "extractall", lambda self, path: None):
            with self.assertRaises(rt.TemplateError):
                rt.require_python()

    def test_success_usage_destination_mapping_and_missing_file_envelopes(self) -> None:
        empty_mapping = self.mapping([])
        status, result = self.invoke("diff", str(self.source), str(self.source), "--mapping", str(empty_mapping))
        self.assertEqual(status, 0)
        self.assertEqual(result, {"ok": True, "data": {"changed_paths": 0, "changes": []}, "error": None})
        # A consumer's real default mapping is unrelated to these synthetic
        # baselines. Explicit fixture mapping must isolate this success case,
        # while omission must still load and validate the consumer default.
        consumer = self.tree("consumer", {"runtime/adoption.toml":
            'version = 1\n[[paths]]\ngenerated = "crates/daemon/src/composition.rs"\n'
            'adopted = "crates/app/src/assembly.rs"\n'})
        before = rt.file_tree(consumer)
        with patch.object(rt, "ROOT", consumer):
            isolated_status, isolated = self.invoke("diff", str(self.source), str(self.source),
                                                    "--mapping", str(empty_mapping))
            self.assertEqual((isolated_status, isolated), (status, result))
            default_status, default = self.invoke("diff", str(self.source), str(self.source))
            self.assertEqual(default_status, 1)
            self.assertEqual(default["error"]["code"], "TEMPLATE.MAPPING_INVALID")
            self.assertIn("stale mapping", default["error"]["message"])
        self.assertEqual(rt.file_tree(consumer), before)
        self.assertEqual(self.invoke("generate")[1]["error"]["code"], "TEMPLATE.USAGE_INVALID")
        self.assertEqual(self.invoke("generate", "--output", str(self.source))[1]["error"]["code"],
                         "TEMPLATE.DESTINATION_INVALID")
        mapping = self.mapping([("missing", "app/missing")])
        self.assertEqual(self.invoke("diff", str(self.source), str(self.source), "--mapping", str(mapping))[1]["error"]["code"],
                         "TEMPLATE.MAPPING_INVALID")
        self.assertEqual(self.invoke("generate", "--pin", str(self.root / "absent.toml"),
                                     "--output", str(self.output))[1]["error"]["code"], "TEMPLATE.IO_ERROR")

    def test_real_schema_failure_has_answers_code_and_recovery(self) -> None:
        answers = json.loads(self.answers.read_text())
        self.answers.write_text(json.dumps({**answers, "mcp": "false"}))
        with patch.object(rt, "export_source", return_value=self.source), patch.object(rt, "run") as run:
            status, result = self.invoke("generate", "--answers", str(self.answers), "--output", str(self.output))
            run.assert_not_called()
        self.assertEqual(status, 1)
        self.assertEqual(result["error"]["code"], "TEMPLATE.ANSWERS_INVALID")
        self.assertIn("wizard schema", result["error"]["message"])
        self.assertIn("Correct", result["error"]["recovery"])

    def test_missing_tool_and_upstream_failure_preserve_structured_cause(self) -> None:
        with self.assertRaises(rt.TemplateError) as missing:
            rt.run([str(self.root / "nonexistent-tool")], self.root)
        self.assertEqual(missing.exception.code, "TEMPLATE.TOOL_MISSING")
        failure = rt.TemplateError("tool exited 42", code="UPSTREAM_FAILED",
                                   cause={"exit_status": 42, "stdout": "upstream stdout", "stderr": "upstream stderr"})
        with patch.object(rt, "check_baseline", side_effect=failure):
            status, result = self.invoke("check", str(self.root))
        self.assertEqual(status, 1)
        self.assertEqual(result["error"]["cause"], failure.cause)
        self.assertEqual(result["error"]["code"], "TEMPLATE.UPSTREAM_FAILED")


class ProcessDeadlineTest(FixtureTest):
    def test_real_child_exceeds_deadline_and_is_reaped(self) -> None:
        with self.assertRaises(rt.TemplateError) as failure:
            rt.run([sys.executable, "-I", "-c", "import threading; threading.Event().wait()"],
                   self.root, timeout=0.01)
        self.assertEqual(failure.exception.code, "TEMPLATE.UPSTREAM_TIMEOUT")
        self.assertIsInstance(failure.exception.cause["exit_status"], int)
        self.assertEqual(failure.exception.cause["timeout_seconds"], 0.01)

    def test_timeout_after_descendant_ready_terminates_tree_and_preserves_diagnostics(self) -> None:
        self.descendant_timeout_proof(parent_exits=False)

    def test_timeout_terminates_descendant_after_direct_child_exits(self) -> None:
        self.descendant_timeout_proof(parent_exits=True)

    def descendant_timeout_proof(self, *, parent_exits: bool) -> None:
        lock = self.root / "descendant.lock"
        lock.write_bytes(b"x")
        before = rt.file_tree(self.root)
        lock_code = ("import msvcrt; msvcrt.locking(handle.fileno(),msvcrt.LK_LOCK,1)" if os.name == "nt"
                     else "import fcntl; fcntl.flock(handle,fcntl.LOCK_EX)")
        descendant = ("import threading,sys; " + f"handle=open({str(lock)!r},'r+b'); " + lock_code + "; "
                      "print('descendant diagnostic',file=sys.stderr,flush=True); "
                      "print('descendant ready',flush=True); threading.Event().wait()")
        parent = ("import subprocess,sys,threading; print('parent diagnostic',flush=True); "
                  f"subprocess.Popen([sys.executable,'-I','-c',{descendant!r}]); "
                  + ("sys.exit(0)" if parent_exits else "threading.Event().wait()"))
        real_popen = rt.subprocess.Popen
        processes, readers = [], []

        def controlled_popen(*args, **kwargs):
            process = real_popen(*args, **kwargs)
            processes.append(process)
            communicate = process.communicate
            first = True
            consumed = []

            def controlled_communicate(*, input=None, timeout=None):
                nonlocal first
                if first:
                    first = False
                    if input is not None:
                        process.stdin.write(input)
                        process.stdin.flush()
                    ready = queue.Queue()

                    def read_ready():
                        while True:
                            line = process.stdout.readline()
                            consumed.append(line)
                            if not line or line == "descendant ready\n":
                                ready.put(line)
                                return

                    reader = threading.Thread(target=read_ready, daemon=True)
                    readers.append(reader)
                    reader.start()
                    self.assertEqual(ready.get(timeout=5), "descendant ready\n")
                    reader.join(timeout=5)
                    self.assertFalse(reader.is_alive())
                    if parent_exits:
                        self.assertEqual(process.wait(timeout=5), 0)
                    self.assertEqual(timeout, 0.25)
                    raise subprocess.TimeoutExpired(args[0], timeout)
                stdout, stderr = communicate(timeout=timeout)
                return "".join(consumed) + stdout, stderr

            process.communicate = controlled_communicate
            return process

        with patch.object(rt.subprocess, "Popen", side_effect=controlled_popen):
            with self.assertRaises(rt.TemplateError) as failure:
                rt.run([sys.executable, "-I", "-c", parent], self.root, timeout=0.25)
        self.assertEqual(failure.exception.code, "TEMPLATE.UPSTREAM_TIMEOUT")
        self.assertIn("parent diagnostic", failure.exception.cause["stdout"])
        self.assertIn("descendant diagnostic", failure.exception.cause["stderr"])
        # The ready descendant holds this file lock forever and inherits the
        # output pipes. Acquiring its lock and EOF from communicate prove it
        # terminated; this covers inherited Windows Jobs as well as POSIX groups.
        with lock.open("r+b") as handle:
            if os.name == "nt":
                import msvcrt
                msvcrt.locking(handle.fileno(), msvcrt.LK_NBLCK, 1)
                msvcrt.locking(handle.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                import fcntl
                fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
                fcntl.flock(handle, fcntl.LOCK_UN)
        self.assertIsNotNone(processes[0].returncode)
        self.assertEqual(rt.file_tree(self.root), before)


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
