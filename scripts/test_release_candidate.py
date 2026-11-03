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
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_release_candidate as rc


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
