#!/usr/bin/env python3
"""Export deterministic frames as SVGs and PNGs rasterized from those SVGs."""

from __future__ import annotations

import argparse
import binascii
import html
import os
import re
import shutil
import struct
import subprocess
import tempfile
import zlib
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
GOLDEN_DIR = ROOT / "crates" / "theywork-render" / "tests" / "goldens"
DARK_FOREGROUND = "#e8e2d6"
DARK_BACKGROUND = "#0d0b14"
LIGHT_FOREGROUND = "#3a352c"
LIGHT_BACKGROUND = "#f4efe4"
REFERENCE_DIR = ROOT / "docs" / "references"
REFERENCE_EXTENSIONS = (".png", ".jpg", ".jpeg", ".webp", ".svg")
SVG_RASTERIZER_ENV = "THEYWORK_SVG_RASTERIZER"
CELL_WIDTH = 12
CELL_HEIGHT = 22
FRAME_PADDING = 4
# Headless Chrome reserves part of --window-size for its outer window. Extra
# height gives the SVG its full viewport; rasterize_svg crops that outer band.
RASTERIZER_WINDOW_SLACK = 128
PRIMARY_TARGET_SIZE = (160, 48)
GOLDEN_VARIANTS = (("primary", "normal"), ("degraded", "small"))

SURFACES = {
    "floor": ("office", "Office floor"),
    "guard-office": ("cameras", "Guard office"),
    "desk": ("desk", "Desk detail"),
    "phone": ("phone", "Phone"),
}

VIEW_ALIASES = {
    "floor": "floor",
    "office": "floor",
    "default": "floor",
    "top": "guard-office",
    "camera": "guard-office",
    "cameras": "guard-office",
    "camera-grid": "guard-office",
    "guard": "guard-office",
    "guard-office": "guard-office",
    "desk": "desk",
    "phone": "phone",
}

RGB_RE = re.compile(r"rgb\((\d+),(\d+),(\d+)\)")
INDEXED_RE = re.compile(r"indexed\((\d+)\)")
METADATA_RE = re.compile(
    r"^view=(?P<view>\S+) theme=(?P<theme>\S+) "
    r"size=(?P<width>\d+)x(?P<height>\d+) now=(?P<now>\d+) "
    r"depth=(?P<depth>\S+)$"
)
SVG_TEXT_RE = re.compile(r"<text\b[^>]*>(.*?)</text>", re.DOTALL)

NAMED_COLORS = {
    "black": "#000000",
    "red": "#800000",
    "green": "#008000",
    "yellow": "#808000",
    "blue": "#000080",
    "magenta": "#800080",
    "cyan": "#008080",
    "gray": "#c0c0c0",
    "dark-gray": "#808080",
    "light-red": "#ff0000",
    "light-green": "#00ff00",
    "light-yellow": "#ffff00",
    "light-blue": "#0000ff",
    "light-magenta": "#ff00ff",
    "light-cyan": "#00ffff",
    "white": "#ffffff",
}


@dataclass(frozen=True)
class Cell:
    symbol: str
    foreground: str
    background: str


@dataclass(frozen=True)
class Frame:
    surface: str
    renderer_view: str
    width: int
    height: int
    now: int
    rows: tuple[tuple[Cell, ...], ...]


def split_unescaped_pipes(token: str) -> list[str]:
    parts: list[str] = []
    start = 0
    escaped = False
    for index, character in enumerate(token):
        if escaped:
            escaped = False
            continue
        if character == "\\":
            escaped = True
            continue
        if character == "|":
            parts.append(token[start:index])
            start = index + 1
    parts.append(token[start:])
    return parts


def decode_symbol(raw: str) -> str:
    if raw == "∅":
        return ""

    output: list[str] = []
    index = 0
    while index < len(raw):
        character = raw[index]
        if character == "·":
            output.append(" ")
            index += 1
            continue
        if character != "\\":
            output.append(character)
            index += 1
            continue

        if index + 1 >= len(raw):
            raise ValueError(f"dangling symbol escape in {raw!r}")
        escaped = raw[index + 1]
        if escaped == "\\":
            output.append("\\")
            index += 2
        elif escaped == "|":
            output.append("|")
            index += 2
        elif escaped == "n":
            output.append("\n")
            index += 2
        elif escaped == "r":
            output.append("\r")
            index += 2
        elif escaped == "u" and index + 3 < len(raw) and raw[index + 2] == "{":
            end = raw.find("}", index + 3)
            if end == -1:
                raise ValueError(f"unterminated unicode escape in {raw!r}")
            output.append(chr(int(raw[index + 3 : end], 16)))
            index = end + 1
        else:
            output.append(escaped)
            index += 2
    return "".join(output)


def xterm_color(index: int) -> str:
    if index < 16:
        base = [
            0x000000,
            0x800000,
            0x008000,
            0x808000,
            0x000080,
            0x800080,
            0x008080,
            0xC0C0C0,
            0x808080,
            0xFF0000,
            0x00FF00,
            0xFFFF00,
            0x0000FF,
            0xFF00FF,
            0x00FFFF,
            0xFFFFFF,
        ]
        return f"#{base[index]:06x}"
    if index < 232:
        level = (index - 16) % 6
        green = ((index - 16) // 6) % 6
        red = (index - 16) // 36
        steps = [0, 95, 135, 175, 215, 255]
        return f"#{steps[red]:02x}{steps[green]:02x}{steps[level]:02x}"
    gray = 8 + (index - 232) * 10
    return f"#{gray:02x}{gray:02x}{gray:02x}"


def parse_color(raw: str, role: str, theme: str) -> str:
    if raw == "reset":
        if theme == "light":
            return LIGHT_FOREGROUND if role == "foreground" else LIGHT_BACKGROUND
        return DARK_FOREGROUND if role == "foreground" else DARK_BACKGROUND

    rgb = RGB_RE.fullmatch(raw)
    if rgb:
        red, green, blue = (int(value) for value in rgb.groups())
        if max(red, green, blue) > 255:
            raise ValueError(f"RGB component out of range: {raw}")
        return f"#{red:02x}{green:02x}{blue:02x}"

    indexed = INDEXED_RE.fullmatch(raw)
    if indexed:
        value = int(indexed.group(1))
        if value > 255:
            raise ValueError(f"indexed color out of range: {raw}")
        return xterm_color(value)

    try:
        return NAMED_COLORS[raw]
    except KeyError as error:
        raise ValueError(f"unknown serialized color: {raw}") from error


def parse_cell(token: str, theme: str) -> Cell:
    fields = split_unescaped_pipes(token)
    if len(fields) != 3:
        raise ValueError(f"expected symbol|foreground|background, got {token!r}")
    return Cell(
        decode_symbol(fields[0]),
        parse_color(fields[1], "foreground", theme),
        parse_color(fields[2], "background", theme),
    )


def parse_golden(
    surface: str, renderer_view: str, theme: str, golden_variant: str
) -> Frame:
    path = GOLDEN_DIR / f"{renderer_view}.{theme}.{golden_variant}.golden"
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise ValueError(f"cannot read {path}: {error}") from error

    if not lines or lines[0] != "they-work golden v1":
        raise ValueError(f"unsupported golden header in {path}")
    if len(lines) < 2:
        raise ValueError(f"missing golden metadata in {path}")
    metadata = METADATA_RE.fullmatch(lines[1])
    if metadata is None:
        raise ValueError(f"invalid golden metadata in {path}: {lines[1]!r}")
    if metadata.group("theme") != theme:
        raise ValueError(
            f"golden metadata theme {metadata.group('theme')!r} does not match "
            f"requested {theme!r} in {path}"
        )

    width = int(metadata.group("width"))
    height = int(metadata.group("height"))
    rows: list[tuple[Cell, ...]] = []
    for expected_row, line in enumerate(lines[2:]):
        match = re.fullmatch(r"(\d{3}):(.*)", line)
        if match is None or int(match.group(1)) != expected_row:
            raise ValueError(f"invalid row {expected_row} in {path}")
        tokens = match.group(2).lstrip().split(" ")
        if len(tokens) != width:
            raise ValueError(
                f"row {expected_row} in {path} has {len(tokens)} cells, expected {width}"
            )
        rows.append(tuple(parse_cell(token, theme) for token in tokens))

    if len(rows) != height:
        raise ValueError(f"{path} has {len(rows)} rows, expected {height}")
    return Frame(
        surface=surface,
        renderer_view=metadata.group("view"),
        width=width,
        height=height,
        now=int(metadata.group("now")),
        rows=tuple(rows),
    )


def frame_text(frame: Frame) -> str:
    """Return the exact cell text that the SVG exporter must carry."""
    return "".join(cell.symbol for row in frame.rows for cell in row if cell.symbol)


def svg_text(svg: str) -> str:
    """Extract text nodes from an SVG, decoding the exporter’s nbsp spaces."""
    return "".join(
        html.unescape(match).replace("\xa0", " ")
        for match in SVG_TEXT_RE.findall(svg)
    )


def assert_svg_text_matches_frame(svg: str, frame: Frame) -> None:
    expected = frame_text(frame)
    actual = svg_text(svg)
    if actual != expected:
        raise ValueError(
            f"SVG text round-trip mismatch for {frame.surface}: "
            f"expected {len(expected)} characters, got {len(actual)}"
        )


def find_svg_rasterizer() -> str:
    configured = os.environ.get(SVG_RASTERIZER_ENV, "").strip()
    candidates = [configured] if configured else []
    candidates.extend(("google-chrome", "chromium", "chromium-browser"))
    for candidate in candidates:
        if candidate and (Path(candidate).is_file() or shutil.which(candidate)):
            return candidate
    raise ValueError(
        "cannot rasterize SVG: install Google Chrome/Chromium or set "
        f"{SVG_RASTERIZER_ENV} to its executable"
    )


def png_dimensions(data: bytes) -> tuple[int, int]:
    if not data.startswith(b"\x89PNG\r\n\x1a\n") or data[12:16] != b"IHDR":
        raise ValueError("SVG rasterizer did not produce a PNG")
    return struct.unpack(">II", data[16:24])


def png_chunk(kind: bytes, data: bytes) -> bytes:
    payload = kind + data
    return (
        struct.pack(">I", len(data))
        + payload
        + struct.pack(">I", binascii.crc32(payload) & 0xFFFFFFFF)
    )


def crop_png(data: bytes, target_width: int, target_height: int) -> bytes:
    """Remove only Chrome's outer-window rows from an RGB, non-interlaced PNG."""
    if not data.startswith(b"\x89PNG\r\n\x1a\n"):
        raise ValueError("SVG rasterizer did not produce a PNG")

    chunks: list[tuple[bytes, bytes, bytes]] = []
    idat_parts: list[bytes] = []
    offset = 8
    ihdr: tuple[int, int, int, int, int, int, int] | None = None
    while offset < len(data):
        if offset + 12 > len(data):
            raise ValueError("SVG rasterizer produced a truncated PNG")
        length = struct.unpack(">I", data[offset : offset + 4])[0]
        end = offset + 12 + length
        if end > len(data):
            raise ValueError("SVG rasterizer produced a truncated PNG chunk")
        kind = data[offset + 4 : offset + 8]
        payload = data[offset + 8 : offset + 8 + length]
        raw_chunk = data[offset:end]
        chunks.append((kind, payload, raw_chunk))
        if kind == b"IHDR":
            if len(payload) != 13:
                raise ValueError("SVG rasterizer produced an invalid PNG header")
            ihdr = struct.unpack(">IIBBBBB", payload)
        elif kind == b"IDAT":
            idat_parts.append(payload)
        offset = end

    if ihdr is None or not idat_parts:
        raise ValueError("SVG rasterizer produced an incomplete PNG")
    width, height, depth, color_type, compression, filter_method, interlace = ihdr
    if (width, depth, color_type, compression, filter_method, interlace) != (
        target_width,
        8,
        2,
        0,
        0,
        0,
    ):
        raise ValueError("SVG rasterizer produced an unsupported PNG format")
    if target_height > height:
        raise ValueError(
            f"SVG rasterizer output is shorter than the SVG canvas: {height} < {target_height}"
        )

    row_bytes = 1 + width * 3
    raw = zlib.decompress(b"".join(idat_parts))
    if len(raw) != row_bytes * height:
        raise ValueError("SVG rasterizer produced an unexpected PNG row layout")
    cropped = raw[: row_bytes * target_height]
    replacement_ihdr = struct.pack(
        ">IIBBBBB", width, target_height, depth, color_type, compression, filter_method, interlace
    )

    output = bytearray(b"\x89PNG\r\n\x1a\n")
    wrote_idat = False
    for kind, payload, raw_chunk in chunks:
        if kind == b"IHDR":
            output.extend(png_chunk(kind, replacement_ihdr))
        elif kind == b"IDAT":
            if not wrote_idat:
                output.extend(png_chunk(kind, zlib.compress(cropped, level=9)))
                wrote_idat = True
        else:
            output.extend(raw_chunk)
    return bytes(output)


def rasterize_svg(svg_path: Path, png_path: Path, frame: Frame, rasterizer: str) -> None:
    """Rasterize the exact SVG file written for this frame with a browser."""
    svg = svg_path.read_text(encoding="utf-8")
    assert_svg_text_matches_frame(svg, frame)
    width = frame.width * CELL_WIDTH + FRAME_PADDING * 2
    height = frame.height * CELL_HEIGHT + FRAME_PADDING * 2
    capture_height = height + RASTERIZER_WINDOW_SLACK

    with tempfile.TemporaryDirectory(prefix="they-work-svg-") as temp_dir:
        temporary = Path(temp_dir)
        screenshot = temporary / "shot.png"
        user_data = temporary / "profile"
        command = [
            rasterizer,
            "--headless=new",
            "--no-sandbox",
            "--disable-gpu",
            "--hide-scrollbars",
            "--force-device-scale-factor=1",
            f"--window-size={width},{capture_height}",
            f"--screenshot={screenshot}",
            f"--user-data-dir={user_data}",
            svg_path.resolve().as_uri(),
        ]
        result = subprocess.run(
            command,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        if result.returncode != 0 or not screenshot.is_file():
            details = result.stderr.strip() or result.stdout.strip() or "no diagnostic"
            raise ValueError(f"SVG rasterization failed for {svg_path}: {details}")
        data = screenshot.read_bytes()
        actual_width, actual_height = png_dimensions(data)
        if actual_width != width or actual_height < height:
            raise ValueError(
                f"SVG rasterization size mismatch for {svg_path}: "
                f"expected at least {width}x{height}, got {actual_width}x{actual_height}"
            )
        cropped = crop_png(data, width, height)
        if png_dimensions(cropped) != (width, height):
            raise ValueError(f"failed to crop SVG rasterization for {svg_path}")
        png_path.write_bytes(cropped)


def reference_asset(surface: str) -> Path | None:
    for extension in REFERENCE_EXTENSIONS:
        candidate = REFERENCE_DIR / f"{surface}{extension}"
        if candidate.is_file():
            return candidate
    return None


def normalize_view(value: str) -> str:
    normalized = value.strip().casefold().replace("_", "-")
    if not normalized:
        return "floor"
    try:
        return VIEW_ALIASES[normalized]
    except KeyError as error:
        choices = ", ".join(sorted(VIEW_ALIASES))
        raise ValueError(f"unknown VIEW={value!r}; choose one of {choices}") from error


def parse_light(value: str) -> bool:
    normalized = value.strip().casefold()
    if normalized in {"", "0", "false", "no", "dark"}:
        return False
    if normalized in {"1", "true", "yes", "light"}:
        return True
    raise ValueError(f"unknown LIGHT={value!r}; use 0 or 1")


def render_svg(frame: Frame, light: bool) -> str:
    cell_width = CELL_WIDTH
    cell_height = CELL_HEIGHT
    padding = FRAME_PADDING
    width = frame.width * cell_width + padding * 2
    height = frame.height * cell_height + padding * 2
    page_background = "#f5f0e6" if light else "#0e0d17"
    border = "#786f5f" if light else "#4e4663"
    frame_background = LIGHT_BACKGROUND if light else DARK_BACKGROUND
    title = (
        f"they-work {frame.surface} at fixed time {frame.now} · "
        f"terminal size {frame.width}×{frame.height}"
    )

    output = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        (
            f'<svg xmlns="http://www.w3.org/2000/svg" '
            f'viewBox="0 0 {width} {height}" width="{width}" height="{height}" '
            f'role="img" aria-label="{html.escape(title, quote=True)}">'
        ),
        f"  <title>{html.escape(title)}</title>",
        (
            f'  <rect x="0" y="0" width="{width}" height="{height}" '
            f'fill="{page_background}"/>'
        ),
        (
            f'  <rect x="{padding}" y="{padding}" '
            f'width="{frame.width * cell_width}" height="{frame.height * cell_height}" '
            f'fill="{frame_background}" stroke="{border}" stroke-width="2"/>'
        ),
    ]

    for row_index, row in enumerate(frame.rows):
        for column_index, cell in enumerate(row):
            x = padding + column_index * cell_width
            y = padding + row_index * cell_height
            output.append(
                f'  <rect x="{x}" y="{y}" width="{cell_width}" height="{cell_height}" '
                f'fill="{cell.background}"/>'
            )
            if cell.symbol:
                symbol = html.escape(cell.symbol, quote=False).replace(" ", "&#160;")
                output.append(
                    f'  <text x="{x + 1}" y="{y + 18}" fill="{cell.foreground}" '
                    'font-family="Cascadia Mono, DejaVu Sans Mono, monospace" '
                    'font-size="18" xml:space="preserve">'
                    f"{symbol}</text>"
                )

    output.append("</svg>")
    return "\n".join(output) + "\n"


def render_index(
    frames_by_variant: dict[str, dict[str, dict[str, Frame]]],
    selected: str,
    selected_theme: str,
) -> str:
    primary_frames = frames_by_variant["primary"]
    degraded_frames = frames_by_variant["degraded"]
    first_surface = next(iter(SURFACES))
    primary_example = primary_frames["dark"][first_surface]
    degraded_example = degraded_frames["dark"][first_surface]
    timestamp = primary_example.now

    def output_panel(
        surface: str, label: str, frame: Frame, theme: str, variant: str
    ) -> str:
        suffix = "" if variant == "primary" else "-small"
        variant_label = "Primary" if variant == "primary" else "Degraded"
        size = f"{frame.width}×{frame.height}"
        panel_class = "dark" if theme == "dark" else "light"
        png_name = f"{surface}-{theme}{suffix}.png"
        svg_name = f"{surface}-{theme}{suffix}.svg"
        return (
            f'<div class="panel {panel_class} {variant}">'
            f'<h3>{theme.title()} · {variant_label} · terminal {size}</h3>'
            f'<a href="{png_name}"><img loading="lazy" src="{png_name}" '
            f'alt="{theme.title()} {variant_label.lower()} {html.escape(label, quote=True)} output at terminal {size}"></a>'
            f'<p><a href="{png_name}">PNG</a> · <a href="{svg_name}">SVG</a> · '
            f'terminal {size} · time {frame.now}</p></div>'
        )

    cards = []
    for surface, (_, label) in SURFACES.items():
        asset = reference_asset(surface)
        if asset is None:
            reference_markup = (
                '<div class="missing">Reference pending.<br>'
                f'<code>docs/references/{surface}.png</code></div>'
            )
            reference_note = "Replace the pending slot with the supplied design image."
        else:
            reference_href = f"../references/{asset.name}"
            escaped_href = html.escape(reference_href, quote=True)
            reference_markup = (
                f'<a href="{escaped_href}"><img loading="lazy" src="{escaped_href}" '
                f'alt="Intended design for {html.escape(label, quote=True)}"></a>'
            )
            reference_note = f"Source: {html.escape(asset.name)}"

        primary_dark = primary_frames["dark"][surface]
        primary_light = primary_frames["light"][surface]
        degraded_dark = degraded_frames["dark"][surface]
        degraded_light = degraded_frames["light"][surface]
        cards.append(
            "".join(
                [
                    '<article class="surface">',
                    f'<div class="surface-heading"><h2>{html.escape(label)}</h2>',
                    f'<p>Fixed demo timestamp: {timestamp} · primary terminal: ',
                    f'{primary_dark.width}×{primary_dark.height} · degraded terminal: ',
                    f'{degraded_dark.width}×{degraded_dark.height}</p></div>',
                    '<div class="comparison">',
                    '<div class="panel reference"><h3>Intended design</h3>',
                    f"{reference_markup}<p>{reference_note}</p></div>",
                    output_panel(surface, label, primary_dark, "dark", "primary"),
                    output_panel(surface, label, primary_light, "light", "primary"),
                    output_panel(surface, label, degraded_dark, "dark", "degraded"),
                    output_panel(surface, label, degraded_light, "light", "degraded"),
                    '</div></article>',
                ]
            )
        )

    primary_size = f"{primary_example.width}×{primary_example.height}"
    degraded_size = f"{degraded_example.width}×{degraded_example.height}"
    return "\n".join(
        [
            "<!doctype html>",
            '<html lang="en"><head><meta charset="utf-8">',
            '<meta name="viewport" content="width=device-width, initial-scale=1">',
            "<title>they-work art review contact sheet</title>",
            "<style>",
            "*{box-sizing:border-box}",
            "body{margin:0;padding:2rem;background:#0e0d17;color:#f8e2b6;font-family:system-ui,sans-serif}",
            "main{display:grid;gap:1.5rem;max-width:150rem;margin:0 auto}",
            ".surface{padding:1rem;border:2px solid #4e4663;border-radius:.6rem;background:#181627}",
            ".surface-heading{display:flex;justify-content:space-between;align-items:baseline;gap:1rem;flex-wrap:wrap}",
            "h1,h2,h3{margin-top:0}",
            ".surface-heading p,.panel>p{margin:.35rem 0;color:#a49dbe;font-size:.9rem}",
            ".comparison{display:grid;grid-template-columns:minmax(15rem,1.15fr) repeat(4,minmax(11rem,1fr));gap:1rem}",
            ".panel{min-width:0;padding:.75rem;border-radius:.4rem;background:#0d0b14}",
            ".panel h3{margin-bottom:.6rem;font-size:.95rem}",
            ".panel img{display:block;width:100%;height:auto;image-rendering:pixelated;border:1px solid #4e4663}",
            ".panel a{color:inherit}",
            ".dark h3{color:#f8e2b6}.light{background:#f4efe4;color:#3a352c}.light h3{color:#3a352c}",
            ".light p{color:#5d566b}.reference{background:#211d2b}.degraded{border-top:3px solid #786f5f}",
            ".missing{min-height:12rem;display:grid;place-items:center;padding:1rem;text-align:center;"
            "border:2px dashed #786f5f;color:#d9c9a8;background:#30293a}",
            "code{font-size:.85em}",
            "@media(max-width:100rem){.comparison{grid-template-columns:1fr 1fr 1fr}.reference{grid-column:1/-1}}",
            "@media(max-width:70rem){.comparison{grid-template-columns:1fr 1fr}.reference{grid-column:1/-1}}",
            "@media(max-width:42rem){body{padding:1rem}.comparison{grid-template-columns:1fr}.reference{grid-column:auto}}",
            "</style></head><body>",
            "<h1>they-work art review contact sheet</h1>",
            f"<p>One page for every surface. Fixed demo timestamp: {timestamp}. "
            f"Primary outputs: terminal {primary_size}; degraded outputs: terminal {degraded_size}. "
            f"Selected compatibility shot: {html.escape(selected)} ({selected_theme}). "
            "Every rendered panel carries its terminal size. Design-only reference boards remain "
            "in docs/references until a matching rendered surface exists.</p>",
            "<main>",
            *cards,
            "</main></body></html>",
            "",
        ]
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--view", default="", help="floor, top, guard-office, desk, or phone")
    parser.add_argument("--light", default="", help="0/dark or 1/light review chrome")
    parser.add_argument("--out-dir", default=str(ROOT / "docs" / "shots"))
    args = parser.parse_args()

    try:
        selected = normalize_view(args.view)
        light = parse_light(args.light)
        frames_by_variant = {
            variant_name: {
                theme: {
                    surface: parse_golden(
                        surface, renderer_view, theme, golden_variant
                    )
                    for surface, (renderer_view, _) in SURFACES.items()
                }
                for theme in ("dark", "light")
            }
            for variant_name, golden_variant in GOLDEN_VARIANTS
        }
        all_frames = [
            frame
            for frames_by_theme in frames_by_variant.values()
            for frames in frames_by_theme.values()
            for frame in frames.values()
        ]
        timestamps = {frame.now for frame in all_frames}
        if len(timestamps) != 1:
            raise ValueError("surface goldens do not share one fixed timestamp")
        for variant_name, frames_by_theme in frames_by_variant.items():
            dimensions = {
                (frame.width, frame.height)
                for frames in frames_by_theme.values()
                for frame in frames.values()
            }
            if len(dimensions) != 1:
                raise ValueError(
                    f"{variant_name} goldens do not share one terminal size"
                )
            if variant_name == "primary" and dimensions != {PRIMARY_TARGET_SIZE}:
                actual = ", ".join(
                    f"{width}x{height}" for width, height in sorted(dimensions)
                )
                raise ValueError(
                    "primary goldens must be 160x48; "
                    f"found {actual}. Update the renderer golden before running make shot"
                )
            for theme, frames in frames_by_theme.items():
                for surface, (renderer_view, _) in SURFACES.items():
                    frame = frames[surface]
                    if frame.renderer_view != renderer_view:
                        raise ValueError(
                            f"{variant_name} {theme} golden for {surface} "
                            f"describes {frame.renderer_view}, expected {renderer_view}"
                        )
    except ValueError as error:
        parser.error(str(error))

    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    try:
        rasterizer = find_svg_rasterizer()
    except ValueError as error:
        parser.error(str(error))
    rendered_by_variant: dict[str, dict[str, dict[str, str]]] = {}
    for variant_name, frames_by_theme in frames_by_variant.items():
        suffix = "" if variant_name == "primary" else "-small"
        rendered_by_variant[variant_name] = {}
        for theme, frames in frames_by_theme.items():
            theme_light = theme == "light"
            rendered_by_variant[variant_name][theme] = {}
            for surface, frame in frames.items():
                svg = render_svg(frame, theme_light)
                rendered_by_variant[variant_name][theme][surface] = svg
                svg_path = out_dir / f"{surface}-{theme}{suffix}.svg"
                svg_path.write_text(svg, encoding="utf-8")
                rasterize_svg(
                    svg_path,
                    out_dir / f"{surface}-{theme}{suffix}.png",
                    frame,
                    rasterizer,
                )

            selected_frame = frames[selected]
            selected_svg_path = out_dir / f"shot-{theme}{suffix}.svg"
            selected_svg_path.write_text(
                rendered_by_variant[variant_name][theme][selected], encoding="utf-8"
            )
            rasterize_svg(
                selected_svg_path,
                out_dir / f"shot-{theme}{suffix}.png",
                selected_frame,
                rasterizer,
            )

    selected_theme = "light" if light else "dark"
    for variant_name, frames_by_theme in frames_by_variant.items():
        suffix = "" if variant_name == "primary" else "-small"
        selected_frames = frames_by_theme[selected_theme]
        for surface, contents in rendered_by_variant[variant_name][
            selected_theme
        ].items():
            svg_path = out_dir / f"{surface}{suffix}.svg"
            svg_path.write_text(contents, encoding="utf-8")
            rasterize_svg(
                svg_path,
                out_dir / f"{surface}{suffix}.png",
                selected_frames[surface],
                rasterizer,
            )
        shot_svg_path = out_dir / f"shot{suffix}.svg"
        shot_svg_path.write_text(
            rendered_by_variant[variant_name][selected_theme][selected],
            encoding="utf-8",
        )
        rasterize_svg(
            shot_svg_path,
            out_dir / f"shot{suffix}.png",
            selected_frames[selected],
            rasterizer,
        )
    (out_dir / "index.html").write_text(
        render_index(frames_by_variant, selected, selected_theme), encoding="utf-8"
    )
    print(
        f"wrote {len(SURFACES)} surfaces in primary/degraded dark/light PNG+SVG to {out_dir} "
        f"(selected: {selected}, {selected_theme})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
