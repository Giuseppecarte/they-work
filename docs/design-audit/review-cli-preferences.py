#!/usr/bin/env python3
"""Exercise CLI appearance overrides against saved preferences in a native PTY."""
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import tempfile
import termios
import time


root = Path(__file__).resolve().parents[2]
binary = root / 'target/native-macos/release/they-work'
cases = [(['--light'], True, 'top-down'),
         (['--dark'], False, 'top-down'),
         (['--view', 'side'], True, 'side')]
for options, expected_light, expected_projection in cases:
    with tempfile.TemporaryDirectory(dir=root / 'docs/design-audit/tmp') as tmp:
        config = Path(tmp)
        appearance = dict(projection='top-down', light=True, motion=True,
                          name_plates=True, color_depth=None, encoding=None,
                          wardrobe={}, office_palettes={})
        (config / 'appearance.json').write_text(json.dumps(appearance))
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 30, 100, 0, 0))
        env = dict(os.environ, TERM='xterm-256color', TERM_PROGRAM='', COLORTERM='truecolor')
        process = subprocess.Popen([str(binary), '--sources', 'none', '--config-dir',
                                    str(config), *options], stdin=slave, stdout=slave,
                                   stderr=slave, env=env)
        os.close(slave)
        started = time.monotonic()
        sent = False
        try:
            while process.poll() is None and time.monotonic() - started < 8:
                if time.monotonic() - started > 1 and not sent:
                    os.write(master, b'q')
                    sent = True
                ready, _, _ = select.select([master], [], [], 0.1)
                if ready:
                    try:
                        os.read(master, 65536)
                    except OSError:
                        break
        finally:
            if process.poll() is None:
                process.terminate()
            process.wait(timeout=3)
            os.close(master)
        result = json.loads((config / 'appearance.json').read_text())
        print(json.dumps(dict(args=options, exit=process.returncode,
                              before={key: appearance[key] for key in ['projection', 'light']},
                              after={key: result[key] for key in ['projection', 'light']})))
        assert process.returncode == 0
        assert result['light'] == expected_light
        assert result['projection'] == expected_projection
print('PASS: explicit CLI settings override remembered appearance without toggling.')
