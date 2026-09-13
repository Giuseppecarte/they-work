#!/usr/bin/env python3
"""Reconstruct encoded pixel geometry; this does not render the terminal's actual font.

Use iteration-2/glyph_metrics.swift to expose font gaps before approving appearance.
"""

import argparse
import importlib.util
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("shot", ROOT / "scripts/shot.py")
shot = importlib.util.module_from_spec(spec)
sys.modules["shot"] = shot
spec.loader.exec_module(shot)


def render_frame(frame, light):
    svg = shot.render_svg(frame, light)
    shot.assert_svg_text_matches_frame(svg, frame)
    # The shared exporter maps blocks to rectangles. Its result is ideal
    # geometry, not evidence that a terminal font can render those blocks.
    return svg


def capture(svg_path, png_path, frame, chrome):
    width = frame.width * shot.CELL_WIDTH + shot.FRAME_PADDING * 2
    height = frame.height * shot.CELL_HEIGHT + shot.FRAME_PADDING * 2
    scratch = svg_path.parent / "tmp"
    scratch.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="capture-", dir=scratch) as temporary:
        temporary = Path(temporary)
        screenshot = temporary / "frame.png"
        command = [
            chrome, "--headless=new", "--no-sandbox", "--disable-gpu",
            "--no-first-run", "--no-default-browser-check", "--hide-scrollbars",
            "--force-device-scale-factor=1", f"--window-size={width},{height + 128}",
            f"--screenshot={screenshot}", f"--user-data-dir={temporary / 'profile'}",
            svg_path.as_uri(),
        ]
        process = subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            deadline = time.monotonic() + 40
            while time.monotonic() < deadline:
                if screenshot.exists():
                    data = screenshot.read_bytes()
                    if data.endswith(b'IEND\xaeB`\x82'):
                        png_path.write_bytes(shot.crop_png(data, width, height))
                        return
                if process.poll() is not None:
                    raise RuntimeError(f"Browser exited without a complete frame: {svg_path}")
                time.sleep(0.2)
            raise TimeoutError(f"Browser did not render within 40 seconds: {svg_path}")
        finally:
            # Some desktop browser builds keep running after --screenshot.
            # Only this process, with its dedicated temporary profile, is closed.
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)


def source_frame(args, view, scratch):
    if not args.revision:
        return shot.parse_golden(view, view, args.theme, args.size, args.encoding)
    filename = f"{view}.{args.theme}.{args.size}.{args.encoding}.golden"
    with tempfile.TemporaryDirectory(prefix="source-", dir=scratch) as temporary:
        path = Path(temporary)
        data = subprocess.check_output([
            "git", "show", f"{args.revision}:crates/theywork-render/tests/goldens/{filename}",
        ], cwd=ROOT)
        (path / filename).write_bytes(data)
        original = shot.GOLDEN_DIR
        try:
            shot.GOLDEN_DIR = path
            return shot.parse_golden(view, view, args.theme, args.size, args.encoding)
        finally:
            shot.GOLDEN_DIR = original


def main():
    print("Geometry reconstruction only: block glyphs become rectangles. "
          "Check native-font evidence before judging terminal appearance.", file=sys.stderr)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", default="after")
    parser.add_argument("--revision", help="Read golden frames from a git revision without changing the working tree")
    parser.add_argument("--encoding", default="sextants", choices=shot.ENCODING_ORDER)
    parser.add_argument("--size", default="normal", choices=("normal", "small"))
    parser.add_argument("--theme", default="dark", choices=("dark", "light"))
    parser.add_argument("--views", nargs="+", default=["office", "cameras", "desk", "phone", "settings", "help"])
    parser.add_argument("--chrome", default=os.environ.get("THEYWORK_SVG_RASTERIZER") or shutil.which("google-chrome") or "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
    args = parser.parse_args()
    out = ROOT / "docs/design-audit/evidence"
    out.mkdir(exist_ok=True)
    scratch = out / "tmp"
    scratch.mkdir(exist_ok=True)
    for view in args.views:
        frame = source_frame(args, view, scratch)
        suffix = "" if args.size == "normal" else "-small"
        theme = "" if args.theme == "dark" else "-light"
        path = out / f"{args.stage}-{view}-{args.encoding}{suffix}{theme}.svg"
        svg = render_frame(frame, args.theme == "light")
        path.write_text(svg)
        capture(path, path.with_suffix('.png'), frame, args.chrome)
        print(path.relative_to(ROOT), flush=True)


if __name__ == "__main__":
    main()
