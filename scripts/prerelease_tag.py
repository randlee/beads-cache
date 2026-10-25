#!/usr/bin/env python3
"""Select, create and push the next GitHub prerelease tag for this branch.

This is the `[prerelease].tag_script` that sc-publish's `/prerelease --publish`
invokes (see `.claude/skills/prerelease/publish.md`). The kit does not choose
the version or create the tag; this script owns that policy:

* The workspace version is used only when it was never published, either as a
  `v<version>` release tag or as a `<tag_prefix><version>` prerelease tag
  (local or on origin). Otherwise the patch component is bumped until the
  version is unpublished, and the bump is committed so the tagged commit's
  workspace version equals the tag version.
* A real run pushes the branch, then creates and pushes the annotated tag, and
  prints exactly `created and pushed <tag> from <commit>` (the kit parses it).
* `--dry-run` prints the plan and changes nothing.
* It refuses an explicit version, a protected branch, a dirty tree and a
  detached HEAD.

Repository-specific values (tag prefix, protected branches, workspace
manifest, Python projects) come from `release/publish-artifacts.toml`.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import tomllib
from collections.abc import Iterable
from pathlib import Path

MANIFEST = Path("release/publish-artifacts.toml")
RELEASE_TAG_PREFIX = "v"
STABLE_VERSION = re.compile(r"^(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)$")


# --- pure version selection -------------------------------------------------


def patch_bump(version: str) -> str:
    match = STABLE_VERSION.fullmatch(version)
    if match is None:
        raise ValueError(f"version must be a stable X.Y.Z value: {version!r}")
    return f"{match['major']}.{match['minor']}.{int(match['patch']) + 1}"


def published_versions(tags: Iterable[str], prerelease_prefix: str) -> set[str]:
    """Return every X.Y.Z published as a release tag or a prerelease tag."""
    versions: set[str] = set()
    for tag in tags:
        for prefix in (prerelease_prefix, RELEASE_TAG_PREFIX):
            if tag.startswith(prefix):
                candidate = tag.removeprefix(prefix)
                if STABLE_VERSION.fullmatch(candidate):
                    versions.add(candidate)
    return versions


def select_version(workspace: str, published: set[str]) -> str:
    """Use the workspace version if never published, else patch-bump past it."""
    if STABLE_VERSION.fullmatch(workspace) is None:
        raise ValueError(f"workspace version must be a stable X.Y.Z value: {workspace!r}")
    version = workspace
    while version in published:
        version = patch_bump(version)
    return version


# --- repository access ------------------------------------------------------


def run(repo: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(list(args), cwd=repo, text=True, capture_output=True, check=False)
    if check and result.returncode != 0:
        detail = (result.stderr or result.stdout).strip()
        raise SystemExit(f"{' '.join(args)} failed: {detail}")
    return result


def git(repo: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return run(repo, "git", *args, check=check)


def release_artifacts(repo: Path, *args: str) -> None:
    run(repo, sys.executable, str(repo / ".github/scripts/release_artifacts.py"), *args)


def load_manifest(repo: Path) -> dict:
    path = repo / MANIFEST
    if not path.is_file():
        raise SystemExit(f"{MANIFEST} is missing; run the sc-publish install first")
    manifest = tomllib.loads(path.read_text(encoding="utf-8"))
    if not isinstance(manifest.get("prerelease"), dict):
        raise SystemExit(f"{MANIFEST} has no [prerelease] table")
    return manifest


def workspace_version(workspace_toml: Path) -> str:
    data = tomllib.loads(workspace_toml.read_text(encoding="utf-8"))
    version = data.get("workspace", {}).get("package", {}).get("version")
    if not isinstance(version, str) or STABLE_VERSION.fullmatch(version) is None:
        raise SystemExit(f"{workspace_toml}: [workspace.package].version must be a stable X.Y.Z value")
    return version


def current_branch(repo: Path, protected: list[str]) -> str:
    branch = git(repo, "branch", "--show-current").stdout.strip()
    if not branch:
        raise SystemExit("prerelease tag script refuses to run from a detached HEAD")
    if branch in protected:
        raise SystemExit(f"prerelease tag script refuses to run on protected branch {branch}")
    return branch


def require_clean_tree(repo: Path) -> None:
    if git(repo, "status", "--porcelain").stdout.strip():
        raise SystemExit("prerelease tag script requires a clean working tree")


def known_tags(repo: Path) -> set[str]:
    local = git(repo, "tag", "--list").stdout.split()
    remote = git(repo, "ls-remote", "--tags", "--refs", "origin").stdout.splitlines()
    names = {line.split("\t", 1)[1].removeprefix("refs/tags/") for line in remote if "\t" in line}
    return set(local) | names


# --- version bump -----------------------------------------------------------


def bump_workspace_toml(text: str, old: str, new: str) -> str:
    """Rewrite [workspace.package].version and path-dependency versions."""
    out: list[str] = []
    section = ""
    bumped = False
    for line in text.splitlines(keepends=True):
        stripped = line.strip()
        if stripped.startswith("[") and not stripped.startswith("[["):
            section = stripped.split("#", 1)[0].strip()
        if section == "[workspace.package]" and re.match(rf'^\s*version\s*=\s*"{re.escape(old)}"', line):
            line = line.replace(f'"{old}"', f'"{new}"', 1)
            bumped = True
        elif "path =" in line:
            line = line.replace(f'version = "{old}"', f'version = "{new}"')
        out.append(line)
    if not bumped:
        raise SystemExit(f"[workspace.package] version {old} not found")
    return "".join(out)


def apply_bump(repo: Path, manifest: dict, workspace_toml: str, old: str, new: str, touched: list[str]) -> None:
    """Write the bump into the working tree.

    Each path is appended to the caller-owned `touched` before it is written,
    so a failure at any step leaves the caller the full set to restore.
    """
    touched.append(workspace_toml)
    root = repo / workspace_toml
    root.write_text(bump_workspace_toml(root.read_text(encoding="utf-8"), old, new), encoding="utf-8")
    for package in manifest.get("python_packages", []):
        pyproject = package.get("manifest")
        if isinstance(pyproject, str) and pyproject.endswith("pyproject.toml"):
            touched.append(pyproject)
            release_artifacts(repo, "sync-python-version", "--workspace-toml", workspace_toml, "--pyproject", pyproject)
    if (repo / "Cargo.lock").is_file():
        touched.append("Cargo.lock")
        run(repo, "cargo", "update", "--workspace", "--offline")
    manifest_path = str(MANIFEST)
    release_artifacts(repo, "verify-version", "--manifest", manifest_path, "--workspace-toml", workspace_toml, "--version", new)
    release_artifacts(repo, "verify-version-lockstep", "--manifest", manifest_path, "--workspace-toml", workspace_toml)


def restore(repo: Path, paths: list[str]) -> None:
    """Return `paths` to HEAD (the tree was clean on entry, so this loses nothing)."""
    if paths:
        git(repo, "reset", "-q", "HEAD", "--", *paths, check=False)
        git(repo, "checkout", "--", *paths, check=False)


def commit_bump(repo: Path, manifest: dict, workspace_toml: str, old: str, new: str) -> None:
    touched: list[str] = []
    try:
        apply_bump(repo, manifest, workspace_toml, old, new, touched)
        git(repo, "add", "--", *touched)
        git(repo, "commit", "-m", f"chore(release): bump workspace version to {new}")
    except BaseException:
        restore(repo, touched)
        raise


def push_tag(repo: Path, tag: str, message: str) -> None:
    """Create and push `tag`; a failed push deletes the local tag so the version stays selectable."""
    git(repo, "tag", "-a", tag, "-m", message)
    pushed = False
    try:
        git(repo, "push", "origin", f"refs/tags/{tag}")
        pushed = True
    finally:
        if not pushed:
            git(repo, "tag", "-d", tag, check=False)


# --- entry point ------------------------------------------------------------


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--dry-run", action="store_true", help="print the plan and change nothing")
    parser.add_argument("version", nargs="?", help=argparse.SUPPRESS)
    args = parser.parse_args(argv)
    if args.version is not None:
        raise SystemExit("prerelease tag script refuses an explicit version; it selects the version itself")

    repo = Path(git(Path.cwd(), "rev-parse", "--show-toplevel").stdout.strip())
    manifest = load_manifest(repo)
    config = manifest["prerelease"]
    prefix = config["tag_prefix"]
    protected = list(config.get("protected_branches", []))
    workspace_toml = manifest.get("project", {}).get("workspace_toml", "Cargo.toml")

    branch = current_branch(repo, protected)
    require_clean_tree(repo)
    old = workspace_version(repo / workspace_toml)
    new = select_version(old, published_versions(known_tags(repo), prefix))
    tag = f"{prefix}{new}"

    if args.dry_run:
        print("prerelease tag dry run:")
        print(f"  current branch: {branch}")
        if new == old:
            print(f"  workspace version: {old} (never published; no bump)")
        else:
            print(f"  workspace version: {old} -> {new} (published versions skipped)")
            print(f"  would commit: chore(release): bump workspace version to {new}")
        print(f"  would push: origin {branch}")
        print(f"  would create and push tag: {tag}")
        return 0

    if new != old:
        commit_bump(repo, manifest, workspace_toml, old, new)
    commit = git(repo, "rev-parse", "HEAD").stdout.strip()
    git(repo, "push", "origin", f"HEAD:refs/heads/{branch}")
    push_tag(repo, tag, f"Prerelease {new}")
    print(f"created and pushed {tag} from {commit}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
