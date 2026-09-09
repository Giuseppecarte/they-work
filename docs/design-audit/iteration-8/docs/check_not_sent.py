#!/usr/bin/env python3
"""REL-01 actual 80x24 keyboard PTY, isolated fake provider and storage only."""
import argparse
import importlib.util
import json
import shutil
import sys
import tempfile
import traceback
from pathlib import Path
from PIL import Image
sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[4]
spec = importlib.util.spec_from_file_location('routes', Path(__file__).with_name('check_routes.py'))
r = importlib.util.module_from_spec(spec); spec.loader.exec_module(r)
w = r.w


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary', type=Path, default=ROOT / 'target/native-macos/release/they-work')
    p.add_argument('--output', type=Path, default=Path(__file__).with_name('evidence') / 'not-sent')
    args = p.parse_args(); args.output.mkdir(parents=True, exist_ok=True)
    scratch = ROOT / 'docs/design-audit/tmp/iteration-8-docs'; scratch.mkdir(parents=True, exist_ok=True)
    folder = Path(tempfile.mkdtemp(prefix='not-sent-', dir=scratch)); w.control.prepare(folder)
    binary = folder / 'they-work'; shutil.copy2(args.binary.resolve(), binary)
    result = {'binary_sha256': w.digest(binary), 'cells': [80,24], 'keyboard_only': True,
              'fixture': str(folder), 'real_provider_calls': 0, 'participant_sessions': 0,
              'method': 'Executable PTY + ANSI replay; no physical terminal or account'}
    s = None; directory = None
    try:
        s = w.Session(binary, folder, columns=80, rows=24, mouse=False)
        s.wait(lambda: len(w.control.records(folder / 'invocations.jsonl')) >= 3, 'fake capability probes')
        s.action('Create local fixture task', b'n'); s.paste(str(folder / 'vertical-project'))
        s.action('Focus message', b'\t'); s.paste('working')
        s.action('Start initial fixture task', r.F5)
        starts = lambda: [x for x in w.control.records(folder / 'provider.jsonl') if x.get('method') == 'turn/start']
        s.wait(lambda: len(starts()) == 1, 'initial fake turn')
        s.wait(lambda: 'Sending to' not in s.text(), 'initial acknowledgement')
        s.action('Close start form', r.ESC)
        w.find(s, 'working')
        s.action('Instruct selected managed task', b'm')
        draft = 'Keep this draft; retry only after storage is restored.'
        s.paste(draft)
        s.wait(lambda: 'INSTRUCTION TO' in s.text(), 'recipient-bound composer')
        s.capture(args.output, '01-draft-before-failure')
        paths = list((folder / 'settings/control').glob('*/state.json'))
        assert len(paths) == 1, paths
        directory = paths[0].parent
        directory.chmod(0o500)
        steers = lambda: [x for x in w.control.records(folder / 'provider.jsonl') if x.get('method') == 'turn/steer']
        assert not steers()
        s.action('Explicit submission while local writes are denied', r.F5)
        s.wait(lambda: 'Not sent:' in s.text(), 'truthful pre-send rejection')
        visible_words = ' '.join(s.text().split())
        assert 'Not sent: local state could not be saved. Restore storage access and submit again.' in visible_words, s.text()
        assert 'Keep this draft' in s.text(), s.text()
        assert not steers(), 'Provider received rejected draft'
        s.capture(args.output, '02-not-sent-draft-retained')
        rejection_text = s.text()
        directory.chmod(0o700)
        s.pump(2)
        assert not steers(), 'Restoring access automatically resubmitted'
        assert 'Keep this draft' in s.text()
        s.action('Explicit resubmit after restoring access', r.F5)
        s.wait(lambda: len(steers()) == 1, 'one explicit new submission')
        s.wait(lambda: 'Sending to' not in s.text(), 'resubmission acknowledgement')
        s.pump(1)
        sent = steers()
        assert len(sent) == 1 and sent[0]['params']['threadId'] == 'managed-1', sent
        assert sent[0]['params']['input'] == [{'type':'text','text':draft}], sent
        operations = w.control.state(folder)['operations']
        rejected = [x for x in operations.values() if x['status'] == 'Rejected']
        confirmed = [x for x in operations.values() if x['status'] == 'Confirmed']
        assert len(rejected) == 1 and len(confirmed) == 2, operations
        assert rejected[0]['id'] not in [x['id'] for x in confirmed]
        s.capture(args.output, '03-explicit-resubmit-confirmed')
        result.update(status='pass', exact_recipient='managed-1', rejected_provider_calls=0,
                      automatic_resends=0, explicit_resubmit_calls=1, rejected_receipt=rejected[0],
                      confirmed_receipts=confirmed, original_error=w.control.state(folder).get('last_error'),
                      draft_preserved=True, rejection_visible=rejection_text, terminal_exit=r.close(s,args.output,'not-sent'))
        for name in ['provider.jsonl','invocations.jsonl']:
            shutil.copy2(folder/name,args.output/name)
    except Exception:
        result.update(status='fail', failure=traceback.format_exc())
        if s:
            s.capture(args.output,'failure')
        raise
    finally:
        if directory:
            directory.chmod(0o700)
        r.cleanup(s,folder)
        (args.output/'results.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({'status':result['status'],'binary_sha256':result['binary_sha256'],'output':str(args.output)}))


if __name__ == '__main__':
    main()
