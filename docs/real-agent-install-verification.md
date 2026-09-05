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

- the fixed 160×86 comparison viewport and fixed demo timestamp, plus matching
  primary character goldens;
- the terminal-reported cell geometry used for the image frame (for example,
  10×20 pixels), the covered cell rectangle, and the resulting PNG dimensions;
- one RGBA PNG for each rendered surface and dark/light variant; and
- the exact Kitty, Sixel, and iTerm2 first-frame byte streams produced from
  that PNG, retained as binary evidence rather than displayed as text.

For the 160×86 / 10×10 comparison target, each full-viewport source PNG must
be labelled `1600×860`; a label such as `320×144` would describe neither that
character presentation nor the graphics image. The source's 160×86 primary
character golden must land with the image dump: the exporter requires matching
viewports rather than comparing frames of different terminal heights. A
manifest that records the viewport, cell geometry, frame dimensions, fixed
timestamp, surface, theme, and protocol filenames gives the contact-sheet
exporter enough information to validate and place the image panel beside its
cell counterpart.

The renderer owner supplied
`target/renderer-dark-floor-1600x860.bmp`: a top-down 32-bit 1600×860 BMP from
the dark floor at the fixed snapshot time. Its captured cell rectangle is
`Rect(0,3,160,86)` in a 160×91 terminal; the RGBA FNV-1a checksum is
`0xc1261bafff5c1448` and the BMP SHA-256 is
`301eff144552d68d4199c3c361543845749ae1ff20aaeb07818d7e5c0d799ccc`.

`make shot IMAGE_FRAME=target/renderer-dark-floor-1600x860.bmp` converts that
source capture to `docs/shots/image-floor-dark-1600x860.png` and places it
beside the matching dark character panel. Both panels label the comparison:
the character canvas is 160×86 and the renderer image is 1600×860 pixels. This
is source-buffer visual evidence, not graphical playback by a Windows or macOS
terminal. A full surface/theme manifest with exact protocol bytes remains the
route for expanding the comparison beyond the captured dark floor.

## Published-image locale handoff

The currently published `v0.1.0` image was inspected directly with
`docker run --rm --entrypoint /usr/bin/env ghcr.io/giuseppecarte/they-work:v0.1.0`.
It contained `TERM=xterm-256color` and the two data-home variables, but neither
`LANG`, `LC_ALL`, nor `LC_CTYPE`. This explains the half-block fallback in a
container even when the host supports UTF-8: the renderer selects quadrants
only when one of those locale variables advertises UTF-8.

`TERM_PROGRAM` has two distinct uses in the current crate implementation. The
renderer uses values such as `kitty` only to select sextants over quadrants for
the character fallback; the graphics transport instead selects Kitty, Sixel,
or iTerm2 from actual terminal probe replies and reported cell geometry.
`TERM_PROGRAM=iTerm.app` is additionally an iTerm2 fallback, intentionally
disabled inside tmux and screen. Therefore the packaging change needs a baked
UTF-8 locale; forwarding `TERM_PROGRAM` through the installer is a separate
crate-owner policy decision, not a prerequisite for graphics negotiation.

The release workflow now pulls its just-published immutable digest and runs
`scripts/test-published-image.py`. That verifier checks the container's baked
locale, runs `--demo` in a 160×48 PTY that replies as a Kitty terminal with
8×16-pixel cells, and confirms an actual Kitty image transmission. A second
no-reply UTF-8 PTY proves the conservative character fallback by requiring
quadrant-specific glyphs. Upper and lower half-block glyphs can legitimately
occur as two masks in the quadrant alphabet, so their raw presence is reported
but is not misclassified as half-block mode. It also replies as iTerm2 and
requires an inline-image packet, preventing a release from silently omitting
that source capability. This is a protocol simulation, not visual playback on
Windows or macOS.

## Remaining proof

- Complete the real-thread office trial and preserve its verbatim private output,
  inputs, and timing; report the local evidence path without publishing content.
- Test a known restricted image and distinguish its observed response from a
  known missing tag. Do not infer package existence from `denied` alone.
- Recheck all final documentation against the completed trial. No files have
  been staged or committed by this work.
