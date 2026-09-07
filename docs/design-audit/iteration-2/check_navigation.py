#!/usr/bin/env python3
"""Exercise the final native executable's office paging and identity on resize."""
import json
import sys

sys.dont_write_bytecode = True
import review_pty as pty


def main():
    output = pty.EVIDENCE / 'final-navigation'
    output.mkdir(parents=True, exist_ok=True)
    session = pty.Session(pty.ROOT / 'target/native-macos/release/they-work', 20, 'apple-terminal')
    results = {}
    try:
        session.resize(80, 24)
        assert 'page 1/7' in '\n'.join(session.screen.display)
        session.key(b'\x1b[6~')  # PageDown
        assert 'page 2/7' in '\n'.join(session.screen.display)
        session.key(b'\x1b[F')  # End
        assert 'page 7/7' in '\n'.join(session.screen.display)
        session.save(output / 'last-page-80x24')
        for width, height, expected in [(192, 58, 'page 2/2'), (120, 32, 'page 4/4'), (80, 24, 'page 7/7')]:
            session.resize(width, height)
            assert expected in '\n'.join(session.screen.display)
        session.resize(120, 32)
        session.key(b'\r')
        # The stable thread IDs are sorted lexically: person-9 is the last one.
        assert 'DESK / Design the office #10' in '\n'.join(session.screen.display)
        session.save(output / 'last-worker-inspector-120x32')
        session.key(b'\x1b')
        session.key(b'\x1b[H')  # Home
        assert 'page 1/4' in '\n'.join(session.screen.display)
        session.key(b'\x1b[6~')
        assert 'page 2/4' in '\n'.join(session.screen.display)
        session.key(b'\x1b[5~')  # PageUp
        assert 'page 1/4' in '\n'.join(session.screen.display)
        results.update(paging=True, resize_preserves_identity=True, inspector=True)
    finally:
        results.update(session.finish())
        (output / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
    print(json.dumps(results))


if __name__ == '__main__':
    main()
