#!/usr/bin/env python3
"""Export every complete encoding rung as deterministic SVG and PNG frames."""

from __future__ import annotations

import argparse
import binascii
import html
import json
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
DEFAULT_ENCODING = "half-blocks"
ENCODING_ORDER = ("sextants", "quadrants", "half-blocks")
GOLDEN_VARIANTS = (("primary", "normal"), ("degraded", "small"))
# The renderer assigns these Unicode code points by looking up the mask in
# this table.  Keep the same map here so the exporter can draw sextants as
# six deterministic rectangles when the rasterizer's font lacks the block.
SEXTANT_MASKS = (
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
    22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39,
    40, 41, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58,
    59, 60, 61, 62,
)

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
    encoding: str
    width: int
    height: int
    now: int
    rows: tuple[tuple[Cell, ...], ...]


@dataclass(frozen=True)
class ImageFrame:
    """One renderer-backed physical-pixel frame for the primary viewport."""

    surface: str
    theme: str
    png: Path
    width: int
    height: int
    columns: int
    rows: int
    cell_width: int
    cell_height: int
    packets: tuple[tuple[str, Path], ...]


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


def normalize_encoding(value: str) -> str:
    normalized = value.strip().casefold()
    aliases = {
        "half": "half-blocks",
        "halfblock": "half-blocks",
        "half-block": "half-blocks",
        "half-blocks": "half-blocks",
        "quadrant": "quadrants",
        "quadrants": "quadrants",
        "sextant": "sextants",
        "sextants": "sextants",
    }
    return aliases.get(normalized, normalized)


def parse_metadata(line: str, path: Path) -> dict[str, str]:
    fields: dict[str, str] = {}
    for token in line.split():
        key, separator, value = token.partition("=")
        if not separator or not key or not value:
            raise ValueError(f"invalid metadata token in {path}: {token!r}")
        fields[key] = value

    required = {"view", "theme", "size", "now", "depth"}
    missing = sorted(required - fields.keys())
    if missing:
        raise ValueError(f"missing metadata fields in {path}: {', '.join(missing)}")
    if re.fullmatch(r"\d+x\d+", fields["size"]) is None:
        raise ValueError(f"invalid metadata size in {path}: {fields['size']!r}")
    if re.fullmatch(r"\d+", fields["now"]) is None:
        raise ValueError(f"invalid metadata timestamp in {path}: {fields['now']!r}")
    depth_encoding = fields["depth"].rsplit("+", 1)[-1] if "+" in fields["depth"] else ""
    encoding = fields.get("encoding", depth_encoding or DEFAULT_ENCODING)
    encoding = normalize_encoding(encoding)
    if re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._+-]*", encoding) is None:
        raise ValueError(f"invalid metadata encoding in {path}: {encoding!r}")
    fields["encoding"] = encoding
    return fields


def golden_path(
    renderer_view: str, theme: str, golden_variant: str, encoding: str
) -> Path | None:
    encoded = GOLDEN_DIR / f"{renderer_view}.{theme}.{golden_variant}.{encoding}.golden"
    if encoded.is_file():
        return encoded
    if encoding == DEFAULT_ENCODING:
        legacy = GOLDEN_DIR / f"{renderer_view}.{theme}.{golden_variant}.golden"
        if legacy.is_file():
            return legacy
    return None


def parse_golden(
    surface: str,
    renderer_view: str,
    theme: str,
    golden_variant: str,
    encoding: str,
) -> Frame:
    path = golden_path(renderer_view, theme, golden_variant, encoding)
    if path is None:
        expected = GOLDEN_DIR / (
            f"{renderer_view}.{theme}.{golden_variant}.{encoding}.golden"
        )
        raise ValueError(f"missing {encoding} golden at {expected}")
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise ValueError(f"cannot read {path}: {error}") from error

    if not lines or lines[0] != "they-work golden v1":
        raise ValueError(f"unsupported golden header in {path}")
    if len(lines) < 2:
        raise ValueError(f"missing golden metadata in {path}")
    metadata = parse_metadata(lines[1], path)
    if metadata["theme"] != theme:
        raise ValueError(
            f"golden metadata theme {metadata['theme']!r} does not match "
            f"requested {theme!r} in {path}"
        )
    if metadata["encoding"] != encoding:
        raise ValueError(
            f"golden metadata encoding {metadata['encoding']!r} does not match "
            f"requested {encoding!r} in {path}"
        )

    width, height = (int(value) for value in metadata["size"].split("x"))
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
        renderer_view=metadata["view"],
        encoding=metadata["encoding"],
        width=width,
        height=height,
        now=int(metadata["now"]),
        rows=tuple(rows),
    )


def discover_encodings() -> tuple[list[str], dict[str, int]]:
    names = set(ENCODING_ORDER)
    for path in GOLDEN_DIR.glob("*.golden"):
        parts = path.name.removesuffix(".golden").split(".")
        if len(parts) >= 4 and parts[-3] in {"dark", "light"} and parts[-2] in {
            "normal",
            "small",
        }:
            names.add(normalize_encoding(parts[-1]))

    ordered = [encoding for encoding in ENCODING_ORDER if encoding in names]
    ordered.extend(sorted(names - set(ENCODING_ORDER)))
    complete: list[str] = []
    partial: dict[str, int] = {}
    required = len(GOLDEN_VARIANTS) * 2 * len(SURFACES)
    for encoding in ordered:
        present = 0
        for _, golden_variant in GOLDEN_VARIANTS:
            for theme in ("dark", "light"):
                for _, (renderer_view, _) in SURFACES.items():
                    if golden_path(renderer_view, theme, golden_variant, encoding):
                        present += 1
        if present == required:
            complete.append(encoding)
        elif present:
            partial[encoding] = required - present
    return complete, partial


def load_frames(
    encodings: list[str],
) -> dict[str, dict[str, dict[str, dict[str, Frame]]]]:
    return {
        encoding: {
            variant_name: {
                theme: {
                    surface: parse_golden(
                        surface,
                        renderer_view,
                        theme,
                        golden_variant,
                        encoding,
                    )
                    for surface, (renderer_view, _) in SURFACES.items()
                }
                for theme in ("dark", "light")
            }
            for variant_name, golden_variant in GOLDEN_VARIANTS
        }
        for encoding in encodings
    }


def validate_frames(
    frames_by_encoding: dict[str, dict[str, dict[str, dict[str, Frame]]]],
) -> None:
    all_frames = [
        frame
        for frames_by_variant in frames_by_encoding.values()
        for frames_by_theme in frames_by_variant.values()
        for frames in frames_by_theme.values()
        for frame in frames.values()
    ]
    timestamps = {frame.now for frame in all_frames}
    if len(timestamps) != 1:
        raise ValueError("surface goldens do not share one fixed timestamp")
    for encoding, frames_by_variant in frames_by_encoding.items():
        for variant_name, frames_by_theme in frames_by_variant.items():
            dimensions = {
                (frame.width, frame.height)
                for frames in frames_by_theme.values()
                for frame in frames.values()
            }
            if len(dimensions) != 1:
                raise ValueError(
                    f"{encoding} {variant_name} goldens do not share one terminal size"
                )
            if variant_name == "primary" and dimensions != {PRIMARY_TARGET_SIZE}:
                actual = ", ".join(
                    f"{width}x{height}" for width, height in sorted(dimensions)
                )
                raise ValueError(
                    f"{encoding} primary goldens must be 160x48; "
                    f"found {actual}. Update the renderer golden before running make shot"
                )
            for theme, frames in frames_by_theme.items():
                for surface, (renderer_view, _) in SURFACES.items():
                    frame = frames[surface]
                    if frame.renderer_view != renderer_view:
                        raise ValueError(
                            f"{encoding} {variant_name} {theme} golden for {surface} "
                            f"describes {frame.renderer_view}, expected {renderer_view}"
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


def image_input_path(root: Path, relative: object, description: str) -> Path:
    if not isinstance(relative, str) or not relative:
        raise ValueError(f"image-frame manifest {description} must be a non-empty filename")
    candidate = (root / relative).resolve()
    try:
        candidate.relative_to(root)
    except ValueError as error:
        raise ValueError(
            f"image-frame manifest {description} escapes its input directory: {relative!r}"
        ) from error
    if not candidate.is_file():
        raise ValueError(f"image-frame manifest {description} is not a file: {relative!r}")
    if candidate.stat().st_size == 0:
        raise ValueError(f"image-frame manifest {description} is empty: {relative!r}")
    return candidate


def manifest_positive_int(value: object, description: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        raise ValueError(f"image-frame manifest {description} must be a positive integer")
    return value


def load_image_frames(
    image_frame_dir: str, expected_timestamp: int
) -> dict[tuple[str, str], ImageFrame]:
    """Load a complete, deterministic graphics-frame dump when one is supplied.

    The dump is deliberately separate from the contact-sheet output. It is
    generated by the graphics owner and includes the terminal packets used to
    make each PNG inspectable evidence rather than a decorative approximation.
    """
    if not image_frame_dir:
        return {}
    root = Path(image_frame_dir).resolve()
    if not root.is_dir():
        raise ValueError(f"image-frame directory does not exist: {root}")
    manifest_path = root / "manifest.json"
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except OSError as error:
        raise ValueError(f"cannot read image-frame manifest {manifest_path}: {error}") from error
    except json.JSONDecodeError as error:
        raise ValueError(f"invalid image-frame manifest {manifest_path}: {error}") from error
    if not isinstance(manifest, dict):
        raise ValueError("image-frame manifest must be a JSON object")
    if manifest.get("version") != 1:
        raise ValueError("image-frame manifest must declare version 1")
    if manifest.get("source") != "renderer-pixel-frame":
        raise ValueError(
            "image-frame manifest source must be renderer-pixel-frame, not a synthetic sample"
        )
    if manifest_positive_int(manifest.get("timestamp"), "timestamp") != expected_timestamp:
        raise ValueError(
            "image-frame manifest timestamp must match the cell-golden fixed timestamp"
        )
    viewport = manifest.get("viewport")
    if not isinstance(viewport, dict):
        raise ValueError("image-frame manifest viewport must be an object")
    columns = manifest_positive_int(viewport.get("columns"), "viewport.columns")
    rows = manifest_positive_int(viewport.get("rows"), "viewport.rows")
    cell_width = manifest_positive_int(viewport.get("cell_width"), "viewport.cell_width")
    cell_height = manifest_positive_int(viewport.get("cell_height"), "viewport.cell_height")
    if (columns, rows) != PRIMARY_TARGET_SIZE:
        raise ValueError(
            "image-frame manifest viewport must be 160x48 to sit beside the primary cell frames"
        )
    frames = manifest.get("frames")
    if not isinstance(frames, list):
        raise ValueError("image-frame manifest frames must be an array")
    expected_keys = {(surface, theme) for surface in SURFACES for theme in ("dark", "light")}
    loaded: dict[tuple[str, str], ImageFrame] = {}
    required_packets = {"kitty-direct", "sixel", "iterm2"}
    for entry in frames:
        if not isinstance(entry, dict):
            raise ValueError("every image-frame manifest frame must be an object")
        surface = entry.get("surface")
        theme = entry.get("theme")
        key = (surface, theme)
        if key not in expected_keys:
            raise ValueError(f"unknown image-frame surface/theme: {surface!r}/{theme!r}")
        if key in loaded:
            raise ValueError(f"duplicate image-frame surface/theme: {surface}/{theme}")
        png = image_input_path(root, entry.get("png"), f"PNG for {surface}/{theme}")
        width = manifest_positive_int(entry.get("width"), f"width for {surface}/{theme}")
        height = manifest_positive_int(entry.get("height"), f"height for {surface}/{theme}")
        if png_dimensions(png.read_bytes()) != (width, height):
            raise ValueError(
                f"image-frame PNG dimensions disagree with manifest for {surface}/{theme}"
            )
        if (width, height) != (columns * cell_width, rows * cell_height):
            raise ValueError(
                f"image-frame {surface}/{theme} is {width}x{height}, expected "
                f"{columns * cell_width}x{rows * cell_height} from the reported cell geometry"
            )
        packets = entry.get("packets")
        if not isinstance(packets, dict) or set(packets) != required_packets:
            raise ValueError(
                f"image-frame {surface}/{theme} needs kitty-direct, sixel, and iterm2 packet files"
            )
        packet_paths = tuple(
            (protocol, image_input_path(root, packets[protocol], f"{protocol} packet for {surface}/{theme}"))
            for protocol in sorted(required_packets)
        )
        loaded[key] = ImageFrame(
            surface=surface,
            theme=theme,
            png=png,
            width=width,
            height=height,
            columns=columns,
            rows=rows,
            cell_width=cell_width,
            cell_height=cell_height,
            packets=packet_paths,
        )
    missing = expected_keys - set(loaded)
    extra = set(loaded) - expected_keys
    if missing or extra or len(loaded) != len(expected_keys):
        summary = ", ".join(f"{surface}/{theme}" for surface, theme in sorted(missing))
        raise ValueError(f"image-frame manifest must cover every surface/theme; missing: {summary}")
    return loaded


def image_frame_stem(frame: ImageFrame) -> str:
    return f"image-{frame.surface}-{frame.theme}-{frame.width}x{frame.height}"


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


def sextant_mask(symbol: str) -> int | None:
    """Return the six-subcell mask for a renderer sextant glyph."""
    if len(symbol) != 1:
        return None
    codepoint = ord(symbol)
    index = codepoint - 0x1F_B00
    if 0 <= index < len(SEXTANT_MASKS):
        return SEXTANT_MASKS[index]
    return None


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
        f"{frame.encoding} encoding · terminal size {frame.width}×{frame.height}"
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
                mask = sextant_mask(cell.symbol)
                if mask is None:
                    output.append(
                        f'  <text x="{x + 1}" y="{y + 18}" fill="{cell.foreground}" '
                        'font-family="Cascadia Mono, DejaVu Sans Mono, monospace" '
                        'font-size="18" xml:space="preserve">'
                        f"{symbol}</text>"
                    )
                else:
                    # Preserve the exact glyph in the SVG text stream for the
                    # round-trip guard, while using vector subcells for pixels.
                    output.append(
                        f'  <text x="{x + 1}" y="{y + 18}" fill="none" '
                        'font-family="Cascadia Mono, DejaVu Sans Mono, monospace" '
                        'font-size="18" xml:space="preserve">'
                        f"{symbol}</text>"
                    )
                    subcell_width = cell_width / 2
                    subcell_height = cell_height / 3
                    for subcell in range(6):
                        if mask & (1 << subcell):
                            subcell_x = x + (subcell % 2) * subcell_width
                            subcell_y = y + (subcell // 2) * subcell_height
                            output.append(
                                f'  <rect x="{subcell_x}" y="{subcell_y}" '
                                f'width="{subcell_width}" height="{subcell_height}" '
                                f'fill="{cell.foreground}"/>'
                            )

    output.append("</svg>")
    return "\n".join(output) + "\n"


def frame_stem(surface: str, theme: str, encoding: str, variant: str) -> str:
    suffix = "" if variant == "primary" else "-small"
    return f"{surface}-{theme}-{encoding}{suffix}"


def render_index(
    frames_by_encoding: dict[str, dict[str, dict[str, dict[str, Frame]]]],
    encodings: list[str],
    partial_encodings: dict[str, int],
    image_frames: dict[tuple[str, str], ImageFrame],
    selected: str,
    selected_theme: str,
) -> str:
    best_encoding = encodings[0]
    best_frames = frames_by_encoding[best_encoding]
    first_surface = next(iter(SURFACES))
    primary_example = best_frames["primary"]["dark"][first_surface]
    degraded_example = best_frames["degraded"]["dark"][first_surface]
    timestamp = primary_example.now

    def output_panel(
        surface: str, label: str, frame: Frame, theme: str, variant: str
    ) -> str:
        variant_label = "Primary rung" if variant == "primary" else "Degraded rung"
        size = f"{frame.width}×{frame.height}"
        panel_class = "dark" if theme == "dark" else "light"
        stem = frame_stem(surface, theme, frame.encoding, variant)
        png_name = f"{stem}.png"
        svg_name = f"{stem}.svg"
        return (
            f'<div class="panel {panel_class} {variant}">'
            f'<h3>{theme.title()} · {variant_label} · {frame.encoding} · terminal {size}</h3>'
            f'<a href="{png_name}"><img loading="lazy" src="{png_name}" '
            f'alt="{theme.title()} {variant_label.lower()} {html.escape(label, quote=True)} output with {frame.encoding} encoding at terminal {size}"></a>'
            f'<p><a href="{png_name}">PNG</a> · <a href="{svg_name}">SVG</a> · '
            f'{frame.encoding} encoding · terminal {size} · time {frame.now}</p></div>'
        )

    def image_panel(surface: str, label: str, frame: ImageFrame) -> str:
        panel_class = "dark" if frame.theme == "dark" else "light"
        png_name = f"{image_frame_stem(frame)}.png"
        packets = ", ".join(protocol for protocol, _ in frame.packets)
        cell_size = f"{frame.columns}×{frame.rows} cells @ {frame.cell_width}×{frame.cell_height} px"
        dimensions = f"{frame.width}×{frame.height} px"
        return (
            f'<div class="panel {panel_class} image-protocol primary">'
            f'<h3>{frame.theme.title()} · Image protocol · source {dimensions}</h3>'
            f'<a href="{png_name}"><img loading="lazy" src="{png_name}" '
            f'alt="{frame.theme.title()} {html.escape(label, quote=True)} renderer image frame at {dimensions}"></a>'
            f'<p><a href="{png_name}">PNG</a> · renderer pixels {dimensions} · '
            f'{cell_size} · packets retained: {html.escape(packets)}</p></div>'
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

        ladder = []
        for encoding in encodings:
            primary = frames_by_encoding[encoding]["primary"]["dark"][surface]
            degraded = frames_by_encoding[encoding]["degraded"]["dark"][surface]
            ladder.append(
                f"{encoding}: primary {primary.width}×{primary.height}, "
                f"degraded {degraded.width}×{degraded.height}"
            )
        panels = [
            '<div class="panel reference"><h3>Intended design</h3>',
            f"{reference_markup}<p>{reference_note}</p></div>",
        ]
        for encoding in encodings:
            for variant_name, _ in GOLDEN_VARIANTS:
                for theme in ("dark", "light"):
                    panels.append(
                        output_panel(
                            surface,
                            label,
                            frames_by_encoding[encoding][variant_name][theme][surface],
                            theme,
                            variant_name,
                        )
                    )
                    if image_frames and encoding == best_encoding and variant_name == "primary":
                        panels.append(image_panel(surface, label, image_frames[(surface, theme)]))
        cards.append(
            "".join(
                [
                    '<article class="surface">',
                    f'<div class="surface-heading"><h2>{html.escape(label)}</h2>',
                    f'<p>Fixed demo timestamp: {timestamp} · complete encoding ladder: ',
                    f'{html.escape(" · ".join(ladder))}</p></div>',
                    '<div class="comparison">',
                    *panels,
                    '</div></article>',
                ]
            )
        )

    primary_size = f"{primary_example.width}×{primary_example.height}"
    degraded_size = f"{degraded_example.width}×{degraded_example.height}"
    partial_note = ""
    if partial_encodings:
        partial_summary = ", ".join(
            f"{encoding} ({missing} missing frame{'s' if missing != 1 else ''})"
            for encoding, missing in partial_encodings.items()
        )
        partial_note = (
            " Partial golden rungs not shown because they are incomplete: "
            f"{html.escape(partial_summary)}."
        )
    output_count = len(encodings) * len(GOLDEN_VARIANTS) * 2 + (2 if image_frames else 0)
    complete_summary = html.escape(", ".join(encodings))
    image_summary = (
        "The renderer-backed image-protocol PNGs sit beside the primary "
        "character-cell frames and are labelled with verified physical-pixel dimensions."
        if image_frames
        else "Image-protocol panels are not present: pass --image-frame-dir with a complete "
        "renderer-backed manifest; synthetic encoder samples are rejected."
    )
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
            f".comparison{{display:grid;grid-template-columns:minmax(15rem,1.15fr) repeat({output_count},minmax(11rem,1fr));gap:1rem}}",
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
            f"Complete encoding rungs: {complete_summary}. Best compatibility shot: "
            f"{html.escape(best_encoding)} ({html.escape(selected)} / {selected_theme}). "
            "Every character-cell panel carries its terminal size and encoding. "
            "Each row repeats one surface at every available resolution and encoding rung."
            f" {image_summary}{partial_note} Design-only reference boards "
            "remain in docs/references until a matching rendered surface exists.</p>",
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
    parser.add_argument(
        "--image-frame-dir",
        default="",
        help="directory containing a renderer-pixel-frame manifest.json and matching PNG/packet files",
    )
    args = parser.parse_args()

    try:
        selected = normalize_view(args.view)
        light = parse_light(args.light)
        encodings, partial_encodings = discover_encodings()
        if not encodings:
            raise ValueError(
                "no complete encoding ladder found; each encoding needs every "
                "dark/light primary/degraded surface golden"
            )
        frames_by_encoding = load_frames(encodings)
        validate_frames(frames_by_encoding)
        primary_timestamp = next(
            iter(frames_by_encoding[encodings[0]]["primary"]["dark"].values())
        ).now
        image_frames = load_image_frames(args.image_frame_dir, primary_timestamp)
    except ValueError as error:
        parser.error(str(error))

    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    for image_frame in image_frames.values():
        destination = out_dir / f"{image_frame_stem(image_frame)}.png"
        shutil.copyfile(image_frame.png, destination)
    try:
        rasterizer = find_svg_rasterizer()
    except ValueError as error:
        parser.error(str(error))
    rendered_by_encoding: dict[str, dict[str, dict[str, dict[str, str]]]] = {}
    for encoding in encodings:
        rendered_by_encoding[encoding] = {}
        for variant_name, _ in GOLDEN_VARIANTS:
            rendered_by_encoding[encoding][variant_name] = {}
            for theme in ("dark", "light"):
                theme_light = theme == "light"
                frames = frames_by_encoding[encoding][variant_name][theme]
                rendered_by_encoding[encoding][variant_name][theme] = {}
                for surface, frame in frames.items():
                    svg = render_svg(frame, theme_light)
                    rendered_by_encoding[encoding][variant_name][theme][surface] = svg
                    stem = frame_stem(surface, theme, encoding, variant_name)
                    svg_path = out_dir / f"{stem}.svg"
                    svg_path.write_text(svg, encoding="utf-8")
                    rasterize_svg(
                        svg_path,
                        out_dir / f"{stem}.png",
                        frame,
                        rasterizer,
                    )

                selected_frame = frames[selected]
                selected_stem = frame_stem("shot", theme, encoding, variant_name)
                selected_svg_path = out_dir / f"{selected_stem}.svg"
                selected_svg_path.write_text(
                    rendered_by_encoding[encoding][variant_name][theme][selected],
                    encoding="utf-8",
                )
                rasterize_svg(
                    selected_svg_path,
                    out_dir / f"{selected_stem}.png",
                    selected_frame,
                    rasterizer,
                )

    selected_theme = "light" if light else "dark"
    best_encoding = encodings[0]
    for variant_name, _ in GOLDEN_VARIANTS:
        suffix = "" if variant_name == "primary" else "-small"
        for theme in ("dark", "light"):
            selected_frames = frames_by_encoding[best_encoding][variant_name][theme]
            rendered = rendered_by_encoding[best_encoding][variant_name][theme]
            for surface, contents in rendered.items():
                theme_alias = out_dir / f"{surface}-{theme}{suffix}.svg"
                theme_alias.write_text(contents, encoding="utf-8")
                rasterize_svg(
                    theme_alias,
                    out_dir / f"{surface}-{theme}{suffix}.png",
                    selected_frames[surface],
                    rasterizer,
                )
            shot_theme_alias = out_dir / f"shot-{theme}{suffix}.svg"
            shot_theme_alias.write_text(rendered[selected], encoding="utf-8")
            rasterize_svg(
                shot_theme_alias,
                out_dir / f"shot-{theme}{suffix}.png",
                selected_frames[selected],
                rasterizer,
            )

        selected_frames = frames_by_encoding[best_encoding][variant_name][selected_theme]
        rendered = rendered_by_encoding[best_encoding][variant_name][selected_theme]
        for surface, contents in rendered.items():
            alias = out_dir / f"{surface}{suffix}.svg"
            alias.write_text(contents, encoding="utf-8")
            rasterize_svg(
                alias,
                out_dir / f"{surface}{suffix}.png",
                selected_frames[surface],
                rasterizer,
            )
        shot_alias = out_dir / f"shot{suffix}.svg"
        shot_alias.write_text(rendered[selected], encoding="utf-8")
        rasterize_svg(
            shot_alias,
            out_dir / f"shot{suffix}.png",
            selected_frames[selected],
            rasterizer,
        )
    (out_dir / "index.html").write_text(
        render_index(
            frames_by_encoding,
            encodings,
            partial_encodings,
            image_frames,
            selected,
            selected_theme,
        ),
        encoding="utf-8",
    )
    print(
        f"wrote {len(SURFACES)} surfaces across {len(encodings)} complete encodings "
        f"in primary/degraded dark/light PNG+SVG to {out_dir} "
        f"(best: {best_encoding}; selected: {selected}, {selected_theme})"
        + (f"; added {len(image_frames)} renderer-pixel image frames" if image_frames else "")
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
