#!/usr/bin/env python3
"""Bounded executable discovery walkthrough; synthetic stores and local stubs only.

This is expert inspection, not a participant study. PNGs replay native ANSI plus
actual transmitted Kitty RGBA at its cursor placement; no terminal app is driven.
"""
import argparse
import base64
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import select
import sqlite3
import sys
import tempfile
import time
from PIL import Image, ImageDraw, ImageFont  # Load this interpreter's compiled Pillow first.

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[4]
spec = importlib.util.spec_from_file_location('control_pty', ROOT / 'docs/design-audit/iteration-6/review_control_pty.py')
control = importlib.util.module_from_spec(spec)
spec.loader.exec_module(control)
base = control.base
base.CW, base.CH = 8, 16
base.FONT = base.ImageFont.truetype('/System/Library/Fonts/Menlo.ttc', 13)
KEY = {'esc': b'\x1b', 'enter': b'\r', 'tab': b'\t', 'backtab': b'\x1b[Z', 'end': b'\x1b[F', 'home': b'\x1b[H'}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class Session(control.Session):
    def __init__(self, *args, **kwargs):
        self.art = None
        self.active_image = None
        self.trace = []
        self.start = time.monotonic()
        super().__init__(*args, **kwargs)

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
                        tail = self.text_pending.endswith(b'\x1b')
                        text = self.text_pending[:-1] if tail else self.text_pending
                        self.stream.feed(self.decoder.decode(text))
                        self.text_pending = self.text_pending[-1:] if tail else b''
                        break
                    self.stream.feed(self.decoder.decode(self.text_pending[:start]))
                    self.text_pending = self.text_pending[start:]
                    end_packet = self.text_pending.find(b'\x1b\\')
                    if end_packet < 0:
                        break
                    self.packet(self.text_pending[:end_packet + 2])
                    self.text_pending = self.text_pending[end_packet + 2:]

    def packet(self, packet):
        if not packet.startswith(b'\x1b_G'):
            return
        header, _, data = packet[3:-2].partition(b';')
        params = dict(part.split(b'=', 1) for part in header.split(b',') if b'=' in part)
        if params.get(b'a') == b'd':
            if self.art and params.get(b'i') == self.art['id']:
                self.art = None
        elif params.get(b'a') == b'T':
            self.active_image = (params, bytearray(data), (self.screen.cursor.x, self.screen.cursor.y))
        elif self.active_image:
            self.active_image[1].extend(data)
        if self.active_image and params.get(b'm', b'0') == b'0':
            info, encoded, position = self.active_image
            decoded = base64.b64decode(encoded, validate=True)
            image = (base.Image.open(io.BytesIO(decoded)).convert('RGBA') if info[b'f'] == b'100'
                     else base.Image.frombytes('RGBA', (int(info[b's']), int(info[b'v'])), decoded))
            self.art = {'id': info.get(b'i'), 'image': image, 'position': position}
            self.active_image = None

    def action(self, label, value):
        self.key(value)
        self.trace.append({'seconds': round(time.monotonic() - self.start, 3), 'action': label,
                           'bytes_hex': value.hex(), 'text': self.text()})

    def capture(self, out, name):
        path = out / name
        image = base.image_of(self.screen, literal_font=True)
        # Canvas marks native labels transparent. The exact current image can be
        # placed over the cell replay without repainting its opaque artwork.
        if self.art:
            x, y = self.art['position']
            image.paste(self.art['image'], (x * 8, y * 16), self.art['image'])
            # The host puts native text above the negative-z image. The protocol
            # stream does not export its full blank-cell mask; replay visible
            # glyphs over artwork, with their actual ANSI foreground/background.
            draw = ImageDraw.Draw(image)
            bold = ImageFont.truetype('/System/Library/Fonts/Menlo.ttc', 13, index=1)
            for row in range(self.rows):
                for col in range(self.cols):
                    cell = self.screen.buffer[row][col]
                    if not cell.data.strip():
                        continue
                    fg, bg = base.color(cell.fg), base.color(cell.bg, True)
                    if cell.reverse:
                        fg, bg = bg, fg
                    draw.rectangle((col * 8, row * 16, col * 8 + 7, row * 16 + 15), fill=bg)
                    draw.text((col * 8, row * 16), cell.data, font=bold if cell.bold else base.FONT, fill=fg)
        image.save(path.with_suffix('.png'))
        path.with_suffix('.txt').write_text('\n'.join(line.rstrip() for line in self.screen.display).rstrip() + '\n')
        self.trace.append({'seconds': round(time.monotonic() - self.start, 3), 'checkpoint': name,
                           'png_sha256': digest(path.with_suffix('.png')),
                           'native_sha256': digest(path.with_suffix('.txt')),
                           'image_position_cells': list(self.art['position']) if self.art else None,
                           'image_size_pixels': list(self.art['image'].size) if self.art else None})

    def close_evidence(self, out, name):
        result = self.finish()
        (out / f'{name}.trace.json').write_text(json.dumps(self.trace, indent=2) + '\n')
        (out / f'{name}.ansi').write_bytes(self.raw)
        return result


def fixture(folder, source=None):
    control.prepare(folder)
    # Remove the unused project so the fixture has exactly three known projects.
    if source:
        for name in ['state_5.sqlite', 'thread_history_1.sqlite']:
            (folder / 'codex' / name).write_bytes((source / 'codex' / name).read_bytes())
        return
    stamp = int(time.time() * 1000) - 10000
    db = sqlite3.connect(folder / 'codex/state_5.sqlite')
    history = sqlite3.connect(folder / 'codex/thread_history_1.sqlite')
    db.execute('CREATE TABLE threads (id TEXT, rollout_path TEXT, created_at INTEGER, updated_at INTEGER, cwd TEXT, title TEXT, tokens_used INTEGER, git_branch TEXT, archived INTEGER)')
    history.executescript('CREATE TABLE thread_items (thread_id TEXT, turn_id TEXT, item_id TEXT, created_at_ms INTEGER, item_type TEXT, item_json TEXT); CREATE TABLE thread_turns (thread_id TEXT, turn_id TEXT, status TEXT, started_at INTEGER, completed_at INTEGER, duration_ms INTEGER, error_json TEXT);')
    entries = [
        ('alpha-shop', 'alpha-checkout', 'Review checkout release', 'inProgress', 'requestApproval', {'reason': 'Run the synthetic checkout migration after the backup check?'}, None),
        ('alpha-shop', 'alpha-summary', 'Checkout test evidence', 'completed', 'agentMessage', {'text': 'Checkout tests passed; review the migration before release.', 'phase': 'final_answer'}, None),
        ('beta-docs', 'beta-draft', 'Draft API guide', 'completed', 'agentMessage', {'text': 'Draft API guide is ready for review.', 'phase': 'final_answer'}, None),
        ('beta-docs', 'beta-check', 'Verify guide links', 'completed', 'agentMessage', {'text': 'Checked the local links; no result event was recorded.'}, None),
        ('gamma-payments', 'gamma-error', 'Investigate payment timeout', 'failed', 'agentMessage', {'text': 'Payment retry exhausted; inspect the connection.'}, {'message': 'Synthetic payment connection refused'}),
        ('gamma-payments', 'gamma-copy', 'Prepare payment release notes', 'completed', 'agentMessage', {'text': 'Release notes draft prepared.'}, None),
    ]
    for project, worker, title, status, kind, payload, error in entries:
        project_path = folder / project
        (project_path / '.git').mkdir(parents=True, exist_ok=True)
        db.execute('INSERT INTO threads VALUES (?,?,?,?,?,?,?,?,?)', (worker, '/synthetic', stamp//1000, stamp//1000, str(project_path), title, 0, 'main', 0))
        history.execute('INSERT INTO thread_turns VALUES (?,?,?,?,?,?,?)', (worker, 'turn-1', status, stamp//1000, None if status == 'inProgress' else stamp//1000, None, json.dumps(error) if error else None))
        history.execute('INSERT INTO thread_items VALUES (?,?,?,?,?,?)', (worker, 'turn-1', 'record-1', stamp - 2000 if error else stamp, kind, json.dumps(payload)))
    db.commit(); history.commit(); db.close(); history.close()
    (folder / 'fixture-facts.json').write_text(json.dumps(entries, indent=2) + '\n')


def find(s, text):
    s.action('Open Find', b'/')
    s.paste(text)
    s.trace.append({'action': 'Type query', 'literal': text, 'text': s.text()})
    s.action('Inspect first match', KEY['enter'])
    s.wait(lambda: 'Observed' in s.text() or 'OBSERVED' in s.text(), 'task panel')


def comparisons(binary, scratch, out):
    source = scratch / 'canonical'
    fixture(source)
    hashes = {name: digest(source / 'codex' / name) for name in ['state_5.sqlite', 'thread_history_1.sqlite']}
    results = {'facts': json.loads((source / 'fixture-facts.json').read_text()), 'source_hashes': hashes, 'conditions': {}}
    for label, graphics, motion in [('tower', True, True), ('compact', False, True), ('reduced', True, False)]:
        # Source paths participate in durable identity. Reuse the exact store
        # path, not just byte-identical databases in different home folders.
        folder = source
        stores_equal = all(digest(folder / 'codex' / name) == value for name, value in hashes.items())
        (folder / 'settings/appearance.json').write_text(json.dumps({'motion': motion}))
        (folder / 'settings/notebook.json').unlink(missing_ok=True)
        s = Session(binary, folder, graphics=graphics, columns=120, rows=36)
        try:
            s.wait(lambda: 'alpha-shop' in s.text(), 'three isolated projects')
            s.action('Show tower', b'0')
            s.capture(out, f'compare-{label}')
            art_hashes = []
            if graphics:
                for _ in range(12):
                    s.pump(.25)
                    art_hashes.append(hashlib.sha256(s.art['image'].tobytes()).hexdigest())
            s.action('Open Attention across all floors', b'b')
            s.wait(lambda: 'APPROVAL NEEDED' in s.text(), 'approval observation')
            s.capture(out, f'attention-{label}')
            assert 'ERROR' in s.text()
            s.action('Close Attention', KEY['esc'])
            results['conditions'][label] = {'graphics': graphics, 'motion': motion,
                'initial_store_hashes_equal': stores_equal, 'same_source_path': str(source / 'codex'),
                'three_second_art_samples': art_hashes, 'distinct_art_samples': len(set(art_hashes)),
                'exit': s.close_evidence(out, label)}
        finally:
            if s.process.poll() is None:
                s.process.terminate(); s.pump(.3)
            control.stop_owned_fixture_hosts(folder)
    s = Session(binary, source, columns=120, rows=36)
    try:
        s.wait(lambda: 'alpha-shop' in s.text(), 'reorientation source loaded')
        results['reorientation'] = reorientation(s, source, out)
        results['documentation'] = documentation(s, out)
        results['reorientation_exit'] = s.close_evidence(out, 'reorientation')
    finally:
        if s.process.poll() is None:
            s.process.terminate(); s.pump(.3)
        control.stop_owned_fixture_hosts(source)
    return results


def reorientation(s, source, out):
    # First visit Beta, leave for Alpha, then record a new Beta output while away.
    find(s, 'Draft API guide')
    s.action('Close task panel', KEY['esc'])
    s.action('Return to tower', b'0')
    find(s, 'Review checkout release')
    s.action('Close task panel', KEY['esc'])
    time.sleep(.1)
    stamp = int(time.time() * 1000)
    db = sqlite3.connect(s.folder / 'codex/thread_history_1.sqlite')
    db.execute('INSERT INTO thread_items VALUES (?,?,?,?,?,?)', ('beta-draft', 'turn-1', 'record-new', stamp, 'agentMessage', json.dumps({'text': 'NEW WHILE AWAY: API examples now include pagination.', 'phase': 'final_answer'})))
    db.commit(); db.close()
    state = sqlite3.connect(s.folder / 'codex/state_5.sqlite')
    state.execute('UPDATE threads SET updated_at=? WHERE id=?', (stamp//1000, 'beta-draft'))
    state.commit(); state.close()
    s.pump(2)
    find(s, 'Draft API guide')
    # Do not open the new output: leave once more, then ask Since your visit.
    s.capture(out, 'return-beta-no-record-action')
    s.action('Close task panel without opening Records', KEY['esc'])
    s.action('Return to tower', b'0')
    find(s, 'Review checkout release')
    s.action('Close task panel', KEY['esc'])
    s.action('Return to tower', b'0')
    find(s, 'Draft API guide')
    s.action('Close task panel', KEY['esc'])
    s.action('Open all-floor notebook', b'b')
    s.action('Show Since your visit', b'3')
    s.action('Limit to current project', b'f')
    s.capture(out, 'beta-since-second-return')
    since = s.text()
    s.action('Show Deliveries', b'2')
    s.capture(out, 'beta-deliveries-unseen')
    deliveries = s.text()
    s.action('Close notebook', KEY['esc'])
    s.action('Open Find again', b'/')
    s.capture(out, 'finder-reopened')
    query = s.text()
    s.action('Close Find', KEY['esc'])
    return {'new_record_at': stamp, 'since_second_return_text': since,
            'delivery_still_available_text': deliveries,
            'reopened_find_text': query,
            'record_opened_or_marked_seen': False,
            'scope': 'Expert route; a view visit is not evidence a person read a record.'}


def documentation(s, out):
    observed = {}
    for label, key in [('v-advanced', b'v'), ('d-design', b'd'), ('c-connections', b'c'), ('uppercase-c-connections', b'C')]:
        # Advanced Esc returns to Settings first. Reset context explicitly so
        # the next shortcut is not swallowed by the previous modal.
        for _ in range(3):
            s.action('Close any retained overlay', KEY['esc'])
        s.action('Enter second project', b'2')
        s.action(label, key); s.capture(out, f'docs-{label}')
        observed[label] = s.text(); s.action('Close overlay', KEY['esc'])
    s.action('Show tower before Tab', b'0'); before = s.text()
    s.action('Tab once on tower', b'\t'); after = s.text()
    s.capture(out, 'docs-tab-focus')
    observed['tab_before'], observed['tab_after'] = before, after
    return observed


def approval(binary, scratch, out):
    folder = scratch / 'managed'
    control.prepare(folder)
    s = Session(binary, folder, columns=120, rows=36)
    try:
        s.wait(lambda: len(control.records(folder / 'invocations.jsonl')) >= 3, 'local stub probes')
        s.action('New task', b'n'); s.paste(str(folder / 'vertical-project'))
        s.action('Focus instruction', b'\t'); s.paste('approval')
        s.action('Start only the local fixture task', b'\x1b[15~')
        s.wait(lambda: control.state(folder).get('pending_requests'), 'fixture approval request')
        s.wait(lambda: 'Sending to' not in s.text(), 'instruction receipt')
        s.action('Close control panel', KEY['esc'])
        s.action('Open Attention', b'b')
        s.wait(lambda: 'APPROVAL NEEDED' in s.text(), 'pending approval in notebook')
        s.capture(out, 'approval-before-seen')
        s.action('Mark seen', b'r')
        s.capture(out, 'approval-after-seen')
        replies = lambda: [record for record in control.records(folder / 'provider.jsonl') if record.get('id') == 'approve-1' and 'result' in record]
        assert not replies(), 'Reading must not approve'
        assert control.state(folder).get('pending_requests'), 'Seen marker cleared the live request'
        seen_text = s.text()
        s.action('Review request', b'\r')
        s.wait(lambda: 'test-only-command' in s.text(), 'exact live request')
        s.capture(out, 'approval-review-no-answer')
        assert not replies(), 'Opening review must not approve'
        assert control.state(folder).get('pending_requests')
        s.trace.append({'action': 'Click Allow this request', 'coordinates': control.click_text(s, 'Allow this request')})
        s.wait(lambda: replies(), 'one scoped reply to fake provider')
        s.wait(lambda: not control.state(folder).get('pending_requests'), 'fake provider resolution')
        assert len(replies()) == 1 and replies()[0]['result'] == {'decision': 'accept'}
        s.capture(out, 'approval-after-explicit-answer')
        s.action('Close request panel', KEY['esc'])
        s.action('Close notebook if retained', KEY['esc'])
        result = {'seen_kept_request': True, 'seen_wrote_no_reply': True,
                  'review_wrote_no_reply': True, 'explicit_answer': replies()[0],
                  'seen_text': seen_text, 'exit': s.close_evidence(out, 'approval')}
        for name in ['provider.jsonl', 'invocations.jsonl']:
            (out / name).write_text((folder / name).read_text())
        return result
    finally:
        if s.process.poll() is None:
            s.process.terminate(); s.pump(.3)
        control.stop_owned_fixture_hosts(folder)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/native-macos/release/they-work')
    parser.add_argument('--only', choices=['compare', 'approval', 'all'], default='all')
    args = parser.parse_args()
    out = ROOT / 'docs/design-audit/iteration-7/workflows/evidence'
    out.mkdir(parents=True, exist_ok=True)
    scratch_root = ROOT / 'docs/design-audit/tmp/iteration-7-workflows'
    scratch_root.mkdir(parents=True, exist_ok=True)
    scratch = Path(tempfile.mkdtemp(prefix='run-', dir=scratch_root))
    result = {'binary_sha256': digest(args.binary), 'fixture_root': str(scratch),
              'method': 'Actual executable in native PTY; local stubs/synthetic SQLite; expert inspection',
              'images': 'Menlo/Pillow native cells + exact transmitted RGBA; no physical-terminal validation',
              'participant_sessions': 0, 'real_provider_calls': 0}
    try:
        if args.only in ('all', 'compare'):
            result['comparison'] = comparisons(args.binary.resolve(), scratch, out)
        if args.only in ('all', 'approval'):
            result['approval'] = approval(args.binary.resolve(), scratch, out)
    except Exception as error:
        result['failure'] = str(error)
        raise
    finally:
        (out / f'results-{args.only}.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'binary_sha256': result['binary_sha256'], 'output': str(out), 'completed': args.only}))


if __name__ == '__main__':
    main()
