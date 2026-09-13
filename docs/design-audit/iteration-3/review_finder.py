#!/usr/bin/env python3
"""Exercise global search through the native PTY using isolated local records.
The PNGs replay ANSI cells; --font additionally replays selected buffers with CoreText.
"""
import argparse
import importlib.util
import json
import pathlib
import re
import subprocess
import sys

sys.dont_write_bytecode = True
ROOT = pathlib.Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location('tower_review', pathlib.Path(__file__).with_name('review_tower.py'))
tower = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tower)
r = tower.base
r.SCRATCH = ROOT / 'docs/design-audit/tmp/iteration-3-finder'
KEY = {'esc': b'\x1b', 'home': b'\x1b[H', 'end': b'\x1b[F', 'left': b'\x1b[D',
       'right': b'\x1b[C', 'down': b'\x1b[B', 'up': b'\x1b[A', 'delete': b'\x1b[3~',
       'pgdown': b'\x1b[6~', 'pgup': b'\x1b[5~', 'clear': b'\x15', 'ctrlk': b'\x0b'}


def text(session):
    return '\n'.join(session.screen.display)


def finder_open(session):
    return 'FIND YOUR TEAM' in text(session) or 'Find team' in text(session)


def floor(session):
    match = re.search(r'Floor (\d+)/(\d+)', text(session))
    assert match, 'Missing tower footer'
    return int(match.group(1))


def paste(session, value):
    session.key(b'\x1b[200~' + value.encode() + b'\x1b[201~')



def contrast_samples(session):
    def luminance(value):
        channels = [int(value[i:i+2], 16) / 255 for i in (1, 3, 5)]
        linear = [x/12.92 if x <= .04045 else ((x+.055)/1.055)**2.4 for x in channels]
        return sum(x*y for x, y in zip(linear, [.2126, .7152, .0722]))
    samples = {}
    for label, needle in [('title', 'FIND YOUR TEAM'), ('keys', '↑↓ choose'),
                          ('selected_context', '03-customer-onboarding'),
                          ('attention', 'ATTENTION'), ('selected_error', 'FAILED')]:
        for y, row in enumerate(session.screen.display):
            if needle not in row:
                continue
            cell = session.screen.buffer[y][row.index(needle)]
            fg, bg = r.color(cell.fg), r.color(cell.bg, True)
            if cell.reverse: fg, bg = bg, fg
            ratio = (max(luminance(fg), luminance(bg))+.05)/(min(luminance(fg), luminance(bg))+.05)
            samples[label] = {'foreground': fg, 'background': bg, 'contrast_ratio': round(ratio, 3)}
            break
    assert len(samples) == 5, 'Could not locate every light finder text role'
    return samples


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=pathlib.Path, default=ROOT / 'target/native-macos/release/they-work')
    parser.add_argument('--stage', default='finder-final')
    parser.add_argument('--font', action='store_true')
    args = parser.parse_args()
    out = ROOT / 'docs/design-audit/iteration-3/evidence' / args.stage
    out.mkdir(parents=True, exist_ok=True)
    results = {}
    session = r.Session(args.binary, 20, 'half-blocks', motion=False)
    current = 'initial'
    try:
        session.key(b'0'); session.key(KEY['home']); session.resize(80, 24)
        for _ in range(7): session.key(KEY['down'])
        assert floor(session) == 8
        current = 'slash-and-escape'
        session.key(b'/'); assert finder_open(session)
        assert '20 projects' in text(session)
        session.save(out / 'finder-projects-80x24')
        session.key(KEY['esc']); assert not finder_open(session)
        assert floor(session) == 8, 'Escape changed the project behind the search'
        results[current] = True

        current = 'modal-global-characters'
        session.key(KEY['ctrlk']); assert finder_open(session)
        session.key(b'qcs?'); assert finder_open(session)
        assert '/ qcs?' in text(session) and 'No matching projects' in text(session)
        assert session.process.poll() is None, 'Typing q quit the application'
        session.save(out / 'finder-modal-80x24')
        results[current] = True

        current = 'provider-pagination'
        session.key(KEY['clear']); session.key(b'codex')
        assert '60 matches' in text(session), text(session)
        first = re.search(r'60 matches · (\d+)–(\d+)', text(session))
        assert first and first.group(1) == '1'
        session.key(KEY['pgdown'])
        second = re.search(r'60 matches · (\d+)–(\d+)', text(session))
        assert second and int(second.group(1)) > 1
        session.save(out / 'finder-provider-page-80x24')
        session.key(KEY['pgup']); assert '60 matches · 1–' in text(session)
        results[current] = True

        current = 'bracketed-paste-exact-target'
        session.key(KEY['clear']); paste(session, '20-billing-service account')
        assert re.search(r'\b1 match(?:es)? ·', text(session)), 'Pasted query did not reach the finder'
        assert 'Build account settings' in text(session) and '20-billing-service' in text(session)
        for width, height in [(80, 24), (120, 32), (110, 80)]:
            session.resize(width, height)
            assert re.search(r'\b1 match(?:es)? ·', text(session)) and finder_open(session)
            session.save(out / f'finder-exact-{width}x{height}')
        session.resize(120, 32); session.key(b'\r')
        assert not finder_open(session)
        assert 'Build account settings' in text(session) and '20-billing-service' in text(session), 'Search opened the wrong conversation'
        assert 'NEEDS ATTENTION' in text(session) and 'Thread: person-57' in text(session), 'The target should be the silent worker on floor 20'
        session.save(out / 'finder-opened-desk-120x32')
        results[current] = True

        current = 'escape-retains-desk'
        before = session.screen.display[0]
        session.key(KEY['ctrlk']); assert finder_open(session)
        session.key(KEY['esc']); assert not finder_open(session)
        assert 'Build account settings' in text(session) and 'NEEDS ATTENTION' in text(session)
        assert session.screen.display[0] == before
        results[current] = True

        current = 'editing-and-unicode-cursor'
        session.key(b'/'); session.key(b'20-billing-service accounX')
        session.key(KEY['end']); session.key(KEY['left']); session.key(KEY['delete']); session.key(b't')
        assert re.search(r'\b1 match(?:es)? ·', text(session))
        session.key(KEY['home']); session.key(KEY['right']); session.key(b'\x7f'); session.key(b'2')
        assert re.search(r'\b1 match(?:es)? ·', text(session)) and '/ 20-billing-service account' in text(session)
        session.key(KEY['clear']); paste(session, '路径 ' * 40 + 'visible-end')
        session.resize(80, 24); session.key(KEY['end'])
        assert 'visible-end' in text(session)
        assert 0 <= session.screen.cursor.x < 80 and 0 <= session.screen.cursor.y < 24
        session.save(out / 'finder-long-query-end-80x24')
        session.key(KEY['home']); assert '路径' in text(session)
        session.key(KEY['delete']); session.key(KEY['right']); session.key(b'\x7f')
        session.save(out / 'finder-long-query-home-80x24')
        results[current] = True

        current = 'tiny-and-resize-recovery'
        session.key(KEY['clear']); session.key(b'20-billing-service account')
        assert re.search(r'\b1 match(?:es)? ·', text(session))
        session.resize(28, 10)
        assert 'Enlarge window' in text(session)
        session.key(b'\r'); assert finder_open(session), 'Enter selected an invisible result'
        session.save(out / 'finder-tiny-28x10')
        session.resize(80, 24); assert finder_open(session)
        assert re.search(r'\b1 match(?:es)? ·', text(session)), 'Tiny mode lost the valid query'
        session.key(KEY['clear']); session.key(b'zzzz-no-such-team')
        assert 'No matching projects' in text(session)
        session.save(out / 'finder-empty-80x24')
        session.key(KEY['esc']); assert not finder_open(session)
        results[current] = True

        current = 'light-appearance'
        session.key(KEY['esc']); session.key(b'0'); session.key(b's'); session.key(KEY['down']); session.key(b'\r'); session.key(KEY['esc'])
        session.key(b'/'); session.key(b'attention')
        assert finder_open(session) and '10 matches' in text(session)
        session.resize(120, 32); session.save(out / 'finder-light-120x32')
        samples = contrast_samples(session)
        (out / 'contrast.json').write_text(json.dumps(samples, indent=2) + '\n')
        assert all(sample['contrast_ratio'] >= 4.5 for sample in samples.values()), samples
        outside = r.color(session.screen.buffer[28][1].bg, True)
        header = r.color(session.screen.buffer[1][1].bg, True)
        assert outside == header, 'Light appearance leaves an unpainted dark region outside the panels'
        results['light-outside-background'] = {'outside': outside, 'header': header}
        session.key(KEY['esc']); results[current] = True
    except Exception as error:
        session.save(out / f'failure-{current}')
        results[current] = {'failure': str(error)}
        raise
    finally:
        if session.process.poll() is None:
            session.key(KEY['esc']); session.key(KEY['esc'])
            results['terminal'] = session.finish()
        (out / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
    if args.font:
        cache = r.SCRATCH / 'swift-cache'; cache.mkdir(parents=True, exist_ok=True)
        for name in ['finder-exact-80x24', 'finder-light-120x32']:
            subprocess.run(['swift', '-module-cache-path', str(cache), str(ROOT / 'docs/design-audit/iteration-2/glyph_metrics.swift'),
                            str(out / (name + '-menlo')), str(out / (name + '.cells.json'))], cwd=ROOT, check=True)
    print(json.dumps(results, indent=2))


if __name__ == '__main__':
    main()
