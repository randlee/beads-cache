#!/usr/bin/env python3
"""Regenerate pristine framework templates in scratch space and report upgrade deltas.

No application files are updated. See docs/development/runtime-updates.md.
Python 3.11+ is required; generation also needs jsonschema, git, cargo-generate
and sc-compose. Comparisons and their fixture tests need only the standard library.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parent.parent


class TemplateError(Exception):
    """An invalid upkeep input or a failed upstream generation step."""


def run(argv: list[str], cwd: Path) -> str:
    """Run with closed stdin; keep upstream output in any failure diagnostic."""
    result = subprocess.run(argv, cwd=cwd, stdin=subprocess.DEVNULL,
                            capture_output=True, text=True, check=False)
    if result.returncode:
        raise TemplateError(
            f"{argv[0]} exited {result.returncode}: {' '.join(argv)}\n"
            f"{result.stdout}{result.stderr}"
        )
    return result.stdout.strip()


def load_pin(path: Path) -> dict:
    pin = tomllib.loads(path.read_text(encoding="utf-8"))
    fields = {"version", "repository", "tag", "commit", "cargo_generate", "sc_compose"}
    if set(pin) != fields or type(pin["version"]) is not int or pin["version"] != 1:
        raise TemplateError("pin must have version 1 and the documented exact fields")
    if any(not isinstance(pin[key], str) or not pin[key] for key in fields - {"version"}):
        raise TemplateError("pin fields must be nonempty strings")
    if not re.fullmatch(r"[0-9a-f]{40}", pin["commit"]):
        raise TemplateError("pin commit must be a full lowercase 40-character SHA")
    if pin["tag"].startswith("-"):
        raise TemplateError("invalid source tag")
    return pin


def export_source(pin: dict, source: Path | None, scratch: Path) -> Path:
    """Export Git objects at the verified tag; never read a checkout's template."""
    if source is None:
        source = scratch / "repository"
        run(["git", "clone", "--no-checkout", "--branch", pin["tag"], "--",
             pin["repository"], str(source)], scratch)
    else:
        source = source.resolve(strict=True)
    actual = run(["git", "rev-parse", "--verify",
                  f"refs/tags/{pin['tag']}^{{commit}}"], source)
    if actual != pin["commit"]:
        raise TemplateError(f"tag {pin['tag']} resolves to {actual}, expected {pin['commit']}")
    archive = scratch / "source.tar"
    run(["git", "archive", "--format=tar", f"--output={archive}", pin["commit"],
         "template", "wizard/answers.schema.json"], source)
    exported = scratch / "source"
    exported.mkdir()
    with tarfile.open(archive) as bundle:
        # Reject links/special files instead of letting an archive escape scratch.
        for member in bundle.getmembers():
            path = PurePosixPath(member.name)
            if path.is_absolute() or ".." in path.parts or not (member.isfile() or member.isdir()):
                raise TemplateError(f"unsafe source archive member: {member.name}")
        bundle.extractall(exported, filter="data")
    return exported


def validate_answers(source: Path, path: Path) -> dict:
    try:
        from jsonschema import Draft202012Validator
        from jsonschema.exceptions import SchemaError, ValidationError
    except ImportError as error:
        raise TemplateError("generation requires the jsonschema package (Draft 2020-12)") from error
    schema = json.loads((source / "wizard/answers.schema.json").read_text(encoding="utf-8"))
    answers = json.loads(path.read_text(encoding="utf-8"))
    try:
        Draft202012Validator.check_schema(schema)
        Draft202012Validator(schema).validate(answers)
    except (ValidationError, SchemaError) as error:
        raise TemplateError(f"answers fail tagged wizard schema: {error.message}") from error
    if answers.get("project_name") != "bd-cache":
        raise TemplateError("upkeep baselines require project_name bd-cache")
    # Explicit recorded answers, rather than mutable defaults, define this baseline.
    if set(answers) != set(schema["properties"]):
        raise TemplateError("answers must explicitly supply every tagged wizard property")
    return answers


def require_tool(argv: list[str], version: str, cwd: Path) -> None:
    output = run([*argv, "--version"], cwd)
    if version not in output.split():
        raise TemplateError(f"{' '.join(argv)} must be {version}; found {output}")


def file_tree(root: Path) -> dict[str, str]:
    """Hash every regular file, including dotfiles; refuse links and special files."""
    if not root.is_dir() or root.is_symlink():
        raise TemplateError(f"baseline is not a regular directory: {root}")
    files = {}
    for path in sorted(root.rglob("*")):
        if path.is_symlink() or not (path.is_dir() or path.is_file()):
            raise TemplateError(f"baseline contains a link or special file: {path}")
        if path.is_file():
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            files[path.relative_to(root).as_posix()] = f"{digest}:{path.stat().st_mode & 0o111}"
    return files


def check_baseline(root: Path) -> dict:
    """Reject development patches and checkout sources in rendered dependencies."""
    files = file_tree(root)
    for name in ("Cargo.toml", "AGENTS.md", "CLAUDE.md"):
        if name not in files:
            raise TemplateError(f"incomplete generated baseline: missing {name}")
    if any(name in files for name in ("AGENTS.md.j2", "CLAUDE.md.j2")):
        raise TemplateError("agent documents were not rendered")
    manifests = [root / name for name in files if PurePosixPath(name).name == "Cargo.toml"]
    for path in manifests:
        manifest = tomllib.loads(path.read_text(encoding="utf-8"))
        if "patch" in manifest or "replace" in manifest:
            raise TemplateError(f"baseline contains a dependency patch/replace: {path}")

        def check_dependencies(table: dict) -> None:
            for key, value in table.items():
                if key in ("dependencies", "dev-dependencies", "build-dependencies") and isinstance(value, dict):
                    for name, dependency in value.items():
                        package = dependency.get("package", name) if isinstance(dependency, dict) else name
                        if package.startswith("sc-") and isinstance(dependency, dict):
                            if any(source in dependency for source in ("path", "git")):
                                raise TemplateError(f"framework dependency is not registry-sourced: {path}: {name}")
                elif isinstance(value, dict):
                    check_dependencies(value)

        check_dependencies(manifest)
    return {"files": len(files), "registry_dependencies": True, "agent_documents": True}


def scratch_destination(path: Path) -> Path:
    # Resolve parents to catch symlinks into the application before any child runs.
    if os.path.lexists(path):
        raise TemplateError(f"destination already exists: {path}")
    path = path.resolve()
    roots = {Path(tempfile.gettempdir()).resolve(), Path("/tmp").resolve()}
    if not any(path.is_relative_to(root) for root in roots) or path.is_relative_to(ROOT):
        raise TemplateError("destination must be outside the application in temporary storage")
    if not path.parent.is_dir():
        raise TemplateError(f"destination parent must exist: {path.parent}")
    return path


def generate(pin_path: Path, answers_path: Path, destination: Path,
             source: Path | None = None) -> dict:
    destination = scratch_destination(destination)
    pin = load_pin(pin_path)
    with tempfile.TemporaryDirectory(prefix="runtime-template-", dir=destination.parent) as temporary:
        scratch = Path(temporary)
        upstream = export_source(pin, source, scratch)
        answers = validate_answers(upstream, answers_path)
        require_tool(["cargo", "generate"], pin["cargo_generate"], scratch)
        compose = os.environ.get("SC_COMPOSE", "sc-compose")
        require_tool([compose], pin["sc_compose"], scratch)
        values = scratch / "values.toml"
        values.write_text("[values]\n" + "".join(
            f"{key} = {json.dumps(value)}\n" for key, value in answers.items()
            if key != "project_name"
        ), encoding="utf-8")
        variables = scratch / "answers.json"
        variables.write_text(json.dumps(answers) + "\n", encoding="utf-8")
        rendered = scratch / "rendered"
        # Upstream 0.25.1 needs an existing empty directory with --init. Only
        # this owned scratch path is created; the caller output stays absent.
        rendered.mkdir()
        run(["cargo", "generate", "--path", str(upstream / "template"),
             "--template-values-file", str(values), "--name", "bd-cache",
             "--destination", str(rendered), "--init", "--silent", "--allow-commands",
             "--force", "--no-workspace"], scratch)
        run(["cargo", "fmt", "--all"], rendered)
        for name in ("AGENTS.md", "CLAUDE.md"):
            template = rendered / f"{name}.j2"
            run([compose, "render", "--mode", "file", "--file", str(template),
                 "--root", str(rendered), "--var-file", str(variables),
                 "--output", str(rendered / name), "--unknown-var-mode", "ignore"], rendered)
            template.unlink()
        checked = check_baseline(rendered)
        # mkdir is exclusive: a destination created while rendering is also refused.
        destination.mkdir()
        try:
            shutil.copytree(rendered, destination, dirs_exist_ok=True)
        except BaseException:
            shutil.rmtree(destination)
            raise
    return {"destination": str(destination), "pin": pin, "answers": answers, **checked}


def mapping_path(value: object) -> str:
    if not isinstance(value, str) or not value or "\\" in value or ":" in value:
        raise TemplateError(f"invalid mapping path: {value!r}")
    path = PurePosixPath(value)
    if path.is_absolute() or any(part in ("", ".", "..") for part in value.split("/")):
        raise TemplateError(f"mapping must name one relative file: {value!r}")
    if any(char in value for char in "*?[]"):
        raise TemplateError(f"mapping globs are unsupported: {value!r}")
    return value


def load_mapping(path: Path | None, available: set[str]) -> dict[str, str]:
    if path is None:
        return {}
    data = tomllib.loads(path.read_text(encoding="utf-8"))
    if set(data) != {"version", "paths"} or type(data["version"]) is not int or data["version"] != 1:
        raise TemplateError("adoption mapping requires version 1 and paths")
    if not isinstance(data["paths"], list):
        raise TemplateError("adoption paths must be an array of tables")
    mapping = {}
    adopted = set()
    for row in data["paths"]:
        if not isinstance(row, dict) or set(row) != {"generated", "adopted"}:
            raise TemplateError("each adoption row requires generated and adopted only")
        generated, destination = mapping_path(row["generated"]), mapping_path(row["adopted"])
        if generated in mapping or destination in adopted:
            raise TemplateError("duplicate generated or adopted path in mapping")
        if generated not in available:
            raise TemplateError(f"stale mapping: {generated} is absent from both baselines")
        mapping[generated] = destination
        adopted.add(destination)
    return mapping


def compare(old: Path, new: Path, mapping: Path | None = None) -> dict:
    before, after = file_tree(old), file_tree(new)
    available = before.keys() | after.keys()
    adopted = load_mapping(mapping, available)
    changes = []
    for path in sorted(available):
        if before.get(path) == after.get(path):
            continue
        status = "added" if path not in before else "deleted" if path not in after else "modified"
        changes.append({"generated": path, "change": status, "adopted": adopted.get(path),
                        "ownership": "adopted" if path in adopted else "unadopted"})
    return {"changes": changes, "changed_paths": len(changes)}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    renderer = commands.add_parser("generate", help="render a pinned pristine scratch baseline")
    renderer.add_argument("--pin", type=Path, default=ROOT / "runtime/pin.toml")
    renderer.add_argument("--answers", type=Path, default=ROOT / "runtime/answers.json")
    renderer.add_argument("--source", type=Path, help="local Git object source; checkout changes are ignored")
    renderer.add_argument("--output", type=Path, required=True, help="absent path in temporary storage")
    diff = commands.add_parser("diff", help="report every old/new pristine file change as JSON")
    diff.add_argument("old", type=Path)
    diff.add_argument("new", type=Path)
    adoption = ROOT / "runtime/adoption.toml"
    diff.add_argument("--mapping", type=Path, default=adoption if adoption.exists() else None,
                      help="version-1 mapping; defaults to runtime/adoption.toml when present")
    check = commands.add_parser("check", help="reject incomplete or checkout-patched baselines")
    check.add_argument("baseline", type=Path)
    args = parser.parse_args(argv)
    try:
        if args.command == "generate":
            result = generate(args.pin, args.answers, args.output, args.source)
        elif args.command == "diff":
            result = compare(args.old, args.new, args.mapping)
        else:
            result = check_baseline(args.baseline)
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0
    except (TemplateError, OSError, ValueError, tarfile.TarError) as error:
        print(f"runtime-templates: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
