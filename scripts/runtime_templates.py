#!/usr/bin/env python3
"""Regenerate pristine framework templates in scratch space and report upgrade deltas.

No application files are updated. See docs/development/runtime-updates.md.
Python 3.11.4+ is required; generation also needs jsonschema, git, cargo-generate
and sc-compose. Comparisons need only the standard library; generation fixtures
also require jsonschema.
"""

from __future__ import annotations

import argparse
import hashlib
import inspect
import json
import math
import os
import re
import shutil
import signal
import subprocess
import sys
import tarfile
import tempfile
try:
    import tomllib
except ModuleNotFoundError:  # Report the minimum version before reading inputs.
    tomllib = None
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parent.parent
MIN_PYTHON = (3, 11, 4)
COMMAND_TIMEOUT = 120.0
REAP_TIMEOUT = 5.0
RECOVERY = {
    "PYTHON_UNSUPPORTED": "Use Python 3.11.4+ with tarfile data-filter support, then rerun the command.",
    "INPUT_INVALID": "Correct the named JSON/TOML input and rerun; see docs/development/runtime-updates.md.",
    "PIN_INVALID": "Record a version-1 pin with an immutable tag, full SHA and exact tool versions.",
    "PIN_MISMATCH": "Verify the intended upstream tag and SHA; correct the pin or Git source before rendering.",
    "ANSWERS_INVALID": "Correct the explicit answers to match the pinned wizard schema and project_name bd-cache.",
    "TOOL_MISSING": "Install the named dependency; use jsonschema>=4.18,<5 and the tool versions in runtime/pin.toml.",
    "TOOL_VERSION": "Install the exact version in runtime/pin.toml or select the intended immutable upgrade pin.",
    "DESTINATION_INVALID": "Choose an absent output outside the application in an existing temporary parent directory.",
    "MAPPING_INVALID": "Correct runtime/adoption.toml using exact unique relative file paths present in either baseline.",
    "BASELINE_INVALID": "Regenerate a pristine baseline from the verified pin without checkout patches or source overrides.",
    "UPSTREAM_FAILED": "Resolve the captured upstream command failure, then rerun into an absent output.",
    "UPSTREAM_TIMEOUT": "Resolve the stalled tool or cache lock using the captured diagnostics, then rerun; no retry occurred.",
    "PROCESS_CLEANUP_FAILED": "Check the captured process IDs and OS termination permissions before rerunning.",
    "IO_ERROR": "Correct the named path or filesystem permissions, then rerun into an absent output.",
    "USAGE_INVALID": "Run the command with --help and supply its required arguments.",
}


class TemplateError(Exception):
    """Stable upkeep error with a machine-readable cause and concrete recovery."""

    def __init__(self, message: str, *, code: str = "INPUT_INVALID", cause: dict | None = None):
        self.code = f"TEMPLATE.{code}"
        self.message = message
        self.cause = cause if cause is not None else {"detail": message}
        self.recovery = RECOVERY[code]
        super().__init__(message + ": " + json.dumps(self.cause, sort_keys=True))

    def as_dict(self) -> dict:
        return {"code": self.code, "message": self.message,
                "cause": self.cause, "recovery": self.recovery}


def require_python() -> None:
    if (sys.version_info[:3] < MIN_PYTHON or tomllib is None
            or "filter" not in inspect.signature(tarfile.TarFile.extractall).parameters
            or not callable(getattr(tarfile, "data_filter", None))):
        raise TemplateError("Python lacks the supported version or safe tar extraction capability",
                            code="PYTHON_UNSUPPORTED", cause={"actual": list(sys.version_info[:3]),
                                                              "minimum": list(MIN_PYTHON)})


def windows_job(process: subprocess.Popen):
    """Assign a blocked launcher to a kill-on-close Job before it starts tools.

    The stdin handshake in run() prevents the target (and any descendant) from
    starting before Job assignment. Children inherit the Job on Windows.
    """
    import ctypes
    from ctypes import wintypes

    class BasicLimits(ctypes.Structure):
        _fields_ = [("process_time", ctypes.c_int64), ("job_time", ctypes.c_int64),
                    ("flags", wintypes.DWORD), ("minimum_ws", ctypes.c_size_t),
                    ("maximum_ws", ctypes.c_size_t), ("active_limit", wintypes.DWORD),
                    ("affinity", ctypes.c_size_t), ("priority", wintypes.DWORD),
                    ("scheduling", wintypes.DWORD)]

    class IoCounters(ctypes.Structure):
        _fields_ = [(name, ctypes.c_uint64) for name in
                    ("read_ops", "write_ops", "other_ops", "read_bytes", "write_bytes", "other_bytes")]

    class ExtendedLimits(ctypes.Structure):
        _fields_ = [("basic", BasicLimits), ("io", IoCounters),
                    ("process_memory", ctypes.c_size_t), ("job_memory", ctypes.c_size_t),
                    ("peak_process_memory", ctypes.c_size_t), ("peak_job_memory", ctypes.c_size_t)]

    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.CreateJobObjectW.argtypes = [ctypes.c_void_p, wintypes.LPCWSTR]
    kernel.CreateJobObjectW.restype = wintypes.HANDLE
    kernel.SetInformationJobObject.argtypes = [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD]
    kernel.SetInformationJobObject.restype = wintypes.BOOL
    kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel.OpenProcess.restype = wintypes.HANDLE
    kernel.AssignProcessToJobObject.argtypes = [wintypes.HANDLE, wintypes.HANDLE]
    kernel.AssignProcessToJobObject.restype = wintypes.BOOL
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel.CloseHandle.restype = wintypes.BOOL
    job = kernel.CreateJobObjectW(None, None)
    if not job:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        limits = ExtendedLimits()
        limits.basic.flags = 0x2000  # JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        if not kernel.SetInformationJobObject(job, 9, ctypes.byref(limits), ctypes.sizeof(limits)):
            raise ctypes.WinError(ctypes.get_last_error())
        handle = kernel.OpenProcess(0x0100 | 0x0001, False, process.pid)  # SET_QUOTA | TERMINATE
        if not handle:
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            if not kernel.AssignProcessToJobObject(job, handle):
                raise ctypes.WinError(ctypes.get_last_error())
        finally:
            kernel.CloseHandle(handle)
    except BaseException:
        kernel.CloseHandle(job)
        raise
    return lambda: kernel.CloseHandle(job)


def run(argv: list[str], cwd: Path, *, timeout: float = COMMAND_TIMEOUT) -> str:
    """Bound each child, kill its process tree, reap it and retain diagnostics."""
    if not math.isfinite(timeout) or timeout <= 0:
        raise TemplateError("command deadline must be positive and finite")
    context = {"command": argv, "cwd": str(cwd), "timeout_seconds": timeout}
    executable = str(cwd / argv[0]) if os.path.dirname(argv[0]) and not os.path.isabs(argv[0]) else argv[0]
    if shutil.which(executable) is None:
        raise TemplateError(f"could not find executable {argv[0]}", code="TOOL_MISSING", cause=context)
    launcher = ("import subprocess,sys; "
                "token=sys.stdin.buffer.read(1); "
                "sys.exit(subprocess.call(sys.argv[1:],stdin=subprocess.DEVNULL) if token==b'x' else 125)")
    try:
        process = subprocess.Popen([sys.executable, "-I", "-c", launcher, *argv] if os.name == "nt" else argv,
                                   cwd=cwd, stdin=subprocess.PIPE if os.name == "nt" else subprocess.DEVNULL,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                                   encoding="utf-8", errors="replace", start_new_session=os.name != "nt")
    except OSError as error:
        raise TemplateError(f"could not start {argv[0]}", code="TOOL_MISSING",
                            cause={**context, "detail": str(error)}) from error
    job_close = None

    def stop_tree() -> None:
        nonlocal job_close
        if os.name == "nt":
            if job_close is not None:
                job_close()
                job_close = None
            elif process.poll() is None:
                process.kill()  # Launcher has not passed its handshake.
        else:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass

    try:
        if os.name == "nt":
            try:
                job_close = windows_job(process)
            except OSError as error:
                raise TemplateError("could not contain tool process tree", code="PROCESS_CLEANUP_FAILED",
                                    cause={**context, "pid": process.pid, "detail": str(error)}) from error
        try:
            stdout, stderr = process.communicate(input="x" if os.name == "nt" else None, timeout=timeout)
        except subprocess.TimeoutExpired as error:
            stop_tree()
            try:
                stdout, stderr = process.communicate(timeout=REAP_TIMEOUT)
            except subprocess.TimeoutExpired as cleanup:
                process.kill()
                process.stdout.close()
                process.stderr.close()
                process.wait(timeout=REAP_TIMEOUT)
                raise TemplateError("tool pipes remained open after process-tree termination",
                                    code="PROCESS_CLEANUP_FAILED", cause={**context, "pid": process.pid}) from cleanup
            raise TemplateError(f"{argv[0]} exceeded its {timeout:g}s deadline", code="UPSTREAM_TIMEOUT",
                                cause={**context, "exit_status": process.returncode,
                                       "stdout": stdout, "stderr": stderr}) from error
        if process.returncode:
            raise TemplateError(f"{argv[0]} exited {process.returncode}", code="UPSTREAM_FAILED",
                                cause={**context, "exit_status": process.returncode,
                                       "stdout": stdout, "stderr": stderr})
        return stdout.strip()
    finally:
        stop_tree()
        # Also reap a launcher if assignment failed before communicate().
        if process.poll() is None:
            process.kill()
            process.wait(timeout=REAP_TIMEOUT)
        for stream in (process.stdin, process.stdout, process.stderr):
            if stream is not None:
                stream.close()


def load_pin(path: Path) -> dict:
    pin = tomllib.loads(path.read_text(encoding="utf-8"))
    fields = {"version", "repository", "tag", "commit", "cargo_generate", "sc_compose"}
    if set(pin) != fields or type(pin["version"]) is not int or pin["version"] != 1:
        raise TemplateError("pin must have version 1 and the documented exact fields", code="PIN_INVALID")
    if any(not isinstance(pin[key], str) or not pin[key] for key in fields - {"version"}):
        raise TemplateError("pin fields must be nonempty strings", code="PIN_INVALID")
    if not re.fullmatch(r"[0-9a-f]{40}", pin["commit"]):
        raise TemplateError("pin commit must be a full lowercase 40-character SHA", code="PIN_INVALID")
    if pin["tag"].startswith("-"):
        raise TemplateError("invalid source tag", code="PIN_INVALID")
    return pin


def export_source(pin: dict, source: Path | None, scratch: Path) -> Path:
    """Export Git objects at the verified tag; never read a checkout's template."""
    require_python()
    if source is None:
        source = scratch / "repository"
        run(["git", "clone", "--no-checkout", "--branch", pin["tag"], "--",
             pin["repository"], str(source)], scratch)
    else:
        source = source.resolve(strict=True)
    actual = run(["git", "rev-parse", "--verify",
                  f"refs/tags/{pin['tag']}^{{commit}}"], source)
    if actual != pin["commit"]:
        raise TemplateError(f"tag {pin['tag']} resolves to {actual}, expected {pin['commit']}", code="PIN_MISMATCH",
                            cause={"tag": pin["tag"], "actual": actual, "expected": pin["commit"]})
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
                raise TemplateError(f"unsafe source archive member: {member.name}", code="BASELINE_INVALID")
        bundle.extractall(exported, filter="data")
    return exported


def validate_answers(source: Path, path: Path) -> dict:
    try:
        from jsonschema import Draft202012Validator
        from jsonschema.exceptions import SchemaError, ValidationError
    except ImportError as error:
        raise TemplateError("generation requires the jsonschema package (Draft 2020-12)", code="TOOL_MISSING") from error
    schema = json.loads((source / "wizard/answers.schema.json").read_text(encoding="utf-8"))
    answers = json.loads(path.read_text(encoding="utf-8"))
    try:
        Draft202012Validator.check_schema(schema)
        Draft202012Validator(schema).validate(answers)
    except (ValidationError, SchemaError) as error:
        raise TemplateError(f"answers fail tagged wizard schema: {error.message}", code="ANSWERS_INVALID") from error
    if answers.get("project_name") != "bd-cache":
        raise TemplateError("upkeep baselines require project_name bd-cache", code="ANSWERS_INVALID")
    # Explicit recorded answers, rather than mutable defaults, define this baseline.
    if set(answers) != set(schema["properties"]):
        raise TemplateError("answers must explicitly supply every tagged wizard property", code="ANSWERS_INVALID")
    return answers


def require_tool(argv: list[str], version: str, cwd: Path) -> None:
    output = run([*argv, "--version"], cwd)
    if version not in output.split():
        raise TemplateError(f"{' '.join(argv)} must be {version}; found {output}", code="TOOL_VERSION")


def file_tree(root: Path) -> dict[str, str]:
    """Hash every regular file, including dotfiles; refuse links and special files."""
    if not root.is_dir() or root.is_symlink():
        raise TemplateError(f"baseline is not a regular directory: {root}", code="BASELINE_INVALID")
    files = {}
    for path in sorted(root.rglob("*")):
        if path.is_symlink() or not (path.is_dir() or path.is_file()):
            raise TemplateError(f"baseline contains a link or special file: {path}", code="BASELINE_INVALID")
        if path.is_file():
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            files[path.relative_to(root).as_posix()] = f"{digest}:{path.stat().st_mode & 0o111}"
    return files


def check_baseline(root: Path) -> dict:
    """Reject development patches and checkout sources in rendered dependencies."""
    files = file_tree(root)
    for name in ("Cargo.toml", "AGENTS.md", "CLAUDE.md"):
        if name not in files:
            raise TemplateError(f"incomplete generated baseline: missing {name}", code="BASELINE_INVALID")
    if any(name in files for name in ("AGENTS.md.j2", "CLAUDE.md.j2")):
        raise TemplateError("agent documents were not rendered", code="BASELINE_INVALID")
    manifests = [root / name for name in files if PurePosixPath(name).name == "Cargo.toml"]
    for path in manifests:
        manifest = tomllib.loads(path.read_text(encoding="utf-8"))
        if "patch" in manifest or "replace" in manifest:
            raise TemplateError(f"baseline contains a dependency patch/replace: {path}", code="BASELINE_INVALID")

        def check_dependencies(table: dict) -> None:
            for key, value in table.items():
                if key in ("dependencies", "dev-dependencies", "build-dependencies") and isinstance(value, dict):
                    for name, dependency in value.items():
                        package = dependency.get("package", name) if isinstance(dependency, dict) else name
                        if package.startswith("sc-") and isinstance(dependency, dict):
                            if any(source in dependency for source in ("path", "git")):
                                raise TemplateError(f"framework dependency is not registry-sourced: {path}: {name}", code="BASELINE_INVALID")
                elif isinstance(value, dict):
                    check_dependencies(value)

        check_dependencies(manifest)
    return {"files": len(files), "registry_dependencies": True, "agent_documents": True}


def scratch_destination(path: Path) -> Path:
    # Resolve parents to catch symlinks into the application before any child runs.
    if os.path.lexists(path):
        raise TemplateError(f"destination already exists: {path}", code="DESTINATION_INVALID")
    path = path.resolve()
    roots = {Path(tempfile.gettempdir()).resolve(), Path("/tmp").resolve()}
    if not any(path.is_relative_to(root) for root in roots) or path.is_relative_to(ROOT):
        raise TemplateError("destination must be outside the application in temporary storage", code="DESTINATION_INVALID")
    if not path.parent.is_dir():
        raise TemplateError(f"destination parent must exist: {path.parent}", code="DESTINATION_INVALID")
    return path


def generate(pin_path: Path, answers_path: Path, destination: Path,
             source: Path | None = None) -> dict:
    require_python()
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
                 "--output", str(rendered / name), "--unknown-var-mode", "ignore"], scratch)
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
        raise TemplateError(f"invalid mapping path: {value!r}", code="MAPPING_INVALID")
    path = PurePosixPath(value)
    if path.is_absolute() or any(part in ("", ".", "..") for part in value.split("/")):
        raise TemplateError(f"mapping must name one relative file: {value!r}", code="MAPPING_INVALID")
    if any(char in value for char in "*?[]"):
        raise TemplateError(f"mapping globs are unsupported: {value!r}", code="MAPPING_INVALID")
    return value


def load_mapping(path: Path | None, available: set[str]) -> dict[str, str]:
    if path is None:
        return {}
    data = tomllib.loads(path.read_text(encoding="utf-8"))
    if set(data) != {"version", "paths"} or type(data["version"]) is not int or data["version"] != 1:
        raise TemplateError("adoption mapping requires version 1 and paths", code="MAPPING_INVALID")
    if not isinstance(data["paths"], list):
        raise TemplateError("adoption paths must be an array of tables", code="MAPPING_INVALID")
    mapping = {}
    adopted = set()
    for row in data["paths"]:
        if not isinstance(row, dict) or set(row) != {"generated", "adopted"}:
            raise TemplateError("each adoption row requires generated and adopted only", code="MAPPING_INVALID")
        generated, destination = mapping_path(row["generated"]), mapping_path(row["adopted"])
        if generated in mapping or destination in adopted:
            raise TemplateError("duplicate generated or adopted path in mapping", code="MAPPING_INVALID")
        if generated not in available:
            raise TemplateError(f"stale mapping: {generated} is absent from both baselines", code="MAPPING_INVALID")
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


class Parser(argparse.ArgumentParser):
    def error(self, message: str) -> None:
        raise TemplateError(message, code="USAGE_INVALID")


def main(argv: list[str] | None = None) -> int:
    parser = Parser(description=__doc__.splitlines()[0])
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
    try:
        require_python()
        args = parser.parse_args(argv)
        if args.command == "generate":
            result = generate(args.pin, args.answers, args.output, args.source)
        elif args.command == "diff":
            result = compare(args.old, args.new, args.mapping)
        else:
            result = check_baseline(args.baseline)
        print(json.dumps({"ok": True, "data": result, "error": None}, indent=2, sort_keys=True))
        return 0
    except TemplateError as error:
        failure = error
    except OSError as error:
        failure = TemplateError("filesystem operation failed", code="IO_ERROR",
                                cause={"detail": str(error), "path": str(error.filename)})
    except (ValueError, tarfile.TarError) as error:
        failure = TemplateError("could not parse upkeep input", cause={"detail": str(error)})
    except subprocess.TimeoutExpired as error:
        failure = TemplateError("could not reap the terminated child within the cleanup deadline",
                                code="PROCESS_CLEANUP_FAILED", cause={"command": error.cmd,
                                                                     "timeout_seconds": error.timeout})
    print(json.dumps({"ok": False, "data": None, "error": failure.as_dict()}, indent=2, sort_keys=True))
    return 1


if __name__ == "__main__":
    sys.exit(main())
