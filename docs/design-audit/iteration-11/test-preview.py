#!/usr/bin/env python3
"""Fail-closed contracts for preview architecture, linkage and archive checks."""
import importlib.util
from pathlib import Path
import struct
import tarfile
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location('preview', HERE / 'preview.py')
PREVIEW = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PREVIEW)


def executable(machine=62, segment=1, needed=False):
    data = bytearray(144)
    data[:6] = b'\x7fELF\x02\x01'
    struct.pack_into('<HH', data, 16, 3, machine)
    struct.pack_into('<Q', data, 32, 64)
    struct.pack_into('<HH', data, 54, 56, 1)
    struct.pack_into('<I', data, 64, segment)
    struct.pack_into('<Q', data, 72, 128)
    struct.pack_into('<Q', data, 96, 16)
    struct.pack_into('<Q', data, 128, 1 if needed else 0)
    return bytes(data)


class PreviewContracts(unittest.TestCase):
    def setUp(self):
        folder = PREVIEW.OUTPUT / 'contract-tests'
        folder.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=folder)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def check_binary(self, data, architecture='amd64'):
        path = self.root / 'they-work'
        path.write_bytes(data)
        return PREVIEW.elf_identity(path, architecture)

    def test_each_static_architecture_is_identified(self):
        for architecture, (_, machine) in PREVIEW.TARGETS.items():
            with self.subTest(architecture=architecture):
                result = self.check_binary(executable(machine), architecture)
                self.assertEqual(result['machine'], machine)
                self.assertFalse(result['interpreter'])
                self.assertFalse(result['needed_shared_libraries'])

    def test_wrong_architecture_is_rejected(self):
        with self.assertRaisesRegex(RuntimeError, 'architecture mismatch'):
            self.check_binary(executable(183))

    def test_dynamic_linker_is_rejected(self):
        with self.assertRaisesRegex(RuntimeError, 'statically linked'):
            self.check_binary(executable(segment=3))

    def test_shared_library_dependency_is_rejected(self):
        with self.assertRaisesRegex(RuntimeError, 'statically linked'):
            self.check_binary(executable(segment=2, needed=True))

    def test_truncated_program_headers_are_rejected(self):
        with self.assertRaisesRegex(RuntimeError, 'Invalid ELF headers'):
            self.check_binary(executable()[:70])

    def test_archive_retains_executable_bytes_and_mode(self):
        entries = {'they-work': (executable(), 0o755), 'LICENSE': (b'license', 0o644)}
        first = self.root / 'first.tar.gz'
        second = self.root / 'second.tar.gz'
        PREVIEW.archive(first, entries)
        PREVIEW.archive(second, entries)
        self.assertEqual(first.read_bytes(), second.read_bytes())
        with tarfile.open(first) as package:
            self.assertEqual(package.getnames(), ['LICENSE', 'they-work'])
            self.assertEqual(package.extractfile('they-work').read(), executable())
            self.assertEqual(package.getmember('they-work').mode, 0o755)

    def test_packaged_guide_has_its_record_and_local_contracts(self):
        documents = PREVIEW.documentation_entries()
        guide = documents['WSL-PREVIEW.md'][0].decode()
        self.assertIn('WSL-SESSION.md', documents)
        self.assertIn('INSTALL.md', documents)
        self.assertIn('docs/CONTROLS.md', documents)
        self.assertIn('(INSTALL.md)', guide)
        self.assertIn('(docs/CONTROLS.md)', guide)
        self.assertNotIn('../../../', guide)


if __name__ == '__main__':
    unittest.main(verbosity=2)
