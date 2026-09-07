"""Exercise native console Ctrl-C over a real controlling PTY, offline."""
import fcntl
import os
import pathlib
import pty
import select
import signal
import subprocess
import sys
import termios
import time

binary, spec, root = sys.argv[1:]
root = pathlib.Path(root)
master, slave = pty.openpty()


def session():
    os.setsid()
    fcntl.ioctl(slave, termios.TIOCSCTTY, 0)


environment = dict(os.environ, THEYWORK_NATIVE_TEST_SPEC=spec)
process = subprocess.Popen([binary, "--exact", "native_console_process_entry", "--nocapture"],
                           stdin=slave, stdout=slave, stderr=slave,
                           env=environment, preexec_fn=session)
os.close(slave)
try:
    deadline = time.monotonic() + 5
    while not (root / "console.ready").exists():
        if process.poll() is not None or time.monotonic() > deadline:
            raise AssertionError("native console did not become ready")
        time.sleep(0.02)
    os.write(master, b"\x03")
    transcript = bytearray()
    while process.poll() is None and time.monotonic() < deadline:
        if select.select([master], [], [], 0.05)[0]:
            try:
                transcript.extend(os.read(master, 65536))
            except OSError:
                break
    process.wait(timeout=2)
    assert process.returncode == 0, transcript.decode(errors="replace")
    assert (root / "console.returned").read_text() == "restored"
finally:
    if process.poll() is None:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
    os.close(master)
