#!/usr/bin/env python3
"""Prepare a synthetic study slot; never launches a terminal or a provider."""
import argparse
import importlib.util
import json
from pathlib import Path
import shlex
import shutil
import sqlite3
import sys
import time
from PIL import Image  # Select this interpreter's Pillow before the legacy helper.

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[4]
spec = importlib.util.spec_from_file_location('walkthrough', Path(__file__).parents[1] / 'workflows/walkthrough.py')
walkthrough = importlib.util.module_from_spec(spec)
spec.loader.exec_module(walkthrough)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('slot', choices=[f'P{i:02}' for i in range(1, 6)])
    parser.add_argument('--reset', action='store_true', help='Replace only this generated study fixture')
    parser.add_argument('--add-result', action='store_true')
    args = parser.parse_args()
    if args.reset and args.add_result:
        parser.error('--reset and --add-result are separate steps')
    folder = ROOT / 'docs/design-audit/tmp/iteration-7-workflows' / f'study-{args.slot}'
    if args.reset and folder.exists():
        shutil.rmtree(folder)
    if not (folder / 'fixture-facts.json').exists():
        if args.add_result:
            parser.error('Prepare the slot before injecting its result')
        walkthrough.fixture(folder)
    if args.add_result:
        stamp = int(time.time() * 1000)
        db = sqlite3.connect(folder / 'codex/thread_history_1.sqlite')
        db.execute('INSERT INTO thread_items VALUES (?,?,?,?,?,?)', ('beta-draft', 'turn-1', f'study-result-{stamp}', stamp,
                   'agentMessage', json.dumps({'text': 'NEW WHILE AWAY: API examples now include pagination.', 'phase': 'final_answer'})))
        db.commit(); db.close()
        db = sqlite3.connect(folder / 'codex/state_5.sqlite')
        db.execute('UPDATE threads SET updated_at=? WHERE id=?', (stamp//1000, 'beta-draft'))
        db.commit(); db.close()
        print(f'Added only a synthetic Beta result at {stamp} to {folder}')
        return
    command = [sys.executable, str(Path(__file__).with_name('launch_fixture.py').resolve()), '--fixture', str(folder)]
    print('Prepared synthetic fixture; no study session or participant has been recorded.')
    print(' '.join(map(shlex.quote, command)) + ' --condition tower')
    print(' '.join(map(shlex.quote, command)) + ' --doctor')


if __name__ == '__main__':
    main()
