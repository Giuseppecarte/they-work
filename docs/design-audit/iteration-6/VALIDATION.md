# Iteration 6 validation

This report distinguishes local compositor behavior, an executable in a PTY, and
physical-terminal/provider/user validation. A passing layer does not certify the
next one. All task fixtures are synthetic; no real model task or personal
conversation store was used for these checks.

## Environment and reproducibility

- macOS 26.6.2, arm64; Rust 1.90; Ratatui 0.29 and Crossterm 0.28.1.
- Native Cargo wrapper: `sh docs/design-audit/native-cargo.sh`; artifacts in
  `target/native-macos`. The wrapper is an audit convenience, not a dependency
  required by the application.
- Image replays use Menlo/Pillow. Source and control PTYs use isolated homes and
  fake provider executables. The simulated supervisor requires a local loopback
  socket. No physical terminal application was controlled.
- Remove inherited `NO_COLOR` only for captures explicitly labeled colored.
  The inventory records effective encoding, colors and cell dimensions.

## Automated checks

The complete serial workspace check passed:
**418 passed, zero failed, three ignored**. The ignored cases are two deliberate
live-store collector checks and the manual work-panel pilot exporter. This run
includes the native age-column and search freshness corrections. The subsequent
copy-only correction from “1 projects” to “1 project” is covered by focused
interaction checks (**13 passed**), formatting and refreshed compositor captures.
The final implementation is commit `5ea7f77`, following the workstation-art
commit `aecd41e`.

```sh
sh docs/design-audit/native-cargo.sh test --workspace -- --test-threads=1
sh docs/design-audit/native-cargo.sh clippy --workspace --all-targets -- -D warnings
sh docs/design-audit/native-cargo.sh fmt --all -- --check
```

[Workspace log](evidence/workspace-tests-final.log), [strict Clippy](evidence/clippy.log),
[format check](evidence/format.log) and [release build](evidence/release-build.log)
retain the actual output. Golden snapshots were intentionally refreshed for the
new navigation, work brief and compatibility layouts, then compared normally.
The later copy correction has its own [focused checks](evidence/final-copy-tests.log),
[format check](evidence/final-copy-format.log) and [release build](evidence/final-copy-build.log).
An [earlier run](evidence/workspace-tests-before-fixture-update.log) exposed two
fixtures that assumed image output while requesting native fallback. They now
explicitly request the image geometry whose packing and navigation they test;
their original behavioral assertions remain in place.
The [art tests](art/checks/focused-tests.log) additionally inspect all twelve
silhouettes, monitor supports, keyboard-hand contact and clipped screen motifs.

Behavioral coverage includes forward and reverse semantic Tab cycles with redraw,
mouse/keyboard identity, stale hit regions after resizing, appearance Apply versus
Cancel, retained records while new activity arrives, expired requests, duplicate
events, missing family members, source recovery, monochrome frames and reduced
motion. Assertions exercise actual actions and state, not only static labels.

## Visual inspection

The [375-entry inventory](evidence/inventory/index.html) schedules every declared
surface at 32×14, 80×24, 120×36, 192×58, 110×80 and 240×70, plus directed states
and native/light/monochrome variants. It contains 366 reconstructed screenshots;
the nine Sources entries are executable-owned placeholders and remain
**not tested in the renderer**. Their independent PTY evidence is below.

The final matrix records **360 visual passes, zero unresolved visual defects,
and 15 not-tested entries**: nine Sources placeholders and six CJK font-fallback
checks. These counts describe the scheduled local review, not publication gates.

The additional [10×20 geometry gallery](evidence/geometry-10x20/index.html) has
**nine visually reviewed passes**: Tower, Office and Now at 80×24, 120×36 and
192×58. Exact source pixels are retained; the exporter honors the cell metadata
and uses the corresponding native font size. Reproduce with
`PYTHONPATH=docs/design-audit/tmp/python python3 docs/design-audit/iteration-6/capture_geometry.py`.
This supplements the main 8×16 matrix rather than claiming a second complete
375-case matrix.

Each visual verdict records the SHA-256 of the inspected PNG. Exact-byte duplicate
images may reuse an identified image review; automatic route/bounds checks remain
independent per case. A changed image cannot inherit a stale review. Full frames
and readable panel crops were inspected. Earlier defects remain in the
[before evidence](evidence/before/before.json) and the intermediate pilot.

At 32×14, work tabs collapse to an emergency brief with source coverage and the
appropriate review action. Requests retain explicit decisions and a page-reading
hint. This does not claim four complete tabs fit at that size. CJK text is retained
and width-bounded, but Menlo-only replay cannot validate its font fallback; those
long-name glyph cases remain **not tested** for visual appearance.

The authored-art [full outbound/action/return sequence](art/gallery/outbound-action-return.gif)
contains 129 frames. It records a complete decorative route, not a sprite-sheet
substitute for motion evidence. The main screen matrix uses reduced motion for
repeatable comparison. Motion/source-priority regressions run separately.

## Executable PTYs

- **Sources: nine passed cases**, six sizes plus light, monochrome and 256-color
  at 80×24. Each covers eight forward and eight reverse focus steps with redraw,
  path edits/cancel, disabled sources, preserved preferences and exit restoration.
  [Report](SOURCES_PTY.md), [machine results](evidence/sources-pty/results.json).
- **Control workflows: both 80×24 and 120×36 passed.** Literal task creation,
  exact approval sent once, known-project selection, recipient-bound inline
  instructions, F7 expansion/collapse, a single steer without a duplicate task,
  console/mouse return, source cancel and mouse-off reopening without replay.
  [80-column results](evidence/control-pty-80/results.json),
  [120-column results](evidence/control-pty/results.json), [scope](WORK-PANEL.md).

These are actual executable runs on macOS PTYs with simulated providers. Their
PNGs reconstruct ANSI cells; they are not terminal-window screenshots.

| Evidence | Executable SHA-256 |
|---|---|
| Nine Sources cases | `44100edbd7e6b4fa34d2c796d42e6281cd463a1d5c0ae72491b466d2f773c8dd` |
| Complete workspace checks and supplementary 10×20 geometry, before singular-copy correction | `56b8d1f93738f6a5cca1cc9097026874d46ff4e800f54ed90baabfca72fd9f01` |
| Final local build, 375-entry compositor matrix and executable demo smoke | `b2c529653f5b9d6a2ee93cd84cb87772ee8e1dee96b431ac9b645f3bfd33de84` |
| Both control workflows | `fbef32cc8ed5bf78aef76949d7ec6061569e6442df711f809c91d67a87d73178` |

Later display-only corrections affect token availability wording, small reading
hints, list observation columns and a legacy sign. The recorded PTY hashes remain
the versions actually exercised; final-source tests and compositor captures cover
those later corrections.

The [final executable demo smoke](evidence/demo-once-final.txt) exited successfully
with `--demo --once --no-save`; its [metadata](evidence/demo-once-generation.json)
records the actual exit code and binary. It is a non-interactive check with
synthetic conversations and temporary preferences.

## Composition performance

The [27-case measurement](COMPOSITION.md) covers 1, 6 and 20 projects,
50 tasks, and Tower, Office and Inspector at 80×24, 120×36 and 192×58. Each case
records 60 samples after ten warmup frames, for 1,620 measured frames. The largest
case p95 is **8.899 ms** and the largest individual sample is **10.361 ms**.
The [generation record](evidence/composition-generation.json) identifies the
measured executable and confirms an otherwise quiet measurement window.

This measures key dispatch through local composition and native masking. It
excludes terminal encoding, IPC, image transport and OS presentation. A compact
Inspector case correctly produces no image canvas. The prior iteration's
largest case p95 was 9.180 ms; the current local measurement remains within that
range. It does **not** demonstrate the end-to-end 150 ms input-response target.

## Remaining publication gates

| Check | Status and reason |
|---|---|
| Authenticated Codex and Claude sessions, including official attach/login | **Not tested:** fixtures replace provider processes; this run did not exercise authenticated sessions. |
| Kitty, iTerm2 and Sixel in real macOS/Linux/Windows/WSL terminal windows | **Not tested:** no cross-platform terminal capture session was used. Encoder/compositor tests do not certify transport or font fallback. |
| End-to-end p95 input response below 150 ms | **Not tested:** local composition measurements exclude terminal encoding, transport and OS presentation. |
| Five new users identify a request in ten seconds and explain task relationships | **Not tested:** no participant sessions were conducted. |
| Terminal accessibility certification, assistive technology and arbitrary user palettes | **Not tested:** built-in contrast targets and monochrome cues are local design checks. |

The implementation can be tried from this branch. These remaining gates prevent
claiming a tested public release on every target platform.
