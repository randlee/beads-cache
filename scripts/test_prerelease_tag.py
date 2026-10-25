"""Tests for the pure version selection in prerelease_tag.py.

Run: python3 -m unittest discover -s scripts -p 'test_prerelease_tag.py'
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import prerelease_tag  # noqa: E402
from prerelease_tag import bump_workspace_toml, patch_bump, published_versions, select_version  # noqa: E402

REPO = Path(__file__).resolve().parent.parent

PREFIX = "prerelease/v"


class SelectVersionTest(unittest.TestCase):
    def test_unpublished_workspace_version_is_used(self) -> None:
        self.assertEqual(select_version("0.1.0", set()), "0.1.0")

    def test_published_prerelease_forces_patch_bump(self) -> None:
        published = published_versions(["prerelease/v0.1.0"], PREFIX)
        self.assertEqual(select_version("0.1.0", published), "0.1.1")

    def test_published_release_forces_patch_bump(self) -> None:
        published = published_versions(["v0.1.0"], PREFIX)
        self.assertEqual(select_version("0.1.0", published), "0.1.1")

    def test_bumps_past_every_published_patch(self) -> None:
        published = published_versions(["v0.1.0", "prerelease/v0.1.1", "prerelease/v0.1.2"], PREFIX)
        self.assertEqual(select_version("0.1.0", published), "0.1.3")

    def test_unrelated_and_malformed_tags_are_ignored(self) -> None:
        tags = ["release-candidate-v0.1.0", "prerelease/v0.1", "v0.1.0-rc1", "other"]
        self.assertEqual(published_versions(tags, PREFIX), set())
        self.assertEqual(select_version("0.1.0", published_versions(tags, PREFIX)), "0.1.0")

    def test_rejects_non_stable_workspace_version(self) -> None:
        with self.assertRaises(ValueError):
            select_version("0.1.0-alpha", set())
        with self.assertRaises(ValueError):
            patch_bump("1.2")


class BumpWorkspaceTomlTest(unittest.TestCase):
    def test_bumps_package_and_path_dependency_versions_only(self) -> None:
        text = (
            '[workspace.package]\nversion = "0.1.0"\n\n'
            "[workspace.dependencies]\n"
            'a = { path = "crates/a", version = "0.1.0" }\n'
            'serde = { version = "0.1.0" }\n'
        )
        bumped = bump_workspace_toml(text, "0.1.0", "0.1.1")
        self.assertIn('[workspace.package]\nversion = "0.1.1"', bumped)
        self.assertIn('a = { path = "crates/a", version = "0.1.1" }', bumped)
        self.assertIn('serde = { version = "0.1.0" }', bumped)

    def test_missing_workspace_version_fails(self) -> None:
        with self.assertRaises(SystemExit):
            bump_workspace_toml('[package]\nversion = "0.1.0"\n', "0.1.0", "0.1.1")


def sh(cwd: Path, *args: str) -> str:
    return subprocess.run(list(args), cwd=cwd, text=True, capture_output=True, check=True).stdout


def fixture_repo(root: Path) -> Path:
    """A committed, clean repository carrying the kit scripts and the real manifest."""
    repo = root / "repo"
    (repo / ".github").mkdir(parents=True)
    shutil.copytree(REPO / ".github/scripts", repo / ".github/scripts", ignore=shutil.ignore_patterns("tests", "__pycache__"))
    (repo / "release").mkdir()
    shutil.copy2(REPO / prerelease_tag.MANIFEST, repo / prerelease_tag.MANIFEST)
    # The repo ignores bytecode; without this the kit scripts' __pycache__ makes the fixture dirty.
    shutil.copy2(REPO / ".gitignore", repo / ".gitignore")
    (repo / "Cargo.toml").write_text('[workspace]\nmembers = []\n\n[workspace.package]\nversion = "0.1.0"\n')
    (repo / "Cargo.lock").write_text("# lock\n")
    manifest = tomllib.loads((REPO / prerelease_tag.MANIFEST).read_text(encoding="utf-8"))
    for package in manifest["python_packages"]:
        pyproject = repo / package["manifest"]
        pyproject.parent.mkdir(parents=True, exist_ok=True)
        pyproject.write_text(f'[project]\nname = "{package["package"]}"\nversion = "0.1.0"\n')
    sh(repo, "git", "init", "-q", "-b", "feature")
    sh(repo, "git", "add", "-A")
    sh(repo, "git", "-c", "user.name=t", "-c", "user.email=t@example.invalid", "commit", "-q", "-m", "fixture")
    return repo


GIT_IDENTITY = {
    "GIT_AUTHOR_NAME": "t",
    "GIT_AUTHOR_EMAIL": "t@example.invalid",
    "GIT_COMMITTER_NAME": "t",
    "GIT_COMMITTER_EMAIL": "t@example.invalid",
}


class PartialBumpRollbackTest(unittest.TestCase):
    def test_a_bump_failing_at_cargo_update_leaves_the_tree_clean(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repo = fixture_repo(root)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            cargo = fake_bin / "cargo"
            cargo.write_text("#!/bin/sh\necho 'fake cargo fails' >&2\nexit 1\n")
            cargo.chmod(0o755)
            manifest = prerelease_tag.load_manifest(repo)
            env = {"PATH": f"{fake_bin}{os.pathsep}{os.environ['PATH']}", **GIT_IDENTITY}
            with mock.patch.dict(os.environ, env), self.assertRaises(SystemExit) as raised:
                prerelease_tag.commit_bump(repo, manifest, "Cargo.toml", "0.1.0", "0.1.1")
            self.assertIn("cargo update", str(raised.exception))
            self.assertEqual(sh(repo, "git", "status", "--porcelain"), "")
            self.assertEqual(sh(repo, "git", "rev-list", "--count", "HEAD").strip(), "1")


class RejectedTagPushTest(unittest.TestCase):
    def test_a_rejected_tag_push_deletes_the_local_tag(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repo = fixture_repo(root)
            origin = root / "origin.git"
            sh(root, "git", "init", "-q", "--bare", str(origin))
            hook = origin / "hooks/pre-receive"
            hook.write_text(
                "#!/bin/sh\nwhile read old new ref; do\n"
                '  case "$ref" in refs/tags/*) echo "tags rejected" >&2; exit 1;; esac\n'
                "done\nexit 0\n"
            )
            hook.chmod(0o755)
            sh(repo, "git", "remote", "add", "origin", str(origin))
            cwd = os.getcwd()
            os.chdir(repo)
            try:
                with mock.patch.dict(os.environ, GIT_IDENTITY), self.assertRaises(SystemExit):
                    prerelease_tag.main([])
            finally:
                os.chdir(cwd)
            self.assertEqual(sh(repo, "git", "tag", "--list"), "")
            self.assertEqual(sh(repo, "git", "ls-remote", "--tags", "origin"), "")
            self.assertEqual(
                prerelease_tag.select_version(
                    "0.1.0", published_versions(prerelease_tag.known_tags(repo), PREFIX)
                ),
                "0.1.0",
            )


if __name__ == "__main__":
    unittest.main()
