#!/usr/bin/env python3
"""Verify a registry index on both runnable platforms through simulated PTYs.

The report distinguishes daemon-native from emulated architecture execution. It
is protocol evidence, not a physical-terminal or authenticated-provider test.
"""
import argparse
import fcntl
import json
import os
from pathlib import Path
import platform as host_platform
import pty
import select
import struct
import subprocess
import sys
import termios
import tempfile
import time

from release_image import PLATFORMS, Registry, index_for, require, write_json

ROOT = Path(__file__).resolve().parents[1]
EXPECTED_LOCALE = {'LANG': 'C.UTF-8', 'LC_ALL': 'C.UTF-8', 'LC_CTYPE': 'C.UTF-8'}
HALF_BLOCKS = ('▀', '▄')
QUADRANT_GLYPHS = ('▘', '▝', '▖', '▌', '▞', '▛', '▗', '▚', '▐', '▜', '▙', '▟')
KITTY_REPLY = b'\x1b_Gi=31;OK\x1b\\\x1b[6;16;8t\x1b[8;48;160t'
ITERM_REPLY = b'\x1bP>|iTerm2 3.5.0\x1b\\\x1b[6;16;8t\x1b[8;48;160t'
PROBE_MARKERS = (b'\x1b_G', b'\x1b[c', b'\x1b[16t', b'\x1b[>q')
# The default image-free office is a native roster, not pixel glyph art.
# These independent demo facts require identity, state, project and navigation.
NATIVE_MARKERS = (b'checkout', b'Checkout lead', b'Timeout tests', b'Retry endpoint',
                  b'Question for you', b'People 3/3', b'[ Back ]')


def fallback_ready(frame):
    return all(marker in frame for marker in NATIVE_MARKERS)


def command(*args, check=True):
    return subprocess.run(['docker', *args], cwd=ROOT, check=check, text=True,
                          stdout=subprocess.PIPE, stderr=subprocess.STDOUT)


def published_digest(image):
    # Resolve at the registry, never pick an arbitrary local RepoDigests entry.
    if '@' not in image:
        manifest = Registry().inspect(image)
        repository = image.rsplit(':', 1)[0] if ':' in image.rsplit('/', 1)[-1] else image
        image = f"{repository}@{manifest['digest']}"
    index_for(Registry(), image)
    return image


def runtime_environment(image, platform):
    result = command('run', '--rm', '--platform', platform, '--network', 'none',
                     '--read-only', '--cap-drop', 'ALL', '--security-opt', 'no-new-privileges',
                     '--entrypoint', '/usr/bin/env', image)
    return dict(line.split('=', 1) for line in result.stdout.splitlines() if '=' in line)


def read_pty(master, process, reply, transmission_marker, timeout):
    output = bytearray()
    replied = False
    quit_sent = False
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        ready, _, _ = select.select([master], [], [], 0.05)
        if ready:
            try:
                chunk = os.read(master, 65536)
            except OSError:
                break
            if not chunk:
                break
            output.extend(chunk)
            if reply is not None and not replied and any(marker in output for marker in PROBE_MARKERS):
                os.write(master, reply)
                replied = True
            graphic = transmission_marker is not None and transmission_marker in output
            native_roster = fallback_ready(output)
            if (graphic if transmission_marker else native_roster) and not quit_sent:
                os.write(master, b'q')
                quit_sent = True
        elif process.poll() is not None:
            break
    return bytes(output), replied, quit_sent


def terminal_frame(image, platform, reply=None, transmission_marker=None, environment=(), timeout=30):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 48, 160, 0, 0))
    process = None
    forced_cleanup = False
    with tempfile.TemporaryDirectory(prefix='they-work-image-pty-') as temporary:
        cidfile = Path(temporary) / 'container-id'
        try:
            invocation = ['docker', 'run', '--rm', '-it', '--platform', platform,
                          '--cidfile', str(cidfile), '--network', 'none', '--read-only',
                          '--cap-drop', 'ALL', '--security-opt', 'no-new-privileges',
                          '-e', 'TERM=xterm-256color', '-e', 'COLORTERM=truecolor']
            for name, value in environment:
                invocation.extend(('-e', f'{name}={value}'))
            process = subprocess.Popen([*invocation, image, '--demo'], cwd=ROOT,
                                       stdin=slave, stdout=slave, stderr=slave, start_new_session=True)
            os.close(slave)
            slave = None
            output, replied, quit_sent = read_pty(master, process, reply, transmission_marker, timeout)
            try:
                exit_code = process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                forced_cleanup = True
                exit_code = None
            return output, {'exit_code': exit_code, 'quit_sent': quit_sent,
                            'probe_answered': replied, 'forced_cleanup': forced_cleanup}
        finally:
            if slave is not None:
                os.close(slave)
            if process is not None and process.poll() is None:
                if cidfile.exists():
                    command('kill', cidfile.read_text().strip(), check=False)
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
            os.close(master)


def glyph_counts(frame):
    text = frame.decode('utf-8', errors='ignore')
    return {'half': sum(text.count(glyph) for glyph in HALF_BLOCKS),
            'quadrant': sum(text.count(glyph) for glyph in QUADRANT_GLYPHS),
            'sextant': sum(0x1FB00 <= ord(glyph) <= 0x1FB3B for glyph in text)}


def check_frame(frame, result, transmission_marker):
    failures = []
    if result['exit_code'] != 0 or not result['quit_sent'] or result['forced_cleanup']:
        failures.append('Interactive process did not exit normally after q')
    if transmission_marker is not None:
        if not result['probe_answered'] or transmission_marker not in frame:
            failures.append('Expected capability probe and graphics transmission')
    elif b'\x1b_Ga=T,' in frame or b'\x1b]1337;File=' in frame:
        failures.append('No-reply fallback unexpectedly transmitted graphics')
    elif not fallback_ready(frame):
        failures.append('No-reply fallback lacks expected project, task, state or navigation facts')
    return failures


def verify_platform(image, platform, digest, daemon_arch, timeout):
    command('pull', '--platform', platform, image)
    environment = runtime_environment(image, platform)
    failures = [f'{name} must be {value}' for name, value in EXPECTED_LOCALE.items()
                if environment.get(name) != value]
    frames = {}
    cases = (('kitty', KITTY_REPLY, b'\x1b_Ga=T,', ()),
             ('iterm2', ITERM_REPLY, b'\x1b]1337;File=', (('TERM_PROGRAM', 'iTerm.app'),)),
             ('fallback', None, None, ()))
    for name, reply, marker, extra in cases:
        frame, result = terminal_frame(image, platform, reply, marker, extra, timeout)
        errors = check_frame(frame, result, marker)
        result.update({'passed': not errors, 'errors': errors, 'glyph_counts': glyph_counts(frame),
                       'native_roster': fallback_ready(frame)})
        frames[name] = result
        failures.extend(f'{name}: {error}' for error in errors)
    selected_arch = platform.split('/')[1]
    return {'passed': not failures, 'manifest_digest': digest, 'frames': frames,
            'locale': {key: environment.get(key) for key in EXPECTED_LOCALE},
            'execution': 'daemon-native' if daemon_arch == selected_arch else 'emulated',
            'errors': failures}


def count_report(graphics_counts, iterm_counts, fallback_counts):
    return (
        "160x48 PTY glyph counts: "
        f"Kitty probe half={graphics_counts['half']} quadrant={graphics_counts['quadrant']} sextant={graphics_counts['sextant']}; "
        f"iTerm2 probe half={iterm_counts['half']} quadrant={iterm_counts['quadrant']} sextant={iterm_counts['sextant']}; "
        f"no-reply fallback half={fallback_counts['half']} quadrant={fallback_counts['quadrant']} sextant={fallback_counts['sextant']}"
    )


def write_github_summary(image, report):
    summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
    if not summary_path:
        return
    with Path(summary_path).open("a", encoding="utf-8") as summary:
        summary.write("## Candidate image PTY verification\n\n")
        summary.write(f"`{image}`\n\n")
        summary.write(f"Result: {'passed' if report['passed'] else 'failed'}. ")
        summary.write("Simulated PTY evidence; not a physical-terminal test.\n\n")
        for platform, result in report['platforms'].items():
            summary.write(f"### {platform} ({result['execution']})\n\n")
            summary.write(f"Manifest: `{result['manifest_digest']}`\n\n")
            frames = result['frames']
            summary.write(count_report(*(frames[mode]['glyph_counts']
                                         for mode in ('kitty', 'iterm2', 'fallback'))) + "\n\n")
            summary.write("| Mode | Result | Normal exit after q |\n")
            summary.write("| --- | --- | --- |\n")
            for mode, frame in frames.items():
                normal_exit = (frame['exit_code'] == 0 and frame['quit_sent']
                               and not frame['forced_cleanup'])
                summary.write(f"| {mode} | {'passed' if frame['passed'] else 'failed'} | "
                              f"{'yes' if normal_exit else 'no'} |\n")
            summary.write("\n")
            for error in result['errors']:
                summary.write(f"- {error}\n")
        if 'error' in report:
            summary.write(f"Verification error: {report['error']}\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--image', required=True, help='Registry index tag or immutable digest')
    parser.add_argument('--platform', action='append', choices=PLATFORMS,
                        help='Default: both platforms; partial reports cannot be promoted')
    parser.add_argument('--commit', required=True, help='Full source commit used to build this image')
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--source-state', choices=('clean', 'dirty', 'unknown'), default='unknown')
    parser.add_argument('--timeout', type=float, default=30, help='Maximum seconds per simulated PTY')
    args = parser.parse_args()
    report = {'schema': 1, 'passed': False, 'commit': args.commit, 'image': args.image,
              'platforms': {}, 'terminal_evidence': 'simulated PTY; not a physical terminal',
              'driver_platform': host_platform.platform(), 'source_state': args.source_state}
    try:
        require(0 < args.timeout <= 120, 'PTY timeout must be within 0–120 seconds')
        image = published_digest(args.image)
        _, manifest, manifests = index_for(Registry(), image)
        report.update({'image': image, 'index': manifest})
        daemon_arch = command('info', '--format', '{{.Architecture}}').stdout.strip()
        daemon_arch = {'aarch64': 'arm64', 'x86_64': 'amd64'}.get(daemon_arch, daemon_arch)
        report['daemon_architecture'] = daemon_arch
        for platform in dict.fromkeys(args.platform or PLATFORMS):
            report['platforms'][platform] = verify_platform(image, platform, manifests[platform],
                                                           daemon_arch, args.timeout)
        report['passed'] = all(value['passed'] for value in report['platforms'].values())
    except Exception as error:
        report['error'] = str(error)
    write_json(args.report, report)
    write_github_summary(report['image'], report)
    print(json.dumps(report, indent=2))
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
