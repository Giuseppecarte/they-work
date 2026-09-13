#!/usr/bin/env python3
"""Executable keyboard-only DOC-01 routes at 80x24 using isolated fake sources."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import shutil
import traceback
from unittest.mock import patch
from PIL import Image

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[4]
spec = importlib.util.spec_from_file_location('workflow7', ROOT / 'docs/design-audit/iteration-7/workflows/walkthrough.py')
w = importlib.util.module_from_spec(spec)
spec.loader.exec_module(w)
ESC, TAB, BACKTAB, F5 = b'\x1b', b'\t', b'\x1b[Z', b'\x1b[15~'


class Session(w.Session):
    def __init__(self, binary, folder, arguments, *, launcher=None, graphics=False, columns=80, rows=24):
        original = subprocess.Popen
        self.arguments = arguments
        def spawn(command, **kwargs):
            # Adapt only the audit wrapper's child argv. The actual executable
            # and its UI receive real bytes; no application API is mocked.
            index = command.index(str(binary))
            target = [str(binary), *arguments] if launcher is None else launcher
            command = [*command[:index], *target]
            return original(command, **kwargs)
        with patch.object(subprocess, 'Popen', spawn):
            super().__init__(binary, folder, graphics=graphics, columns=columns, rows=rows, mouse=False)

    def capture(self, out, name):
        super().capture(out, name)


def config(folder):
    return folder / 'xdg/they-work'


def arguments(folder, extra=()):
    return ['--codex-home', str(folder / 'codex'), '--claude-home', str(folder / 'claude'),
            '--mouse', 'off', *extra]


def state_hashes(path):
    return {str(p.relative_to(path)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in path.rglob('*') if p.is_file()} if path.exists() else {}


def cleanup(s, folder):
    if s and s.process.poll() is None:
        s.process.terminate(); s.pump(.2)
        if s.process.poll() is None:
            s.process.kill()
    w.control.stop_owned_fixture_hosts(folder)


def close(s, output, label):
    for _ in range(3):
        s.action('Close retained dialog', ESC)
    return s.close_evidence(output, label)


def prepare(folder):
    w.fixture(folder)
    assert not config(folder).exists()


def first_run(binary, folder, output):
    prepare(folder)
    s = Session(binary, folder, arguments(folder))
    try:
        s.wait(lambda: 'Connections / Sources' in s.text(), 'unconfigured startup')
        assert '[x] Remember' in s.text()
        assert not config(folder).exists(), 'Chooser created settings before Connect'
        s.capture(output, 'first-run-remember')
        # Default focus is Connect. Two reverse tabs select Codex's folder.
        s.action('Focus Remember', BACKTAB)
        s.action('Focus Codex folder', BACKTAB)
        s.action('Edit focused folder', b'e')
        s.wait(lambda: 'Editing folder' in s.text(), 'folder editor')
        s.action('Clear folder text', b'\x15')
        s.paste(str(folder / 'codex'))
        s.action('Apply folder text', b'\r')
        assert 'Editing folder' not in s.text()
        s.action('Enter acts on the same folder control', b'\r')
        assert 'Editing folder' in s.text(), 'Enter did not retain field context'
        s.capture(output, 'enter-reopens-folder')
        s.action('Cancel reopened editor', ESC)
        s.action('Connect with F5', F5)
        s.wait(lambda: 'alpha-shop' in s.text() and 'Connections / Sources' not in s.text(), 'saved connection')
        saved = json.loads((config(folder) / 'connections.json').read_text())
        assert saved['codex'] and saved['claude'] and saved['codex_home'] == str(folder / 'codex')
        s.action('Open second floor', b'2')
        s.wait(lambda: 'beta-docs / Office' in s.text(), 'second floor')
        result = close(s, output, 'first-run')
        assert (config(folder) / 'project').read_text().strip() == str(folder / 'beta-docs')
        return {'exit': result, 'default_settings_path': str(config(folder)),
                'no_explicit_config_dir': True, 'saved_only_after_connect': True,
                'enter_reopened_editor': True, 'f5_connected': True, 'selected_project_saved': True}
    finally:
        cleanup(s, folder)


def navigation(binary, folder, output):
    s = Session(binary, folder, arguments(folder))
    try:
        s.wait(lambda: 'beta-docs / Office' in s.text(), 'remembered floor restored')
        assert 'Connections / Sources' not in s.text()
        s.capture(output, 'reopened-remembered-floor')
        for label, key in [('connections-lowercase', b'c'), ('connections-uppercase', b'C')]:
            s.action(label, key)
            s.wait(lambda: 'CONNECTIONS' in s.text(), 'Connections panel')
            assert 'Local sources' in s.text()
            s.capture(output, label)
            s.action('Local sources keyboard shortcut', b'3')
            s.wait(lambda: 'Connections / Sources' in s.text(), 'Local sources')
            s.action('Cancel Sources', ESC)
            s.action('Close Connections', ESC)
        s.action('Open Advanced', b'v')
        assert 'Advanced' in s.text() and 'Camera' in s.text()
        s.capture(output, 'advanced-camera')
        s.action('Choose next camera', b'\x1b[C')
        s.action('Esc returns to Settings first', ESC)
        assert 'Settings' in s.text() and 'appearance' in s.text()
        s.action('Close Settings', ESC)
        s.action('Office Design', b'd')
        assert 'OFFICE DESIGN' in s.text()
        s.capture(output, 'office-design')
        s.action('Cancel Office Design', ESC)
        s.action('Tower', b'0')
        s.wait(lambda: 'Software tower' in s.text(), 'tower')
        s.action('Focus visible control', TAB)
        s.action('Reverse focus', BACKTAB)
        assert 'Software tower' in s.text(), 'Tab changed view instead of focus'
        s.capture(output, 'tower-focus')
        s.action('Clear control focus', ESC)
        s.action('Open first floor by number', b'1')
        assert 'alpha-shop / ' in s.text(), s.text()
        s.action('Next office floor with PageDown', b'\x1b[6~')
        assert 'beta-docs / ' in s.text(), s.text()
        s.action('Previous office floor with PageUp', b'\x1b[5~')
        assert 'alpha-shop / ' in s.text(), s.text()
        s.capture(output, 'floor-page-keys')
        w.find(s, 'Draft API guide')
        assert 'Draft API guide' in s.text()
        s.action('Character editor', b'a')
        assert 'CHARACTER' in s.text()
        s.capture(output, 'character-editor')
        s.action('Cancel Character', ESC)
        s.action('Close brief', ESC)
        s.action('Open all-floor Attention', b'b')
        assert 'APPROVAL NEEDED' in s.text()
        s.action('Close Attention', ESC)
        s.action('Select next attention task', b'!')
        assert 'FOR YOU' in s.text() or 'Review checkout' in s.text() or 'Approval' in s.text()
        result = close(s, output, 'navigation')
        return {'exit': result, 'reopen_restored_floor': True, 'modal_routes': True,
                'keyboard_only': True, 'sources_cancelled_without_save': True,
                'office_page_keys_changed_floors': True}
    finally:
        cleanup(s, folder)


def aliases(binary, folder, output):
    results = []
    for index, keys in enumerate([b'wo', b'wo', b'WO']):
        s = Session(binary, folder, arguments(folder))
        try:
            s.wait(lambda: 'alpha-shop' in s.text() or 'beta-docs' in s.text(), 'saved session')
            w.find(s, 'Review checkout release')
            for key in keys:
                s.action('Compatibility key ' + chr(key), bytes([key]))
            s.capture(output, f'compatibility-{index + 1}')
            exit_result = close(s, output, f'aliases-{index + 1}')
            prefs = json.loads((config(folder) / 'appearance.json').read_text())
            wardrobe, palette = prefs.get('wardrobe', {}), prefs.get('office_palettes', {})
            if index < 2:
                assert list(wardrobe.values()) == [index], wardrobe
                assert len(palette) == 1 and 0 <= next(iter(palette.values())) < 4, palette
                if index == 1:
                    prior = next(iter(results[0]['office_palettes'].values()))
                    assert next(iter(palette.values())) == (prior + 1) % 4, palette
            else:
                assert wardrobe == {} and palette == {}, (wardrobe, palette)
            results.append({'keys': keys.decode(), 'wardrobe': wardrobe, 'office_palettes': palette, 'exit': exit_result})
        finally:
            cleanup(s, folder)
    return results


def temporary(binary, folder, output):
    prepare(folder)
    s = Session(binary, folder, arguments(folder))
    try:
        s.wait(lambda: 'Connections / Sources' in s.text(), 'temporary chooser')
        s.action('Turn Remember off', b'm')
        assert '[ ] Remember' in s.text()
        s.capture(output, 'temporary-remember-off')
        s.action('Connect temporarily', F5)
        s.wait(lambda: 'alpha-shop' in s.text(), 'temporary sources')
        s.action('Change temporary palette', b'o')
        exit_result = close(s, output, 'temporary')
        assert not config(folder).exists()
    finally:
        cleanup(s, folder)
    s = Session(binary, folder, arguments(folder))
    try:
        s.wait(lambda: 'Connections / Sources' in s.text(), 'temporary session asks again')
        s.action('Start demo from Sources', b'd')
        s.wait(lambda: 'Connections / Sources' not in s.text() and 'Checkout lead' in s.text(), 'contextual demo key')
        s.capture(output, 'sources-d-demo')
        demo_exit = close(s, output, 'sources-demo')
        assert not config(folder).exists()
        return {'exit': exit_result, 'settings_created': False, 'reopen_asks_again': True, 'demo_exit': demo_exit}
    finally:
        cleanup(s, folder)


def no_save(binary, folder, output):
    before = state_hashes(config(folder))
    s = Session(binary, folder, arguments(folder, ['--setup', '--no-save']))
    try:
        s.wait(lambda: 'Connections / Sources' in s.text(), 'no-save Sources')
        assert 'Temporary' in s.text() or 'temporary' in s.text()
        s.capture(output, 'no-save-sources')
        # Connect focus5 → Demo6 → Back7 → Claude0; toggle only a fake source.
        for _ in range(3):
            s.action('Focus next source control', TAB)
        s.action('Disable Claude for this run', b' ')
        assert '[ ] Claude Code' in s.text()
        s.action('Connect without saving', F5)
        s.wait(lambda: 'alpha-shop' in s.text(), 'no-save observation')
        s.action('Change temporary palette', b'o')
        s.action('Change selected floor temporarily', b'3')
        result = close(s, output, 'no-save')
        after = state_hashes(config(folder))
        assert before == after, {'before': before, 'after': after}
        return {'exit': result, 'all_existing_setting_hashes_unchanged': True, 'file_count': len(before)}
    finally:
        cleanup(s, folder)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary', type=Path, default=ROOT / 'target/native-macos/release/they-work')
    p.add_argument('--output', type=Path, default=Path(__file__).with_name('evidence'))
    args = p.parse_args()
    binary = args.binary.resolve(); args.output.mkdir(parents=True, exist_ok=True)
    scratch = ROOT / 'docs/design-audit/tmp/iteration-8-docs'; scratch.mkdir(parents=True, exist_ok=True)
    folder = Path(tempfile.mkdtemp(prefix='routes-', dir=scratch))
    candidate = binary
    binary = folder / 'they-work'
    shutil.copy2(candidate, binary)
    result = {'binary_sha256': w.digest(binary), 'keyboard_only': True, 'cells': [80, 24],
              'fixture_root': str(folder), 'real_provider_calls': 0, 'participant_sessions': 0,
              'capture_method': 'Native executable PTY; Menlo/Pillow ANSI replay, not a physical terminal', 'cases': {}}
    try:
        result['cases']['first_run'] = first_run(binary, folder / 'saved', args.output)
        result['cases']['navigation'] = navigation(binary, folder / 'saved', args.output)
        result['cases']['aliases'] = aliases(binary, folder / 'saved', args.output)
        result['cases']['no_save'] = no_save(binary, folder / 'saved', args.output)
        result['cases']['temporary_and_demo'] = temporary(binary, folder / 'temporary', args.output)
    except Exception as error:
        result['failure'] = traceback.format_exc()
        raise
    finally:
        (args.output / 'results.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'passed': list(result['cases']), 'output': str(args.output), 'binary_sha256': result['binary_sha256']}))


if __name__ == '__main__':
    main()
