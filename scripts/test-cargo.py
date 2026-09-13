#!/usr/bin/env python3
"""Exercise Cargo wrapper argument and CI environment handling without Rust or Docker."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]


class CargoWrapper(unittest.TestCase):
    def setUp(self):
        scratch = ROOT / 'target' / 'cargo-wrapper-tests'
        scratch.mkdir(parents=True, exist_ok=True)
        temporary = tempfile.TemporaryDirectory(dir=scratch)
        self.addCleanup(temporary.cleanup)
        self.base = Path(temporary.name)
        self.calls = self.base / 'calls.jsonl'
        self.env = dict(os.environ, PATH=f"{self.base}:{os.environ['PATH']}",
                        CARGO_WRAPPER_CALLS=str(self.calls), THEYWORK_TOOLCHAIN='native')
        for name in ('CI', 'RUST_TEST_THREADS', 'THEYWORK_DEV_IMAGE', 'THEYWORK_CARGO_NETWORK'):
            self.env.pop(name, None)
        for name in ('cargo', 'docker'):
            command = self.base / name
            command.write_text('''#!/usr/bin/env python3
import json, os, pathlib, sys
name = pathlib.Path(sys.argv[0]).name
arguments = sys.argv[1:]
record = {'command': name, 'arguments': arguments,
          'threads': os.environ.get('RUST_TEST_THREADS')}
if name == 'docker' and arguments and arguments[0] == 'run':
    forwarded = {}
    for index, argument in enumerate(arguments[:-1]):
        if argument == '-e':
            setting = arguments[index + 1]
            key, separator, value = setting.partition('=')
            forwarded[key] = value if separator else os.environ.get(key)
    record['container_environment'] = forwarded
with pathlib.Path(os.environ['CARGO_WRAPPER_CALLS']).open('a') as stream:
    stream.write(json.dumps(record) + '\\n')
''')
            command.chmod(0o755)

    def invoke(self, *arguments):
        self.calls.unlink(missing_ok=True)
        result = subprocess.run(['sh', str(ROOT / 'scripts' / 'cargo'), *arguments],
                                cwd=self.base, env=self.env, stdin=subprocess.DEVNULL,
                                text=True, capture_output=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        calls = [json.loads(line) for line in self.calls.read_text().splitlines()]
        call = calls[-1]
        self.assertEqual(call['arguments'][-len(arguments):] if arguments else [],
                         list(arguments))
        return call

    def test_ci_tests_default_to_one_thread(self):
        self.env['CI'] = 'true'
        call = self.invoke('test', '--locked', '--workspace', '--', '--nocapture')
        self.assertEqual(call['command'], 'cargo')
        self.assertEqual(call['threads'], '1')

    def test_explicit_environment_is_preserved(self):
        self.env['CI'] = 'true'
        for value in ('4', '', 'invalid'):
            with self.subTest(value=value):
                self.env['RUST_TEST_THREADS'] = value
                self.assertEqual(self.invoke('test')['threads'], value)

    def test_explicit_cli_thread_arguments_are_preserved(self):
        self.env['CI'] = 'true'
        for options in (('--test-threads', '4'), ('--test-threads=4',)):
            with self.subTest(options=options):
                call = self.invoke('test', '--workspace', '--', *options)
                self.assertIsNone(call['threads'])

    def test_cli_and_environment_overrides_are_both_preserved(self):
        self.env.update(CI='true', RUST_TEST_THREADS='8')
        call = self.invoke('test', '--', '--test-threads=3')
        self.assertEqual(call['threads'], '8')

    def test_local_test_default_is_unchanged(self):
        self.assertIsNone(self.invoke('test', '--workspace')['threads'])

    def test_false_and_nonstandard_ci_values_do_not_change_defaults(self):
        for value in ('false', '', '1'):
            with self.subTest(value=value):
                self.env['CI'] = value
                self.assertIsNone(self.invoke('test')['threads'])

    def test_ci_non_test_commands_do_not_gain_thread_settings(self):
        self.env['CI'] = 'true'
        for arguments in (('build', '--release'), ('clippy', '--all-targets'),
                          ('fmt', '--all', '--', '--check'), ('--version',), ()):
            with self.subTest(arguments=arguments):
                self.assertIsNone(self.invoke(*arguments)['threads'])

    def test_docker_receives_ci_test_default(self):
        self.env.update(CI='true', THEYWORK_TOOLCHAIN='docker')
        call = self.invoke('test', '--workspace')
        self.assertEqual(call['command'], 'docker')
        self.assertEqual(call['arguments'][0], 'run')
        self.assertEqual(call['container_environment']['RUST_TEST_THREADS'], '1')

    def test_docker_preserves_explicit_environment_and_arguments(self):
        self.env.update(CI='true', THEYWORK_TOOLCHAIN='docker', RUST_TEST_THREADS='6')
        call = self.invoke('test', '--', '--test-threads=2')
        self.assertEqual(call['container_environment']['RUST_TEST_THREADS'], '6')

    def test_docker_without_default_keeps_threads_unset(self):
        self.env['THEYWORK_TOOLCHAIN'] = 'docker'
        for arguments, ci in ((('test',), None), (('build',), 'true'),
                              (('test', '--', '--test-threads=4'), 'true')):
            with self.subTest(arguments=arguments, ci=ci):
                if ci is None:
                    self.env.pop('CI', None)
                else:
                    self.env['CI'] = ci
                call = self.invoke(*arguments)
                self.assertIsNone(call['container_environment']['RUST_TEST_THREADS'])


if __name__ == '__main__':
    unittest.main(verbosity=2)
