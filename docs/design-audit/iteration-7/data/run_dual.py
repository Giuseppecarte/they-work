#!/usr/bin/env python3
"""Add three dual-feed cases without modifying the frozen eight-case evidence."""
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


env = os.environ.copy()
bundled = REPO / 'docs/design-audit/tmp/native-rust'
if not shutil.which('cargo') and (bundled / 'cargo-home/bin/cargo').exists():
    env.update(CARGO_HOME=str(bundled / 'cargo-home'), RUSTUP_HOME=str(bundled / 'rustup-home'))
    env['PATH'] = str(bundled / 'cargo-home/bin') + os.pathsep + env.get('PATH', '')
env.setdefault('CARGO_TARGET_DIR', str(REPO / 'target/native-macos'))
out = HERE / 'evidence/dual-feed'
out.mkdir(parents=True, exist_ok=True)
scratch = REPO / 'docs/design-audit/tmp/iteration-7-data' / ('dual-' + datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S'))
scratch.mkdir(parents=True, exist_ok=False)
frozen = {p.name: sha(p) for p in (HERE / 'evidence').iterdir() if p.is_file()}
with (out / 'build.log').open('w') as log:
    subprocess.run(['cargo', 'build', '--offline', '--locked', '--manifest-path', str(HERE / 'Cargo.toml'), '--bin', 'dual_feed'],
                   cwd=REPO, env=env, check=True, stdout=log, stderr=subprocess.STDOUT)
binary = Path(env['CARGO_TARGET_DIR']) / 'debug/dual_feed'
with (out / 'run.log').open('w') as log:
    subprocess.run([str(binary), str(scratch), str(out)], cwd=REPO, env=env, check=True, stdout=log, stderr=subprocess.STDOUT)
assert frozen == {p.name: sha(p) for p in (HERE / 'evidence').iterdir() if p.is_file()}, 'Prior evidence changed'
sources = ['crates/theywork-collect/src/codex_source.rs', 'crates/theywork-collect/src/util.rs',
           'crates/theywork-control/src/bridge.rs', 'crates/theywork-core/src/world.rs',
           'crates/theywork-tui/src/main.rs', 'crates/theywork-render/src/views/workboard.rs']
metadata = {
    'generated_at_utc': datetime.now(timezone.utc).isoformat(),
    'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPO, text=True).strip(),
    'binary_sha256': sha(binary),
    'runner_sha256': sha(HERE / 'src/bin/dual_feed.rs'),
    'source_sha256': {name: sha(REPO / name) for name in sources},
    'evidence_sha256': {p.name: sha(p) for p in out.iterdir() if p.is_file() and p.name != 'metadata.json'},
    'prior_eight_case_evidence_unchanged': True,
    'scratch': str(scratch.relative_to(REPO)),
    'platform': platform.system() + ' ' + platform.machine(),
    'measurement_limit': 'Concurrent with a lightweight soak/other history probes; no timing claim.',
}
(out / 'metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
print((out / 'run.log').read_text(), end='')
