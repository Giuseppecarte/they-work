"""Drive the real binary with a synthetic Sixel terminal and queued mouse input."""
import fcntl
import os
import pty
import select
import signal
import struct
import sys
import termios
import time

pid, master = pty.fork()
if pid == 0:
    os.environ.pop("NO_COLOR", None)
    os.environ["TERM"] = "xterm-256color"
    os.execv(sys.argv[1], [sys.argv[1], "--demo", "--no-save"])

fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 36, 120, 0, 0))
os.set_blocking(master, False)


def exchange(pending, marker, timeout):
    output = bytearray()
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        readable, writable, _ = select.select(
            [master], [master] if pending else [], [], 0.05
        )
        if writable:
            pending = pending[os.write(master, pending):]
        if readable:
            try:
                data = os.read(master, 1024 * 1024)
            except OSError:
                break
            if not data:
                break
            output.extend(data)
            if marker in output and not pending:
                return bytes(output)
    raise AssertionError(f"Terminal did not emit {marker!r} within {timeout}s")


try:
    exchange(b"", b"\x1b[c", 5)
    # Simulated Windows Terminal replies, including Sixel and cell geometry.
    exchange(b"\x1b[?61;4c\x1b[6;20;10t\x1b[?2026;2$y", b"\x1b[?2026l", 10)
    # Old hosts redraw for every ignored motion, stranding meaningful input.
    motion = b"\x1b[<35;50;15M" * 1000
    exchange(motion + b"?", b"HELP", 5)
    exchange(b"\x1b", b"Design", 5)
    # Help is the second footer button in the demo office at 120x36.
    exchange(motion + b"\x1b[<0;19;36M\x1b[<0;19;36m", b"HELP", 5)
    exchange(b"\x1b", b"Design", 5)
    os.write(master, b"q")
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        exited, status = os.waitpid(pid, os.WNOHANG)
        if exited:
            assert os.waitstatus_to_exitcode(status) == 0
            pid = None
            break
        if select.select([master], [], [], 0.05)[0]:
            try:
                os.read(master, 1024 * 1024)
            except OSError:
                pass
    assert pid is None, "Quit key was not handled promptly"
finally:
    if pid is not None:
        os.kill(pid, signal.SIGKILL)
        os.waitpid(pid, 0)
    os.close(master)
