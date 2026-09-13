#!/usr/bin/env python3
"""Deterministic release boundaries; no daemon, credentials or registry writes."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import shlex
import shutil
import subprocess
import tempfile
import tomllib
import unittest
from unittest.mock import patch

import release_image as release

SPEC = importlib.util.spec_from_file_location('verify_image', Path(__file__).with_name('test-published-image.py'))
verify = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(verify)
CANDIDATE = 'sha256:' + 'a' * 64
OLD = 'sha256:' + 'b' * 64
OTHER = 'sha256:' + 'c' * 64
COMMIT = 'd' * 40
REPOSITORY = 'localhost:5058/fixture'


class DockerCacheContextTests(unittest.TestCase):
    def test_dependency_cache_contains_explicit_cargo_targets(self):
        root = Path(__file__).resolve().parents[1]
        dockerfile = (root / 'docker/Dockerfile').read_text().replace('\\\n', ' ')
        cache = dockerfile.split('COPY . .', 1)[0]
        copies = [shlex.split(line)[1:] for line in cache.splitlines() if line.startswith('COPY ')]
        stubs = next(line[4:] for line in cache.splitlines() if line.startswith('RUN for c in '))
        self.assertIn(' && cargo build --release --locked', stubs)
        # Execute the actual cache-layer file preparation without compiling or
        # accessing a registry. Cargo validates explicit targets even for build.
        stubs = stubs.rsplit(' && cargo build --release --locked', 1)[0]
        with tempfile.TemporaryDirectory() as temporary:
            context = Path(temporary)
            for *patterns, destination in copies:
                dest = context / destination
                dest.mkdir(parents=True, exist_ok=True)
                for pattern in patterns:
                    for source in root.glob(pattern):
                        shutil.copy2(source, dest / source.name)
            subprocess.run(['sh', '-eu', '-c', stubs], cwd=context, check=True)
            checked = 0
            for manifest_path in context.rglob('Cargo.toml'):
                manifest = tomllib.loads(manifest_path.read_text())
                for kind, directory in [('test', 'tests'), ('bench', 'benches'),
                                        ('example', 'examples'), ('bin', 'src/bin')]:
                    for target in manifest.get(kind, []):
                        default = f"{directory}/{target['name']}"
                        paths = ([target['path']] if 'path' in target else
                                 [default + '.rs', default + '/main.rs'])
                        self.assertTrue(any((manifest_path.parent / path).is_file() for path in paths),
                                        f"Cache layer omits {kind} {target['name']} in {manifest_path.relative_to(context)}")
                        checked += 1
            self.assertGreater(checked, 0, 'The explicit-target regression must inspect at least one target')


def manifest(digest=CANDIDATE):
    return {'digest': digest, 'mediaType': release.INDEX_TYPES[0], 'manifests': [
        {'digest': 'sha256:' + '1' * 64, 'platform': {'os': 'linux', 'architecture': 'amd64'}},
        {'digest': 'sha256:' + '2' * 64, 'platform': {'os': 'linux', 'architecture': 'arm64'}},
        {'digest': 'sha256:' + '3' * 64, 'platform': {'os': 'unknown', 'architecture': 'unknown'},
         'annotations': {'vnd.docker.reference.type': 'attestation-manifest'}},
    ]}


def proof():
    return {'schema': 1, 'commit': COMMIT, 'image': f'{REPOSITORY}@{CANDIDATE}',
            'index': manifest(), 'passed': True, 'platforms': {
                platform: {'passed': True, 'manifest_digest': 'sha256:' + str(i) * 64,
                           'frames': {mode: {'passed': True, 'exit_code': 0, 'quit_sent': True}
                                      for mode in ('kitty', 'iterm2', 'fallback')}}
                for i, platform in enumerate(release.PLATFORMS, 1)}}


def native_fixture(directory):
    for target in release.TARGETS:
        name = f"they-work-{target}.{'zip' if 'windows' in target else 'tar.gz'}"
        data = f'checksum fixture only: {target}'.encode()
        (directory / name).write_bytes(data)
        (directory / (name + '.sha256')).write_text(f'{hashlib.sha256(data).hexdigest()}  {name}\n')


class FakeRegistry:
    def __init__(self):
        self.tags = {f'{REPOSITORY}:latest': OLD}
        self.calls = []
        self.faults = {}
        self.index = manifest()

    def inspect(self, reference, missing_ok=False):
        if '@' in reference:
            return copy.deepcopy(self.index)
        if reference not in self.tags:
            if missing_ok:
                return None
            raise RuntimeError('manifest unknown')
        return manifest(self.tags[reference])

    def promote(self, source, destination):
        self.calls.append((source, destination))
        fault = self.faults.pop(destination, None)
        if fault == 'before':
            raise RuntimeError('injected before PUT')
        self.tags[destination] = source.rsplit('@', 1)[1]
        if fault == 'after':
            raise RuntimeError('injected lost PUT acknowledgement')
        if fault == 'wrong':
            self.tags[destination] = OTHER


class PromotionTests(unittest.TestCase):
    def setUp(self):
        scratch = Path(__file__).resolve().parents[1] / 'target/release-tests'
        scratch.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=scratch)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.verification = self.root / 'verification.json'
        release.write_json(self.verification, proof())
        self.native = self.root / 'native'
        self.native.mkdir()
        native_fixture(self.native)
        self.registry = FakeRegistry()
        self.intent = release.prepare(self.registry, self.verification, self.native, 'v1.2.3', COMMIT)
        self.record = self.root / 'promotion.json'

    def run_promotion(self):
        return release.promote(self.registry, self.intent, self.verification, self.native, self.record)

    def test_success_copies_exact_index_including_attestations_and_retries_without_writes(self):
        result = self.run_promotion()
        self.assertEqual(result['status'], 'images-promoted')
        self.assertEqual(list(result['after'].values()), [CANDIDATE, CANDIDATE])
        self.assertEqual(self.registry.calls, [(self.intent['candidate'], self.intent[name])
                                              for name in ('version', 'latest')])
        self.assertEqual(self.registry.index['manifests'][-1]['platform']['os'], 'unknown')
        self.run_promotion()
        self.assertEqual(len(self.registry.calls), 2)

    def test_failed_smoke_never_prepares_or_promotes(self):
        value = proof()
        value['passed'] = False
        release.write_json(self.verification, value)
        with self.assertRaisesRegex(ValueError, 'failed'):
            release.prepare(self.registry, self.verification, self.native, 'v1.2.3', COMMIT)
        self.assertEqual(self.registry.calls, [])

    def test_missing_architecture_and_missing_mode_are_rejected(self):
        for mutate in (lambda value: value['platforms'].pop('linux/arm64'),
                       lambda value: value['platforms']['linux/amd64']['frames'].pop('fallback')):
            value = proof()
            mutate(value)
            release.write_json(self.verification, value)
            with self.assertRaises(ValueError):
                release.prepare(self.registry, self.verification, self.native, 'v1.2.3', COMMIT)
        self.assertEqual(self.registry.calls, [])

    def test_checksum_failure_before_prepare_and_after_intent_does_not_change_tags(self):
        archive = next(self.native.glob('*.zip'))
        archive.write_bytes(b'corrupt')
        with self.assertRaisesRegex(ValueError, 'checksum failed'):
            release.prepare(self.registry, self.verification, self.native, 'v1.2.3', COMMIT)
        with self.assertRaises(ValueError):
            self.run_promotion()
        self.assertEqual(self.registry.tags, {self.intent['latest']: OLD})

    def test_missing_native_target_is_not_a_successful_subset(self):
        next(self.native.glob('*.zip')).unlink()
        with self.assertRaisesRegex(ValueError, 'exactly the six'):
            self.run_promotion()
        self.assertEqual(self.registry.calls, [])

    def test_report_or_commit_or_descriptor_change_blocks_publication(self):
        value = proof()
        value['commit'] = 'e' * 40
        release.write_json(self.verification, value)
        with self.assertRaisesRegex(ValueError, 'artifact changed'):
            self.run_promotion()
        with self.assertRaisesRegex(ValueError, 'different source'):
            release.prepare(self.registry, self.verification, self.native, 'v1.2.3', COMMIT)
        release.write_json(self.verification, proof())
        self.registry.index['manifests'].pop()
        with self.assertRaisesRegex(ValueError, 'descriptors'):
            self.run_promotion()
        self.assertEqual(self.registry.calls, [])

    def test_failure_before_version_leaves_both_tags(self):
        self.registry.faults[self.intent['version']] = 'before'
        with self.assertRaises(RuntimeError):
            self.run_promotion()
        self.assertEqual(self.registry.tags, {self.intent['latest']: OLD})

    def test_failure_between_tags_records_partial_and_retries_only_latest(self):
        self.registry.faults[self.intent['latest']] = 'before'
        with self.assertRaises(RuntimeError):
            self.run_promotion()
        result = release.read_json(self.record)
        self.assertEqual(result['completed_steps'], ['version'])
        self.assertEqual(result['observed_after_failure'], {'version': CANDIDATE, 'latest': OLD})
        before = len(self.registry.calls)
        self.run_promotion()
        self.assertEqual(self.registry.calls[before:], [(self.intent['candidate'], self.intent['latest'])])

    def test_lost_acknowledgements_are_read_back_as_success(self):
        self.registry.faults = {self.intent[name]: 'after' for name in ('version', 'latest')}
        result = self.run_promotion()
        self.assertEqual(result['status'], 'images-promoted')
        self.assertEqual(result['recovered_acknowledgements'], ['version', 'latest'])

    def test_already_promoted_latest_after_crash_is_a_noop(self):
        self.registry.tags.update({self.intent[name]: CANDIDATE for name in ('version', 'latest')})
        self.assertEqual(self.run_promotion()['status'], 'images-promoted')
        self.assertEqual(self.registry.calls, [])

    def test_version_conflict_blocks_even_latest(self):
        self.registry.tags[self.intent['version']] = OTHER
        with self.assertRaisesRegex(ValueError, 'Version conflict'):
            self.run_promotion()
        self.assertEqual(self.registry.calls, [])

    def test_later_latest_blocks_even_an_absent_version(self):
        self.registry.tags[self.intent['latest']] = OTHER
        with self.assertRaisesRegex(ValueError, 'Latest changed'):
            self.run_promotion()
        self.assertEqual(self.registry.calls, [])

    def test_wrong_written_digest_fails_and_never_rolls_back(self):
        self.registry.faults[self.intent['version']] = 'wrong'
        with self.assertRaisesRegex(ValueError, 'differs'):
            self.run_promotion()
        self.assertEqual(self.registry.tags[self.intent['version']], OTHER)
        self.assertEqual(len(self.registry.calls), 1)

    def test_prepare_cannot_rebase_a_previous_release_on_new_latest(self):
        self.run_promotion()
        self.registry.tags[self.intent['latest']] = OTHER
        with self.assertRaisesRegex(ValueError, 'original retained intent'):
            release.prepare(self.registry, self.verification, self.native, 'v1.2.3', COMMIT)

    def test_intent_is_never_overwritten(self):
        path = self.root / 'intent.json'
        release.write_json(path, self.intent, exclusive=True)
        with self.assertRaises(FileExistsError):
            release.write_json(path, {'replacement': True}, exclusive=True)
        self.assertEqual(release.read_json(path), self.intent)

    def test_authentication_and_network_errors_are_not_absence(self):
        for error in ('unauthorized: manifest not found', 'connection refused', 'lookup host: no such host',
                      'lookup registry.example: host not found', 'proxy not found', 'TLS proxy: manifest unknown'):
            response = subprocess.CompletedProcess([], 1, '', error)
            with patch.object(release.subprocess, 'run', return_value=response):
                with self.assertRaises(RuntimeError):
                    release.Registry().inspect('example.invalid/a:v1', missing_ok=True)


    def test_only_explicit_registry_manifest_absence_is_accepted(self):
        for error in ('ERROR: example.invalid/a:v1: not found', 'manifest unknown', 'MANIFEST_UNKNOWN'):
            response = subprocess.CompletedProcess([], 1, '', error)
            with patch.object(release.subprocess, 'run', return_value=response):
                self.assertIsNone(release.Registry().inspect('example.invalid/a:v1', missing_ok=True))



class VerifierTests(unittest.TestCase):
    def test_github_summary_retains_platform_glyph_and_exit_evidence(self):
        result = {'passed': True, 'exit_code': 0, 'forced_cleanup': False,
                  'quit_sent': True, 'glyph_counts': {'half': 0, 'quadrant': 0, 'sextant': 0}}
        platforms = {}
        for platform, execution in [('linux/amd64', 'daemon-native'), ('linux/arm64', 'emulated')]:
            platforms[platform] = {'execution': execution, 'manifest_digest': CANDIDATE,
                                   'frames': {mode: copy.deepcopy(result)
                                              for mode in ('kitty', 'iterm2', 'fallback')},
                                   'errors': []}
        platforms['linux/arm64']['frames']['iterm2']['glyph_counts']['half'] = 2
        with tempfile.TemporaryDirectory() as temporary:
            summary = Path(temporary) / 'summary.md'
            with patch.dict(verify.os.environ, {'GITHUB_STEP_SUMMARY': str(summary)}):
                verify.write_github_summary(f'{REPOSITORY}@{CANDIDATE}',
                                            {'passed': True, 'platforms': platforms})
            content = summary.read_text()
        self.assertIn('linux/amd64 (daemon-native)', content)
        self.assertIn('linux/arm64 (emulated)', content)
        self.assertIn('iTerm2 probe half=2 quadrant=0 sextant=0', content)
        self.assertIn('no-reply fallback half=0 quadrant=0 sextant=0', content)
        self.assertEqual(content.count('| fallback | passed | yes |'), 2)
        self.assertIn('not a physical-terminal test', content)

    def test_github_summary_reports_failure_before_any_platform_execution(self):
        with tempfile.TemporaryDirectory() as temporary:
            summary = Path(temporary) / 'summary.md'
            with patch.dict(verify.os.environ, {'GITHUB_STEP_SUMMARY': str(summary)}):
                verify.write_github_summary('registry.invalid/candidate',
                                            {'passed': False, 'platforms': {},
                                             'error': 'registry unavailable'})
            content = summary.read_text()
        self.assertIn('Result: failed', content)
        self.assertIn('Verification error: registry unavailable', content)

    def test_graphics_then_crash_or_forced_cleanup_is_a_failure(self):
        frame = b'\x1b_Ga=T,fixture'
        for code, cleanup, quit_sent in ((1, False, True), (None, True, True), (0, False, False)):
            result = {'exit_code': code, 'forced_cleanup': cleanup, 'quit_sent': quit_sent,
                      'probe_answered': True}
            self.assertTrue(verify.check_frame(frame, result, b'\x1b_Ga=T,'))

    def test_fallback_must_not_transmit_graphics(self):
        result = {'exit_code': 0, 'forced_cleanup': False, 'quit_sent': True, 'probe_answered': False}
        self.assertFalse(verify.check_frame(b' '.join(verify.NATIVE_MARKERS), result, None))
        self.assertTrue(verify.check_frame('▘'.encode(), result, None))
        self.assertTrue(verify.check_frame('▘'.encode() + b'\x1b]1337;File=fixture', result, None))

    def test_each_platform_is_passed_to_pull_environment_and_every_pty(self):
        result = {'exit_code': 0, 'forced_cleanup': False, 'quit_sent': True, 'probe_answered': True}
        frames = [(b'\x1b_Ga=T,fixture', result), (b'\x1b]1337;File=fixture', result), (b' '.join(verify.NATIVE_MARKERS), result)]
        with patch.object(verify, 'command') as command, \
             patch.object(verify, 'runtime_environment', return_value=verify.EXPECTED_LOCALE) as env, \
             patch.object(verify, 'terminal_frame', side_effect=frames) as terminal:
            value = verify.verify_platform('repo@digest', 'linux/arm64', CANDIDATE, 'amd64', 30)
            command.assert_called_once_with('pull', '--platform', 'linux/arm64', 'repo@digest')
            env.assert_called_once_with('repo@digest', 'linux/arm64')
            self.assertTrue(all(call.args[1] == 'linux/arm64' for call in terminal.call_args_list))
            self.assertTrue(value['passed'])
            self.assertEqual(value['execution'], 'emulated')


if __name__ == '__main__':
    unittest.main(verbosity=2)
