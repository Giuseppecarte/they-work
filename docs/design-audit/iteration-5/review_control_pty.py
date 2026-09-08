#!/usr/bin/env python3
"""Real TUI → local fake providers, with isolated homes and a real native PTY.

Never invokes an installed provider, logs in, or executes the displayed command.
PNGs replay ANSI cells, not a terminal-window screenshot.
"""
import argparse
import base64
import codecs
import fcntl
import hashlib
import importlib.util
import io
import json
import os
import re
from pathlib import Path
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location('pty_base', ROOT / 'docs/design-audit/iteration-2/review_pty.py')
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)
KEY = {'esc': b'\x1b', 'f4': b'\x1bOS', 'f5': b'\x1b[15~', 'right': b'\x1b[C', 'f7': b'\x1b[18~', 'backtab': b'\x1b[Z'}


def records(path):
    return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []


class Session(base.Session):
    def __init__(self, binary, folder, graphics=False, columns=120, rows=36, mouse=True):
        self.folder = folder
        self.graphics, self.replied = graphics, False
        self.text_pending = b''
        self.cols, self.rows = columns, rows
        self.master, self.slave = os.openpty()
        self.raw = bytearray()
        self.screen = base.pyte.Screen(self.cols, self.rows)
        self.stream = base.pyte.Stream(self.screen)
        self.decoder = codecs.getincrementaldecoder('utf8')('replace')
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack('HHHH', self.rows, self.cols, self.cols * 8, self.rows * 16))

        def terminal_owner():
            os.setsid()
            fcntl.ioctl(self.slave, termios.TIOCSCTTY, 0)

        # Deliberately do not inherit credentials or provider session variables.
        env = {'PATH': str(folder / 'bin') + ':/usr/bin:/bin:/usr/sbin:/sbin',
               'HOME': str(folder / 'home'), 'XDG_CONFIG_HOME': str(folder / 'xdg'),
               'TMPDIR': str(folder / 'tmp'), 'LANG': 'en_US.UTF-8',
               'TERM': 'xterm-256color', 'COLORTERM': 'truecolor',
               'THEYWORK_ENCODING': 'half-blocks',
               'CODEX_HOME': str(folder / 'codex'), 'CLAUDE_CONFIG_DIR': str(folder / 'claude')}
        if graphics:
            env.update(TERM='xterm-kitty', TERM_PROGRAM='kitty')
            del env['THEYWORK_ENCODING']
        args = [str(binary), '--sources', 'all', '--codex-home', str(folder / 'codex'),
                '--claude-home', str(folder / 'claude'), '--config-dir', str(folder / 'settings'), '--mouse=' + ('on' if mouse else 'off')]
        helper = "import subprocess,sys,termios,json,signal; before=termios.tcgetattr(0); p=subprocess.Popen(sys.argv[1:]); signal.signal(signal.SIGTERM,lambda *_:p.send_signal(signal.SIGTERM)); code=p.wait(); after=termios.tcgetattr(0); mask=~getattr(termios,'PENDIN',0);before[3]&=mask;after[3]&=mask;print('PTY_RESULT '+json.dumps({'exit':code,'terminal_restored':before==after}),flush=True)"
        self.process = subprocess.Popen([sys.executable, '-c', helper, *args],
            stdin=self.slave, stdout=self.slave, stderr=self.slave, env=env,
            cwd=folder / 'vertical-project', preexec_fn=terminal_owner)
        self.pump(1)

    def pump(self, duration=.25):
        end = time.monotonic() + duration
        while time.monotonic() < end:
            if select.select([self.master], [], [], max(0, end - time.monotonic()))[0]:
                data = os.read(self.master, 1048576)
                if not data:
                    break
                self.raw.extend(data)
                if self.graphics and not self.replied and b'a=q' in self.raw:
                    os.write(self.master, f'\x1b_Gi=31;OK\x1b\\\x1b[6;16;8t\x1b[8;{self.rows};{self.cols}t'.encode())
                    self.replied = True
                self.text_pending += data
                while self.text_pending:
                    start = self.text_pending.find(b'\x1b_')
                    if start < 0:
                        # Preserve a split ESC introducer across reads.
                        tail = 1 if self.text_pending.endswith(b'\x1b') else 0
                        text = self.text_pending[:-tail] if tail else self.text_pending
                        self.stream.feed(self.decoder.decode(text))
                        self.text_pending = self.text_pending[-1:] if tail else b''
                        break
                    self.stream.feed(self.decoder.decode(self.text_pending[:start]))
                    self.text_pending = self.text_pending[start:]
                    end_packet = self.text_pending.find(b'\x1b\\')
                    if end_packet < 0:
                        break
                    self.text_pending = self.text_pending[end_packet + 2:]

    def save(self, path):
        super().save(path)
        path.with_suffix('.cells.json').unlink()
        if self.graphics and path.name.startswith(('04-', '08-')):
            image, active = None, None
            for packet in re.finditer(rb'\x1b_G([^;\x1b]*);([^\x1b]*)\x1b\\', self.raw):
                params = dict(part.split(b'=', 1) for part in packet[1].split(b',') if b'=' in part)
                if params.get(b'a') == b'T' and params.get(b'f') in (b'32', b'100'):
                    active = (params, bytearray(packet[2]))
                elif active is not None:
                    active[1].extend(packet[2])
                if active is not None and params.get(b'm', b'0') == b'0':
                    info, data = active
                    decoded = base64.b64decode(data, validate=True)
                    if info[b'f'] == b'100':
                        image = base.Image.open(io.BytesIO(decoded)).convert('RGBA')
                    else:
                        image = base.Image.frombytes('RGBA', (int(info[b's']), int(info[b'v'])), decoded)
                    active = None
            assert image is not None, 'Room has no complete transmitted RGBA frame'
            image.save(path.with_name(path.name + '-art.png'))

    def text(self):
        return '\n'.join(self.screen.display)

    def wait(self, check, label, timeout=12):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            if check():
                return
            self.pump(.15)
            assert self.process.poll() is None, f'TUI exited during {label}: {self.text()}'
        raise AssertionError(f'Timed out: {label}\n{self.text()}')

    def paste(self, value):
        self.key(b'\x1b[200~' + value.encode() + b'\x1b[201~')


def prepare(folder):
    for name in ['bin', 'home', 'xdg', 'tmp', 'codex', 'claude', 'settings', 'vertical-project/.git']:
        (folder / name).mkdir(parents=True, exist_ok=True)
    (folder / 'settings/connections.json').write_text(json.dumps({
        'codex': True, 'claude': True,
        'codex_home': str(folder / 'codex'), 'claude_home': str(folder / 'claude')}))
    (folder / 'settings/appearance.json').write_text(json.dumps({'motion': False}))
    original = ROOT / 'crates/theywork-control/tests/fake_provider.py'
    source = original.read_text()
    completed = '                event("turn/completed", {"threadId": child, "turn": {"id": f"child-turn-{index}", "status": "completed"}})'
    assert source.count(completed) == 1
    source = source.replace('if prompt == "team":', 'if prompt in ("team", "team-active"):')
    source = source.replace(completed, '                if prompt != "team-active":\n    ' + completed)
    # Preserve the fixture protocol but keep delegated turns active to exercise
    # the meeting room, rather than only the completed-family roster.
    fixture = folder / 'fake-provider.py'
    fixture.write_text(source)
    for provider in ['codex', 'claude']:
        # Both provider names resolve exclusively to this generated Python stub.
        program = folder / 'bin' / provider
        body = f'''#!{sys.executable}
import json,os,pathlib,sys,termios
folder=pathlib.Path({str(folder)!r})
provider={provider!r}
args=sys.argv[1:]
with (folder/'invocations.jsonl').open('a') as f:
    f.write(json.dumps({{'provider':provider,'args':args,'pid':os.getpid(),'ppid':os.getppid(),'cwd':os.getcwd(),'home':os.environ.get('HOME'),'source':os.environ.get('CODEX_HOME' if provider=='codex' else 'CLAUDE_CONFIG_DIR')}})+'\\n')
if args==['--version']:
    print('offline-console-fixture 2.1.248')
elif args==['--help']:
    print('--help --version')
elif provider=='codex' and args==['app-server']:
    os.execv(sys.executable,[sys.executable,{str(fixture)!r},str(folder/'provider.jsonl')])
elif provider=='claude' and args[:1]==['--']:
    flags=termios.tcgetattr(0)
    (folder/'native.json').write_text(json.dumps({{'args':args,'cwd':os.getcwd(),'source':os.environ.get('CLAUDE_CONFIG_DIR'),'tty':os.isatty(0),'canonical':bool(flags[3]&termios.ICANON),'echo':bool(flags[3]&termios.ECHO)}}))
    print('NATIVE_FIXTURE_READY: press Enter to return',flush=True)
    sys.stdin.readline()
    print('NATIVE_FIXTURE_RETURN',flush=True)
else:
    print('Unexpected fixture invocation',file=sys.stderr)
    sys.exit(4)
'''
        program.write_text(body)
        program.chmod(0o700)


def state(folder):
    paths = list((folder / 'settings/control').glob('*/state.json'))
    return json.loads(paths[0].read_text()) if paths else {}


def stop_owned_fixture_hosts(folder):
    # Only parent/child PIDs recorded by our own fake app-server are candidates.
    for invocation in records(folder / 'invocations.jsonl'):
        if invocation['provider'] == 'codex' and invocation['args'] == ['app-server']:
            for pid in [invocation['pid'], invocation['ppid']]:
                try:
                    os.kill(pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass


def click_text(session, label):
    for y, row in reversed(list(enumerate(session.screen.display))):
        x = row.find(label)
        if x >= 0:
            # SGR press and release use terminal coordinates, starting at one.
            x += len(label) // 2 + 1
            session.key(f'\x1b[<0;{x};{y + 1}M\x1b[<0;{x};{y + 1}m'.encode())
            return [x, y + 1]
    raise AssertionError(f'No visible click target {label!r}\n{session.text()}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/native-macos/release/they-work')
    parser.add_argument('--stage', default='control-pty')
    parser.add_argument('--columns', type=int, default=120)
    parser.add_argument('--rows', type=int, default=36)
    args = parser.parse_args()
    binary = args.binary.resolve()
    scratch = ROOT / 'docs/design-audit/tmp/iteration-5-control-pty'
    scratch.mkdir(parents=True, exist_ok=True)
    out = ROOT / 'docs/design-audit/iteration-5/evidence' / args.stage
    out.mkdir(parents=True, exist_ok=True)
    folder = Path(tempfile.mkdtemp(prefix='run-', dir=scratch))
    prepare(folder)
    result = {'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
              'fixture_root': str(folder), 'real_provider_calls': 0,
              'terminal_cells': [args.columns, args.rows],
              'encoding': 'half-block fallback', 'images': 'ANSI-cell replay, not a terminal-window screenshot'}
    session = None
    current = 'startup'
    try:
        session = Session(binary, folder, columns=args.columns, rows=args.rows)
        session.wait(lambda: len(records(folder / 'invocations.jsonl')) >= 3, 'offline capability probes')
        assert b'\x1b[?1000h' in session.raw, 'Mouse capture was not enabled'
        session.key(b'n')
        session.wait(lambda: 'NEW TASK' in session.text(), 'new task panel')
        session.paste(str(folder / 'vertical-project'))
        session.key(b'\t')
        session.paste('approval')
        session.save(out / '01-new-task')
        current = 'ui-start-approval'
        click_text(session, 'Start task')
        session.wait(lambda: any(r.get('method') == 'turn/start' for r in records(folder / 'provider.jsonl')), 'visible start button reaches provider')
        session.wait(lambda: state(folder).get('pending_requests'), 'pending request returned')
        start = next(r for r in records(folder / 'provider.jsonl') if r.get('method') == 'thread/start')
        turn = next(r for r in records(folder / 'provider.jsonl') if r.get('method') == 'turn/start')
        assert start['params']['cwd'] == str(folder / 'vertical-project')
        assert turn['params']['threadId'] == 'managed-1'
        assert turn['params']['input'] == [{'type': 'text', 'text': 'approval'}]
        session.wait(lambda: 'Sending to' not in session.text(), 'acknowledgement in UI')
        result[current] = {'native_thread': 'managed-1', 'native_turn': 'turn-1', 'literal_project_and_prompt': True, 'visible_start_click': True}

        current = 'explicit-scoped-approval'
        session.key(KEY['f4'])
        session.wait(lambda: 'test-only-command' in session.text(), 'exact approval command visible')
        assert 'REVIEW REQUEST' in session.text() and 'NEW TASK' not in session.text()
        assert 'Allow this request' in session.text() and 'managed-1' in session.text()
        session.save(out / '02-review-request')
        coordinates = click_text(session, 'Allow this request')
        session.wait(lambda: any(r.get('id') == 'approve-1' and 'result' in r for r in records(folder / 'provider.jsonl')), 'exact clicked approval written')
        replies = [r for r in records(folder / 'provider.jsonl') if r.get('id') == 'approve-1' and 'result' in r]
        assert len(replies) == 1 and replies[0]['result'] == {'decision': 'accept'}, replies
        session.wait(lambda: not state(folder).get('pending_requests'), 'provider resolution clears approval')
        result[current] = {'command_visible': 'test-only-command', 'reply': replies[0], 'reply_count': 1, 'click_coordinates': coordinates}
        session.key(KEY['esc'])
        session.wait(lambda: 'vertical-project' in session.text(), 'new worker reaches world')

        current = 'known-project-picker'
        session.key(b'n')
        session.key(KEY['f7'])
        session.wait(lambda: 'CHOOSE A PROJECT' in session.text(), 'known project picker')
        assert 'vertical-project' in session.text()
        session.save(out / '03-project-picker')
        session.key(b'\r')
        session.wait(lambda: 'NEW TASK' in session.text() and 'CHOOSE A PROJECT' not in session.text(), 'selected known project returns to form')
        session.paste('working')
        session.key(KEY['f5'])
        session.wait(lambda: len([r for r in records(folder / 'provider.jsonl') if r.get('method') == 'turn/start']) == 2, 'second start uses known project')
        starts = [r for r in records(folder / 'provider.jsonl') if r.get('method') == 'thread/start']
        assert all(r['params']['cwd'] == str(folder / 'vertical-project') for r in starts)
        result[current] = {'selected_project': str(folder / 'vertical-project'), 'literal_cwd_preserved': True}
        session.key(KEY['esc'])

        current = 'native-console-mouse-roundtrip'
        session.key(b'n')
        session.key(KEY['backtab'])  # message → project
        session.key(KEY['backtab'])  # project → provider
        session.key(KEY['right'])
        session.key(b'\t\t')  # provider → project → message
        session.paste('native return; literal $(text)')
        native_start = len(session.raw)
        session.key(KEY['f5'])
        session.wait(lambda: (folder / 'native.json').exists() and b'NATIVE_FIXTURE_READY' in session.raw, 'native child receives real terminal')
        native = json.loads((folder / 'native.json').read_text())
        assert native['args'] == ['--', 'native return; literal $(text)'], native
        assert native['cwd'] == str(folder / 'vertical-project') and native['source'] == str(folder / 'claude')
        assert native['tty'] and native['canonical'] and native['echo'], native
        pre_native = bytes(session.raw[native_start:]).split(b'NATIVE_FIXTURE_READY', 1)[0]
        assert b'\x1b[?1000l' in pre_native and b'\x1b[?1006l' in pre_native, 'Native console kept office mouse capture'
        assert pre_native.rfind(b'\x1b[?1000l') > pre_native.rfind(b'\x1b[?1000h')
        session.save(out / '04-native-console')
        session.key(b'\r')
        session.wait(lambda: b'NATIVE_FIXTURE_RETURN' in session.raw and 'NEW TASK' in session.text(), 'office returns after native console')
        returned = bytes(session.raw).split(b'NATIVE_FIXTURE_RETURN', 1)[1]
        assert b'\x1b[?1000h' in returned and b'\x1b[?1006h' in returned, 'Office did not recapture mouse after native console'
        session.save(out / '05-native-return')
        result[current] = {**native, 'mouse_released_before_child': True, 'mouse_restored_after_child': True}
        session.key(KEY['esc'])

        current = 'connections-local-sources-cancel'
        saved_sources = (folder / 'settings/connections.json').read_bytes()
        click_text(session, 'Connections')
        session.wait(lambda: 'Local sources' in session.text(), 'unified connections panel')
        click_text(session, 'Local sources')
        session.wait(lambda: 'Connect your team' in session.text(), 'local source chooser')
        click_text(session, '[x] CODEX')
        session.wait(lambda: '[ ] CODEX' in session.text(), 'source title toggles the displayed choice')
        click_text(session, '/claude')
        session.wait(lambda: '[Esc cancel]' in session.text(), 'source folder click enters editing')
        session.key(b'\x15')
        session.paste(str(folder / 'unapplied-folder'))
        session.save(out / '06-source-edit')
        click_text(session, 'Esc cancel')
        session.wait(lambda: '[Esc back]' in session.text(), 'folder cancellation returns to chooser')
        assert 'unapplied-folder' not in session.text()
        click_text(session, 'Esc back')
        session.wait(lambda: 'Connect your team' not in session.text(), 'source chooser returns to office')
        assert (folder / 'settings/connections.json').read_bytes() == saved_sources
        result[current] = {'source_title_toggles': True, 'folder_click_edits': True, 'folder_cancel_preserves_path': True, 'back_preserves_saved_sources': True}
        # The calling connections panel may remain open after the chooser.
        session.key(KEY['esc'])
        result['terminal_exit'] = session.finish()
        exit_raw = bytes(session.raw).rsplit(b'PTY_RESULT ', 1)[0]
        assert exit_raw.rfind(b'\x1b[?1000l') > exit_raw.rfind(b'\x1b[?1000h')
        (out / 'mouse-on.ansi').write_bytes(session.raw)
        session = None

        current = 'mouse-off-reopen-no-replay'
        turns_before = len([r for r in records(folder / 'provider.jsonl') if r.get('method') == 'turn/start'])
        session = Session(binary, folder, columns=args.columns, rows=args.rows, mouse=False)
        session.wait(lambda: 'vertical-project' in session.text(), 'managed workers reappear with mouse off')
        session.pump(1.2)
        assert len([r for r in records(folder / 'provider.jsonl') if r.get('method') == 'turn/start']) == turns_before
        assert len([r for r in records(folder / 'invocations.jsonl') if r['args'] == ['app-server']]) == 1
        session.key(b'c')
        if 'Local sources' in session.text():
            session.key(b'3')
        session.wait(lambda: 'Connect your team' in session.text(), 'keyboard opens sources with mouse off')
        click_text(session, '[x] CODEX')
        assert '[x] CODEX' in session.text() and '[ ] CODEX' not in session.text()
        click_text(session, '/claude')
        assert '[Esc cancel]' not in session.text()
        session.save(out / '07-mouse-off-sources')
        session.key(KEY['esc'])
        session.key(KEY['esc'])
        assert (folder / 'settings/connections.json').read_bytes() == saved_sources
        result['mouse_off_terminal_exit'] = session.finish()
        assert b'\x1b[?1000h' not in session.raw and b'\x1b[?1006h' not in session.raw, 'Mouse disabled still captured input'
        (out / 'mouse-off.ansi').write_bytes(session.raw)
        result[current] = {'same_provider_process': True, 'turn_start_count': turns_before, 'no_mouse_capture': True, 'source_clicks_ignored': True}
        session = None
    except Exception as error:
        result[current] = {'failure': str(error)}
        if session is not None:
            session.save(out / f'failure-{current}')
        raise
    finally:
        if session is not None and session.process.poll() is None:
            session.process.terminate()
            session.pump(.3)
            if session.process.poll() is None:
                session.process.kill()
            os.close(session.master)
            os.close(session.slave)
        stop_owned_fixture_hosts(folder)
        (out / 'results.json').write_text(json.dumps(result, indent=2) + '\n')
        for name in ['provider.jsonl', 'invocations.jsonl']:
            (out / name).write_text((folder / name).read_text() if (folder / name).exists() else '')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
