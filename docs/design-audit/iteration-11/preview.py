#!/usr/bin/env python3
"""Build and verify local static Linux previews from one committed source tree.

This helper never pushes images or creates releases. Build and PTY logs go to
ignored target/audit/iteration-11; only an explicit --run starts Docker work.
Requires Python 3.12+, Docker, and network access for build dependencies.
"""

import argparse
import gzip
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import platform
import shutil
import struct
import subprocess
import sys
import tarfile
import tempfile
import time


ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
OUTPUT = ROOT / 'target/audit/iteration-11'
TARGETS = {'amd64': ('x86_64-unknown-linux-musl', 62),
           'arm64': ('aarch64-unknown-linux-musl', 183)}


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + '\n')


def command(arguments, log=None, **kwargs):
    if log is not None:
        log.parent.mkdir(parents=True, exist_ok=True)
        with log.open('w') as stream:
            stream.write('$ ' + ' '.join(arguments) + '\n')
            stream.flush()
            result = subprocess.run(arguments, cwd=ROOT, stdout=stream,
                                    stderr=subprocess.STDOUT, **kwargs)
        require(result.returncode == 0, f'Command failed; see {log}')
        return result
    return subprocess.run(arguments, cwd=ROOT, check=True, capture_output=True, **kwargs)


def elf_identity(path, architecture):
    data = path.read_bytes()
    require(len(data) >= 64 and data[:6] == b'\x7fELF\x02\x01',
            'Expected a 64-bit little-endian ELF executable')
    kind, machine = struct.unpack_from('<HH', data, 16)
    require(kind in (2, 3), 'ELF is not an executable or PIE')
    require(machine == TARGETS[architecture][1], 'ELF architecture mismatch')
    offset = struct.unpack_from('<Q', data, 32)[0]
    size, count = struct.unpack_from('<HH', data, 54)
    require(size >= 56 and offset + size * count <= len(data), 'Invalid ELF headers')
    interpreter = False
    needed = False
    for index in range(count):
        header = offset + index * size
        segment = struct.unpack_from('<I', data, header)[0]
        interpreter |= segment == 3
        if segment == 2:
            start = struct.unpack_from('<Q', data, header + 8)[0]
            length = struct.unpack_from('<Q', data, header + 32)[0]
            require(start + length <= len(data), 'Invalid ELF dynamic segment')
            for position in range(start, start + length - 15, 16):
                tag = struct.unpack_from('<Q', data, position)[0]
                needed |= tag == 1
                if tag == 0:
                    break
    require(not interpreter and not needed, 'Preview must be statically linked')
    return {'machine': machine, 'architecture': architecture, 'elf_type': kind,
            'interpreter': interpreter, 'needed_shared_libraries': needed,
            'bytes': len(data), 'sha256': sha(path)}


def archive(output, entries):
    with output.open('wb') as raw:
        with gzip.GzipFile(fileobj=raw, mode='wb', filename='', mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode='w') as package:
                for name, (data, mode) in sorted(entries.items()):
                    item = tarfile.TarInfo(name)
                    item.size = len(data)
                    item.mode = mode
                    item.mtime = 0
                    package.addfile(item, io.BytesIO(data))


def prepare(commit, build_root):
    require(not build_root.exists(), f'Refusing to overwrite retained evidence: {build_root}')
    context = build_root / 'source'
    context.mkdir(parents=True)
    source = command(['git', 'archive', commit, 'Cargo.toml', 'Cargo.lock',
                      'rust-toolchain.toml', 'crates', 'LICENSE']).stdout
    with tarfile.open(fileobj=io.BytesIO(source)) as package:
        package.extractall(context, filter='data')
    shutil.copy2(HERE / 'preview.Dockerfile', context / 'Dockerfile')
    return context


def runtime_smoke(docker, image, architecture, logs):
    invocation = [docker, 'run', '--rm', '--platform', f'linux/{architecture}',
                  '--network', 'none', '--read-only', '--cap-drop', 'ALL',
                  '--security-opt', 'no-new-privileges', image]
    cases = (('help', ['--help']), ('once', ['--demo', '--no-save', '--once']),
             ('headless', ['--demo', '--no-save', '--headless', '--exit-after', '100ms']))
    results = []
    for name, arguments in cases:
        start = time.monotonic()
        command([*invocation, *arguments], log=logs / f'{name}.log', timeout=45)
        results.append({'name': name, 'exit_code': 0,
                        'elapsed_seconds': round(time.monotonic() - start, 3)})
    return results


def pty_smoke(image, architecture, logs):
    # Reuse the bounded release PTY contract without its registry lookup/pull.
    sys.path.insert(0, str(ROOT / 'scripts'))
    spec = importlib.util.spec_from_file_location('preview_pty', ROOT / 'scripts/test-published-image.py')
    verifier = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(verifier)
    results = {}
    cases = (('kitty', verifier.KITTY_REPLY, b'\x1b_Ga=T,', ()),
             ('iterm2', verifier.ITERM_REPLY, b'\x1b]1337;File=', (('TERM_PROGRAM', 'iTerm.app'),)),
             ('sixel', b'\x1b[?1;2;4c\x1b[6;16;8t\x1b[8;48;160t', b'\x1bPq', ()),
             ('fallback', None, None, ()))
    for name, reply, marker, environment in cases:
        frame, result = verifier.terminal_frame(image, f'linux/{architecture}',
                                               reply, marker, environment, 45)
        (logs / f'{name}.pty').write_bytes(frame)
        errors = verifier.check_frame(frame, result, marker)
        result.update({'passed': not errors, 'errors': errors,
                       'frame_sha256': hashlib.sha256(frame).hexdigest(),
                       'frame_bytes': len(frame),
                       'mouse_released': b'\x1b[?1000l' in frame,
                       'alternate_screen_left': b'\x1b[?1049l' in frame})
        require(result['mouse_released'] and result['alternate_screen_left'],
                f'{name}: terminal was not restored')
        results[name] = result
        write_json(logs / 'pty-results.json', results)
        require(not errors, f'{name}: {errors}')
    return results


def build_platform(docker, commit, context, architecture, daemon, build_root):
    target, _ = TARGETS[architecture]
    logs = build_root / architecture
    logs.mkdir()
    image = f'they-work-preview:iteration11-{commit[:12]}-{architecture}'
    command([docker, 'buildx', 'build', '--load', '--platform', f'linux/{architecture}',
             '--progress', 'plain', '--label', f'org.opencontainers.image.revision={commit}',
             '-t', image, str(context)], log=logs / 'build.log', timeout=3600)
    inspection = json.loads(command([docker, 'image', 'inspect', image], text=True).stdout)[0]
    require(inspection['Architecture'] == architecture, 'Image architecture mismatch')
    identifier = command([docker, 'create', '--platform', f'linux/{architecture}', image], text=True).stdout.strip()
    binary = logs / 'they-work'
    try:
        command([docker, 'cp', f'{identifier}:/usr/local/bin/they-work', str(binary)])
        command([docker, 'cp', f'{identifier}:/toolchain.txt', str(logs / 'toolchain.txt')])
    finally:
        command([docker, 'rm', identifier])
    identity = elf_identity(binary, architecture)
    report = {'target': target, 'source_revision': commit, 'binary': identity,
              'image_id': inspection['Id'], 'image': image,
              'execution': 'daemon-native' if architecture == daemon else 'emulated',
              'toolchain_sha256': sha(logs / 'toolchain.txt'),
              'provider_stores_mounted': False, 'runtime_network': 'none',
              'runtime_filesystem': 'read-only',
              'pty_geometry': {'columns': 160, 'rows': 48, 'cell_pixels': [8, 16]},
              'real_terminal': 'not tested', 'wsl': 'not tested'}
    report['process_smoke'] = runtime_smoke(docker, image, architecture, logs)
    report['pty_smoke'] = pty_smoke(image, architecture, logs)
    report['passed'] = True
    write_json(logs / 'result.json', report)
    return binary, report


def documentation_entries():
    guide = (HERE / 'WSL-PREVIEW.md').read_text()
    guide = guide.replace('(../../../INSTALL.md)', '(INSTALL.md)')
    guide = guide.replace('(../../CONTROLS.md)', '(docs/CONTROLS.md)')
    documents = {'WSL-PREVIEW.md': guide.encode(),
                 'WSL-SESSION.md': (HERE / 'WSL-SESSION.md').read_bytes(),
                 'vscode-settings.json': (HERE / 'vscode-settings.json').read_bytes(),
                 'INSTALL.md': (ROOT / 'INSTALL.md').read_bytes(),
                 'docs/CONTROLS.md': (ROOT / 'docs/CONTROLS.md').read_bytes()}
    return {name: (data, 0o644) for name, data in documents.items()}


def package_platform(binary, result, context, dist):
    # Package only bytes that already passed execution. Repack may update the
    # included guide, but cannot silently change the executable or architecture.
    result = {key: value for key, value in result.items() if key != 'archive'}
    target = result['target']
    architecture = result['binary']['architecture']
    require(elf_identity(binary, architecture) == result['binary'], 'Executed binary changed')
    require(result['passed'], 'Cannot package a failed preview')
    name = f'they-work-{target}.tar.gz'
    destination = dist / name
    require(not destination.exists(), f'Refusing to replace existing archive: {destination}')
    documents = documentation_entries()
    result['documentation_sha256'] = {name: hashlib.sha256(data).hexdigest()
                                      for name, (data, _) in documents.items()}
    readme = (f'They-work iteration 11 local preview\nSource: {result["source_revision"]}\nTarget: {target}\n\n'
              'Run inside Ubuntu WSL, not PowerShell. No Rust or Docker required.\n'
              './they-work --doctor\n./they-work --demo --no-save\n'
              'If NO_COLOR is inherited: env -u NO_COLOR ./they-work --demo --no-save\n'
              'Press q from the main view to quit. Images depend on terminal support.\n'
              'VS Code: opt in to terminal.integrated.enableImages in User Settings.\n'
              'See WSL-PREVIEW.md for the complete guide and WSL-SESSION.md for your observations.\n')
    archive(destination, {**documents, 'they-work': (binary.read_bytes(), 0o755),
                          'LICENSE': ((context / 'LICENSE').read_bytes(), 0o644),
                          'README.txt': (readme.encode(), 0o644),
                          'PREVIEW.json': ((json.dumps(result, indent=2) + '\n').encode(), 0o644)})
    with tarfile.open(destination) as package:
        extracted = package.extractfile('they-work').read()
    require(hashlib.sha256(extracted).hexdigest() == result['binary']['sha256'],
            'Packaged executable differs from the executed binary')
    checksum = f'{sha(destination)}  {name}\n'
    (dist / f'{name}.sha256').write_text(checksum)
    result['archive'] = {'name': name, 'sha256': sha(destination), 'bytes': destination.stat().st_size}
    return result, checksum


def repack(commit, build_root):
    report = json.loads((build_root / 'report.json').read_text())
    require(report['passed'] and report['source_revision'] == commit,
            'Repack requires a complete passing build of the specified commit')
    require(set(report['platforms']) == set(TARGETS), 'Both platforms must have passed')
    dist = OUTPUT / 'dist'
    dist.mkdir(exist_ok=True)
    sums = []
    for architecture, result in report['platforms'].items():
        result, checksum = package_platform(build_root / architecture / 'they-work', result,
                                            build_root / 'source', dist)
        report['platforms'][architecture] = result
        sums.append(checksum)
    report['packaging_helper_sha256'] = sha(Path(__file__))
    report['packaging_note'] = 'Documentation bundle repaired; executed binaries unchanged'
    (dist / 'SHA256SUMS').write_text(''.join(sums))
    write_json(dist / 'PREVIEW.json', report)
    write_json(build_root / 'repack-report.json', report)
    print(json.dumps(report, indent=2))
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--commit', required=True, help='Committed source revision, never the working tree')
    action = parser.add_mutually_exclusive_group()
    action.add_argument('--run', action='store_true', help='Actually build and execute both Linux platforms')
    action.add_argument('--repack', action='store_true', help='Package passing --output binaries without rebuilding')
    parser.add_argument('--output', type=Path, default=OUTPUT / 'preview-build')
    args = parser.parse_args()
    require(sys.version_info >= (3, 12), 'Python 3.12 or newer is required')
    commit = command(['git', 'rev-parse', '--verify', f'{args.commit}^{{commit}}'], text=True).stdout.strip()
    build_root = args.output.resolve()
    require(build_root.is_relative_to(OUTPUT), 'Output must stay under ignored target/audit/iteration-11')
    if args.repack:
        return repack(commit, build_root)
    if not args.run:
        print(f'Prepared plan: frozen {commit}; static amd64 and arm64 builds, then process/PTY smoke.')
        print('Add --run to build locally. Nothing is pushed or published.')
        return 0
    docker = shutil.which('docker') or '/usr/local/bin/docker'
    os.environ['PATH'] = str(Path(docker).parent) + os.pathsep + os.environ.get('PATH', '')
    context = prepare(commit, build_root)
    temporary = build_root / 'tmp'
    temporary.mkdir()
    tempfile.tempdir = str(temporary)
    daemon = command([docker, 'info', '--format', '{{.Architecture}}'], text=True).stdout.strip()
    daemon = {'aarch64': 'arm64', 'x86_64': 'amd64'}.get(daemon, daemon)
    report = {'schema': 1, 'passed': False, 'source_revision': commit,
              'source_mode': 'git archive; working-tree edits excluded',
              'driver': platform.platform(), 'daemon_architecture': daemon,
              'docker': command([docker, 'version', '--format', '{{json .}}'], text=True).stdout.strip(),
              'recipe_sha256': sha(HERE / 'preview.Dockerfile'),
              'helper_sha256': sha(Path(__file__)),
              'pty_helper_sha256': sha(ROOT / 'scripts/test-published-image.py'),
              'platforms': {}, 'evidence_kind': 'container execution and simulated PTY, not real terminal',
              'physical_wsl_validation': 'not tested'}
    dist = OUTPUT / 'dist'
    dist.mkdir(parents=True, exist_ok=True)
    sums = []
    try:
        for architecture in TARGETS:
            binary, result = build_platform(docker, commit, context, architecture, daemon, build_root)
            result, checksum = package_platform(binary, result, context, dist)
            sums.append(checksum)
            report['platforms'][architecture] = result
            write_json(build_root / 'report.json', report)
        report['passed'] = True
        (dist / 'SHA256SUMS').write_text(''.join(sums))
        write_json(dist / 'PREVIEW.json', report)
    except Exception as error:
        report['error'] = str(error)
        raise
    finally:
        write_json(build_root / 'report.json', report)
    print(json.dumps(report, indent=2))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
