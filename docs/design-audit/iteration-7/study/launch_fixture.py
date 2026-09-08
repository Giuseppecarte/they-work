#!/usr/bin/env python3
"""Owner-run launcher confined to the prepared synthetic study slot."""
import argparse
import hashlib
import json
import os
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixture', type=Path, required=True)
    parser.add_argument('--condition', choices=['tower', 'compact', 'reduced'], default='tower')
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/native-macos/release/they-work')
    parser.add_argument('--doctor', action='store_true')
    args = parser.parse_args()
    folder = args.fixture.resolve()
    parent = (ROOT / 'docs/design-audit/tmp/iteration-7-workflows').resolve()
    if folder.parent != parent or folder.name not in {f'study-P{i:02}' for i in range(1, 6)}:
        parser.error('Only an explicitly prepared study-P01..P05 fixture is allowed')
    if not (folder / 'fixture-facts.json').is_file():
        parser.error('Run prepare_session.py first')
    binary = args.binary.resolve()
    preferences = folder / 'settings/appearance.json'
    appearance = json.loads(preferences.read_text()) if preferences.exists() else {}
    appearance.update(motion=args.condition != 'reduced', projection='auto')
    if not args.doctor:
        preferences.write_text(json.dumps(appearance, indent=2) + '\n')
    # Keep only terminal capability hints; no inherited provider credentials.
    env = {key: value for key, value in os.environ.items() if key in {
        'TERM', 'TERM_PROGRAM', 'TERM_PROGRAM_VERSION', 'COLORTERM', 'NO_COLOR',
        'WT_SESSION', 'KITTY_WINDOW_ID', 'ITERM_SESSION_ID', 'TERM_FEATURES'}}
    env.update(PATH=str(folder / 'bin') + ':/usr/bin:/bin:/usr/sbin:/sbin',
               HOME=str(folder / 'home'), XDG_CONFIG_HOME=str(folder / 'xdg'), TMPDIR=str(folder / 'tmp'),
               LANG='en_US.UTF-8', CODEX_HOME=str(folder / 'codex'), CLAUDE_CONFIG_DIR=str(folder / 'claude'))
    if args.condition == 'compact':
        env['THEYWORK_ENCODING'] = 'half-blocks'
    command = [str(binary), '--sources', 'all', '--codex-home', str(folder / 'codex'),
               '--claude-home', str(folder / 'claude'), '--config-dir', str(folder / 'settings')]
    if args.doctor:
        command.append('--doctor')
    print('Synthetic fixture only. Binary SHA-256: ' + hashlib.sha256(binary.read_bytes()).hexdigest(), flush=True)
    os.chdir(folder / 'vertical-project')
    os.execve(binary, command, env)


if __name__ == '__main__':
    main()
