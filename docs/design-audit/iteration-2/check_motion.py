#!/usr/bin/env python3
"""Measure native PTY motion using character-clothing pixels, not timestamps.

Images reconstruct the application's actual ANSI cells. They do not claim to
capture native font rasterization. All conversation data and preferences are
synthetic fixtures confined to the audit scratch directory.
"""
import argparse
import hashlib
import importlib.util
import itertools
import json
from pathlib import Path
import sqlite3
import sys
import time

sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location("review_pty", Path(__file__).with_name("review_pty.py"))
pty = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pty)
pty.SCRATCH = ROOT / "docs/design-audit/tmp/iteration-2-motion"
CLOTH = {(79, 158, 232), (47, 111, 174), (127, 189, 242)}
# Independent exhaustive nearest-colour lookup, not a copy of the renderer's
# fast channel-index calculation. ANSI 0–15 depend on terminal theme, so the
# renderer maps authored RGB pixels into the fixed cube/gray entries 16–255.
XTERM = list(itertools.product([0, 95, 135, 175, 215, 255], repeat=3))
XTERM += [(value, value, value) for value in range(8, 239, 10)]
CLOTH_256 = {min(XTERM, key=lambda candidate: sum((a - b) ** 2 for a, b in zip(rgb, candidate))) for rgb in CLOTH}
SKIN = {(240,201,160),(201,154,114),(255,227,196),(217,157,120),(185,120,88),(240,187,152),
        (143,84,61),(211,154,121),(155,98,77),(117,66,53),(189,128,104),(125,73,61),
        (91,48,41),(166,106,91),(240,207,173),(201,162,127),(255,230,204)}
SKIN_256 = {min(XTERM, key=lambda candidate: sum((a-b)**2 for a,b in zip(rgb,candidate))) for rgb in SKIN}
original_fixture = pty.fixture
original_popen = pty.subprocess.Popen
STATE = "idle"
ENV_MODE = "truecolor"
VIEW = "iso"


def native_process(*args, **kwargs):
    if args and isinstance(args[0], (list, tuple)) and "--view" in args[0]:
        command = list(args[0])
        command[command.index("--view") + 1] = VIEW
        args = (command, *args[1:])
    if ENV_MODE == "apple256" and kwargs.get("env") is not None:
        env = kwargs["env"].copy()
        env["TERM_PROGRAM"] = "Apple_Terminal"
        env["TERM"] = "xterm-256color"
        env.pop("COLORTERM", None)
        env.pop("THEYWORK_COLOR", None)
        kwargs["env"] = env
    return original_popen(*args, **kwargs)


def fixture(count, running=False):
    project, source = original_fixture(count, running)
    if STATE == "quiet":
        # An open turn with no reported tool use is still running. The renderer
        # should show contemplation without inventing command execution.
        with sqlite3.connect(source / "thread_history_1.sqlite") as db:
            db.execute("DELETE FROM thread_items")
    return project, source


pty.fixture = fixture
pty.subprocess.Popen = native_process


def fingerprint(image):
    return hashlib.sha256(image.tobytes()).hexdigest()


def clothing(image, screen):
    palette = CLOTH_256 if ENV_MODE == "apple256" else CLOTH
    skin = SKIN_256 if ENV_MODE == "apple256" else SKIN
    skin_hex = {"%02x%02x%02x" % value for value in skin}
    skin_cells = [{x for x in range(screen.columns)
                   if screen.buffer[y][x].fg in skin_hex or screen.buffer[y][x].bg in skin_hex}
                  for y in range(screen.lines)]
    # Blue sky can share a shirt's ANSI index. Count blue only within five
    # terminal rows below nearby face/hand skin, where the actual worker is.
    near_skin = {(y-4)*screen.columns+x for y in range(4,screen.lines-3) for x in range(screen.columns)
                 if any(column in skin_cells[row] for row in range(max(0,y-5),y+1)
                        for column in range(max(0,x-1),min(screen.columns,x+2)))}
    return bytes(int(pixel in palette and ((index//image.width//pty.CH)*screen.columns+index%image.width//pty.CW) in near_skin)
                 for index,pixel in enumerate(image.getdata()))


def main():
    global STATE, ENV_MODE, VIEW
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/native-macos/release/they-work")
    parser.add_argument("--stage", default="motion")
    parser.add_argument("--encodings", nargs="+", default=["half-blocks", "quadrants", "sextants"])
    parser.add_argument("--sizes", nargs="+", default=["160x48", "192x58"])
    parser.add_argument("--states", nargs="+", choices=["idle", "working", "quiet"], default=["idle", "working", "quiet"])
    parser.add_argument("--environment", choices=["truecolor", "apple256"], default="truecolor")
    parser.add_argument("--views", nargs="+", choices=["iso", "top", "side"], default=["iso"])
    parser.add_argument("--resume", action="store_true", help="Keep successful recorded cases and rerun only missing/failed cases")
    args = parser.parse_args()
    ENV_MODE = args.environment
    pty.SCRATCH = ROOT / "docs/design-audit/tmp" / f"iteration-2-{args.stage}"
    output = ROOT / "docs/design-audit/iteration-2/evidence" / args.stage
    output.mkdir(parents=True, exist_ok=True)
    results_path = output / "results.json"
    results = json.loads(results_path.read_text()) if args.resume and results_path.exists() else {}
    failures = []
    for state, encoding, view, motion in itertools.product(args.states, args.encodings, args.views, [True, False]):
        STATE, VIEW = state, view
        view_suffix = "" if view == "iso" else f"-{view}"
        labels = {size:f"{state}-{encoding}{view_suffix}-{size}-{'on' if motion else 'off'}" for size in args.sizes}
        exit_label = f"{state}-{encoding}-{view}-{'on' if motion else 'off'}-exit"
        if (all(results.get(label,{}).get("passed") for label in labels.values())
                and results.get(exit_label,{}).get("terminal_restored")):
            continue
        session = pty.Session(args.binary, 3, encoding, running=state != "idle", motion=motion)
        try:
            for size in args.sizes:
                label = labels[size]
                if results.get(label,{}).get("passed"):
                    continue
                session.resize(*map(int, size.split("x")))
                session.pump(.25)
                frames, room_hashes, body_hashes, clothing_counts, sample_times = [], [], [], [], []
                started = time.monotonic()
                for _ in range(12):
                    session.pump(.24)
                    image = pty.image_of(session.screen)
                    sample_times.append(round((time.monotonic() - started) * 1000))
                    room = image.crop((0, pty.CH * 4, image.width, image.height - pty.CH * 3))
                    mask = clothing(room, session.screen)
                    frames.append(image)
                    room_hashes.append(fingerprint(room))
                    body_hashes.append(hashlib.sha256(mask).hexdigest())
                    clothing_counts.append(sum(mask))
                expected_label = " • idle • " if state == "idle" else " • running • "
                state_preserved = expected_label in "\n".join(session.screen.display)
                record = {
                    "samples": len(frames), "minimum_sample_interval_ms": 240,
                    "sample_times_ms": sample_times,
                    "distinct_room_frames": len(set(room_hashes)),
                    "distinct_clothing_masks": len(set(body_hashes)),
                    "min_clothing_pixels": min(clothing_counts),
                    "max_clothing_pixels": max(clothing_counts),
                    "motion_enabled": motion,
                    "terminal_environment": ENV_MODE,
                    "requested_projection": view,
                    "clothing_rgb_lookup": sorted(CLOTH_256 if ENV_MODE == "apple256" else CLOTH),
                    "source_turn": "completed" if state == "idle" else "inProgress",
                    "status_preserved": state_preserved,
                    "observed_header": session.screen.display[1].strip(),
                    "skin_adjacent_clothing_only": True,
                }
                if motion:
                    passed = (record["distinct_clothing_masks"] >= 2
                              and record["distinct_room_frames"] >= 3
                              and record["min_clothing_pixels"] >= 12)
                else:
                    passed = record["distinct_room_frames"] == 1
                if ENV_MODE == "apple256":
                    record["indexed_ansi_observed"] = (b"\x1b[38;5;" in session.raw or b"\x1b[48;5;" in session.raw)
                    passed = passed and record["indexed_ansi_observed"]
                passed = passed and state_preserved
                record["passed"] = passed
                results[label] = record
                results_path.write_text(json.dumps(results, indent=2) + "\n")
                if not passed:
                    failures.append(label)
                frames[0].save(output / f"{label}.png")
                if encoding == "quadrants" and size == "192x58":
                    durations = [after - before for before, after in zip(sample_times, sample_times[1:])]
                    durations.append(durations[-1])
                    frames[0].save(output / f"{label}.gif", save_all=True, append_images=frames[1:], duration=durations, loop=0)
                print(label, json.dumps(record), flush=True)
        finally:
            results[exit_label] = session.finish()
            results_path.write_text(json.dumps(results, indent=2) + "\n")
    assert not failures, "Motion checks failed: " + ", ".join(failures)


if __name__ == "__main__":
    main()
