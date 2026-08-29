#!/usr/bin/env python3
"""Export the renderer's checked-in deterministic frames as reviewable SVGs."""

from __future__ import annotations

import argparse
import html
import re
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
GOLDEN_DIR = ROOT / "crates" / "theywork-render" / "tests" / "goldens"
DARK_FOREGROUND = "#e8e2d6"
DARK_BACKGROUND = "#0d0b14"
LIGHT_FOREGROUND = "#3a352c"
LIGHT_BACKGROUND = "#f4efe4"

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


def parse_golden(surface: str, renderer_view: str, theme: str) -> Frame:
    path = GOLDEN_DIR / f"{renderer_view}.{theme}.normal.golden"
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
    cell_width = 12
    cell_height = 22
    padding = 4
    width = frame.width * cell_width + padding * 2
    height = frame.height * cell_height + padding * 2
    page_background = "#f5f0e6" if light else "#0e0d17"
    border = "#786f5f" if light else "#4e4663"
    frame_background = LIGHT_BACKGROUND if light else DARK_BACKGROUND
    title = f"they-work {frame.surface} at fixed time {frame.now}"

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


def render_index(frames: dict[str, Frame], selected: str, light: bool) -> str:
    page_background = "#f5f0e6" if light else "#0e0d17"
    ink = "#211d2b" if light else "#f8e2b6"
    muted = "#5d566b" if light else "#a49dbe"
    cards = []
    for surface, (_, label) in SURFACES.items():
        selected_marker = " selected" if surface == selected else ""
        cards.append(
            f'<section class="card{selected_marker}"><h2>{html.escape(label)}</h2>'
            f'<p>{frames[surface].width}×{frames[surface].height} cells · '
            f'fixed time {frames[surface].now}</p><img src="{surface}.svg" '
            f'alt="{html.escape(label)} frame"></section>'
        )
    return "\n".join(
        [
            "<!doctype html>",
            '<html lang="en"><head><meta charset="utf-8">',
            "<title>they-work review shots</title>",
            "<style>",
            f"body{{margin:2rem;background:{page_background};color:{ink};font-family:system-ui,sans-serif}}",
            "main{display:grid;grid-template-columns:repeat(auto-fit,minmax(30rem,1fr));gap:1.5rem}",
            f".card{{padding:1rem;border:2px solid {muted};border-radius:.5rem}}",
            f".card.selected{{border-color:{ink};box-shadow:0 0 0 3px {muted}}}",
            "h1,h2{margin-top:0}p{color:" + muted + "}img{display:block;max-width:100%;height:auto}",
            "</style></head><body>",
            "<h1>they-work review shots</h1>",
            f"<p>Selected view: {html.escape(selected)} · light mode: {str(light).lower()}</p>",
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
        theme = "light" if light else "dark"
        frames = {
            surface: parse_golden(surface, renderer_view, theme)
            for surface, (renderer_view, _) in SURFACES.items()
        }
        timestamps = {frame.now for frame in frames.values()}
        dimensions = {(frame.width, frame.height) for frame in frames.values()}
        if len(timestamps) != 1 or len(dimensions) != 1:
            raise ValueError("surface goldens do not share one fixed timestamp and size")
    except ValueError as error:
        parser.error(str(error))

    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    rendered = {surface: render_svg(frame, light) for surface, frame in frames.items()}
    for surface, contents in rendered.items():
        (out_dir / f"{surface}.svg").write_text(contents, encoding="utf-8")
    (out_dir / "shot.svg").write_text(rendered[selected], encoding="utf-8")
    (out_dir / "index.html").write_text(
        render_index(frames, selected, light), encoding="utf-8"
    )
    print(f"wrote {len(rendered)} surfaces to {out_dir} (selected: {selected})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
