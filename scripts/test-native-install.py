#!/usr/bin/env python3
"""Offline installer contract tests; all temporary files stay in the checkout."""
import hashlib
import io
import os
from pathlib import Path
import subprocess
import shutil
import tarfile
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]


class NativeInstall(unittest.TestCase):
    def setUp(self):
        scratch = ROOT / 'target' / 'installer-tests'
        scratch.mkdir(parents=True, exist_ok=True)
        self.tmp = tempfile.TemporaryDirectory(dir=scratch)
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.bin = self.base / 'tools'
        self.bin.mkdir()
        self.env = dict(os.environ, TMPDIR=str(self.base),
                        PATH=f"{self.bin}:{os.environ['PATH']}",
                        INSTALL_FIXTURE=str(self.base), INSTALL_OS='Darwin',
                        INSTALL_ARCH='arm64', INSTALL_MODE='valid')
        self.mock('uname', '''#!/bin/sh
if [ "$1" = -s ]; then echo "$INSTALL_OS"; else echo "$INSTALL_ARCH"; fi
''')
        self.mock('curl', '''#!/usr/bin/env python3
import os, pathlib, shutil, sys
base = pathlib.Path(os.environ['INSTALL_FIXTURE'])
if os.environ['INSTALL_MODE'] == 'download-failure':
    sys.exit(22)
output = pathlib.Path(sys.argv[sys.argv.index('-o') + 1])
shutil.copyfile(base / ('SHA256SUMS' if output.name == 'SHA256SUMS' else 'release.tar.gz'), output)
''')
        self.destination = self.base / 'path with spaces'
        self.payload = b'#!/bin/sh\necho native-release\n'
        self.archive()

    def mock(self, name, source):
        path = self.bin / name
        path.write_text(source)
        path.chmod(0o755)

    def archive(self, name='they-work', target='aarch64-apple-darwin', checksum=None):
        with tarfile.open(self.base / 'release.tar.gz', 'w:gz') as archive:
            member = tarfile.TarInfo(name)
            member.size = len(self.payload)
            member.mode = 0o755
            archive.addfile(member, io.BytesIO(self.payload))
        digest = checksum or hashlib.sha256((self.base / 'release.tar.gz').read_bytes()).hexdigest()
        (self.base / 'SHA256SUMS').write_text(f'{digest}  they-work-{target}.tar.gz\n')

    def run_installer(self, *args):
        return subprocess.run(['sh', str(ROOT / 'scripts/install.sh'), '--install-dir',
                               str(self.destination), *args], env=self.env,
                              text=True, capture_output=True, timeout=20)

    def test_valid_archive_installs_executable_in_path_with_spaces(self):
        result = self.run_installer('--version', 'v0.2.0')
        self.assertEqual(result.returncode, 0, result.stderr)
        executable = self.destination / 'they-work'
        self.assertEqual(executable.read_bytes(), self.payload)
        self.assertTrue(os.access(executable, os.X_OK))
        self.assertIn('--setup', result.stdout)

    def test_both_linux_and_macos_architectures_resolve_assets(self):
        for system, suffix in [('Linux', 'unknown-linux-musl'), ('Darwin', 'apple-darwin')]:
            for machine, arch in [('x86_64', 'x86_64'), ('arm64', 'aarch64')]:
                with self.subTest(system=system, machine=machine):
                    self.env.update(INSTALL_OS=system, INSTALL_ARCH=machine)
                    self.archive(target=f'{arch}-{suffix}')
                    result = self.run_installer()
                    self.assertEqual(result.returncode, 0, result.stderr)

    def test_checksum_failure_preserves_existing_install(self):
        self.destination.mkdir()
        previous = self.destination / 'they-work'
        previous.write_text('previous executable')
        self.archive(checksum='0' * 64)
        result = self.run_installer()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('verification failed', result.stderr)
        self.assertEqual(previous.read_text(), 'previous executable')

    @unittest.skipUnless(shutil.which('shasum'), 'shasum is not installed')
    def test_macos_shasum_fallback_without_sha256sum(self):
        for name in ['sh', 'shasum', 'python3', 'tar', 'gzip', 'mktemp', 'awk', 'cp', 'chmod', 'mv', 'rm', 'mkdir']:
            path = shutil.which(name)
            self.assertIsNotNone(path, name)
            (self.bin / name).symlink_to(path)
        self.env['PATH'] = str(self.bin)
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.destination / 'they-work').read_bytes(), self.payload)

    def test_missing_release_has_actionable_failure(self):
        self.env['INSTALL_MODE'] = 'download-failure'
        result = self.run_installer('--version', 'v0.1.0')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Docker-only', result.stderr)
        self.assertFalse(self.destination.exists())

    def test_duplicate_checksum_rejected(self):
        checksums = self.base / 'SHA256SUMS'
        checksums.write_text(checksums.read_text() * 2)
        self.assertNotEqual(self.run_installer().returncode, 0)
        self.assertFalse(self.destination.exists())

    def test_unexpected_archive_member_cannot_escape(self):
        self.archive(name='../escaped')
        result = self.run_installer()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.base / 'escaped').exists())
        self.assertFalse(self.destination.exists())

    def test_symlink_binary_rejected(self):
        with tarfile.open(self.base / 'release.tar.gz', 'w:gz') as archive:
            member = tarfile.TarInfo('they-work')
            member.type = tarfile.SYMTYPE
            member.linkname = '/bin/sh'
            archive.addfile(member)
        digest = hashlib.sha256((self.base / 'release.tar.gz').read_bytes()).hexdigest()
        (self.base / 'SHA256SUMS').write_text(f'{digest}  they-work-aarch64-apple-darwin.tar.gz\n')
        self.assertNotEqual(self.run_installer().returncode, 0)
        self.assertFalse(self.destination.exists())

    def test_unknown_architecture_and_bad_arguments_fail(self):
        self.env['INSTALL_ARCH'] = 'unsupported'
        self.assertNotEqual(self.run_installer().returncode, 0)
        self.assertEqual(self.run_installer('--version').returncode, 2)
        self.assertEqual(self.run_installer('--version', '../../bad').returncode, 2)


if __name__ == '__main__':
    unittest.main(verbosity=2)
