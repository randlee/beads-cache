"""ADR-BDC-0016: independently resolve consumers, never workspace feature union."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile


def violations(meta):
    packages = {p['id']: p for p in meta['packages']}
    errors = []
    for node in meta['resolve']['nodes']:
        p = packages[node['id']]
        name = p['name']
        if name in {'clap', 'clap_builder', 'sc-runtime', 'sc-transport'}:
            errors.append(f"isolated consumer contains {name}")
        if name == 'sc-command' and 'server' in node['features']:
            errors.append('isolated consumer enables sc-command/server')
        if name in {'sc-command', 'sc-cron'}:
            if p['version'] != '0.1.1' or not (p['source'] or '').startswith('registry+'):
                errors.append(f"{name} must be published 0.1.1, got {p['version']} {p['source']}")
        if name == 'rusqlite' and p['version'] != '0.39.0':
            errors.append(f"rusqlite must be 0.39.0, got {p['version']}")
    sqlite = [p for p in packages.values() if p.get('links') == 'sqlite3']
    if len(sqlite) > 1:
        errors.append(f"multiple SQLite link providers: {sqlite}")
    return errors


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
            raw = subprocess.check_output(['cargo', 'metadata', '--format-version', '1', '--manifest-path', str(consumer / 'Cargo.toml')])
            errors = violations(json.loads(raw))
            if errors:
                raise SystemExit('\n'.join(errors))
            print(f'{package}: isolated graph PASS (published scheduler/contracts; no clap/server/runtime)')


if __name__ == '__main__':
    main()
