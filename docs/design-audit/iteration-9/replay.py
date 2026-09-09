#!/usr/bin/env python3
"""Replay the bounded iteration-9 exports; this does not capture terminal paint."""
import hashlib
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'scripts'))
from audit_replay import glyphs, verify_fonts, FONT_DIR
from PIL import Image, ImageDraw, ImageFont, features, __version__


def replay(source, destination):
    data = json.loads(source.read_text())
    cols, rows = data['columns'], data['rows']
    cw, ch = data['cell_width'], data['cell_height']
    if (cols, rows) not in [(32, 14), (80, 24), (120, 36), (192, 58), (110, 80), (240, 70)] or (cw, ch) not in [(8, 16), (10, 20)]:
        raise ValueError('Outside the bounded iteration-9 capture matrix')
    if len(data['cells']) != rows or any(len(row) != cols for row in data['cells']):
        raise ValueError('Malformed cell matrix')
    fonts_manifest = verify_fonts()
    paths = [FONT_DIR / 'DejaVuSansMono.ttf', FONT_DIR / 'DejaVuSansMono-Bold.ttf']
    coverage = [glyphs(path) for path in paths]
    size = 13 if cw == 8 else 16
    fonts = [ImageFont.truetype(str(path), size, layout_engine=ImageFont.Layout.BASIC) for path in paths]
    missing = set()
    for row in data['cells']:
        for cell in row:
            if cell['native']:
                missing.update({ord(c) for c in cell['text']} - coverage[bool(cell['bold'])])
    if missing:
        raise ValueError('Bundled font lacks glyphs: ' + ', '.join(f'U+{c:04X}' for c in sorted(missing)))
    image = Image.new('RGB', (cols * cw, rows * ch))
    draw = ImageDraw.Draw(image)
    for y, row in enumerate(data['cells']):
        for x, cell in enumerate(row):
            draw.rectangle((x*cw, y*ch, (x+1)*cw-1, (y+1)*ch-1), fill=cell['bg'])
    box = data['image']
    if box is not None:
        x, y, width, height = box['x'] * cw, box['y'] * ch, box['pixel_width'], box['pixel_height']
        if min(x, y, width, height) < 0 or not width or not height or x+width > image.width or y+height > image.height:
            raise ValueError('Image outside viewport')
        raw = source.with_name(source.name.removesuffix('.cells.json') + '.rgba').read_bytes()
        if len(raw) != width * height * 4:
            raise ValueError('Malformed RGBA')
        art = Image.frombytes('RGBA', (width, height), raw)
        image.paste(art, (x, y), art)
    draw = ImageDraw.Draw(image)
    for y, row in enumerate(data['cells']):
        for x, cell in enumerate(row):
            if cell['native']:
                draw.rectangle((x*cw, y*ch, (x+1)*cw-1, (y+1)*ch-1), fill=cell['bg'])
    for y, row in enumerate(data['cells']):
        for x, cell in enumerate(row):
            if cell['native']:
                draw.text((x*cw, y*ch-1), cell['text'], font=fonts[bool(cell['bold'])], fill=cell['fg'])
    image.save(destination, optimize=False)
    return {'source': source.name, 'png': destination.name,
            'sha256': hashlib.sha256(destination.read_bytes()).hexdigest(),
            'rgb_sha256': hashlib.sha256(image.tobytes()).hexdigest(),
            'width': image.width, 'height': image.height, 'pillow': __version__,
            'freetype': features.version('freetype2'), 'font_size': size,
            'fonts': fonts_manifest, 'visual_review': 'not-tested', 'terminal_paint': 'not-tested'}


if __name__ == '__main__':
    source_dir = ROOT / 'target/audit/iteration-9/captures'
    output = ROOT / 'target/audit/iteration-9/replayed'
    output.mkdir(parents=True, exist_ok=True)
    requested = sys.argv[1:]
    if not requested:
        raise SystemExit('Provide one or more capture IDs from coverage-manifest.json')
    known = {case['id'] for case in json.loads((source_dir / 'coverage-manifest.json').read_text())['cases']}
    if any(name not in known for name in requested):
        raise SystemExit('Unknown capture ID; no replay performed')
    results = [replay(source_dir / f'{name}.cells.json', output / f'{name}.png') for name in requested]
    (output / 'replay-manifest.json').write_text(json.dumps(results, indent=2) + '\n')
    print(f'Replayed {len(results)} complete surfaces with bundled fonts in {output}')
