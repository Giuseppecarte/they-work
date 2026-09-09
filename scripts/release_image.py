#!/usr/bin/env python3
"""Prepare a verified release intent, then promote that exact registry index.

No build, provider invocation, automatic rollback, or GitHub Release mutation.
Registry credentials come from the operator's existing Docker login.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

PLATFORMS = ('linux/amd64', 'linux/arm64')
TARGETS = ('x86_64-unknown-linux-musl', 'aarch64-unknown-linux-musl',
           'x86_64-apple-darwin', 'aarch64-apple-darwin',
           'x86_64-pc-windows-msvc', 'aarch64-pc-windows-msvc')
INDEX_TYPES = ('application/vnd.oci.image.index.v1+json',
               'application/vnd.docker.distribution.manifest.list.v2+json')
DIGEST = re.compile(r'^sha256:[0-9a-f]{64}$')
TAG = re.compile(r'^[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}$')


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha256(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def read_json(path):
    return json.loads(Path(path).read_text())


def write_json(path, value, exclusive=False):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(value, indent=2, sort_keys=True) + '\n'
    if exclusive:
        with path.open('x') as stream:
            stream.write(text)
            stream.flush()
            os.fsync(stream.fileno())
    else:
        temporary = path.with_suffix(path.suffix + '.tmp')
        temporary.write_text(text)
        os.replace(temporary, path)


class Registry:
    def inspect(self, reference, missing_ok=False):
        result = subprocess.run(['docker', 'buildx', 'imagetools', 'inspect',
                                 reference, '--format', '{{json .Manifest}}'],
                                text=True, capture_output=True)
        if result.returncode:
            error = result.stderr.strip() or result.stdout.strip()
            # Authentication, transport and parse failures must not mean absent.
            lower = error.lower()
            named_missing = re.fullmatch(r'(?:ERROR:\s*)?' + re.escape(reference) + r': not found', error)
            manifest_missing = any(term in lower for term in ('manifest unknown', 'manifest_unknown', 'no such manifest'))
            if missing_ok and (named_missing or manifest_missing) and not any(
                    term in lower for term in ('unauthorized', 'denied', 'proxy', 'lookup ', 'dial tcp', 'tls')):
                return None
            raise RuntimeError(f'Cannot inspect {reference}: {error}')
        manifest = json.loads(result.stdout)
        require(DIGEST.fullmatch(manifest.get('digest', '')), 'Registry returned no valid digest')
        return manifest

    def promote(self, source, destination):
        subprocess.run(['docker', 'buildx', 'imagetools', 'create', '--tag', destination, source],
                       check=True)


def index_for(registry, image):
    require('@' in image, 'An immutable repository@sha256 digest is required')
    repository, digest = image.rsplit('@', 1)
    require(DIGEST.fullmatch(digest), 'Invalid candidate digest')
    require(repository and '://' not in repository and not repository.startswith('-'),
            'Invalid repository')
    require(':' not in repository.rsplit('/', 1)[-1], 'Use an untagged repository@digest')
    manifest = registry.inspect(image)
    require(manifest['digest'] == digest, 'Registry index differs from supplied candidate digest')
    require(manifest.get('mediaType') in INDEX_TYPES, 'Candidate must be a multi-platform index')
    platforms = {}
    for descriptor in manifest.get('manifests', []):
        platform = descriptor.get('platform', {})
        key = f"{platform.get('os')}/{platform.get('architecture')}"
        if key in PLATFORMS:
            require(key not in platforms, f'Duplicate runnable platform: {key}')
            require(DIGEST.fullmatch(descriptor.get('digest', '')), 'Invalid platform digest')
            platforms[key] = descriptor['digest']
    require(set(platforms) == set(PLATFORMS), 'Candidate lacks required amd64/arm64 manifests')
    return repository, manifest, platforms


def checked_verification(path, registry):
    proof = read_json(path)
    require(proof.get('schema') == 1 and proof.get('passed') is True,
            'Candidate verification is missing or failed')
    require(re.fullmatch(r'[0-9a-f]{40,64}', proof.get('commit', '')), 'Missing source commit')
    repository, manifest, platforms = index_for(registry, proof['image'])
    require(proof.get('index') == manifest, 'Verified index descriptors no longer match')
    checks = proof.get('platforms', {})
    require(set(checks) == set(PLATFORMS), 'Both platform checks are required')
    for platform, digest in platforms.items():
        check = checks[platform]
        require(check.get('passed') is True and check.get('manifest_digest') == digest,
                f'Unverified platform: {platform}')
        frames = check.get('frames', {})
        require(set(frames) == {'kitty', 'iterm2', 'fallback'}, 'All PTY modes are required')
        require(all(frame.get('exit_code') == 0 and frame.get('quit_sent') is True
                    and frame.get('passed') is True for frame in frames.values()),
                f'PTY did not exit normally: {platform}')
    return proof, repository


def native_checksums(directory):
    directory = Path(directory)
    expected = {f"they-work-{target}.{'zip' if 'windows' in target else 'tar.gz'}" for target in TARGETS}
    actual = {path.name for pattern in ('*.tar.gz', '*.zip') for path in directory.glob(pattern)}
    require(actual == expected, 'Native archive set must contain exactly the six supported targets')
    checks = {}
    for name in sorted(expected):
        archive = directory / name
        require(archive.is_file() and not archive.is_symlink(), f'Invalid archive: {name}')
        parts = (directory / (name + '.sha256')).read_text().strip().split()
        require(len(parts) == 2 and parts[1] == name and re.fullmatch(r'[0-9a-f]{64}', parts[0]),
                f'Invalid checksum file: {name}')
        require(sha256(archive) == parts[0], f'Native checksum failed: {name}')
        checks[name] = parts[0]
    return checks


def tag_digest(registry, reference):
    manifest = registry.inspect(reference, missing_ok=True)
    return manifest['digest'] if manifest else None


def prepare(registry, verification, native_dir, version, commit):
    proof, repository = checked_verification(verification, registry)
    require(proof['commit'] == commit, 'Verification belongs to a different source commit')
    require(TAG.fullmatch(version) and version != 'latest', 'Invalid version tag')
    checks = native_checksums(native_dir)
    version_ref = f'{repository}:{version}'
    require(tag_digest(registry, version_ref) is None,
            'Version already exists; recover with the original retained intent, not a new build')
    return {'schema': 1, 'commit': commit, 'candidate': proof['image'], 'version': version_ref,
            'latest': f'{repository}:latest', 'prior_latest': tag_digest(registry, f'{repository}:latest'),
            'verification_sha256': sha256(verification), 'native_checksums': checks}


def promote(registry, intent, verification, native_dir, record_path):
    record = {'schema': 1, 'candidate': intent.get('candidate'), 'completed_steps': [],
              'status': 'not-promoted', 'native_release': 'not-attempted'}
    def save():
        write_json(record_path, record)
    try:
        require(intent.get('schema') == 1, 'Unknown intent schema')
        require(sha256(verification) == intent['verification_sha256'], 'Verification artifact changed')
        proof, repository = checked_verification(verification, registry)
        require(proof['image'] == intent['candidate'] and proof['commit'] == intent['commit'],
                'Intent does not match verified candidate')
        require(native_checksums(native_dir) == intent['native_checksums'], 'Native artifacts changed')
        require(intent['latest'] == f'{repository}:latest', 'Intent points to a different repository')
        prefix = f'{repository}:'
        require(intent['version'].startswith(prefix) and TAG.fullmatch(intent['version'][len(prefix):])
                and intent['version'] != intent['latest'], 'Invalid version destination')
        desired = proof['index']['digest']
        prior = intent['prior_latest']
        require(prior is None or DIGEST.fullmatch(prior), 'Invalid prior latest digest')
        current_version = tag_digest(registry, intent['version'])
        current_latest = tag_digest(registry, intent['latest'])
        record['before'] = {'version': current_version, 'latest': current_latest}
        require(current_version in (None, desired), 'Version conflict; no tags were changed')
        require(current_latest in (prior, desired), 'Latest changed; no tags were changed')
        save()
        for name in ('version', 'latest'):
            destination = intent[name]
            current = tag_digest(registry, destination)
            allowed = (None, desired) if name == 'version' else (prior, desired)
            require(current in allowed, f'{name} changed during promotion; inspect partial state')
            if current != desired:
                try:
                    registry.promote(intent['candidate'], destination)
                except Exception as error:
                    # A registry may accept PUT and lose its response. Read back
                    # before deciding failure; never roll back a verified tag.
                    if tag_digest(registry, destination) != desired:
                        raise RuntimeError(f'{name} promotion did not complete: {error}') from error
                    record.setdefault('recovered_acknowledgements', []).append(name)
            require(tag_digest(registry, destination) == desired, f'{name} digest differs from candidate')
            record['completed_steps'].append(name)
            record['status'] = 'partially-promoted' if name == 'version' else 'images-promoted'
            save()
        record['after'] = {name: tag_digest(registry, intent[name]) for name in ('version', 'latest')}
        require(all(value == desired for value in record['after'].values()), 'Final tag verification failed')
        save()
        return record
    except Exception as error:
        record['error'] = str(error)
        record['status'] = 'promotion-failed-inspect-registry'
        for name in ('version', 'latest'):
            try:
                record.setdefault('observed_after_failure', {})[name] = tag_digest(registry, intent[name])
            except Exception as inspect_error:
                record.setdefault('observed_after_failure', {})[name] = {'unknown': str(inspect_error)}
        save()
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    first = commands.add_parser('prepare', help='Read-only checks; write immutable intent before uploading it')
    first.add_argument('--version', required=True)
    first.add_argument('--commit', required=True)
    first.add_argument('--out', type=Path, required=True)
    second = commands.add_parser('promote', help='Apply a previously retained intent; never rebuild')
    second.add_argument('--intent', type=Path, required=True)
    second.add_argument('--record', type=Path, required=True)
    for sub in (first, second):
        sub.add_argument('--verification', type=Path, required=True)
        sub.add_argument('--native-dir', type=Path, required=True)
    outcome = commands.add_parser('outcome', help='Record workflow outcome, not inferred remote atomicity')
    outcome.add_argument('--record', type=Path, required=True)
    outcome.add_argument('--native-release', choices=('success', 'failure', 'cancelled', 'skipped'), required=True)
    args = parser.parse_args()
    try:
        if args.command == 'prepare':
            intent = prepare(Registry(), args.verification, args.native_dir, args.version, args.commit)
            write_json(args.out, intent, exclusive=True)
            (args.native_dir / 'SHA256SUMS').write_text(''.join(
                f'{digest}  {name}\n' for name, digest in intent['native_checksums'].items()))
            print(f'Intent prepared at {args.out}; retain it before promoting tags')
        elif args.command == 'promote':
            print(json.dumps(promote(Registry(), read_json(args.intent), args.verification,
                                     args.native_dir, args.record)))
        else:
            record = read_json(args.record) if args.record.exists() else {'schema': 1, 'status': 'not-promoted'}
            record['native_release'] = args.native_release
            if args.native_release == 'failure':
                record['native_release_note'] = 'Publication attempt failed; inspect remote release/assets before recovery'
            write_json(args.record, record)
        return 0
    except Exception as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
