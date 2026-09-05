#!/usr/bin/env python3
"""Verify the published runtime image through the interactive terminal boundary.

The test intentionally pulls the supplied reference before resolving its immutable
repo digest.  It never accepts a locally built tag as evidence of a release.
"""
import argparse
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import sys
import termios
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]
EXPECTED_LOCALE = {
    "LANG": "C.UTF-8",
    "LC_ALL": "C.UTF-8",
    "LC_CTYPE": "C.UTF-8",
}
HALF_BLOCKS = ("▀", "▄")
QUADRANT_GLYPHS = ("▘", "▝", "▖", "▌", "▞", "▛", "▗", "▚", "▐", "▜", "▙", "▟")
KITTY_REPLY = b"\x1b_Gi=31;OK\x1b\\\x1b[6;16;8t\x1b[8;48;160t"
ITERM_REPLY = b"\x1bP>|iTerm2 3.5.0\x1b\\\x1b[6;16;8t\x1b[8;48;160t"
PROBE_MARKERS = (b"\x1b_G", b"\x1b[c", b"\x1b[16t", b"\x1b[>q")


def command(*args, capture=True, check=True):
    return subprocess.run(
        ["docker", *args],
        cwd=ROOT,
        check=check,
        text=True,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.STDOUT if capture else None,
    )


def published_digest(image):
    command("pull", image, capture=False)
    result = command("image", "inspect", image, "--format", "{{json .RepoDigests}}")
    digests = json.loads(result.stdout)
    if not digests:
        raise RuntimeError(f"docker pull did not yield a published repo digest for {image}")
    return digests[0]


def runtime_environment(image):
    result = command("run", "--rm", "--entrypoint", "/usr/bin/env", image)
    return dict(line.split("=", 1) for line in result.stdout.splitlines() if "=" in line)


def read_pty(master, process, reply, transmission_marker):
    output = bytearray()
    has_graphics = False
    has_quadrants = False
    replied = False
    quit_sent_at = None
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline and process.poll() is None:
        ready, _, _ = select.select([master], [], [], 0.05)
        if ready:
            try:
                chunk = os.read(master, 65536)
            except OSError:
                break
            if not chunk:
                break
            output.extend(chunk)
            if transmission_marker is not None and not has_graphics:
                has_graphics = transmission_marker in output
            if not has_quadrants:
                has_quadrants = any(glyph.encode() in output for glyph in QUADRANT_GLYPHS)
            if reply is not None and not replied and any(marker in output for marker in PROBE_MARKERS):
                os.write(master, reply)
                replied = True
        ready_to_quit = has_graphics if transmission_marker is not None else has_quadrants
        if ready_to_quit and quit_sent_at is None:
            os.write(master, b"q")
            quit_sent_at = time.monotonic()
        if quit_sent_at is not None and time.monotonic() - quit_sent_at > 4:
            break
    return bytes(output), has_graphics, replied


def terminal_frame(image, reply=None, transmission_marker=None, environment=()):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 48, 160, 0, 0))
    with tempfile.TemporaryDirectory(prefix="they-work-published-pty-") as temporary:
        cidfile = Path(temporary) / "container-id"
        try:
            invocation = [
                "docker", "run", "--rm", "-it", "--cidfile", str(cidfile),
                "--network", "none", "--read-only", "--cap-drop", "ALL",
                "--security-opt", "no-new-privileges",
                "-e", "TERM=xterm-256color", "-e", "COLORTERM=truecolor",
            ]
            for name, value in environment:
                invocation.extend(("-e", f"{name}={value}"))
            process = subprocess.Popen(
                [*invocation, image, "--demo"],
                cwd=ROOT,
                stdin=slave,
                stdout=slave,
                stderr=slave,
                preexec_fn=os.setsid,
            )
        finally:
            os.close(slave)
        try:
            output, has_graphics, replied = read_pty(master, process, reply, transmission_marker)
            if cidfile.exists():
                stopped = command("kill", cidfile.read_text().strip(), check=False)
                if stopped.returncode and "No such container" not in stopped.stdout:
                    raise RuntimeError(f"could not stop PTY container: {stopped.stdout.strip()}")
            try:
                process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
            return output, has_graphics, replied
        finally:
            if process.poll() is None:
                process.kill()
                process.wait(timeout=5)
            os.close(master)


def glyph_counts(frame):
    text = frame.decode("utf-8", errors="ignore")
    return {
        "half": sum(text.count(glyph) for glyph in HALF_BLOCKS),
        "quadrant": sum(text.count(glyph) for glyph in QUADRANT_GLYPHS),
        "sextant": sum(0x1FB00 <= ord(glyph) <= 0x1FB3B for glyph in text),
    }


def check_frame(name, frame, has_graphics, replied, expect_graphics, transmission_name):
    failures = []
    if expect_graphics and not replied:
        failures.append(f"{name}: the Kitty-capable PTY did not receive the capability probe")
    if expect_graphics != has_graphics:
        expected = f"a {transmission_name} transmission" if expect_graphics else f"no {transmission_name} transmission"
        failures.append(f"{name}: expected {expected}")
    counts = glyph_counts(frame)
    if not expect_graphics and counts["quadrant"] == 0:
        failures.append(f"{name}: expected quadrant-specific glyphs, got none")
    return failures, counts


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", required=True, help="published image tag or digest to pull")
    args = parser.parse_args()

    image = published_digest(args.image)
    environment = runtime_environment(image)
    failures = [
        f"{name}={environment.get(name)!r}, expected {value!r}"
        for name, value in EXPECTED_LOCALE.items()
        if environment.get(name) != value
    ]
    kitty_frame, kitty_graphics, kitty_replied = terminal_frame(
        image, reply=KITTY_REPLY, transmission_marker=b"\x1b_Ga=T,"
    )
    iterm_frame, iterm_graphics, iterm_replied = terminal_frame(
        image,
        reply=ITERM_REPLY,
        transmission_marker=b"\x1b]1337;File=",
        environment=(("TERM_PROGRAM", "iTerm.app"),),
    )
    fallback_frame, fallback_graphics, fallback_replied = terminal_frame(image)
    graphics_failures, graphics_counts = check_frame(
        "Kitty probe", kitty_frame, kitty_graphics, kitty_replied, expect_graphics=True,
        transmission_name="Kitty graphics",
    )
    iterm_failures, iterm_counts = check_frame(
        "iTerm2 probe", iterm_frame, iterm_graphics, iterm_replied, expect_graphics=True,
        transmission_name="iTerm2 inline-image",
    )
    fallback_failures, fallback_counts = check_frame(
        "no-reply fallback", fallback_frame, fallback_graphics, fallback_replied, expect_graphics=False,
        transmission_name="graphics",
    )
    failures.extend(graphics_failures)
    failures.extend(iterm_failures)
    failures.extend(fallback_failures)

    if failures:
        print("published image verification failed:", file=sys.stderr)
        print("\n".join(f"- {failure}" for failure in failures), file=sys.stderr)
        return 1
    print(f"verified published image {image}")
    print("runtime locale: LANG=C.UTF-8 LC_ALL=C.UTF-8 LC_CTYPE=C.UTF-8")
    print(
        "160x48 PTY glyph counts: "
        f"Kitty probe half={graphics_counts['half']} quadrant={graphics_counts['quadrant']} sextant={graphics_counts['sextant']}; "
        f"iTerm2 probe half={iterm_counts['half']} quadrant={iterm_counts['quadrant']} sextant={iterm_counts['sextant']}; "
        f"no-reply fallback half={fallback_counts['half']} quadrant={fallback_counts['quadrant']} sextant={fallback_counts['sextant']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
