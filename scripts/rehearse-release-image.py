#!/usr/bin/env python3
"""Rehearse promotion faults using a verified image in a LOOPBACK registry only.

Start an owned disposable registry first. This does not build an image or publish
outside localhost. Native archives are small checksum fixtures, not native tests.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time
from urllib.parse import urlsplit

import release_image as release


class FaultRegistry(release.Registry):
    def __init__(self, destination=None, after=False):
        self.destination = destination
        self.after = after
        self.calls = []
        self.injected = False

    def promote(self, source, destination):
        self.calls.append({'source': source, 'destination': destination})
        inject = destination == self.destination and not self.injected
        if inject:
            self.injected = True
            if not self.after:
                raise RuntimeError('Rehearsal: interrupted before registry write')
        super().promote(source, destination)
        if inject:
            raise RuntimeError('Rehearsal: registry accepted write but acknowledgement was lost')


def native_fixture(directory):
    directory.mkdir()
    for target in release.TARGETS:
        name = f"they-work-{target}.{'zip' if 'windows' in target else 'tar.gz'}"
        body = f'Checksums only; this is not a native archive: {target}'.encode()
        (directory / name).write_bytes(body)
        (directory / (name + '.sha256')).write_text(f'{hashlib.sha256(body).hexdigest()}  {name}\n')


def run(verification, output, scratch):
    registry = release.Registry()
    proof = release.read_json(verification)
    repository = proof['image'].split('@')[0]
    host = urlsplit('http://' + repository.split('/')[0]).hostname
    release.require(host in ('localhost', '127.0.0.1', '::1'), 'Rehearsal refuses non-loopback registries')
    release.checked_verification(verification, registry)
    output.mkdir(parents=True, exist_ok=True)
    scratch.mkdir(parents=True, exist_ok=True)
    suffix = str(time.time_ns())
    baseline_ref = f'{repository}:audit-baseline-{suffix}'
    subprocess.run(['docker', 'buildx', 'imagetools', 'create',
                    '--annotation', f'index:org.they-work.audit.baseline={suffix}',
                    '--tag', baseline_ref, proof['image']], check=True)
    baseline = registry.inspect(baseline_ref)['digest']
    desired = proof['index']['digest']
    release.require(baseline != desired, 'Baseline fixture must have a different index digest')
    results = {'schema': 1, 'candidate': proof['image'], 'commit': proof['commit'],
               'verification_sha256': release.sha256(verification), 'baseline_digest': baseline,
               'scope': 'Actual local-registry index promotion; deterministic interruptions at write boundaries',
               'native_scope': 'Six checksum fixtures, not native executable validation', 'cases': []}
    with tempfile.TemporaryDirectory(dir=scratch) as temporary:
        root = Path(temporary)
        native = root / 'native'
        native_fixture(native)
        for number, name in enumerate(('failed-smoke', 'failed-checksum', 'success', 'between-tags',
                                       'lost-version-ack', 'lost-latest-ack', 'version-conflict',
                                       'later-latest', 'native-publication-failed')):
            version = f'audit-{suffix}-{number}'
            version_ref = f'{repository}:{version}'
            registry.promote(f'{repository}@{baseline}', f'{repository}:latest')
            case_dir = output / name
            case_dir.mkdir()
            local_proof = root / f'{name}.json'
            release.write_json(local_proof, proof)
            observation = {'case': name, 'passed': False}
            try:
                if name == 'failed-smoke':
                    changed = dict(proof, passed=False)
                    release.write_json(local_proof, changed)
                if name == 'failed-checksum':
                    next(native.glob('*.zip')).write_bytes(b'corrupt')
                if name in ('failed-smoke', 'failed-checksum'):
                    try:
                        release.prepare(registry, local_proof, native, version, proof['commit'])
                    except ValueError as error:
                        observation['expected_rejection'] = str(error)
                    else:
                        raise AssertionError('Invalid input prepared a release')
                    release.require(release.tag_digest(registry, version_ref) is None, 'Version changed')
                    release.require(release.tag_digest(registry, f'{repository}:latest') == baseline, 'Latest changed')
                    if name == 'failed-checksum':
                        for path in native.iterdir():
                            path.unlink()
                        native.rmdir()
                        native_fixture(native)
                else:
                    intent = release.prepare(registry, local_proof, native, version, proof['commit'])
                    release.write_json(case_dir / 'intent.json', intent, exclusive=True)
                    # Retain verification and intent before the first public-shaped
                    # tag mutation, just as the workflow artifact gate does.
                    release.write_json(case_dir / 'verification.json', proof)
                    fault_destination = intent['latest'] if name in ('between-tags', 'lost-latest-ack') else intent['version']
                    active = FaultRegistry(fault_destination if name in ('between-tags', 'lost-latest-ack', 'lost-version-ack') else None,
                                           after=name.startswith('lost-'))
                    if name == 'version-conflict':
                        registry.promote(f'{repository}@{baseline}', intent['version'])
                    if name == 'later-latest':
                        # A separate index represents another publisher, neither
                        # the original baseline nor the tested candidate.
                        other_ref = f'{repository}:audit-other-{suffix}'
                        subprocess.run(['docker', 'buildx', 'imagetools', 'create', '--annotation',
                                        f'index:org.they-work.audit.other={suffix}', '--tag', other_ref,
                                        proof['image']], check=True)
                        other = registry.inspect(other_ref)['digest']
                        registry.promote(f'{repository}@{other}', intent['latest'])
                    expect_failure = name in ('between-tags', 'version-conflict', 'later-latest')
                    try:
                        release.promote(active, intent, local_proof, native, case_dir / 'promotion.json')
                    except (ValueError, RuntimeError) as error:
                        if not expect_failure:
                            raise
                        observation['expected_rejection'] = str(error)
                    else:
                        release.require(not expect_failure, 'Injected fault did not fail')
                    if name == 'between-tags':
                        release.require(release.tag_digest(registry, intent['version']) == desired, 'Verified version was lost')
                        release.require(release.tag_digest(registry, intent['latest']) == baseline, 'Latest unexpectedly changed')
                        retry = FaultRegistry()
                        release.promote(retry, intent, local_proof, native, case_dir / 'retry.json')
                        release.require([call['destination'] for call in retry.calls] == [intent['latest']], 'Retry repeated version write')
                        observation['retry_calls'] = retry.calls
                    elif name in ('version-conflict', 'later-latest'):
                        release.require(not active.calls, 'Conflict caused promotion writes')
                    else:
                        retry = FaultRegistry()
                        release.promote(retry, intent, local_proof, native, case_dir / 'retry.json')
                        release.require(not retry.calls, 'Completed promotion was repeated')
                    if name == 'native-publication-failed':
                        subprocess.run([sys.executable, str(Path(__file__).with_name('release_image.py')),
                                        'outcome', '--record', str(case_dir / 'promotion.json'),
                                        '--native-release', 'failure'], check=True)
                        record = release.read_json(case_dir / 'promotion.json')
                        release.require(record['status'] == 'images-promoted' and record['native_release'] == 'failure',
                                        'Partial native publication was concealed')
                        observation['native_failure_scope'] = 'Injected workflow outcome; no GitHub service was called'
                    observation['promotion_calls'] = active.calls
                observation['after'] = {'version': release.tag_digest(registry, version_ref),
                                        'latest': release.tag_digest(registry, f'{repository}:latest')}
                observation['passed'] = True
            except Exception as error:
                observation['error'] = str(error)
            results['cases'].append(observation)
            release.write_json(output / 'results.json', results)
            if not observation['passed']:
                break
    results['passed'] = len(results['cases']) == 9 and all(case['passed'] for case in results['cases'])
    release.write_json(output / 'results.json', results)
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--verification', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--scratch', type=Path, default=Path('target/release-rehearsal'))
    args = parser.parse_args()
    try:
        result = run(args.verification, args.output, args.scratch)
        print(json.dumps(result, indent=2))
        return 0 if result['passed'] else 1
    except Exception as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
