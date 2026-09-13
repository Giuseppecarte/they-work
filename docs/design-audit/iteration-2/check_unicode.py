#!/usr/bin/env python3
"""Check emitted-map semantics using Python's Unicode character names as an oracle."""
import json
import importlib.util
from pathlib import Path
import re
import subprocess
import sys
import unicodedata

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent / 'evidence' / 'unicode-mapping.json'
CURRENT = ROOT / 'crates/theywork-render/src/canvas.rs'

def canonical_mask(glyph):
    if glyph == ' ': return 0
    if glyph == '█': return 63
    if glyph == '▌': return 21
    if glyph == '▐': return 42
    name = unicodedata.name(glyph)
    return sum(1 << (int(digit) - 1) for digit in name.removeprefix('BLOCK SEXTANT-'))

def mapped_glyphs(source):
    sextants = source.split('fn sextant_glyph(mask: u8)')[1].split('fn shade_char')[0]
    table = re.search(r'const MASKS: \[u8; 60\] = \[(.*?)\];', sextants, re.S)[1]
    masks = [int(n) for n in re.findall(r'\d+', table)]
    result = {0: ' ', 63: '█', **{mask: chr(0x1fb00 + i) for i, mask in enumerate(masks)}}
    for mask, glyph in [(21, '▌'), (42, '▐')]:
        if f"mask == {mask}" in sextants and f"Some('{glyph}')" in sextants:
            result[mask] = glyph
    return result

def check(source):
    mappings = mapped_glyphs(source)
    mismatches = {str(mask): unicodedata.name(glyph) for mask, glyph in mappings.items() if mask != canonical_mask(glyph)}
    loss = []
    for wanted in range(64):
        candidates = []
        for fg in (1, 0):
            for bg in (1, 0):
                for mask, glyph in mappings.items():
                    actual = canonical_mask(glyph)
                    reconstructed = sum((fg if actual & (1 << bit) else bg) << bit for bit in range(6))
                    changed = (wanted ^ reconstructed).bit_count()
                    candidates.append(((changed, -mask.bit_count(), mask), glyph, fg, bg, reconstructed))
        _, glyph, fg, bg, reconstructed = min(candidates, key=lambda x: x[0])
        if wanted != reconstructed:
            loss.append({'input_mask': wanted, 'glyph': glyph, 'unicode_name': unicodedata.name(glyph), 'foreground': fg, 'background': bg, 'reconstructed_mask': reconstructed, 'wrong_subpixels': (wanted ^ reconstructed).bit_count()})
    return {'mapped_patterns': len(mappings), 'wrong_unicode_mappings': mismatches, 'two_colour_patterns_with_loss': loss}

revision = sys.argv[1] if len(sys.argv) > 1 else '8331017071f5790cad51f04936213de096a833f0'
baseline = subprocess.check_output(['git', 'show', revision + ':crates/theywork-render/src/canvas.rs'], cwd=ROOT, text=True)
result = {'unicode_version': unicodedata.unidata_version, 'baseline_commit': subprocess.check_output(['git', 'rev-parse', revision], cwd=ROOT, text=True).strip(), 'baseline': check(baseline), 'current': check(CURRENT.read_text()), 'method': 'Canonical masks come from Unicode names; colour enumeration mirrors the documented encoder tie-break, while Rust tests independently exercise its real implementation.'}
assert not result['current']['wrong_unicode_mappings']
assert not result['current']['two_colour_patterns_with_loss']
assert result['current']['mapped_patterns'] == 64
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('shot', ROOT / 'scripts/shot.py')
shot = importlib.util.module_from_spec(spec)
sys.modules['shot'] = shot
spec.loader.exec_module(shot)
for expected, glyph in mapped_glyphs(CURRENT.read_text()).items():
    pattern, columns, rows = shot.block_pattern(glyph)
    actual = sum((bool(pattern & (1 << ((bit // 2 * rows // 3) * columns + bit % 2)))) << bit for bit in range(6))
    assert actual == canonical_mask(glyph), (glyph, actual, expected)
assert shot.block_pattern('▄') == (12, 2, 2)
assert shot.block_pattern('▀') == (3, 2, 2)
result['exporter_patterns_verified'] = 66
OUT.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
print(json.dumps(result, ensure_ascii=False, indent=2))
