#!/usr/bin/env python3
"""Rehearse checksum, extraction and execution from both completed preview archives.

Uses the local immutable image IDs recorded by preview.py. No provider folders,
network access or writable host directories enter these runtime containers.
"""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parents[3]
OUTPUT = ROOT / 'target/audit/iteration-11'
DIST = OUTPUT / 'dist'
SCRIPT = '''
set -eu
uname -m
cat /etc/os-release
cd /dist
sha256sum -c "$1.tar.gz.sha256"
mkdir /tmp/preview
tar -xzf "$1.tar.gz" -C /tmp/preview
cd /tmp/preview
./they-work --help
./they-work --demo --no-save --once
./they-work --demo --no-save --headless --exit-after 100ms
test ! -e /nonexistent/they-work
'''


def main():
    manifest = json.loads((DIST / 'PREVIEW.json').read_text())
    if not manifest.get('passed') or set(manifest['platforms']) != {'amd64', 'arm64'}:
        raise RuntimeError('Both preview builds and smoke tests must pass first')
    docker = shutil.which('docker') or '/usr/local/bin/docker'
    folder = OUTPUT / 'archive-extraction'
    folder.mkdir(exist_ok=True)
    report = {'source_revision': manifest['source_revision'], 'passed': False,
              'evidence_kind': 'container execution of extracted archives; not an actual WSL terminal',
              'provider_stores_mounted': False, 'network': 'none',
              'root_filesystem': 'read-only', 'extraction': 'isolated tmpfs', 'platforms': {}}
    for architecture, candidate in manifest['platforms'].items():
        archive = DIST / candidate['archive']['name']
        if hashlib.sha256(archive.read_bytes()).hexdigest() != candidate['archive']['sha256']:
            raise RuntimeError('Archive no longer matches the verified preview manifest')
        command = [docker, 'run', '--rm', '--platform', f'linux/{architecture}',
                   '--network', 'none', '--read-only', '--cap-drop', 'ALL',
                   '--security-opt', 'no-new-privileges',
                   '--tmpfs', '/tmp:rw,exec,nosuid,nodev,size=64m,mode=1777',
                   '--mount', f'type=bind,source={DIST},target=/dist,readonly',
                   '--entrypoint', '/bin/sh', candidate['image_id'], '-ec', SCRIPT,
                   'preview-extraction', archive.name.removesuffix('.tar.gz')]
        start = time.monotonic()
        result = subprocess.run(command, capture_output=True, timeout=60)
        log = result.stdout + result.stderr
        (folder / f'{architecture}.log').write_bytes(log)
        report['platforms'][architecture] = {
            'passed': result.returncode == 0, 'exit_code': result.returncode,
            'execution': candidate['execution'], 'archive': candidate['archive'],
            'elapsed_seconds': round(time.monotonic() - start, 3),
            'image_id': candidate['image_id'], 'log_sha256': hashlib.sha256(log).hexdigest()}
        (folder / 'results.json').write_text(json.dumps(report, indent=2) + '\n')
        if result.returncode:
            raise RuntimeError(f'Extraction rehearsal failed; see {folder / (architecture + ".log")}')
    report['passed'] = True
    (folder / 'results.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
