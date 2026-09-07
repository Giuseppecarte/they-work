#!/usr/bin/env python3
"""Capture a lossless Kitty frame from a published they-work image.

The program runs the supplied registry tag or digest in a real PTY, answers its
Kitty capability probe, and writes the transmitted RGBA pixels as a PNG.  It is
intended for release documentation: the output comes from the pulled immutable
image, never from a local build or a source-side renderer.
"""

from __future__ import annotations

import argparse
import base64
import fcntl
import hashlib
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
import zlib


ROOT = Path(__file__).resolve().parents[1]
KITTY_REPLY = b"\x1b_Gi=31;OK\x1b\\\x1b[6;16;8t\x1b[8;48;160t"
PROBE_MARKERS = (b"\x1b_G", b"\x1b[c", b"\x1b[16t", b"\x1b[>q")
ESC = b"\x1b"
KITTY_START = ESC + b"_G"
ST = ESC + b"\\"
KEY_BYTES = {
    "enter": b"\r",
    "left": b"\x1b[D",
    "right": b"\x1b[C",
    "up": b"\x1b[A",
    "down": b"\x1b[B",
    "phone": b"p",
}


def docker(
    *args: str, capture: bool = True, check: bool = True
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["docker", *args],
        cwd=ROOT,
        check=check,
        text=True,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.STDOUT if capture else None,
    )


def published_digest(image: str) -> str:
    docker("pull", image, capture=False)
    result = docker("image", "inspect", image, "--format", "{{json .RepoDigests}}")
    digests = json.loads(result.stdout)
    if not digests:
        raise RuntimeError(f"docker pull did not yield a published repo digest for {image}")
    return digests[0]


def kitty_parameters(control: bytes) -> dict[bytes, bytes]:
    parameters: dict[bytes, bytes] = {}
    for item in control.split(b","):
        name, separator, value = item.partition(b"=")
        if separator:
            parameters[name] = value
    return parameters


def complete_kitty_frame(stream: bytes) -> tuple[int, int, bytes] | None:
    """Return the newest complete direct Kitty RGBA transmission in *stream*."""
    cursor = 0
    active: tuple[int, int, bytearray] | None = None
    newest: tuple[int, int, bytes] | None = None
    while True:
        start = stream.find(KITTY_START, cursor)
        if start < 0:
            return newest
        end = stream.find(ST, start + len(KITTY_START))
        if end < 0:
            return newest
        packet = stream[start + len(KITTY_START) : end]
        cursor = end + len(ST)
        control, separator, payload = packet.partition(b";")
        if not separator:
            continue
        parameters = kitty_parameters(control)
        if parameters.get(b"a") == b"T" and parameters.get(b"f") == b"32":
            try:
                width = int(parameters[b"s"])
                height = int(parameters[b"v"])
            except (KeyError, ValueError) as error:
                raise ValueError("Kitty frame is missing valid pixel dimensions") from error
            active = (width, height, bytearray(payload))
        elif active is not None:
            active[2].extend(payload)
        if active is None or parameters.get(b"m") != b"0":
            continue
        width, height, encoded = active
        active = None
        try:
            pixels = base64.b64decode(encoded, validate=True)
        except ValueError as error:
            raise ValueError("Kitty frame has invalid base64 data") from error
        expected = width * height * 4
        if len(pixels) != expected:
            raise ValueError(
                f"Kitty frame is {len(pixels)} bytes, expected {expected} RGBA bytes for {width}x{height}"
            )
        newest = (width, height, pixels)


def capture_kitty_frame(
    image: str, timeout: float, keys: tuple[bytes, ...]
) -> tuple[int, int, bytes]:
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 48, 160, 0, 0))
    with tempfile.TemporaryDirectory(prefix="they-work-published-capture-") as temporary:
        cidfile = Path(temporary) / "container-id"
        process = subprocess.Popen(
            [
                "docker", "run", "--rm", "-it", "--cidfile", str(cidfile),
                "--network", "none", "--read-only", "--cap-drop", "ALL",
                "--security-opt", "no-new-privileges", "-e", "TERM=xterm-256color",
                "-e", "COLORTERM=truecolor", image, "--demo",
            ],
            cwd=ROOT,
            stdin=slave,
            stdout=slave,
            stderr=slave,
            preexec_fn=os.setsid,
        )
        os.close(slave)
        stream = bytearray()
        replied = False
        deadline = time.monotonic() + timeout
        final_chunk_seen = False
        next_key = 0
        try:
            while time.monotonic() < deadline and process.poll() is None:
                ready, _, _ = select.select([master], [], [], 0.05)
                if not ready:
                    continue
                try:
                    chunk = os.read(master, 1024 * 1024)
                except OSError:
                    break
                if not chunk:
                    break
                tail = bytes(stream[-4:])
                stream.extend(chunk)
                if not replied and any(marker in stream for marker in PROBE_MARKERS):
                    os.write(master, KITTY_REPLY)
                    replied = True
                final_chunk_seen = final_chunk_seen or b"m=0;" in tail + chunk
                if not final_chunk_seen:
                    continue
                frame = complete_kitty_frame(bytes(stream))
                if frame is not None:
                    if next_key < len(keys):
                        os.write(master, keys[next_key])
                        next_key += 1
                        stream.clear()
                        final_chunk_seen = False
                        continue
                    os.write(master, b"q")
                    return frame
            if not replied:
                raise RuntimeError("published image did not request Kitty capability information")
            raise RuntimeError("published image did not emit a complete Kitty RGBA frame")
        finally:
            if cidfile.exists():
                stopped = docker("kill", cidfile.read_text().strip(), check=False)
                benign = ("No such container", "is not running")
                if stopped.returncode and not any(message in stopped.stdout for message in benign):
                    print(f"warning: could not stop capture container: {stopped.stdout.strip()}", file=sys.stderr)
            if process.poll() is None:
                process.kill()
            process.wait(timeout=5)
            os.close(master)


def png(width: int, height: int, pixels: bytes) -> bytes:
    def chunk(kind: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    stride = width * 4
    scanlines = b"".join(b"\0" + pixels[offset : offset + stride] for offset in range(0, len(pixels), stride))
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(scanlines, 9)) + chunk(b"IEND", b"")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", required=True, help="published image tag or digest to pull")
    parser.add_argument("--output", required=True, type=Path, help="destination PNG path")
    parser.add_argument("--record", type=Path, help="optional JSON provenance record")
    parser.add_argument(
        "--key",
        action="append",
        choices=tuple(KEY_BYTES),
        default=[],
        help="UI key to send after each complete frame; repeat to traverse views",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=60,
        help="maximum seconds to wait for the complete Kitty transmission (default: 60)",
    )
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error("--timeout must be positive")

    image = published_digest(args.image)
    keys = tuple(KEY_BYTES[name] for name in args.key)
    width, height, pixels = capture_kitty_frame(image, args.timeout, keys)
    encoded = png(width, height, pixels)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(encoded)
    record = {
        "source": "published-kitty-transmission",
        "image": image,
        "protocol": "kitty-direct",
        "keys": args.key,
        "width": width,
        "height": height,
        "rgba_sha256": hashlib.sha256(pixels).hexdigest(),
        "png_sha256": hashlib.sha256(encoded).hexdigest(),
    }
    if args.record:
        args.record.parent.mkdir(parents=True, exist_ok=True)
        args.record.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"captured {width}x{height} Kitty frame from {image} to {args.output}")
    print(json.dumps(record, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
