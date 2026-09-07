#!/usr/bin/env python3
"""Smoke-test and archive the executable for one already-built native target."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tarfile
import zipfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', required=True)
    parser.add_argument('--binary', type=Path, help='Override the already-built executable path')
    parser.add_argument('--out-dir', type=Path, help='Override the distribution output directory')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    windows = 'windows' in args.target
    name = 'they-work.exe' if windows else 'they-work'
    executable = (args.binary or root / 'target' / args.target / 'release' / name).resolve()
    subprocess.run([str(executable), '--help'], check=True)
    subprocess.run([str(executable), '--demo', '--once'], check=True)
    subprocess.run([str(executable), '--demo', '--headless', '--exit-after', '100ms'], check=True)
    dist = args.out_dir or root / 'target' / 'dist'
    dist.mkdir(parents=True, exist_ok=True)
    filename = f"they-work-{args.target}.{'zip' if windows else 'tar.gz'}"
    output = dist / filename
    if windows:
        with zipfile.ZipFile(output, 'w', zipfile.ZIP_DEFLATED) as archive:
            archive.write(executable, arcname=name)
            archive.write(root / 'LICENSE', arcname='LICENSE')
    else:
        with tarfile.open(output, 'w:gz') as archive:
            archive.add(executable, arcname=name)
            archive.add(root / 'LICENSE', arcname='LICENSE')
    digest = hashlib.sha256(output.read_bytes()).hexdigest()
    (dist / f'{filename}.sha256').write_text(f'{digest}  {filename}\n')
    print(f'Packaged {filename}: {output.stat().st_size} bytes; SHA256 {digest}')


if __name__ == '__main__':
    main()
