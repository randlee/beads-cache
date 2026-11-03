#!/usr/bin/env python3
"""Build-only candidate packaging and exact-source installed qualification.

Python 3.9+; TOML capability is used only by preflight (Python 3.11+).
No command publishes a release, creates tags or changes channel settings.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tarfile
import tempfile
import zipfile
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
BINARIES = ('bd-cache', 'bdc', 'bd-cache-daemon')
TARGETS = ('x86_64-unknown-linux-gnu', 'aarch64-apple-darwin', 'x86_64-apple-darwin', 'x86_64-pc-windows-msvc')
WHEEL_TARGETS = TARGETS[:2]
KIT = '5fd4d0db1b7cbb4ebf9316588047391a8e4be496'


class QualificationError(Exception):
    """A candidate cannot satisfy a release contract."""


def require(condition, message):
    if not condition:
        raise QualificationError(message)


def run(command, *, cwd=None, timeout=120, env=None):
    """Bound and reap a process tree, including descendants that retain pipes."""
    child = subprocess.Popen([str(x) for x in command], cwd=cwd, env=env,
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                             start_new_session=os.name != 'nt',
                             creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if os.name == 'nt' else 0)
    try:
        stdout, stderr = child.communicate(timeout=timeout)
    except subprocess.TimeoutExpired as error:
        terminate_tree(child)
        raise QualificationError('command timed out; retry in a fresh candidate workspace: ' + str(command[0])) from error
    require(child.returncode == 0, 'command failed (%s): %s\n%s' % (child.returncode, command, stderr[-4000:]))
    return stdout


def terminate_tree(child):
    if os.name == 'nt':
        subprocess.run(['taskkill', '/PID', str(child.pid), '/T', '/F'], capture_output=True, timeout=15, check=False)
    else:
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    child.communicate(timeout=15)


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest() if hasattr(hashlib, 'file_digest') else hashlib.sha256(stream.read()).hexdigest()


def version():
    # This scalar is available on Python3.9 without an undeclared TOML dependency.
    text = (ROOT / 'Cargo.toml').read_text(encoding='utf-8')
    return re.search(r'(?m)^version\s*=\s*"([^"]+)"', text).group(1)


def commit():
    return run(['git', 'rev-parse', 'HEAD'], cwd=ROOT).strip()


def archive_name(target, release_version):
    return 'beads-cache-v%s-%s.%s' % (release_version, target, 'zip' if 'windows' in target else 'tar.gz')


def pack(args):
    require(args.target in TARGETS, 'unsupported target')
    require(not args.output.exists(), 'output exists; choose a fresh candidate directory')
    require(not run(['git', 'status', '--porcelain', '--untracked-files=no'], cwd=ROOT).strip(), 'tracked source is dirty; commit before candidate packaging')
    args.output.mkdir(parents=True)
    suffix = '.exe' if 'windows' in args.target else ''
    payload = {('bin/' + name + suffix): args.bin_dir / (name + suffix) for name in BINARIES}
    for name, path in payload.items():
        require(path.is_file(), 'missing binary: ' + name)
    receipt = {'schema_version': 1, 'commit': commit(), 'version': version(), 'target': args.target,
               'binaries': {name: digest(path) for name, path in payload.items()}, 'assets': {}}
    with tempfile.TemporaryDirectory(prefix='candidate-package-') as temp:
        staging = Path(temp)
        (staging / 'provenance.json').write_text(json.dumps({k: v for k, v in receipt.items() if k != 'assets'}, sort_keys=True) + '\n')
        for name, path in payload.items():
            dest = staging / name
            dest.parent.mkdir(exist_ok=True)
            shutil.copy2(path, dest)
        archive = args.output / archive_name(args.target, receipt['version'])
        if suffix:
            with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as out:
                for path in sorted(staging.rglob('*')):
                    if path.is_file():
                        out.write(path, path.relative_to(staging).as_posix())
        else:
            with tarfile.open(archive, 'w:gz') as out:
                for path in sorted(staging.rglob('*')):
                    if path.is_file():
                        out.add(path, arcname=path.relative_to(staging).as_posix(), recursive=False)
    wheels = list(args.wheel_dir.glob('*.whl')) if args.wheel_dir else []
    require(bool(wheels) == (args.target in WHEEL_TARGETS) and len(wheels) <= 1, 'wheel set differs from declared target matrix')
    for wheel in wheels:
        require('-cp39-abi3-' in wheel.name and receipt['version'].replace('-', '_') in wheel.name, 'wheel version/abi3-py39 tag mismatch')
        shutil.copy2(wheel, args.output / wheel.name)
    receipt['assets'] = {p.name: digest(p) for p in sorted(args.output.iterdir())}
    (args.output / 'candidate.json').write_text(json.dumps(receipt, indent=2) + '\n')
    (args.output / 'checksums.txt').write_text(''.join('%s  %s\n' % (sha, name) for name, sha in sorted(receipt['assets'].items())))
    validate(args.output, receipt['commit'], receipt['version'], args.target)


def validate(directory, expected_commit, expected_version, target):
    require(target in TARGETS, 'unsupported candidate target')
    receipt = json.loads((directory / 'candidate.json').read_text())
    require(receipt.get('schema_version') == 1, 'unsupported provenance schema')
    require((receipt['commit'], receipt['version'], receipt['target']) == (expected_commit, expected_version, target), 'mixed candidate commit/version/target')
    assets = receipt['assets']
    require(set(p.name for p in directory.iterdir()) == set(assets) | {'candidate.json', 'checksums.txt'}, 'unexpected asset/configuration in candidate directory')
    archive = archive_name(target, expected_version)
    wheels = [name for name in assets if name.endswith('.whl')]
    require(set(assets) == {archive} | set(wheels) and len(wheels) == int(target in WHEEL_TARGETS), 'archive/wheel membership differs from declared matrix')
    for name in assets:
        require(Path(name).name == name and digest(directory / name) == assets[name], 'asset checksum mismatch: ' + name)
    require((directory / 'checksums.txt').read_text() == ''.join('%s  %s\n' % (sha, name) for name, sha in sorted(assets.items())), 'checksums do not cover exactly intended assets')
    suffix = '.exe' if 'windows' in target else ''
    expected = {'bin/' + name + suffix for name in BINARIES}
    require(set(receipt['binaries']) == expected, 'missing/extra binary in receipt')
    path = directory / archive
    if suffix:
        with zipfile.ZipFile(path) as src:
            names = src.namelist()
            require(len(names) == len(set(names)) and set(names) == expected | {'provenance.json'}, 'unsafe or incorrect archive membership')
            contents = {name: src.read(name) for name in names}
    else:
        with tarfile.open(path) as src:
            members = src.getmembers()
            require(len(members) == len(set(m.name for m in members)) and all(m.isfile() for m in members) and {m.name for m in members} == expected | {'provenance.json'}, 'unsafe or incorrect archive membership')
            contents = {m.name: src.extractfile(m).read() for m in members}
    provenance = json.loads(contents.pop('provenance.json'))
    require(provenance == {k: v for k, v in receipt.items() if k != 'assets'}, 'archive provenance differs from receipt')
    for name, data in contents.items():
        require(hashlib.sha256(data).hexdigest() == receipt['binaries'][name], 'binary checksum mismatch: ' + name)
    for name in wheels:
        require('-cp39-abi3-' in name, 'wheel is not abi3-py39')
        platform_tag = 'manylinux_2_28_x86_64' if target == TARGETS[0] else 'macosx_11_0_arm64'
        require(name.endswith(platform_tag + '.whl'), 'wheel platform differs from candidate target')
        with zipfile.ZipFile(directory / name) as src:
            metadata = [n for n in src.namelist() if n.endswith('.dist-info/METADATA')]
            require(len(metadata) == 1 and '\nVersion: %s\n' % expected_version in src.read(metadata[0]).decode(), 'wheel metadata version mismatch')
    return receipt, contents


def smoke(args):
    receipt, contents = validate(args.directory, args.commit, args.version, args.target)
    with tempfile.TemporaryDirectory(prefix='installed-candidate-') as temp:
        installed = Path(temp)
        for name, data in contents.items():
            path = installed / name
            path.parent.mkdir(exist_ok=True)
            path.write_bytes(data)
            path.chmod(0o755)
        suffix = '.exe' if 'windows' in args.target else ''
        for binary in BINARIES:
            output = run([installed / 'bin' / (binary + suffix), '--version'], cwd=installed)
            require(args.version in output, 'installed binary version mismatch: ' + binary)
        # Existing C2 acceptance procedure executes installed binaries only.
        run([sys.executable, '-I', ROOT / 'scripts/consumer_checks.py', '--bdc', installed / 'bin' / ('bdc' + suffix), '--daemon', installed / 'bin' / ('bd-cache-daemon' + suffix), '--fake-bd', args.fake_bd.resolve(), '--fake-python', args.fake_python.resolve()], cwd=installed, timeout=300)
        wheel = list(args.directory.glob('*.whl'))
        if wheel:
            # A clean venv forbids source or previously installed package shadowing.
            venv = installed / 'venv'
            run([sys.executable, '-m', 'venv', venv])
            python = venv / ('Scripts/python.exe' if os.name == 'nt' else 'bin/python')
            run([python, '-m', 'pip', 'install', '--no-deps', wheel[0].resolve()], cwd=installed)
            probe = "import bd_cache_rs as m, pathlib, sys; assert m.__version__ == %r; assert pathlib.Path(m.__file__).resolve().is_relative_to(pathlib.Path(sys.prefix).resolve()); print(m.__file__)" % args.version
            # is_relative_to exists in the declared Python3.9 minimum.
            run([python, '-I', '-c', probe], cwd=installed)
            run([python, '-m', 'pip', 'install', 'pytest>=7,<9'], cwd=installed)
            env = dict(os.environ, BD_CACHE_BDC=str(installed / 'bin/bdc'), BD_CACHE_FAKE_BD=str(args.fake_bd.resolve()))
            run([python, '-I', '-m', 'pytest', ROOT / 'crates/bd-cache-py/tests', '--rootdir=' + str(ROOT / 'crates/bd-cache-py'), '-p', 'no:cacheprovider', '-q'], cwd=installed, env=env, timeout=300)
    print(json.dumps({'status': 'pass', 'commit': args.commit, 'version': args.version, 'target': args.target, 'python': sys.version.split()[0], 'installed': True}))


def manifest():
    require(sys.version_info >= (3, 11), 'preflight requires Python3.11+ TOML support; use the release runner')
    import tomllib
    data = json.loads((ROOT / 'release/sc-publish-consumer-input.json').read_text())
    rendered = tomllib.loads((ROOT / 'release/publish-artifacts.toml').read_text())
    for source in (data, rendered):
        require([c['package'] for c in source['crates']] == ['sc-beads', 'sc-bd-cli'], 'local sc-cron publication or unexpected crate')
        require({b['name'] for b in source['release_binaries']} == set(BINARIES), 'manifest missing daemon or extra binary')
        require(set(source['prerelease']['binaries']) == set(BINARIES), 'prerelease binary set mismatch')
    pin = tomllib.loads((ROOT / 'release/sc-publish-pin.toml').read_text())
    require(pin['sc-publish']['revision'] == KIT, 'unqualified kit: stable preflight requires reviewed isolated asset staging fix')
    workflow = (ROOT / '.github/workflows/release.yml').read_text()
    require(re.search(r'files:\s*\|?\s*\$\{\{ env.RELEASE_ASSET_DIR \}\}/\*', workflow)
            and 'RELEASE_ASSET_DIR=$RUNNER_TEMP/release-assets' in workflow,
            'unsafe legacy release staging; qualify kit #135 before stable publication')
    print('manifest and kit staging contract PASS')


def api(path):
    # The kit's consumer extension has no token environment. Public read-only
    # run/job/artifact metadata needs no credential; never read checkout secrets.
    headers = {'Accept': 'application/vnd.github+json', 'User-Agent': 'beads-cache-candidate-preflight'}
    if os.environ.get('GH_TOKEN'):
        headers['Authorization'] = 'Bearer ' + os.environ['GH_TOKEN']
    request = urllib.request.Request('https://api.github.com/repos/randlee/beads-cache/' + path, headers=headers)
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.load(response)


def preflight(args):
    manifest()
    sha = commit()
    runs = api('actions/workflows/release-qualification.yml/runs?head_sha=' + sha + '&status=success&per_page=10')['workflow_runs']
    runs = [r for r in runs if r['head_sha'] == sha and r['path'] == '.github/workflows/release-qualification.yml']
    require(runs, 'no successful installed-candidate workflow for exact commit; run Release artifact qualification and retry')
    selected = runs[0]
    jobs = api('actions/runs/%s/jobs?per_page=100' % selected['id'])['jobs']
    expected_jobs = {'candidate (%s)' % t for t in TARGETS}
    expected_jobs |= {'installed abi3 (%s, Python %s)' % (t, p) for t in WHEEL_TARGETS for p in ('3.9', '3.11', '3.13')}
    successful = {j['name'] for j in jobs if j['conclusion'] == 'success'}
    require(expected_jobs <= successful, 'same-commit installed platform/Python matrix incomplete')
    artifacts = api('actions/runs/%s/artifacts?per_page=100' % selected['id'])['artifacts']
    expected_artifacts = {'candidate-' + t for t in TARGETS} | {'evidence-' + t for t in TARGETS}
    expected_artifacts |= {'evidence-%s-py%s' % (t, p) for t in WHEEL_TARGETS for p in ('3.9', '3.11', '3.13')}
    require(expected_artifacts <= {a['name'] for a in artifacts if not a['expired']}, 'candidate archives or installed receipts missing/expired; rerun qualification')
    print(json.dumps({'status': 'pass', 'commit': sha, 'version': version(), 'candidate_run': selected['id'],
                      'installed_jobs': sorted(expected_jobs), 'evidence': selected['html_url']}))


def main(argv=None):
    parser = argparse.ArgumentParser(__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    p = commands.add_parser('pack')
    p.add_argument('--target', required=True)
    p.add_argument('--bin-dir', type=Path, required=True)
    p.add_argument('--wheel-dir', type=Path)
    p.add_argument('--output', type=Path, required=True)
    for action in ('check', 'smoke'):
        p = commands.add_parser(action)
        p.add_argument('--directory', type=Path, required=True)
        p.add_argument('--commit', required=True)
        p.add_argument('--version', required=True)
        p.add_argument('--target', required=True)
        if action == 'smoke':
            p.add_argument('--fake-bd', type=Path, required=True)
            p.add_argument('--fake-python', type=Path, required=True)
    commands.add_parser('manifest')
    commands.add_parser('preflight')
    args = parser.parse_args(argv)
    try:
        if args.command == 'pack':
            pack(args)
        elif args.command == 'check':
            validate(args.directory, args.commit, args.version, args.target)
        elif args.command == 'smoke':
            smoke(args)
        elif args.command == 'manifest':
            manifest()
        else:
            preflight(args)
    except (QualificationError, OSError, ValueError, KeyError, tarfile.TarError, zipfile.BadZipFile) as error:
        print(json.dumps({'status': 'blocked', 'code': 'CANDIDATE.QUALIFICATION', 'cause': str(error), 'recovery': 'Build and install a fresh complete candidate set from the exact committed source; rerun qualification.'}), file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
