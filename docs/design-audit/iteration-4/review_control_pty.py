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
KEY = {'esc': b'\x1b', 'f4': b'\x1bOS', 'f5': b'\x1b[15~', 'right': b'\x1b[C'}


def records(path):
    return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []


class Session(base.Session):
    def __init__(self, binary, folder, graphics=False, columns=132, rows=42):
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
                '--claude-home', str(folder / 'claude'), '--config-dir', str(folder / 'settings')]
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/native-macos/release/they-work')
    parser.add_argument('--stage', default='control-pty')
    parser.add_argument('--graphics', action='store_true', help='Reply to Kitty probes and save transmitted room RGBA separately')
    parser.add_argument('--expect-resolved-state', action='store_true', help='Assert a resolved approval no longer leaves the worker waiting')
    parser.add_argument('--columns', type=int, default=132)
    parser.add_argument('--rows', type=int, default=42)
    parser.add_argument('--expect-adjacent', action='store_true', help='Require adjacent desk and meeting rooms at a wide size')
    parser.add_argument('--check-responsive', action='store_true', help='Check full meeting at 132x42 then adjacent rooms at 152x24')
    args = parser.parse_args()
    binary = args.binary.resolve()
    scratch = ROOT / 'docs/design-audit/tmp/iteration-4-control-pty'
    scratch.mkdir(parents=True, exist_ok=True)
    out = ROOT / 'docs/design-audit/iteration-4/evidence' / args.stage
    out.mkdir(parents=True, exist_ok=True)
    folder = Path(tempfile.mkdtemp(prefix='run-', dir=scratch))
    prepare(folder)
    result = {'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
              'fixture_root': str(folder), 'real_provider_calls': 0,
              'terminal_cells': [args.columns, args.rows],
              'encoding': 'controlled Kitty' if args.graphics else 'half-block fallback'}
    session = None
    current = 'startup'
    try:
        session = Session(binary, folder, args.graphics, args.columns, args.rows)
        session.wait(lambda: len(records(folder / 'invocations.jsonl')) >= 3, 'native capability probes')
        session.key(b'n')
        session.wait(lambda: 'NEW TASK' in session.text(), 'new task panel')
        session.paste(str(folder / 'vertical-project'))
        session.key(b'\t')
        session.paste('approval')
        session.save(out / '01-new-task')
        current = 'ui-start-approval'
        session.key(KEY['f5'])
        session.wait(lambda: any(r.get('method') == 'turn/start' for r in records(folder / 'provider.jsonl')), 'UI start reaches provider')
        session.wait(lambda: state(folder).get('pending_requests'), 'pending request returned')
        requests = records(folder / 'provider.jsonl')
        start = next(r for r in requests if r.get('method') == 'thread/start')
        turn = next(r for r in requests if r.get('method') == 'turn/start')
        assert start['params']['cwd'] == str(folder / 'vertical-project')
        assert turn['params']['threadId'] == 'managed-1'
        assert turn['params']['input'] == [{'type': 'text', 'text': 'approval'}]
        session.wait(lambda: 'Sending to' not in session.text(), 'acknowledgement in UI')
        session.save(out / '02-accepted')
        result[current] = {'native_thread': 'managed-1', 'native_turn': 'turn-1', 'literal_project_and_prompt': True}

        current = 'explicit-scoped-approval'
        session.key(KEY['f4'])
        session.wait(lambda: 'test-only-command' in session.text(), 'exact approval command visible')
        assert 'Allow this request' in session.text() and 'managed-1' in session.text()
        session.save(out / '03-approval')
        session.key(b'1')
        session.wait(lambda: any(r.get('id') == 'approve-1' and 'result' in r for r in records(folder / 'provider.jsonl')), 'approval response written')
        reply = next(r for r in records(folder / 'provider.jsonl') if r.get('id') == 'approve-1' and 'result' in r)
        assert reply['result'] == {'decision': 'accept'}, reply
        session.wait(lambda: not state(folder).get('pending_requests'), 'provider resolution clears approval')
        result[current] = {'command_visible': 'test-only-command', 'reply': reply}
        session.key(KEY['esc'])
        session.wait(lambda: 'vertical-project' in session.text(), 'new worker reaches world')

        current = 'delegated-team-in-world'
        session.key(b'n')
        # The selected floor now supplies the project; focus starts in message.
        session.paste('team-active')
        session.key(KEY['f5'])
        session.wait(lambda: 'child-1' in state(folder).get('threads', {}), 'parent and two children')
        session.key(KEY['esc'])
        if args.graphics:
            # Select the controlling parent through the real global finder.
            # A compact room follows selection instead of shrinking every group.
            session.key(b'/')
            session.paste('team-active')
            session.wait(lambda: '1 match' in session.text(), 'find the team parent')
            session.key(b'\r')
            session.key(KEY['esc'])
        session.key(b'0')
        if not args.graphics:
            session.key(b'\r')
        session.wait(lambda: ('MEETING' in session.text() and 'Lead: team-active' in session.text()) if args.graphics else ('4 workers' in session.text() and session.text().count('Agent c') == 2), 'four distinct people in office or selected parent and children in meeting')
        adjacent = args.graphics and 'DESKS' in session.text()
        if args.expect_adjacent:
            assert adjacent, 'Wide terminal did not show adjacent desk and meeting rooms'
        session.save(out / '04-team-office')
        if args.check_responsive:
            assert args.graphics and (args.columns, args.rows) == (132, 42)
            assert not adjacent, 'Tall window shrinks the meeting to preserve a split'
            session.resize(152, 24)
            session.wait(lambda: 'MEETING' in session.text() and 'DESKS' in session.text(), 'responsive adjacent rooms')
            session.save(out / '04-team-adjacent')
            session.resize(132, 42)
            session.wait(lambda: 'MEETING' in session.text() and 'DESKS' not in session.text(), 'restore readable full meeting')
            result['responsive-meeting'] = {'132x42': 'selected full room', '152x24': 'adjacent rooms', 'restored_selection': 'team-active'}
        session.key(b'g')
        session.wait(lambda: 'child-0' in session.text() or 'Delivered result' in session.text(), 'team collaboration board')
        session.save(out / '05-team-board')
        if args.expect_resolved_state:
            assert not re.search(r'approval · family root · waiting', session.text()), 'Resolved approval still appears waiting'
        threads = state(folder)['threads']
        assert threads['managed-2']['managed'] and not threads['child-0']['managed'] and not threads['child-1']['managed']
        result[current] = {'people': len(threads), 'parent': 'managed-2', 'children': ['child-0', 'child-1'], 'children_observed_without_control': True, 'active_meeting_visible': args.graphics, 'adjacent_rooms': adjacent, 'resolved_approval_not_waiting': args.expect_resolved_state}
        session.key(KEY['esc'])

        current = 'native-console-roundtrip'
        session.key(b'n')
        session.key(b'\t')  # message → provider
        session.key(KEY['right'])
        session.key(b'\t\t')  # provider → project → message
        session.paste('native return; literal $(text)')
        session.key(KEY['f5'])
        session.wait(lambda: (folder / 'native.json').exists(), 'native child receives real terminal')
        native = json.loads((folder / 'native.json').read_text())
        assert native['args'] == ['--', 'native return; literal $(text)'], native
        assert native['cwd'] == str(folder / 'vertical-project') and native['source'] == str(folder / 'claude')
        assert native['tty'] and native['canonical'] and native['echo'], native
        session.save(out / '06-native-console')
        session.key(b'\r')
        session.wait(lambda: 'NEW TASK' in session.text(), 'office returns after native console')
        session.save(out / '07-native-return')
        result[current] = native
        session.key(KEY['esc'])
        result['terminal_exit'] = session.finish()
        session = None

        current = 'close-reopen-no-replay'
        turns_before = len([r for r in records(folder / 'provider.jsonl') if r.get('method') == 'turn/start'])
        owned = [r for r in records(folder / 'invocations.jsonl') if r['args'] == ['app-server']]
        assert len(owned) == 1
        os.kill(owned[0]['ppid'], 0)
        session = Session(binary, folder, args.graphics, args.columns, args.rows)
        session.wait(lambda: 'vertical-project' in session.text(), 'persisted managed team reappears')
        session.pump(2.2)
        assert len([r for r in records(folder / 'provider.jsonl') if r.get('method') == 'turn/start']) == turns_before
        assert len([r for r in records(folder / 'invocations.jsonl') if r['args'] == ['app-server']]) == 1
        session.save(out / '08-reopened')
        result[current] = {'supervisor_survived': True, 'turn_start_count': turns_before, 'same_provider_process': True}
        result['reopened_terminal_exit'] = session.finish()
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
        (out / 'provider.jsonl').write_text((folder / 'provider.jsonl').read_text() if (folder / 'provider.jsonl').exists() else '')
        (out / 'invocations.jsonl').write_text((folder / 'invocations.jsonl').read_text() if (folder / 'invocations.jsonl').exists() else '')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
