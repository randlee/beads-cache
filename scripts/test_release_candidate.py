"""Candidate contract negatives; no live backend, publication or application build."""
from __future__ import annotations

import hashlib
import io
import json
import os
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

    @unittest.skipIf(os.name == 'nt', 'POSIX process-group descendant probe')
    def test_timeout_reaps_descendant_retaining_pipes(self):
        script = "import subprocess,sys,time; subprocess.Popen([sys.executable,'-c','import time; time.sleep(60)']); time.sleep(60)"
        began = time.monotonic()
        with self.assertRaisesRegex(rc.QualificationError, 'timed out'):
            rc.run([sys.executable, '-c', script], timeout=0.1)
        self.assertLess(time.monotonic() - began, 5)


if __name__ == '__main__':
    unittest.main()
