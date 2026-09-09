#!/usr/bin/env python3
"""Replay exported Ui cells and RGBA at 1:1 pixels with bundled fonts.

This is a deterministic compositor review, not a terminal transport screenshot.
It deliberately does not discover or substitute fonts from the operating system.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct

FONT_DIR = Path(__file__).resolve().parent / "audit_assets" / "fonts"


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def verify_fonts(directory=FONT_DIR):
    manifest = json.loads((directory / "manifest.json").read_text())
    for filename, expected in manifest["files"].items():
        path = directory / filename
        if not path.is_file() or sha256(path) != expected:
            raise ValueError(f"Bundled font/license checksum mismatch: {path}. Restore this file from the checkout.")
    return manifest


def glyphs(path):
    """Read Unicode cmap formats 4/12 from our checksum-verified TrueType files."""
    data = Path(path).read_bytes()
    u16 = lambda at: struct.unpack_from(">H", data, at)[0]
    u32 = lambda at: struct.unpack_from(">I", data, at)[0]
    tables = {data[12 + i * 16:16 + i * 16]: u32(20 + i * 16)
              for i in range(u16(4))}
    cmap = tables[b"cmap"]
    result = set()
    for i in range(u16(cmap + 2)):
        record = cmap + 4 + i * 8
        platform, encoding = u16(record), u16(record + 2)
        if platform != 0 and (platform, encoding) not in {(3, 1), (3, 10)}:
            continue
        at = cmap + u32(record + 4)
        form = u16(at)
        if form == 12:
            for n in range(u32(at + 12)):
                start, end, first = struct.unpack_from(">III", data, at + 16 + n * 12)
                result.update(range(start + (first == 0), end + 1))
        elif form == 4:
            count = u16(at + 6) // 2
            ends, starts = at + 14, at + 16 + count * 2
            deltas, offsets = starts + count * 2, starts + count * 4
            for n in range(count):
                delta, offset = u16(deltas + n * 2), u16(offsets + n * 2)
                for point in range(u16(starts + n * 2), u16(ends + n * 2) + 1):
                    glyph = (point + delta) & 0xFFFF
                    if offset:
                        glyph = u16(offsets + n * 2 + offset + 2 * (point - u16(starts + n * 2)))
                        if glyph:
                            glyph = (glyph + delta) & 0xFFFF
                    if glyph:
                        result.add(point)
    return result


def replay(source, destination):
    from PIL import Image, ImageDraw, ImageFont, features, __version__

    manifest = verify_fonts()
    data = json.loads(source.read_text())
    columns, rows = data["columns"], data["rows"]
    cw, ch = data["cell_width"], data["cell_height"]
    if (columns, rows, cw, ch) != (80, 24, 8, 16):
        raise ValueError("This bounded smoke requires exactly 80x24 cells at 8x16 pixels.")
    if len(data["cells"]) != rows or any(len(row) != columns for row in data["cells"]):
        raise ValueError("Native cell matrix does not match the viewport.")
    font_paths = [FONT_DIR / "DejaVuSansMono.ttf", FONT_DIR / "DejaVuSansMono-Bold.ttf"]
    coverage = [glyphs(path) for path in font_paths]
    fonts = [ImageFont.truetype(str(path), 13, layout_engine=ImageFont.Layout.BASIC)
             for path in font_paths]
    for row in data["cells"]:
        for cell in row:
            if cell["native"]:
                missing = {ord(c) for c in cell["text"]} - coverage[bool(cell["bold"])]
                if missing:
                    raise ValueError("Bundled font lacks native glyphs: " + ", ".join(f"U+{c:04X}" for c in sorted(missing)))
    image = Image.new("RGB", (columns * cw, rows * ch))
    draw = ImageDraw.Draw(image)
    for y, row in enumerate(data["cells"]):
        for x, cell in enumerate(row):
            draw.rectangle((x * cw, y * ch, (x + 1) * cw - 1, (y + 1) * ch - 1), fill=cell["bg"])
    box = data["image"]
    if box is None:
        raise ValueError("Expected the full image-mode tower composition, but the exporter returned no image.")
    x, y = box["x"] * cw, box["y"] * ch
    width, height = box["pixel_width"], box["pixel_height"]
    if min(x, y, width, height) < 0 or width == 0 or height == 0 or x + width > image.width or y + height > image.height:
        raise ValueError("Image rectangle exceeds the full terminal viewport.")
    raw = source.with_name(source.name.removesuffix(".cells.json") + ".rgba").read_bytes()
    if len(raw) != width * height * 4:
        raise ValueError("RGBA byte length does not match exported dimensions.")
    art = Image.frombytes("RGBA", (width, height), raw)
    image.paste(art, (x, y), art)
    draw = ImageDraw.Draw(image)
    # First restore every opaque native cell, then paint text. This preserves
    # glyph overhangs while ensuring native panel backgrounds mask the artwork.
    for y, row in enumerate(data["cells"]):
        for x, cell in enumerate(row):
            if cell["native"]:
                draw.rectangle((x * cw, y * ch, (x + 1) * cw - 1, (y + 1) * ch - 1), fill=cell["bg"])
    for y, row in enumerate(data["cells"]):
        for x, cell in enumerate(row):
            if cell["native"]:
                draw.text((x * cw, y * ch - 1), cell["text"], font=fonts[bool(cell["bold"])], fill=cell["fg"])
    image.save(destination, optimize=False)
    return {"width": image.width, "height": image.height, "sha256": sha256(destination),
            "rgb_sha256": hashlib.sha256(image.tobytes()).hexdigest(), "zlib": features.version("zlib"),
            "pillow": __version__, "freetype": features.version("freetype2"),
            "layout_engine": "BASIC", "font_size": 13, "fonts": manifest,
            "native_cells": sum(bool(cell["native"]) for row in data["cells"] for cell in row),
            "visual_review": "not-tested", "terminal_transport": "not-tested"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    print(json.dumps(replay(args.source, args.destination), indent=2))


if __name__ == "__main__":
    main()
