"""Candidate contract negatives; no live backend, publication or application build."""
from __future__ import annotations

import hashlib
import io
import json
import os
import subprocess
import queue
import threading
from pathlib import Path
import sys
import tarfile
import tempfile
import time
import unittest
import zipfile
from types import SimpleNamespace
from unittest.mock import MagicMock, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_release_candidate as rc


class ProducerTests(unittest.TestCase):
    """Real git snapshots and source-dependent fixture outputs, without Rust."""

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.repo = self.root / 'repo'
        self.repo.mkdir()
        self.target = rc.TARGETS[0]
        self.git('init', '-q')
        self.git('config', 'user.name', 'Candidate Fixture')
        self.git('config', 'user.email', 'candidate@example.invalid')
        (self.repo / 'Cargo.toml').write_text('[workspace.package]\nversion = "0.2.0"\n')
        (self.repo / 'Cargo.lock').write_text('fixture lock\n')
        self.producer = self.root / 'producer.py'
        self.producer.write_text(
            "import os,sys,zipfile\n"
            "from pathlib import Path\n"
            "args=sys.argv[1:]\n"
            "assert '--locked' in args\n"
            "assert os.environ['CARGO_INCREMENTAL']=='0'\n"
            "assert Path('Cargo.lock').read_text()=='fixture lock\\n'\n"
            "data=Path('payload').read_bytes()\n"
            "if '--out' in args:\n"
            " out=Path(args[args.index('--out')+1]); out.mkdir()\n"
            " with zipfile.ZipFile(out/'bd_cache_rs-0.2.0-cp39-abi3-manylinux_2_28_x86_64.whl','w') as wheel:\n"
            "  wheel.writestr('bd_cache_rs-0.2.0.dist-info/METADATA','Metadata-Version: 2.1\\nName: bd-cache-rs\\nVersion: 0.2.0\\n')\n"
            "  wheel.writestr('bd_cache_rs/source.bin',data)\n"
            "else:\n"
            " out=Path(os.environ['CARGO_TARGET_DIR'])/args[args.index('--target')+1]/'release'\n"
            " out.mkdir(parents=True,exist_ok=True)\n"
            " names=('fake-bd','fake-python') if '--features' in args else ('bd-cache','bdc','bd-cache-daemon')\n"
            " for name in names:\n"
            "  path=out/name; path.write_bytes(b'#!/usr/bin/env python3\\n# version 0.2.0\\n# '+data); path.chmod(0o755)\n")
        self.actual_run = rc.run
        self.builds = []

    def git(self, *args):
        return subprocess.check_output(['git', *args], cwd=self.repo, text=True).strip()

    def source(self, payload):
        (self.repo / 'payload').write_text(payload)
        self.git('add', '.')
        self.git('commit', '-qm', payload)
        return self.git('rev-parse', 'HEAD')

    def run_producer(self, command, **kwargs):
        if str(command[0]) == 'cargo' or list(command[1:3]) == ['-m', 'maturin']:
            self.builds.append((command, kwargs))
            command = [sys.executable, '-I', self.producer, *command[1:]]
        return self.actual_run(command, **kwargs)

    def pack(self, name):
        args = SimpleNamespace(target=self.target, output=self.root / name, helpers=self.root / (name + '-helpers'))
        with patch.object(rc, 'ROOT', self.repo), patch.object(rc, 'run', side_effect=self.run_producer):
            rc.pack(args)
        return args.output

    def test_same_version_foreign_source_substitution_cannot_enter_pack(self):
        foreign_sha = self.source('foreign source')
        foreign = self.pack('foreign')
        foreign_receipt, foreign_binaries = rc.validate(foreign, foreign_sha, '0.2.0', self.target)
        current_sha = self.source('current source')
        # Plant actual same-version, different-commit artifacts at the former
        # arbitrary inputs, including the caller's configured Cargo target cache.
        cache = self.repo / 'target' / self.target / 'release'
        cache.mkdir(parents=True)
        for name, data in foreign_binaries.items():
            (cache / Path(name).name).write_bytes(data)
        wheels = self.repo / 'candidate-wheels'
        wheels.mkdir()
        foreign_wheel = next(foreign.glob('*.whl'))
        (wheels / foreign_wheel.name).write_bytes(foreign_wheel.read_bytes())
        with patch.dict(os.environ, {'CARGO_TARGET_DIR': str(self.repo / 'target')}):
            current = self.pack('current')
        current_receipt, current_binaries = rc.validate(current, current_sha, '0.2.0', self.target)
        self.assertNotEqual(current_receipt['binaries'], foreign_receipt['binaries'])
        self.assertTrue(all(b'current source' in data and b'foreign source' not in data for data in current_binaries.values()))
        with zipfile.ZipFile(next(current.glob('*.whl'))) as wheel:
            self.assertEqual(wheel.read('bd_cache_rs/source.bin'), b'current source')
        # Its existing receipt cannot be admitted as the current source either.
        with self.assertRaisesRegex(rc.QualificationError, 'mixed candidate'):
            rc.validate(foreign, current_sha, '0.2.0', self.target)
        self.assertTrue(all(Path(options['cwd']) != self.repo for _, options in self.builds))
        self.assertTrue(all(Path(options['env']['CARGO_TARGET_DIR']) != self.repo / 'target' for _, options in self.builds))

    def test_old_arbitrary_artifact_input_options_are_rejected(self):
        self.source('foreign source')
        foreign = self.pack('foreign')
        with patch.object(sys, 'stderr', io.StringIO()), self.assertRaises(SystemExit) as caught:
            rc.main(['pack', '--target', self.target, '--output', str(self.root / 'relabeled'),
                     '--bin-dir', str(foreign), '--wheel-dir', str(foreign)])
        self.assertEqual(caught.exception.code, 2)
        self.assertFalse((self.root / 'relabeled').exists())

    def test_missing_owned_binary_cannot_fall_back_to_foreign_build(self):
        foreign_sha = self.source('foreign source')
        foreign = self.pack('foreign')
        _, binaries = rc.validate(foreign, foreign_sha, '0.2.0', self.target)
        shared = self.repo / 'target' / self.target / 'release'
        shared.mkdir(parents=True)
        for name, data in binaries.items():
            (shared / Path(name).name).write_bytes(data)
        self.source('current source')
        producer = self.run_producer

        def omit_owned_binary(command, **kwargs):
            result = producer(command, **kwargs)
            if str(command[0]) == 'cargo' and '--bins' in command:
                (Path(kwargs['env']['CARGO_TARGET_DIR']) / self.target / 'release/bdc').unlink()
            return result

        with patch.object(self, 'run_producer', side_effect=omit_owned_binary):
            with self.assertRaisesRegex(rc.QualificationError, 'missing binary: bin/bdc'):
                self.pack('missing')

    def test_unchanged_committed_source_positive_is_reproducible(self):
        sha = self.source('same source')
        first = self.pack('first')
        second = self.pack('second')
        first_receipt, first_binaries = rc.validate(first, sha, '0.2.0', self.target)
        second_receipt, second_binaries = rc.validate(second, sha, '0.2.0', self.target)
        self.assertEqual(first_binaries, second_binaries)
        self.assertEqual(first_receipt['binaries'], second_receipt['binaries'])
        self.assertEqual({path.name for path in (self.root / 'second-helpers').iterdir()}, {'fake-bd', 'fake-python'})
        self.assertFalse(any('fake' in name for name in second_receipt['binaries']))

    def test_dirty_source_and_existing_outputs_refused_before_build(self):
        self.source('committed source')
        (self.repo / 'payload').write_text('uncommitted source')
        with self.assertRaisesRegex(rc.QualificationError, 'tracked source is dirty'):
            self.pack('dirty')
        self.assertFalse(self.builds)
        (self.root / 'occupied').mkdir()
        with self.assertRaisesRegex(rc.QualificationError, 'output exists'):
            self.pack('occupied')
        (self.root / 'existing-helpers').mkdir()
        with self.assertRaisesRegex(rc.QualificationError, 'helper output exists'):
            self.pack('existing')
        self.assertFalse(self.builds)


class CandidateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.target = rc.TARGETS[0]
        self.sha = 'a' * 40

    def candidate(self, *, omit=None, archive_commit=None):
        binaries = {'bin/' + name: name.encode() for name in rc.BINARIES if name != omit}
        receipt = {'schema_version': 1, 'commit': self.sha, 'version': '0.2.0', 'target': self.target,
                   'binaries': {name: hashlib.sha256(data).hexdigest() for name, data in binaries.items()}}
        embedded = dict(receipt, commit=archive_commit or self.sha)
        archive = self.directory / rc.archive_name(self.target, '0.2.0')
        with tarfile.open(archive, 'w:gz') as out:
            for name, data in dict(binaries, **{'provenance.json': json.dumps(embedded).encode()}).items():
                info = tarfile.TarInfo(name); info.size = len(data)
                out.addfile(info, io.BytesIO(data))
        wheel = self.directory / 'bd_cache_rs-0.2.0-cp39-abi3-manylinux_2_28_x86_64.whl'
        with zipfile.ZipFile(wheel, 'w') as out:
            out.writestr('bd_cache_rs-0.2.0.dist-info/METADATA', 'Metadata-Version: 2.1\nName: bd-cache-rs\nVersion: 0.2.0\n')
        receipt['assets'] = {p.name: rc.digest(p) for p in self.directory.iterdir()}
        (self.directory / 'candidate.json').write_text(json.dumps(receipt))
        (self.directory / 'checksums.txt').write_text(''.join('%s  %s\n' % (sha, name) for name, sha in sorted(receipt['assets'].items())))

    def check(self):
        return rc.validate(self.directory, self.sha, '0.2.0', self.target)

    def test_complete_candidate(self):
        self.candidate(); self.check()

    def test_omitted_daemon_fails(self):
        self.candidate(omit='bd-cache-daemon')
        with self.assertRaisesRegex(rc.QualificationError, 'missing/extra binary'):
            self.check()

    def test_mixed_archive_commit_fails(self):
        self.candidate(archive_commit='b' * 40)
        with self.assertRaisesRegex(rc.QualificationError, 'provenance'):
            self.check()

    def test_mixed_wheel_receipt_commit_fails(self):
        self.candidate()
        with self.assertRaisesRegex(rc.QualificationError, 'mixed candidate'):
            rc.validate(self.directory, 'b' * 40, '0.2.0', self.target)

    def test_configuration_asset_fails_even_with_checksum(self):
        self.candidate()
        (self.directory / 'sc-publish-consumer-input.json').write_text('{}')
        with self.assertRaisesRegex(rc.QualificationError, 'unexpected asset'):
            self.check()

    def test_corrupt_asset_fails(self):
        self.candidate()
        next(self.directory.glob('*.whl')).write_bytes(b'corrupt')
        with self.assertRaisesRegex(rc.QualificationError, 'checksum'):
            self.check()

    def test_missing_checksum_fails(self):
        self.candidate(); (self.directory / 'checksums.txt').write_text('')
        with self.assertRaisesRegex(rc.QualificationError, 'checksums'):
            self.check()

    def test_legacy_kit_and_restored_cron_fail(self):
        root = rc.ROOT
        with tempfile.TemporaryDirectory() as temp:
            fake = Path(temp); (fake / 'release').mkdir(); (fake / '.github/workflows').mkdir(parents=True)
            for path in ('release/sc-publish-consumer-input.json', 'release/publish-artifacts.toml', 'release/sc-publish-pin.toml', '.github/workflows/release.yml'):
                (fake / path).write_bytes((root / path).read_bytes())
            with patch.object(rc, 'ROOT', fake):
                rc.manifest()
                pin = fake / 'release/sc-publish-pin.toml'; pin.write_text(pin.read_text().replace(rc.KIT, '6' * 40))
                with self.assertRaisesRegex(rc.QualificationError, 'unqualified kit'):
                    rc.manifest()
                pin.write_bytes((root / 'release/sc-publish-pin.toml').read_bytes())
                workflow = fake / '.github/workflows/release.yml'; workflow.write_text('files: release/*')
                with self.assertRaisesRegex(rc.QualificationError, 'unsafe legacy'):
                    rc.manifest()
                data = json.loads((fake / 'release/sc-publish-consumer-input.json').read_text()); data['crates'].append({'package': 'sc-cron'})
                (fake / 'release/sc-publish-consumer-input.json').write_text(json.dumps(data))
                with self.assertRaisesRegex(rc.QualificationError, 'local sc-cron'):
                    rc.manifest()

    def test_missing_matrix_run_blocks_preflight(self):
        with patch.object(rc, 'manifest'), patch.object(rc, 'commit', return_value=self.sha), patch.object(rc, 'api', return_value={'workflow_runs': []}):
            with self.assertRaisesRegex(rc.QualificationError, 'no successful installed-candidate'):
                rc.preflight(None)

    def test_exact_installed_versions_reject_prefix_matches(self):
        self.assertEqual(rc.binary_version('bd-cache-daemon', 'bd-cache-daemon 0.2.01\n'), '0.2.01')
        self.assertNotEqual(rc.binary_version('bdc', json.dumps({'ok': True, 'data': {'version': '0.2.01'}})), '0.2.0')
        with self.assertRaises(rc.QualificationError):
            rc.binary_version('bd-cache-daemon', 'other-daemon 0.2.0')

    @unittest.skipIf(os.name == 'nt', 'POSIX group termination state; Windows uses its Job')
    def test_timeout_requests_group_termination_once(self):
        child = MagicMock()
        child.communicate.side_effect = [subprocess.TimeoutExpired(['fixture'], 1), ('', '')]
        child.poll.return_value = -9
        child.returncode = -9
        with patch.object(rc.subprocess, 'Popen', return_value=child), patch.object(rc.os, 'killpg', side_effect=[None, PermissionError('second signal forbidden')]) as kill:
            with self.assertRaisesRegex(rc.QualificationError, 'timed out'):
                rc.run(['fixture'])
        self.assertEqual(kill.call_count, 1)

    @unittest.skipIf(os.name == 'nt', 'POSIX permission failure; Windows uses its Job')
    def test_initial_group_permission_failure_is_reported_without_retry(self):
        child = MagicMock()
        child.communicate.side_effect = subprocess.TimeoutExpired(['fixture'], 1)
        child.poll.return_value = -9
        with patch.object(rc.subprocess, 'Popen', return_value=child), patch.object(rc.os, 'killpg', side_effect=PermissionError('denied')) as kill:
            with self.assertRaisesRegex(rc.QualificationError, 'could not terminate owned'):
                rc.run(['fixture'])
        self.assertEqual(kill.call_count, 1)

    @unittest.skipIf(os.name == 'nt', 'POSIX permission failure; Windows uses its Job')
    def test_final_cleanup_permission_failure_still_closes_pipes(self):
        child = MagicMock()
        child.communicate.return_value = ('ok', '')
        child.returncode = 0
        child.poll.return_value = 0
        with patch.object(rc.subprocess, 'Popen', return_value=child), patch.object(rc.os, 'killpg', side_effect=PermissionError('denied')):
            with self.assertRaisesRegex(rc.QualificationError, 'could not terminate owned'):
                rc.run(['fixture'])
        child.stdout.close.assert_called_once()
        child.stderr.close.assert_called_once()

    def test_acceptance_python_decodes_utf8_native_diagnostics(self):
        # Match a native child writing UTF-8 bytes, rather than Python's
        # configurable console encoding. The C2 acceptance uses text=True.
        diagnostic = 'CFG.PARSE invalid JSON — Fix the JSON.'
        native = 'import sys; sys.stdout.buffer.write(%r)' % diagnostic.encode('utf-8')
        probe = ("import subprocess,sys; assert sys.flags.utf8_mode == 1; "
                 "print(subprocess.check_output([sys.executable,'-c',%r],text=True))") % native
        self.assertEqual(rc.run(rc.isolated_python('-c', probe)).strip(), diagnostic)

    def test_timeout_reaps_ready_descendant(self):
        self.descendant_proof(parent_exits=False)

    def test_timeout_reaps_descendant_after_parent_exits(self):
        self.descendant_proof(parent_exits=True)

    def descendant_proof(self, *, parent_exits):
        lock = self.directory / 'descendant.lock'
        lock.write_bytes(b'x')
        lock_code = ("import msvcrt; msvcrt.locking(handle.fileno(),msvcrt.LK_LOCK,1)" if os.name == 'nt'
                     else "import fcntl; fcntl.flock(handle,fcntl.LOCK_EX)")
        descendant = ("import threading,sys; handle=open(%r,'r+b'); " % str(lock) + lock_code +
                      "; print('descendant ready',flush=True); threading.Event().wait()")
        parent = ("import subprocess,sys,threading; subprocess.Popen([sys.executable,'-I','-c',%r]); " % descendant +
                  ("sys.exit(0)" if parent_exits else "threading.Event().wait()"))
        actual_popen = subprocess.Popen
        processes = []

        def controlled_popen(*args, **kwargs):
            child = actual_popen(*args, **kwargs); processes.append(child)
            communicate = child.communicate
            first = True

            def controlled_communicate(*, input=None, timeout=None):
                nonlocal first
                if first:
                    first = False
                    if input is not None:
                        child.stdin.write(input); child.stdin.flush()
                    ready = queue.Queue()
                    reader = threading.Thread(target=lambda: ready.put(child.stdout.readline()), daemon=True)
                    reader.start()
                    self.assertEqual(ready.get(timeout=10), 'descendant ready\n')
                    reader.join(timeout=5)
                    self.assertFalse(reader.is_alive())
                    if parent_exits:
                        self.assertEqual(child.wait(timeout=5), 0)
                    raise subprocess.TimeoutExpired(args[0], timeout)
                return communicate(timeout=timeout)
            child.communicate = controlled_communicate
            return child

        began = time.monotonic()
        with patch.object(rc.subprocess, 'Popen', side_effect=controlled_popen):
            with self.assertRaisesRegex(rc.QualificationError, 'timed out'):
                rc.run([sys.executable, '-I', '-c', parent], timeout=0.25)
        self.assertLess(time.monotonic() - began, 15)
        self.assertIsNotNone(processes[0].poll())
        with lock.open('r+b') as handle:
            if os.name == 'nt':
                import msvcrt
                msvcrt.locking(handle.fileno(), msvcrt.LK_NBLCK, 1)
                msvcrt.locking(handle.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                import fcntl
                fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
                fcntl.flock(handle, fcntl.LOCK_UN)


if __name__ == '__main__':
    unittest.main()
