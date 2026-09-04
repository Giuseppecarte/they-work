# Real-agent installation verification — M12

Status: incomplete. Readability diagnostics and all seven local failure-boundary
tests pass. The fresh-container interactive real-thread trial was rejected before
execution because the approval service reported a usage limit. There is no live
office transcript for this milestone yet; the earlier demo recording is not one.

## Data boundary

Only this repository's existing Claude session files were selected. Their mode
is `0600`, owned by UID 1000. The originals were never changed. An isolated copy
of the five session files was staged under
`/tmp/they-work-m12-real.oNAI2V/data/projects/they-work`, private to test UID 10002.
These are real session copies, not synthetic fixtures; copied content was not
modified to manufacture activity. The staged files remain outside the repository.
No real transcript contents are included in this public report.

The staging parent was created with `mktemp -d`, files copied with permissions
preserved, and group/other access removed. Host `sudo chown` could not proceed
without a password; a separately approved one-shot container changed ownership
of only the temporary staging tree to `10002:10002`. It had no original-data mount.

## Actual unreadable-data test

~~~sh
docker run --rm --user 10002:10002 --network none --read-only --cap-drop ALL --security-opt no-new-privileges --mount type=bind,src=/home/gc/.claude/projects/-home-gc-AIStudio-projects-they-work,dst=/data/claude/projects/they-work,readonly -e THEYWORK_CLAUDE_HOME=/data/claude -e THEYWORK_CODEX_HOME=/missing-codex ghcr.io/giuseppecarte/they-work:v0.1.0 --doctor
~~~

Exit 1. Relevant verbatim diagnostic:

~~~text
claude_store=unavailable projects=0 threads=0 active=0 reason=could not scan Claude projects: Permission denied (os error 13); owner=0:0 permissions=0o0755 status=unreadable action=check_permissions_or_set_THEYWORK_CLAUDE_HOME
~~~

The owner/mode suffix describes the home container directory, not the failing
individual session file. Improving that attribution belongs to
`crates/theywork-collect`; no crate files were edited here.

## Actual readable-data control

~~~sh
docker run --rm --user 10002:10002 --network none --read-only --cap-drop ALL --security-opt no-new-privileges --mount type=bind,src=/tmp/they-work-m12-real.oNAI2V/data,dst=/data/claude,readonly -e THEYWORK_CLAUDE_HOME=/data/claude -e THEYWORK_CODEX_HOME=/missing-codex ghcr.io/giuseppecarte/they-work:v0.1.0 --doctor
~~~

Exit 0. Relevant verbatim output:

~~~text
claude_home=found path=/data/claude discovery=override
claude_store=readable projects=1 threads=5 active=2 status=ready action=read_only
~~~

This proves diagnostic visibility, not an interactive office. The planned next
trial is a fresh UID-10002 container, no checkout, the staged store mounted at
the same absolute path, and the checksum-verified INSTALL.md block without demo
arguments. Its terminal recording must stay private because it may show real
thread text. It also requires the approved host Docker-socket access.

## Failure boundaries and documentation corrections

`python3 scripts/test-install.py` passes seven tests. These cover failed download,
syntactically valid truncation, pull failure propagation, missing image wording,
ambiguous denial wording, other Docker errors, and unreadable data stopping the
documented bootstrap before interactive startup. Network/registry behavior in
these tests is mocked; actual diagnostic probes above are separate evidence.

INSTALL.md now runs `--doctor` before interactive startup under `set -e`. It
explains the required `projects/` layout, absolute host paths, remote-daemon
limits, and UID ownership. This prevents the documented path from opening an
empty/misleading screen after an unreadable-data check. Direct interactive CLI
behavior without the preflight belongs to `crates/theywork-tui` and has not been
changed. The pinned v0.1.0 installer remains unchanged remotely.

The previously proven missing tag in the public GHCR package returned `not found`
and exit 1. A known restricted GHCR package has not yet been tested; an arbitrary
nonexistent name returning `denied` would not prove restriction. No package
visibility settings were changed to fabricate that test. The current denial
message intentionally does not guess between missing and restricted packages.

## Release size

The release notes now state the published index digest and **30,057,713 bytes
(28.67 MiB)** of compressed Linux/amd64 layers. This was measured from the
published platform manifest, not by pulling layers or guessing from disk usage.
See [release notes](release.md#v010-publication) for the exact command and scope.

## Terminal graphics coverage

The high-resolution graphics path was evaluated separately from the installer.
This host is Linux under WSL2 with `TERM=xterm-256color`; it is not an active
Windows Terminal session. `wt.exe` exists on the Windows side but could not
start from this session (`UtilBindVsockAnyPort: socket failed 1`). Therefore
Windows Terminal Sixel visual output was **not tested**. No macOS machine,
Kitty terminal, or iTerm2 instance was available, so macOS visual output was
**not tested** too.

`./scripts/cargo test -p theywork-terminal-image` passed all 12 protocol unit
tests. A release-mode synthetic 1600×960 measurement in a 160×48-cell
rectangle whose reported cells are 10×20 pixels produced these 30-frame encoder
results on this host:

| Pattern | Protocol | Bytes/frame | Encode time/frame |
| --- | --- | ---: | ---: |
| Gradient | Kitty direct | 8,210,043 | 14,381 µs |
| Gradient | Sixel | 36,286 | 2,813 µs |
| Gradient | iTerm2 inline PNG | 1,052,940 | 2,838 µs |
| Noise | Kitty direct | 8,210,043 | 14,024 µs |
| Noise | Sixel | 7,213,723 | 43,261 µs |
| Noise | iTerm2 inline PNG | 8,194,097 | 25,184 µs |

These are synthetic encoding measurements, not screen-rendering proof and are
specific to this host. The transport now encodes iTerm2 inline PNG as well as
Kitty and Sixel, but no visual playback was tested on Windows Terminal, Kitty,
or iTerm2. The final test run passed 12 terminal-image tests, 65 renderer
tests, and 27 TUI tests.
Its native-density renderer test confirms that a 160×48 area with reported
10×20-pixel cells creates a 1600×960 RGBA frame, and the TUI test confirms that
the presenter receives that frame only when terminal cell geometry is present.

## Contact-sheet handoff

The existing contact sheet deliberately exports the character-cell golden
frames only. It must not invent an image-protocol counterpart from the
synthetic `measure` example: that image is a gradient or noise pattern, not a
they-work scene.

To add image frames beside the existing cell frames, the graphics owner needs
to supply a deterministic renderer-backed dump with all of the following:

- the fixed 160×48 viewport and fixed demo timestamp used by the golden-frame
  exporter;
- the terminal-reported cell geometry used for the image frame (for example,
  10×20 pixels), the covered cell rectangle, and the resulting PNG dimensions;
- one RGBA PNG for each rendered surface and dark/light variant; and
- the exact Kitty, Sixel, and iTerm2 first-frame byte streams produced from
  that PNG, retained as binary evidence rather than displayed as text.

For the 160×48 / 10×20 example, each full-viewport source PNG must be labelled
`1600×960`; a label such as `320×144` would describe only the character
fallback's logical canvas, not the graphics image. A manifest that records the
viewport, cell geometry, frame dimensions, fixed timestamp, surface, theme,
and protocol filenames gives the contact-sheet exporter enough information to
validate and place the image panel beside its cell counterpart.

The current renderer and TUI now produce that physical-pixel frame, but the
only `--dump` interface currently available emits a synthetic benchmark image.
There is therefore still no deterministic, renderer-backed eight-panel dump to
add to the contact sheet. The exporter, Make target, and manifest contract are
ready for it; supplying that dump will place verified PNGs beside the primary
cell panels without representing a benchmark pattern as office art.

## Remaining proof

- Complete the real-thread office trial and preserve its verbatim private output,
  inputs, and timing; report the local evidence path without publishing content.
- Test a known restricted image and distinguish its observed response from a
  known missing tag. Do not infer package existence from `denied` alone.
- Recheck all final documentation against the completed trial. No files have
  been staged or committed by this work.
