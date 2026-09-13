#!/usr/bin/env python3
"""Exercise source setup in a native PTY with an isolated home. Images reconstruct cells."""
import argparse, codecs, fcntl, importlib.util, json, os, pathlib, select, shutil, signal, struct, subprocess, sys, termios, time
ROOT = pathlib.Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location('pty_review', ROOT / 'docs/design-audit/iteration-2/review_pty.py')
review = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(review)
SCRATCH = ROOT / 'docs/design-audit/tmp/iteration-3-entry'
OUT = ROOT / 'docs/design-audit/iteration-3/evidence'

class Session(review.Session):

    def __init__(self, binary, home, args=(), size=(80, 24), overrides=True, environment=None, cwd=None):
        home.mkdir(parents=True, exist_ok=True)
        self.cols, self.rows = size
        self.master, self.slave = os.openpty()
        self.raw = bytearray()
        self.screen = review.pyte.Screen(self.cols, self.rows)
        self.stream = review.pyte.Stream(self.screen)
        self.decoder = codecs.getincrementaldecoder('utf8')('replace')
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack('HHHH', self.rows, self.cols, 0, 0))

        def child():
            os.setsid()
            fcntl.ioctl(self.slave, termios.TIOCSCTTY, 0)
        env = os.environ.copy()
        env.update(HOME=str(home), USERPROFILE=str(home), XDG_CONFIG_HOME=str(home / 'settings'), APPDATA=str(home / 'settings'), THEYWORK_CLAUDE_HOME=str(home / '.claude'), THEYWORK_CODEX_HOME=str(home / '.codex'), TERM='xterm-256color', TERM_PROGRAM='Apple_Terminal', LANG='en_US.UTF-8')
        if not overrides:
            env.pop('THEYWORK_CLAUDE_HOME', None)
            env.pop('THEYWORK_CODEX_HOME', None)
        for key in ['COLORTERM', 'NO_COLOR', 'THEYWORK_COLOR', 'THEYWORK_ENCODING']:
            env.pop(key, None)
        env.update(environment or {})
        helper = "import subprocess,sys,termios,json,signal; before=termios.tcgetattr(0); p=subprocess.Popen(sys.argv[1:]); signal.signal(signal.SIGTERM,lambda *_:p.send_signal(signal.SIGTERM)); code=p.wait(); after=termios.tcgetattr(0); mask=~getattr(termios,'PENDIN',0);before[3]&=mask;after[3]&=mask;print('PTY_RESULT '+json.dumps({'exit':code,'terminal_restored':before==after}),flush=True)"
        self.process = subprocess.Popen([sys.executable, '-c', helper, str(binary.resolve()), *args], stdin=self.slave, stdout=self.slave, stderr=self.slave, env=env, cwd=cwd or home, preexec_fn=child)
        self.pump(0.8)
        assert self.process.poll() is None, bytes(self.raw[-2000:])

    def text(self):
        return '\n'.join(self.screen.display)

    def capture(self, name):
        path = OUT / name
        path.parent.mkdir(parents=True, exist_ok=True)
        review.image_of(self.screen, literal_font=True).save(path.with_suffix('.png'))
        path.with_suffix('.txt').write_text('\n'.join(line.rstrip() for line in self.screen.display).rstrip() + '\n')

def fresh(name):
    path = SCRATCH / name
    if path.exists():
        shutil.rmtree(path)
    (path / '.claude').mkdir(parents=True)
    return path

def chosen(home):
    return home / 'settings/they-work/connections.json'

def paste(s, text):
    s.key(b'\x1b[200~' + str(text).encode() + b'\x1b[201~')

def paths(binary):
    results = {}
    home = fresh('relative-source-after')
    folder = home / 'relative-claude'
    folder.mkdir()
    s = Session(binary, home, environment={'THEYWORK_CLAUDE_HOME': 'relative-claude'})
    s.key(b'\r')
    results['relative_folder_confirm'] = s.finish()
    stored = json.loads(chosen(home).read_text())
    assert stored['claude_home'] == str(folder)
    elsewhere = home / 'another-project'
    elsewhere.mkdir()
    s = Session(binary, home, overrides=False, cwd=elsewhere)
    assert 'Connect your team' not in s.text()
    s.key(b'c')
    assert 'Ready' in s.text() and 'relative-claude' in s.text()
    s.capture('entry-after-relative-restart')
    s.key(b'\x1b')
    results['relative_folder_restart_other_directory'] = s.finish()
    return results

def boundaries(binary):
    results = {}
    home = fresh('after-tiny')
    s = Session(binary, home, size=(30, 8))
    s.key(b'\r')
    assert 'Enlarge to 40 x 16' in s.text()
    assert not (home / 'settings').exists()
    s.resize(80, 24)
    assert 'Connect your team' in s.text()
    results['tiny_terminal_requires_visible_choices'] = s.finish()
    home = fresh('after-demo-corrupt')
    chosen(home).parent.mkdir(parents=True)
    chosen(home).write_text('broken')
    s = Session(binary, home, ['--demo', '--no-save'])
    s.key(b'c')
    assert 'Connect your team' in s.text()
    assert '[ ] CODEX' in s.text()
    s.capture('entry-after-demo-connect')
    s.key(b'\r')
    assert 'Connect your team' not in s.text()
    results['demo_can_repair_corrupt_connection'] = s.finish()
    assert chosen(home).read_text() == 'broken'
    return results

def after(binary):
    results = {}
    home = fresh('after-repair')
    s = Session(binary, home, ['--setup', '--sources', 'codex'])
    assert not (home / 'settings').exists()
    s.capture('entry-after-initial')
    s.key(b'\r')
    assert '> [x] CODEX' in s.text()
    s.capture('entry-after-missing')
    s.key(b'e')
    s.capture('entry-after-editor')
    s.key(b'\x15')
    folder = home / 'team 東京' / 'a long folder to test the visible insertion cursor' / '.codex'
    paste(s, folder)
    assert s.screen.cursor.x < 80
    assert '.codex' in s.text()
    s.capture('entry-after-long-path')
    s.key(b'\r')
    assert 'Folder not found' in s.text()
    folder.mkdir(parents=True)
    s.pump(0.3)
    assert 'Ready' in s.text()
    s.key(b'\r')
    assert 'CONNECT YOUR TEAM' not in s.text() and 'Connect your team' not in s.text()
    data = json.loads(chosen(home).read_text())
    assert data['codex'] and (not data['claude'])
    assert pathlib.Path(data['codex_home']) == folder
    results['repair_focus_cursor_unicode_remember'] = s.finish()
    s = Session(binary, home, overrides=False)
    assert 'Connect your team' not in s.text()
    s.key(b'c')
    assert '> [ ] CLAUDE CODE' in s.text()
    assert '[x] CODEX' in s.text()
    assert '.codex' in s.text()
    s.capture('entry-after-restart')
    before = chosen(home).read_bytes()
    s.key(b' ')
    s.key(b'\x1b')
    assert chosen(home).read_bytes() == before
    s.key(b'c')
    assert '[ ] CLAUDE CODE' in s.text()
    s.key(b'd')
    s.key(b'c')
    assert '[ ] CLAUDE CODE' in s.text()
    s.key(b'\x1b')
    results['restart_cancel_and_demo_preserve_choice'] = s.finish()
    assert chosen(home).read_bytes() == before
    home = fresh('after-temporary')
    s = Session(binary, home)
    s.key(b'm')
    assert '[ ] Remember' in s.text()
    s.capture('entry-after-temporary')
    s.key(b'\r')
    results['temporary_first_launch'] = s.finish()
    assert not (home / 'settings').exists()
    s = Session(binary, home)
    assert 'Connect your team' in s.text()
    s.key(b'd')
    results['demo_from_first_launch'] = s.finish()
    assert not (home / 'settings').exists()
    home = fresh('after-no-save')
    s = Session(binary, home, ['--no-save'])
    s.key(b'm')
    s.key(b'\t\t ')
    assert '[ ] Remember' in s.text()
    s.capture('entry-after-no-save')
    s.key(b'\r')
    s.key(b'c')
    assert '[ ] Remember' in s.text()
    s.key(b'\x1b')
    results['no_save_locked_and_reconnection'] = s.finish()
    assert not (home / 'settings').exists()
    home = fresh('after-save-failure')
    blocked = home / 'not-a-directory'
    blocked.write_text('keep me')
    s = Session(binary, home, ['--setup', '--config-dir', str(blocked)])
    s.key(b'\r')
    assert 'Cannot save settings here' in s.text()
    assert '> [x] Remember' in s.text()
    s.capture('entry-after-save-error')
    s.key(b' ')
    s.key(b'\r')
    results['save_failure_temporary_recovery'] = s.finish()
    assert blocked.read_text() == 'keep me'
    home = fresh('after-corrupt')
    chosen(home).parent.mkdir(parents=True)
    chosen(home).write_text('broken')
    s = Session(binary, home, ['--setup', '--no-save'])
    s.key(b'\r')
    results['corrupt_saved_choice_temporary_recovery'] = s.finish()
    assert chosen(home).read_text() == 'broken'
    home = fresh('after-small')
    s = Session(binary, home, size=(40, 16))
    assert 'Remember on this computer' in s.text()
    assert 'Enter connect' in s.text()
    s.capture('entry-after-40x16')
    s.key(b' ')
    assert 'Enter empty tower' in s.text()
    s.key(b'm')
    s.key(b'\r')
    results['small_empty_tower'] = s.finish()
    assert not (home / 'settings').exists()
    home = fresh('after-editor')
    s = Session(binary, home, ['--no-save'])
    s.key(b'e')
    s.key(b'\x15')
    s.key(b'\r')
    assert 'Enter a folder path' in s.text()
    paste(s, '/one\n/two')
    assert 'Paste one folder path' in s.text()
    paste(s, '/東京/foler')
    s.key(b'\x1b[D\x1b[Dd')
    assert '/東京/folder' in s.text()
    s.key(b'\x1b[H')
    assert s.screen.cursor.x == 1
    s.key(b'\x1b[F')
    assert s.screen.cursor.x > 1
    s.key(b'\x1b')
    results['editor_rejects_empty_and_multiline_paste'] = s.finish()
    assert not (home / 'settings').exists()
    results.update(boundaries(binary))
    results.update(paths(binary))
    return results

def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--binary', type=pathlib.Path, default=ROOT / 'target/native-macos/release/they-work')
    ap.add_argument('--stage', choices=['before', 'after', 'boundaries', 'paths'], default='before')
    args = ap.parse_args()
    results = {}
    if args.stage == 'before':
        home = fresh('before-repair')
        s = Session(args.binary, home, ['--setup', '--sources', 'codex'])
        s.capture('entry-before-initial')
        s.key(b'\r')
        s.capture('entry-before-missing')
        s.key(b'e')
        s.capture('entry-before-editor')
        s.key(b'\x1b')
        results['repair'] = s.finish()
    elif args.stage == 'paths':
        results = paths(args.binary)
    elif args.stage == 'boundaries':
        results = boundaries(args.binary)
    else:
        results = after(args.binary)
    (OUT / ('entry-' + args.stage + '-results.json')).write_text(json.dumps(results, indent=2) + '\n')
    print(json.dumps(results, indent=2))
if __name__ == '__main__':
    main()
