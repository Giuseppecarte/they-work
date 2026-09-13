#!/usr/bin/env python3
"""Rasterize a captured they-work terminal view, including its Kitty image.

The input is a raw PTY stream from capture-published-image.py.  This is a
documentation aid: it rebuilds terminal cells from the emitted ANSI sequences,
then overlays the final direct Kitty image at its recorded cell placement.
"""

from __future__ import annotations

import argparse
import base64
from dataclasses import dataclass
import html
from pathlib import Path
import shutil
import subprocess
import tempfile

from shot import crop_png, png_dimensions


ESC = "\x1b"
DEFAULT_FOREGROUND = "#e8e2d6"
DEFAULT_BACKGROUND = "#0d0b14"
CELL_WIDTH = 12
CELL_HEIGHT = 22
PADDING = 4


@dataclass
class Cell:
    character: str = " "
    foreground: str = DEFAULT_FOREGROUND
    background: str = DEFAULT_BACKGROUND
    bold: bool = False


@dataclass
class Style:
    foreground: str = DEFAULT_FOREGROUND
    background: str = DEFAULT_BACKGROUND
    bold: bool = False


def rgb(red: str, green: str, blue: str) -> str:
    return f"#{int(red):02x}{int(green):02x}{int(blue):02x}"


def parse_parameters(value: str) -> list[str]:
    return value.split(";") if value else [""]


def number(value: str, default: int = 1) -> int:
    try:
        return int(value) if value else default
    except ValueError:
        return default


def strip_kitty_packets(raw: bytes) -> str:
    """Replace direct image starts with a small in-band placement marker."""
    output = bytearray()
    cursor = 0
    while cursor < len(raw):
        start = raw.find(b"\x1b_G", cursor)
        if start < 0:
            output.extend(raw[cursor:])
            break
        output.extend(raw[cursor:start])
        end = raw.find(b"\x1b\\", start + 3)
        if end < 0:
            break
        control = raw[start + 3 : end].partition(b";")[0]
        if b"a=T" in control:
            output.extend(b"\x1b]they-work-image;" + control + b"\x07")
        cursor = end + 2
    return output.decode("utf-8", errors="replace")


def new_screen(columns: int, rows: int) -> list[list[Cell]]:
    return [[Cell() for _ in range(columns)] for _ in range(rows)]


def erase_line(screen: list[list[Cell]], row: int, start: int, style: Style) -> None:
    for column in range(max(start, 0), len(screen[row])):
        screen[row][column] = Cell(foreground=style.foreground, background=style.background)


def apply_sgr(style: Style, parameters: str) -> None:
    values = parse_parameters(parameters)
    index = 0
    while index < len(values):
        value = number(values[index], 0)
        if value == 0:
            style.foreground = DEFAULT_FOREGROUND
            style.background = DEFAULT_BACKGROUND
            style.bold = False
        elif value == 1:
            style.bold = True
        elif value == 22:
            style.bold = False
        elif value == 39:
            style.foreground = DEFAULT_FOREGROUND
        elif value == 49:
            style.background = DEFAULT_BACKGROUND
        elif value in (38, 48) and index + 4 < len(values) and values[index + 1] == "2":
            color = rgb(values[index + 2], values[index + 3], values[index + 4])
            if value == 38:
                style.foreground = color
            else:
                style.background = color
            index += 4
        index += 1


def parse_terminal(text: str, columns: int, rows: int):
    screen = new_screen(columns, rows)
    style = Style()
    row = 0
    column = 0
    image_placement: tuple[int, int, int, int] | None = None
    index = 0

    while index < len(text):
        character = text[index]
        if character == ESC and index + 1 < len(text):
            next_character = text[index + 1]
            if next_character == "[":
                end = index + 2
                while end < len(text) and not ("@" <= text[end] <= "~"):
                    end += 1
                if end >= len(text):
                    break
                parameters = text[index + 2 : end]
                command = text[end]
                values = parse_parameters(parameters.lstrip("?"))
                if command in ("H", "f"):
                    row = min(max(number(values[0]) - 1, 0), rows - 1)
                    column = min(max(number(values[1]) - 1 if len(values) > 1 else 0, 0), columns - 1)
                elif command == "G":
                    column = min(max(number(values[0]) - 1, 0), columns - 1)
                elif command == "A":
                    row = max(row - number(values[0]), 0)
                elif command == "B":
                    row = min(row + number(values[0]), rows - 1)
                elif command == "C":
                    column = min(column + number(values[0]), columns - 1)
                elif command == "D":
                    column = max(column - number(values[0]), 0)
                elif command == "J":
                    if number(values[0], 0) in (2, 3):
                        screen = new_screen(columns, rows)
                        row = 0
                        column = 0
                elif command == "K":
                    erase_line(screen, row, column, style)
                elif command == "m":
                    apply_sgr(style, parameters)
                elif command == "h" and parameters == "?1049":
                    screen = new_screen(columns, rows)
                    row = 0
                    column = 0
                index = end + 1
                continue
            if next_character == "]":
                end = text.find("\x07", index + 2)
                if end < 0:
                    break
                payload = text[index + 2 : end]
                if payload.startswith("they-work-image;"):
                    fields = dict(
                        item.split("=", 1)
                        for item in payload.partition(";")[2].split(",")
                        if "=" in item
                    )
                    image_placement = (row, column, number(fields.get("c", "0"), 0), number(fields.get("r", "0"), 0))
                index = end + 1
                continue
            index += 2
            continue
        if character == "\r":
            column = 0
        elif character == "\n":
            row = min(row + 1, rows - 1)
        elif character >= " ":
            if column < columns:
                screen[row][column] = Cell(character, style.foreground, style.background, style.bold)
            column += 1
            if column >= columns:
                column = columns - 1
        index += 1
    return screen, image_placement


def render_svg(screen: list[list[Cell]], image: Path, placement: tuple[int, int, int, int] | None) -> str:
    rows = len(screen)
    columns = len(screen[0])
    width = columns * CELL_WIDTH + PADDING * 2
    height = rows * CELL_HEIGHT + PADDING * 2
    output = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">',
        f'<rect width="{width}" height="{height}" fill="#0e0d17"/>',
        f'<rect x="{PADDING}" y="{PADDING}" width="{columns * CELL_WIDTH}" height="{rows * CELL_HEIGHT}" fill="{DEFAULT_BACKGROUND}" stroke="#4e4663" stroke-width="2"/>',
    ]
    for row, cells in enumerate(screen):
        for column, cell in enumerate(cells):
            x = PADDING + column * CELL_WIDTH
            y = PADDING + row * CELL_HEIGHT
            if cell.background != DEFAULT_BACKGROUND:
                output.append(f'<rect x="{x}" y="{y}" width="{CELL_WIDTH}" height="{CELL_HEIGHT}" fill="{cell.background}"/>')
    if placement:
        row, column, span_columns, span_rows = placement
        encoded = base64.b64encode(image.read_bytes()).decode("ascii")
        output.append(
            f'<image x="{PADDING + column * CELL_WIDTH}" y="{PADDING + row * CELL_HEIGHT}" '
            f'width="{span_columns * CELL_WIDTH}" height="{span_rows * CELL_HEIGHT}" '
            f'href="data:image/png;base64,{encoded}" preserveAspectRatio="none" style="image-rendering:pixelated"/>'
        )
    for row, cells in enumerate(screen):
        for column, cell in enumerate(cells):
            if cell.character == " ":
                continue
            x = PADDING + column * CELL_WIDTH + 1
            y = PADDING + row * CELL_HEIGHT + 18
            weight = ' font-weight="bold"' if cell.bold else ""
            output.append(
                f'<text x="{x}" y="{y}" fill="{cell.foreground}"{weight} '
                'font-family="Cascadia Mono, DejaVu Sans Mono, monospace" font-size="18" xml:space="preserve">'
                f'{html.escape(cell.character, quote=False)}</text>'
            )
    output.append("</svg>")
    return "\n".join(output) + "\n"


def rasterize(svg: str, output: Path, columns: int, rows: int) -> None:
    chrome = shutil.which("google-chrome") or shutil.which("chromium")
    if not chrome:
        raise RuntimeError("Google Chrome or Chromium is required to rasterize the terminal frame")
    width = columns * CELL_WIDTH + PADDING * 2
    height = rows * CELL_HEIGHT + PADDING * 2
    with tempfile.TemporaryDirectory(prefix="they-work-published-terminal-") as temporary:
        root = Path(temporary)
        source = root / "frame.svg"
        screenshot = root / "frame.png"
        source.write_text(svg, encoding="utf-8")
        result = subprocess.run(
            [chrome, "--headless=new", "--no-sandbox", "--disable-gpu", "--hide-scrollbars", "--force-device-scale-factor=1", f"--window-size={width},{height + 128}", f"--screenshot={screenshot}", f"--user-data-dir={root / 'profile'}", source.as_uri()],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
        if result.returncode or not screenshot.is_file():
            raise RuntimeError(result.stderr.strip() or result.stdout.strip() or "Chrome did not create a screenshot")
        data = screenshot.read_bytes()
        if png_dimensions(data)[0] != width:
            raise RuntimeError("Chrome rendered the terminal frame at the wrong width")
        output.write_bytes(crop_png(data, width, height))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pty", required=True, type=Path, help="raw PTY stream")
    parser.add_argument("--image", required=True, type=Path, help="final Kitty PNG")
    parser.add_argument("--output", required=True, type=Path, help="destination PNG")
    parser.add_argument("--columns", type=int, default=160)
    parser.add_argument("--rows", type=int, default=48)
    args = parser.parse_args()
    if args.columns <= 0 or args.rows <= 0:
        parser.error("--columns and --rows must be positive")
    if not args.image.is_file() or not args.pty.is_file():
        parser.error("--pty and --image must name existing files")
    screen, placement = parse_terminal(strip_kitty_packets(args.pty.read_bytes()), args.columns, args.rows)
    if placement is None:
        raise RuntimeError("PTY stream has no direct Kitty image placement")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    rasterize(render_svg(screen, args.image, placement), args.output, args.columns, args.rows)
    print(f"rendered {args.columns}x{args.rows} terminal frame to {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
