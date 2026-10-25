"""Tests for the pure version selection in prerelease_tag.py.

Run: python3 -m unittest discover -s scripts -p 'test_prerelease_tag.py'
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from prerelease_tag import bump_workspace_toml, patch_bump, published_versions, select_version  # noqa: E402

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


if __name__ == "__main__":
    unittest.main()
