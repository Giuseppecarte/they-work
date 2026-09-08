#!/usr/bin/env python3
"""Native PTY receipt timings, never physical visible-response measurements."""
import argparse
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import platform
import random
import select
import shutil
import sqlite3
import sys
import tempfile
import time
from PIL import Image

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[5]
spec = importlib.util.spec_from_file_location('workflow', ROOT / 'docs/design-audit/iteration-7/workflows/walkthrough.py')
w = importlib.util.module_from_spec(spec)
spec.loader.exec_module(w)


class ObservedStream:
    def __init__(self, session, stream):
        self.session, self.stream = session, stream

    def feed(self, text):
        self.stream.feed(text)
        s = self.session
        if s.transaction and s.transaction.get('native_ns') is None and s.predicate():
            s.transaction['native_ns'] = s.read_ns


class Session(w.Session):
    def __init__(self, *args, **kwargs):
        self.transaction, self.predicate = None, lambda: False
        self.read_ns = 0
        self.frame_events = []
        self.received_bytes = 0
        super().__init__(*args, **kwargs)
        self.stream = ObservedStream(self, self.stream)

    def pump(self, duration=.01):
        end = time.monotonic() + duration
        while time.monotonic() < end:
            if select.select([self.master], [], [], max(0, end - time.monotonic()))[0]:
                data = os.read(self.master, 1048576)
                self.read_ns = time.monotonic_ns()
                if not data:
                    break
                self.received_bytes += len(data)
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
        # The receipt timestamp precedes image decoding/hash work. It is the read
        # containing the terminating ESC-backslash of the final image chunk.
        previous = self.active_image
        header = packet[3:-2].partition(b';')[0]
        params = dict(part.split(b'=', 1) for part in header.split(b',') if b'=' in part)
        candidate = params.get(b'a') == b'T' or previous is not None
        super().packet(packet)
        if candidate and self.active_image is None and self.art is not None:
            h = hashlib.sha256(self.art['image'].tobytes()).hexdigest()
            event = {'received_ns': self.read_ns, 'sha256': h, 'size': list(self.art['image'].size)}
            self.frame_events.append(event)
            if self.transaction and h == self.transaction.get('expected_frame') and self.read_ns >= self.transaction['start_ns']:
                self.transaction.setdefault('frame_ns', self.read_ns)

    def until(self, condition, label, timeout=5):
        end = time.monotonic() + timeout
        while not condition():
            if time.monotonic() > end:
                raise AssertionError(f'Timeout: {label}\n{self.text()}')
            self.pump(.002)
            assert self.process.poll() is None, f'Exited during {label}'

    def move(self, key, condition, label):
        os.write(self.master, key)
        self.until(condition, label)
        self.pump(.06)

    def art_hash(self):
        return hashlib.sha256(self.art['image'].tobytes()).hexdigest() if self.art else None


def summary(values):
    if not values:
        return {'status': 'not-tested', 'count': 0}
    ordered = sorted(values)
    return {'status': 'measured-pty-receipt', 'count': len(values),
            'p50_ms': ordered[math.ceil(len(values) * .50) - 1],
            'p95_ms': ordered[math.ceil(len(values) * .95) - 1],
            'min_ms': ordered[0], 'max_ms': ordered[-1]}


def run(binary, folder, graphics, count):
    w.fixture(folder)
    (folder / 'settings/appearance.json').write_text(json.dumps({'motion': False}))
    s = Session(binary, folder, graphics=graphics, columns=120, rows=36)
    result = {'mode': 'kitty-simulated' if graphics else 'image-free-half-blocks',
              'terminal_cells': [120, 36], 'declared_cell_pixels': [8, 16],
              'motion': False, 'failures': [], 'input_samples': [], 'source_samples': []}
    tower = lambda: 'Software tower' in s.text()
    office = lambda: 'alpha-shop / Office' in s.text()
    try:
        s.until(lambda: 'alpha-shop' in s.text(), 'fixture loaded')
        s.move(b'0', tower, 'tower baseline')
        # Two unmeasured cycles verify target pixels are stable and distinct
        # from baseline. Without that evidence graphical correlation is refused.
        calibration = []
        for _ in range(2):
            before = s.art_hash()
            s.move(b'1', office, 'calibrate floor entry')
            after = s.art_hash()
            calibration.append({'tower_hash': before, 'office_hash': after,
                                'office_pixels': list(s.art['image'].size) if s.art else None})
            s.move(b'0', tower, 'reset calibrated tower')
        expected = calibration[0]['office_hash']
        reliable = bool(graphics and expected and expected != calibration[0]['tower_hash']
                        and calibration[1]['office_hash'] == expected
                        and calibration[1]['tower_hash'] == calibration[0]['tower_hash'])
        result['graphical_calibration'] = {'cycles': calibration, 'correlation_verified': reliable}
        for index in range(count):
            s.move(b'0', tower, 'reset tower for measured entry')
            assert not office()
            s.raw.clear(); s.frame_events.clear()
            s.predicate = office
            s.transaction = {'start_ns': time.monotonic_ns(), 'expected_frame': expected if reliable else None}
            os.write(s.master, b'1')
            s.until(lambda: s.transaction.get('native_ns') is not None, 'native floor feedback')
            if reliable:
                s.until(lambda: 'frame_ns' in s.transaction, 'calibrated office image complete')
            transaction = s.transaction; s.transaction = None
            row = {'sample': index + 1, 'key_hex': '31',
                   'correct_feedback': 'alpha-shop / Office',
                   'native_ms': (transaction['native_ns'] - transaction['start_ns']) / 1e6}
            if reliable:
                row.update(graphical_ms=(transaction['frame_ns'] - transaction['start_ns']) / 1e6,
                           expected_frame_sha256=expected,
                           matched_frames=sum(e['sha256'] == expected for e in s.frame_events))
            result['input_samples'].append(row)
        # Select one brief once. Every later source sample has a unique literal
        # token so receipt cannot be satisfied by an earlier observation.
        s.move(b'/', lambda: 'FIND YOUR TEAM' in s.text(), 'open finder')
        s.paste('Draft API guide')
        s.move(b'\r', lambda: 'Draft API guide' in s.text() and 'OBSERVED' in s.text(), 'selected Beta brief')
        phases = random.Random(731).sample(range(0, 1101), count)
        for index in range(count):
            # Avoid always appending immediately after the preceding collection
            # tick. Same deterministic delay schedule is used in both modes.
            s.pump(phases[index] / 1000)
            token = f'LATENCY-SOURCE-{index + 1:03}-{time.monotonic_ns()}'
            stamp = int(time.time() * 1000)
            history = sqlite3.connect(folder / 'codex/thread_history_1.sqlite')
            state = sqlite3.connect(folder / 'codex/state_5.sqlite')
            history.execute('INSERT INTO thread_items VALUES (?,?,?,?,?,?)', ('beta-draft', 'turn-1', token, stamp,
                            'agentMessage', json.dumps({'text': token, 'phase': 'final_answer'})))
            state.execute('UPDATE threads SET updated_at=? WHERE id=?', (stamp//1000, 'beta-draft'))
            s.predicate = lambda token=token: token in s.text() and 'Draft API guide' in s.text() and 'OBSERVED' in s.text()
            commit_start = time.monotonic_ns()
            history.commit(); state.commit()
            ready = time.monotonic_ns()
            history.close(); state.close()
            s.transaction = {'start_ns': ready}
            s.until(lambda: s.transaction.get('native_ns') is not None, 'unique appended observation', timeout=8)
            transaction = s.transaction; s.transaction = None
            result['source_samples'].append({'sample': index + 1, 'token': token,
                'pre_append_phase_delay_ms': phases[index],
                'commit_ms': (ready - commit_start) / 1e6,
                'committed_source_to_brief_ms': (transaction['native_ns'] - ready) / 1e6})
            # Drain queued unchanged output; do not save large ANSI streams.
            s.raw.clear(); s.frame_events.clear()
            s.pump(.02)
        s.move(b'\x1b', lambda: 'OBSERVED' not in s.text(), 'close brief')
        result['exit'] = s.finish()
    except Exception as error:
        result['failures'].append({'error': str(error), 'native_tail': s.text()})
    finally:
        if s.process.poll() is None:
            s.process.terminate(); s.pump(.3)
            if s.process.poll() is None:
                s.process.kill()
        w.control.stop_owned_fixture_hosts(folder)
        result['bytes_received_total'] = s.received_bytes
        result['input_native'] = summary([x['native_ms'] for x in result['input_samples']])
        result['graphical_complete'] = summary([x['graphical_ms'] for x in result['input_samples'] if 'graphical_ms' in x])
        result['source_to_brief'] = summary([x['committed_source_to_brief_ms'] for x in result['source_samples']])
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--samples', type=int, default=25)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/native-macos/release/they-work')
    args = parser.parse_args()
    if not 20 <= args.samples <= 50:
        parser.error('Use 20–50 samples per mode')
    scratch = ROOT / 'docs/design-audit/tmp/iteration-7-workflows'
    scratch.mkdir(parents=True, exist_ok=True)
    folder = Path(tempfile.mkdtemp(prefix='latency-', dir=scratch))
    result = {'binary_sha256': hashlib.sha256(args.binary.read_bytes()).hexdigest(),
              'kind': 'PTY read receipt, not physical visible response',
              'fixture_root': str(folder), 'samples_requested_each': args.samples,
              'clock': 'time.monotonic_ns', 'handshake': 'Synthetic Kitty direct-image OK and 8x16 cell / 120x36 terminal reply',
              'host_platform': platform.platform(), 'python': platform.python_version(),
              'source_phase_schedule': 'random.Random(731).sample(range(0,1101), count), milliseconds; same schedule in both modes',
              'modes': []}
    for mode, graphics in [('native', False), ('kitty', True)]:
        fixture = folder / 'fixture'
        if fixture.exists():
            shutil.rmtree(fixture)  # Only this run's generated fixture; stable path preserves identity.
        result['modes'].append(run(args.binary.resolve(), fixture, graphics, args.samples))
    out = Path(__file__).with_name('results.json')
    out.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({x['mode']: {k:x[k] for k in ['input_native','graphical_complete','source_to_brief','failures']} for x in result['modes']}, indent=2))
    return int(any(x['failures'] for x in result['modes']))


if __name__ == '__main__':
    raise SystemExit(main())
