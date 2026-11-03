"""ADR-BDC-0016: independently resolve consumers, never workspace feature union."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile


def violations(meta):
    packages = {p['id']: p for p in meta['packages']}
    nodes = {n['id']: n for n in meta['resolve']['nodes']}
    errors = []
    pending = [meta['resolve']['root']]
    reached = set()
    while pending:
        identity = pending.pop()
        if identity in reached:
            continue
        reached.add(identity)
        node = nodes[identity]
        p = packages[identity]
        name = p['name']
        if name in {'clap', 'clap_builder', 'sc-runtime', 'sc-transport', 'axum', 'rmcp'}:
            errors.append(f"isolated consumer contains {name}")
        if name == 'sc-command' and 'server' in node['features']:
            errors.append('isolated consumer enables sc-command/server')
        if name in {'sc-command', 'sc-cron'}:
            if p['version'] != '0.1.1' or not (p['source'] or '').startswith('registry+'):
                errors.append(f"{name} must be published 0.1.1, got {p['version']} {p['source']}")
        if name == 'rusqlite' and p['version'] != '0.39.0':
            errors.append(f"rusqlite must be 0.39.0, got {p['version']}")
        for edge in node['deps']:
            if not any(k['kind'] in (None, 'build') for k in edge['dep_kinds']):
                continue
            target = packages[edge['pkg']]
            if target['name'] == 'sc-command' and not (
                name == 'sc-cron' and p['version'] == '0.1.1'
                and (p['source'] or '').startswith('registry+')
            ):
                errors.append(f"{name} -> sc-command: only published sc-cron may introduce the contract/logging exception")
            pending.append(edge['pkg'])
    sqlite = [packages[i] for i in reached if packages[i].get('links') == 'sqlite3']
    if len(sqlite) > 1:
        errors.append(f"multiple SQLite link providers: {sqlite}")
    return errors


def metadata(command, timeout=120):
    """Own, kill and reap the Cargo child on a hard subprocess deadline."""
    try:
        completed = subprocess.run(command, check=True, capture_output=True,
                                   text=True, timeout=timeout)
    except subprocess.TimeoutExpired as error:
        raise RuntimeError(
            f"cargo metadata exceeded {timeout}s; owned child killed and reaped; "
            f"stdout={error.stdout!r}; stderr={error.stderr!r}"
        ) from error
    return json.loads(completed.stdout)


def main():
    parser = argparse.ArgumentParser(__doc__)
    parser.add_argument('--negative-server', action='store_true')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    for package in ('bd-cache-api', 'bd-cache-py'):
        with tempfile.TemporaryDirectory(prefix='bd-cache-consumer-') as temporary:
            consumer = Path(temporary)
            (consumer / 'src').mkdir()
            (consumer / 'src/lib.rs').write_text('// independent resolution root\n')
            text = '[workspace]\n[package]\nname="isolated-consumer"\nversion="0.0.0"\nedition="2024"\n[dependencies]\n'
            text += f'{package} = {{ path = {json.dumps(str(root / package))} }}\n'
            if args.negative_server:
                text += 'sc-command = { version = "=0.1.1", default-features = false, features = ["server"] }\n'
            (consumer / 'Cargo.toml').write_text(text)
            resolved = metadata(['cargo', 'metadata', '--format-version', '1', '--manifest-path', str(consumer / 'Cargo.toml')])
            errors = violations(resolved)
            if errors:
                raise SystemExit('\n'.join(errors))
            print(f'{package}: isolated graph PASS (published scheduler/contracts; no clap/server/runtime)')


if __name__ == '__main__':
    main()
