#!/usr/bin/env python3
"""Replay release installation with the actual locally built macOS archive.

The curl fixture substitutes a local archive for unpublished GitHub assets.
This checks extraction, checksum verification, installation, and launch; it does
not claim that a native release has been published or downloaded successfully.
"""
import os
from pathlib import Path
import subprocess
import tempfile


root = Path(__file__).resolve().parents[2]
dist = root / 'docs/design-audit/tmp/native-dist'
asset = dist / 'they-work-aarch64-apple-darwin.tar.gz'
checksums = asset.with_name(asset.name + '.sha256')
with tempfile.TemporaryDirectory(dir=root / 'docs/design-audit/tmp') as temporary:
    work = Path(temporary)
    tools = work / 'tools'
    tools.mkdir()
    adapter = tools / 'curl'
    adapter.write_text('''#!/usr/bin/env python3
import os, pathlib, shutil, sys
out = pathlib.Path(sys.argv[sys.argv.index('-o') + 1])
source = os.environ['AUDIT_CHECKSUMS'] if out.name == 'SHA256SUMS' else os.environ['AUDIT_ASSET']
shutil.copyfile(source, out)
''')
    adapter.chmod(0o755)
    destination = work / 'installed with spaces'
    env = dict(os.environ, PATH=f"{tools}:{os.environ['PATH']}", TMPDIR=str(work),
               AUDIT_ASSET=str(asset), AUDIT_CHECKSUMS=str(checksums))
    subprocess.run(['sh', str(root / 'scripts/install.sh'), '--install-dir',
                    str(destination)], env=env, check=True)
    subprocess.run([str(destination / 'they-work'), '--demo', '--once'], check=True)
    subprocess.run([str(destination / 'they-work'), '--demo', '--headless',
                    '--exit-after', '100ms'], check=True)
    print('PASS: actual native archive verified, installed, and launched; GitHub download substituted locally.')
