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

slow = len(sys.argv) > 3 and sys.argv[3] == "slow"
pid, master = pty.fork()
if pid == 0:
    os.environ.pop("NO_COLOR", None)
    os.environ["TERM"] = "xterm-256color"
    os.execv(sys.argv[1], [sys.argv[1], "--demo", "--no-save", "--graphics", sys.argv[2] if len(sys.argv) > 2 else "auto"])

fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 36, 120, 0, 0))
os.set_blocking(master, False)
buffered = bytearray()


def exchange(pending, marker, timeout):
    global buffered
    output = buffered
    buffered = bytearray()
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        position = output.find(marker)
        end = (position + len(marker)) if marker == b"\x1b[c" else output.find(b"\x1b[?2026l", position)
        if position >= 0 and end >= 0 and not pending:
            if marker != b"\x1b[c":
                end += len(b"\x1b[?2026l")
            buffered = output[end:]
            return bytes(output[:end])
        readable, writable, _ = select.select(
            [master], [master] if pending else [], [], 0.05
        )
        if writable:
            pending = pending[os.write(master, pending):]
        if readable:
            try:
                data = os.read(master, 4096 if slow else 1024 * 1024)
            except OSError:
                break
            if not data:
                break
            output.extend(data)
            if slow:
                time.sleep(0.01)
    raise AssertionError(f"Terminal did not emit {marker!r} within {timeout}s")


try:
    exchange(b"", b"\x1b[c", 5)
    # Simulated Windows Terminal replies, including Sixel and cell geometry.
    exchange(b"\x1b[?61;4c\x1b[6;20;10t\x1b[?2026;2$y", b"\x1b[?2026l", 10)
    if slow:
        exchange(b"?", b"HELP", 10)
        exchange(b"\x1b", b"Design", 10)
        exchange(b"", b"Simpler graphics", 10)
        slow = False
    # Old hosts redraw for every ignored motion, stranding meaningful input.
    motion = b"\x1b[<35;50;15M" * 1000
    exchange(motion + b"?", b"HELP", 5)
    exchange(b"\x1b", b"Design", 5)
    # A physical Windows key can arrive uppercase when Caps Lock is enabled.
    exchange(b"S", b"appearance", 5)
    exchange(b"Q", b"Design", 5)
    # Help is the second footer button in the demo office at 120x36.
    exchange(motion + b"\x1b[<0;19;36M\x1b[<0;19;36m", b"HELP", 5)
    exchange(b"\x1b", b"Design", 5)
    os.write(master, b"Q")
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
