#!/usr/bin/env python3
"""Execute the actual source chooser in an isolated native PTY.

No terminal app is controlled. No real provider or conversation directory is
available to the process. PNGs replay captured ANSI cells with Menlo/Pillow.
"""
import argparse
import codecs
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time

# Load the active interpreter's Pillow before the legacy helper adds its pyte cache.
from PIL import Image, ImageDraw, ImageFont  # noqa: F401

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location('pty_base', ROOT / 'docs/design-audit/iteration-2/review_pty.py')
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)
BOLD = base.ImageFont.truetype('/System/Library/Fonts/Menlo.ttc', 15, index=1)
TAB, BACKTAB, ESC, F5 = b'\t', b'\x1b[Z', b'\x1b', b'\x1b[15~'


class Session(base.Session):
    def __init__(self, binary, folder, columns, rows, mode):
        self.cols, self.rows = columns, rows
        self.master, self.slave = os.openpty()
        self.raw = bytearray()
        self.screen = base.pyte.Screen(columns, rows)
        self.stream = base.pyte.Stream(self.screen)
        self.decoder = codecs.getincrementaldecoder('utf8')('replace')
        self.focus = 5
        self.trace = []
        for name in ['home/.codex', 'home/.claude', 'config', 'xdg', 'tmp', 'bin']:
            (folder / name).mkdir(parents=True, exist_ok=True)
        (folder / 'config/appearance.json').write_text(json.dumps({'light': mode == 'light', 'motion': False}))
        for provider in ['codex', 'claude']:
            stub = folder / 'bin' / provider
            stub.write_text(f'''#!{sys.executable}
import json,sys
with open({str(folder / 'provider-probes.jsonl')!r},'a') as f:
    f.write(json.dumps({{'provider':{provider!r},'args':sys.argv[1:]}})+'\\n')
print('Isolated source chooser fixture: provider unavailable',file=sys.stderr)
raise SystemExit(127)
''')
            stub.chmod(0o700)
        env = {'PATH': str(folder / 'bin') + ':/usr/bin:/bin:/usr/sbin:/sbin',
               'HOME': str(folder / 'home'), 'XDG_CONFIG_HOME': str(folder / 'xdg'),
               'TMPDIR': str(folder / 'tmp'), 'LANG': 'en_US.UTF-8',
               'TERM': 'xterm-256color', 'COLORTERM': 'truecolor',
               'CODEX_HOME': str(folder / 'home/.codex'),
               'CLAUDE_CONFIG_DIR': str(folder / 'home/.claude'),
               'THEYWORK_ENCODING': 'half-blocks'}
        if mode == 'no-color':
            env['NO_COLOR'] = '1'
        if mode == '256':
            env['THEYWORK_COLOR'] = '256'
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack('HHHH', rows, columns, columns * 8, rows * 16))

        def child():
            os.setsid()
            fcntl.ioctl(self.slave, termios.TIOCSCTTY, 0)

        args = [str(binary), '--setup', '--config-dir', str(folder / 'config'),
                '--codex-home', str(folder / 'home/.codex'), '--claude-home', str(folder / 'home/.claude')]
        helper = "import subprocess,sys,termios,json,signal; before=termios.tcgetattr(0); p=subprocess.Popen(sys.argv[1:]); signal.signal(signal.SIGTERM,lambda *_:p.send_signal(signal.SIGTERM)); code=p.wait(); after=termios.tcgetattr(0); mask=~getattr(termios,'PENDIN',0); before[3]&=mask; after[3]&=mask; print('PTY_RESULT '+json.dumps({'exit':code,'terminal_restored':before==after}),flush=True)"
        self.process = subprocess.Popen([sys.executable, '-c', helper, *args], stdin=self.slave, stdout=self.slave, stderr=self.slave, env=env, cwd=folder, preexec_fn=child)

    def text(self):
        return '\n'.join(self.screen.display)

    def pump(self, duration=.12):
        until = time.monotonic() + duration
        while time.monotonic() < until:
            if select.select([self.master], [], [], max(0, until - time.monotonic()))[0]:
                try:
                    data = os.read(self.master, 1048576)
                except OSError:
                    break
                if not data:
                    break
                self.raw.extend(data)
                self.stream.feed(self.decoder.decode(data))

    def key(self, value):
        os.write(self.master, value)
        self.pump()

    def wait(self, check, label, timeout=8):
        until = time.monotonic() + timeout
        while time.monotonic() < until:
            if check():
                return
            self.pump(.1)
            assert self.process.poll() is None, f'Exited during {label}: {self.text()}'
        raise AssertionError(f'Timeout during {label}: {self.text()}')

    def row_with(self, text):
        for y, row in enumerate(self.screen.display):
            if text in row:
                return y, row
        raise AssertionError(f'Missing visible text {text!r}: {self.text()}')

    def assert_focus(self, expected):
        if expected < 4:
            y, row = self.row_with('Claude Code' if expected < 2 else 'Codex')
            row = self.screen.display[y + expected % 2]
            assert row.lstrip().startswith('>'), (expected, row)
        elif expected == 4:
            _, row = self.row_with('Remember on this computer')
            assert row.lstrip().startswith('>'), row
        else:
            token = ['Connect', 'Demo', 'Back'][expected - 5]
            y, row = self.row_with('[ ' + token + ' ]')
            x = row.index('[ ' + token + ' ]')
            assert self.screen.buffer[y][x + 2].bold, (expected, row)
        self.focus = expected
        self.trace.append({'focus': expected, 'screen_sha256': hashlib.sha256(self.text().encode()).hexdigest()})

    def go(self, target):
        for _ in range((target - self.focus) % 8):
            self.key(TAB)
            self.assert_focus((self.focus + 1) % 8)

    def save(self, path):
        path.parent.mkdir(parents=True, exist_ok=True)
        image = base.image_of(self.screen, literal_font=True)
        draw = base.ImageDraw.Draw(image)
        for y in range(self.rows):
            for x in range(self.cols):
                cell = self.screen.buffer[y][x]
                if not cell.bold:
                    continue
                fg, bg = base.color(cell.fg), base.color(cell.bg, True)
                if cell.reverse:
                    fg, bg = bg, fg
                ox, oy = x * base.CW, y * base.CH
                draw.rectangle((ox, oy, ox + base.CW - 1, oy + base.CH - 1), fill=bg)
                draw.text((ox, oy), cell.data, font=BOLD, fill=fg)
        image.save(path.with_suffix('.png'))
        path.with_suffix('.txt').write_text('\n'.join(line.rstrip() for line in self.screen.display).rstrip() + '\n')

    def finish(self):
        result = super().finish()
        return result


def run_case(binary, scratch, output, cols, rows, mode):
    label = f'{cols}x{rows}-{mode}'
    folder = Path(tempfile.mkdtemp(prefix=label + '-', dir=scratch))
    out = output / label
    out.mkdir(parents=True, exist_ok=True)
    result = {'cells': [cols, rows], 'mode': mode, 'fixture_root': str(folder), 'real_provider_calls': 0}
    session = None
    try:
        session = Session(binary, folder, cols, rows, mode)
        session.wait(lambda: 'Connections / Sources' in session.text(), 'source chooser startup')
        session.assert_focus(5)
        for text in ['Claude Code', 'Codex', 'Remember on this computer', '[ Connect ]', '[ Demo ]', '[ Back ]']:
            session.row_with(text)
        session.save(out / '01-chooser')
        initial_ansi = bytes(session.raw)
        result['initial_rgb_sgr'] = bool(re.search(rb'\x1b\[[0-9;]*(?:38|48);2;', initial_ansi))
        result['initial_background'] = base.color(session.screen.buffer[0][0].bg, True)
        if mode == 'light':
            value = result['initial_background'].lstrip('#')
            assert sum(int(value[i:i + 2], 16) for i in (0, 2, 4)) / 3 > 180, 'Saved light appearance was not applied'
        for direction, key in [(1, TAB), (-1, BACKTAB)]:
            for step in range(8):
                session.key(key)
                session.assert_focus((session.focus + direction) % 8)
                session.save(out / f'focus-{"forward" if direction == 1 else "reverse"}-{step + 1:02}')
        assert session.focus == 5
        session.go(1)
        before = session.text()
        session.key(b'e')
        session.wait(lambda: 'Editing folder' in session.text(), 'folder editor')
        session.key(b'\x15')
        session.key(b'\x1b[200~/not-a-real-source\x1b[201~')
        session.save(out / '02-editing')
        session.key(ESC)
        session.wait(lambda: 'Editing folder' not in session.text(), 'cancel folder edit')
        assert session.text() == before, 'Cancelled edit changed the source path or form state'
        session.save(out / '03-edit-cancelled')
        session.go(0)
        session.key(b' ')
        assert '[ ] Claude Code' in session.text()
        session.go(2)
        session.key(b' ')
        assert '[ ] Codex' in session.text()
        session.save(out / '04-both-off')
        session.key(F5)
        session.wait(lambda: 'Connections / Sources' not in session.text() and ('TOWER' in session.text() or 'No sources' in session.text()), 'empty tower after F5')
        session.save(out / '05-empty-tower')
        saved = json.loads((folder / 'config/connections.json').read_text())
        assert saved['codex'] is False and saved['claude'] is False, saved
        assert saved['codex_home'] == str(folder / 'home/.codex') and saved['claude_home'] == str(folder / 'home/.claude'), saved
        result.update(session.finish())
        result['alternate_screen_restored'] = session.raw.rfind(b'\x1b[?1049l') > session.raw.rfind(b'\x1b[?1049h') >= 0
        assert result['alternate_screen_restored'], 'The alternate screen was not left'
        result['focus_transitions'] = session.trace
        result['edited_path_was_cancelled'] = True
        result['both_disabled_persisted'] = True
        if mode in ['256', 'no-color']:
            assert not result['initial_rgb_sgr'], f'{mode} source chooser emitted truecolor SGR'
        result['passed'] = True
    except Exception as error:
        result['passed'] = False
        result['error'] = str(error)
        if session is not None:
            session.save(out / 'failure')
            if session.process.poll() is None:
                session.process.send_signal(signal.SIGTERM)
                session.pump(.3)
                if session.process.poll() is None:
                    session.process.kill()
    finally:
        if session is not None:
            (out / 'session.ansi').write_bytes(session.raw)
            for fd in [session.master, session.slave]:
                try:
                    os.close(fd)
                except OSError:
                    pass
        probes = folder / 'provider-probes.jsonl'
        result['fake_provider_probes'] = [json.loads(line) for line in probes.read_text().splitlines()] if probes.exists() else []
        result['evidence'] = {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(out.iterdir()) if path.suffix in ['.png', '.txt', '.ansi']}
        (out / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    return result


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary', type=Path, required=True)
    p.add_argument('--output', type=Path, default=ROOT / 'docs/design-audit/iteration-6/evidence/sources-pty')
    p.add_argument('--cases', nargs='*', help='Optional COLSxROWS-MODE subset')
    a = p.parse_args()
    binary = a.binary.resolve()
    scratch = ROOT / 'docs/design-audit/tmp/iteration-6-sources-pty'
    scratch.mkdir(parents=True, exist_ok=True)
    a.output.mkdir(parents=True, exist_ok=True)
    cases = [(32, 14, 'dark'), (80, 24, 'dark'), (120, 36, 'dark'), (192, 58, 'dark'), (110, 80, 'dark'), (240, 70, 'dark'), (80, 24, 'light'), (80, 24, 'no-color'), (80, 24, '256')]
    if a.cases:
        cases = [case for case in cases if f'{case[0]}x{case[1]}-{case[2]}' in a.cases]
    result = {'kind': 'Actual executable in a native PTY; ANSI-cell PNG replay, not terminal-window screenshots', 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'binary': str(binary), 'cases': {}}
    for cols, rows, mode in cases:
        name = f'{cols}x{rows}-{mode}'
        assert hashlib.sha256(binary.read_bytes()).hexdigest() == result['binary_sha256'], 'Binary changed during the matrix'
        result['cases'][name] = run_case(binary, scratch, a.output, cols, rows, mode)
        print(name, 'PASS' if result['cases'][name]['passed'] else 'FAIL', flush=True)
        (a.output / 'results.json').write_text(json.dumps(result, indent=2) + '\n')
    if not all(case['passed'] for case in result['cases'].values()):
        raise SystemExit(1)


if __name__ == '__main__':
    main()
