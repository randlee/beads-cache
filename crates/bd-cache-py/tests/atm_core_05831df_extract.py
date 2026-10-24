"""Extracts, with file:line, every `bd_cache.py` name the atm-bd scripts use at atm-core 05831df.

Run (needs `gh` authenticated):

    python3 atm_core_05831df_extract.py > atm_core_05831df_calls.json

It reads the scripts through the GitHub contents API at the pinned commit
(REF), parses each with `ast`, and records:

- `method`: a call or attribute use `<cache>.name` where the receiver is a
  `BdCache` (`cache`, `self.cache()`, `runtime.cache()`), for every name defined
  on `class BdCache` in bd_cache.py;
- `module`: `bd_cache.NAME` (the class, `CacheError`, `UNREACHABLE`, `__file__`);
- `error`: `.code`, `.detail`, `.fix`, `.extra`, `.as_json` on a caught `CacheError`;
- `cli`: `bd_cache.py <command>` command lines the scripts' messages and spawns name.

The commands used are in the output's `commands` key.
"""

import ast
import json
import re
import subprocess
import sys

REPO = "randlee/atm-core"
REF = "05831dfe866d389feed138add3c29cb93cd6c82b"
DIR = ".claude/skills/atm-bd-orchestration/scripts"
CLIENTS = ["sanity_task_close_hook.py", "close_fields_backfill.py", "close-vars.py", "bd_cache_cron.py"]
RECEIVERS = {"cache", "self.cache()", "runtime.cache()", "self.beads", "cache_factory(repo)"}
ERROR_NAMES = {"code", "detail", "fix", "extra", "as_json"}
ERROR_RECEIVERS = {"error", "cached_error"}
CLI_COMMANDS = ["get", "list", "write", "drain", "sync", "prune", "resolve", "status"]


def fetch(name):
    command = ["gh", "api", f"repos/{REPO}/contents/{DIR}/{name}?ref={REF}", "-H", "Accept: application/vnd.github.raw"]
    return command, subprocess.run(command, check=True, capture_output=True, text=True).stdout


def main():
    commands = [f"gh api repos/{REPO}/git/trees/{REF}?recursive=1 --jq '.tree[].path'"]
    cmd, source = fetch("bd_cache.py")
    commands.append(" ".join(cmd[:3]) + " ... (bd_cache.py and each client below)")
    tree = ast.parse(source)
    cls = next(n for n in tree.body if isinstance(n, ast.ClassDef) and n.name == "BdCache")
    defined = {n.name for n in cls.body if isinstance(n, ast.FunctionDef) and not n.name.startswith("_")}
    defined.add("path")  # an attribute set in __init__
    module_names = {n.name for n in tree.body if isinstance(n, (ast.ClassDef, ast.FunctionDef))}
    module_names |= {t.id for n in tree.body if isinstance(n, ast.Assign) for t in n.targets if isinstance(t, ast.Name)}
    uses = []
    for name in CLIENTS:
        _, text = fetch(name)
        for node in ast.walk(ast.parse(text)):
            if not isinstance(node, ast.Attribute):
                continue
            receiver = ast.unparse(node.value)
            if (receiver in RECEIVERS or receiver.startswith("bd_cache.BdCache(")) and node.attr in defined:
                uses.append({"kind": "method", "name": node.attr, "file": name, "line": node.lineno})
            elif receiver == "bd_cache" and (node.attr in module_names or node.attr == "__file__"):
                uses.append({"kind": "module", "name": node.attr, "file": name, "line": node.lineno})
            elif receiver in ERROR_RECEIVERS and node.attr in ERROR_NAMES:
                uses.append({"kind": "error", "name": node.attr, "file": name, "line": node.lineno})
        for number, line in enumerate(text.splitlines(), 1):
            for command in CLI_COMMANDS:
                named = "bd_cache.py" in line or "bd_cache.__file__" in line
                if named and re.search(rf"bd_cache\.py {command}\b|\"{command}\"\]", line):
                    uses.append({"kind": "cli", "name": command, "file": name, "line": number})
    uses.sort(key=lambda u: (u["kind"], u["name"], u["file"], u["line"]))
    print(json.dumps({
        "repo": REPO,
        "ref": REF,
        "scripts_dir": DIR,
        "clients": CLIENTS,
        "commands": commands + [f"python3 atm_core_05831df_extract.py > atm_core_05831df_calls.json"],
        "uses": uses,
        "names": sorted({f'{u["kind"]}:{u["name"]}' for u in uses}),
    }, indent=1))


if __name__ == "__main__":
    sys.exit(main())
