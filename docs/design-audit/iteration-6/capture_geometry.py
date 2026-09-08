#!/usr/bin/env python3
"""Capture the nine supplementary 10x20 UI cases with explicit provenance."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
from datetime import datetime, timezone

repo = Path(__file__).resolve().parents[3]
os.chdir(repo)
raw = repo / 'docs/design-audit/tmp/iteration-6-geometry-10x20'
out = repo / 'docs/design-audit/iteration-6/evidence/geometry-10x20'
raw.mkdir(parents=True, exist_ok=True)
out.mkdir(parents=True, exist_ok=True)
env = os.environ.copy()
for key in ('NO_COLOR', 'THEYWORK_COLOR', 'THEYWORK_PIXELS'):
    env.pop(key, None)
env.update(TERM='xterm-256color', COLORTERM='truecolor')
binary = repo / 'target/native-macos/release/examples/ui_inventory'
cases = []
for surface in ('tower', 'office-auto', 'inspector-now'):
    for size in ('80x24', '120x36', '192x58'):
        name = f'{surface}-image-{size}'
        subprocess.run([str(binary), str(raw), name, '10x20'], env=env,
                       check=True, stdout=subprocess.DEVNULL)
        cases.extend(json.loads((raw / 'inventory.json').read_text())['cases'])
(raw / 'inventory.json').write_text(json.dumps({'cases': cases}, indent=2) + '\n')
subprocess.run([sys.executable, 'docs/design-audit/iteration-6/export_inventory.py',
                str(raw), str(out)], check=True)
metadata = {
    'kind': 'Actual Ui compositor and native cells, not a terminal-window screenshot',
    'cell_pixels': [10, 20], 'cases': len(cases),
    'surfaces': ['tower', 'office-auto', 'inspector-now'],
    'sizes': ['80x24', '120x36', '192x58'],
    'generated_at_utc': datetime.now(timezone.utc).isoformat(),
    'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
    'built_they_work_sha256': hashlib.sha256(
        (repo / 'target/native-macos/release/they-work').read_bytes()).hexdigest(),
    'command': 'PYTHONPATH=docs/design-audit/tmp/python python3 docs/design-audit/iteration-6/capture_geometry.py',
}
(out / 'generation.json').write_text(json.dumps(metadata, indent=2) + '\n')
print(f'Captured {len(cases)} geometry cases; visual verdicts remain separate.')
