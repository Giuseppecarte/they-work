# Validation of the colorful local preview

Candidate: `17ae09f85982b2e29c701fe47b7ce03fa34c488e`.
Baseline: `282722294b8b372d7e20bda4a3705f3e19b62b1b`.
Host: macOS ARM64, pinned Rust 1.90.0 and the repository lockfile.
Compositor replay: Python 3.12, pinned Pillow and bundled licensed DejaVu fonts;
exact dependency/font hashes are in `visual/comparison.json`.

## Compositor and interaction checks

**128 complete before/after cases passed** their routes, native-label equality,
hit-target equality and physical image geometry comparison. The inventory
contains:

- 48 three-worker scenes: three presets × four palettes × independent/team
  seating × Tower/Office at 80×24, 8×16 cell geometry.
- 80 workflow specimens: Tower, Office, Now, Attention and Connections at
  32×14, 80×24, 120×36 and 192×58, with both 8×16 and 10×20 cells. Extra light,
  256-color, monochrome and image-free specimens run at 80×24. These use 20
  projects and 50 tasks, multiple teams, current synthetic requests and visible
  coverage limitations.

All captures use fixed time 1000 and reduced motion. Of these, **62 cases were
visually reviewed**, using 48 complete palette specimens and the retained
individual full-screen comparisons (two overlap). Their verdicts and evidence
are explicit in `visual/comparison.json`; the other visual verdicts remain
**not tested**. No terminal paint or live provider is involved.

At 80×24 the Now panel exposes task identity, current observation, request and
latest recorded update. At 120×36 it stays beside the scene. The reviewed compact
32×14 specimen uses the native roster; it does not try to shrink character art.
Both cell geometries retain integer-sized characters. Five floors remain
readable in the crowded 192×58 tower, with request markers and team selection.

Material tests cover 288 workstation compositions and 96 whole-scene
combinations. They check changed chair/computer/desk-edge crops with unchanged
occupied masks, anchors, worker IDs and costumes. Existing request interruption,
two-decoration limit, clipping, faces/hands, screen motifs and motion geometry
checks remain passing. See [materials/REVIEW.md](materials/REVIEW.md).

The shared native palette tests require text contrast ≥4.5:1 in dark/light and
truecolor/xterm-256, including attention backgrounds; filled action text also
requires ≥4.5:1 and its essential selection boundary ≥3:1. Monochrome tests keep
labels, brackets, bold emphasis and target geometry. These are palette targets,
not terminal accessibility certification.

## Integrated checks

Run from the repository root with the pinned local wrapper:

```sh
sh docs/design-audit/native-cargo.sh fmt --all -- --check
sh docs/design-audit/native-cargo.sh test --locked --workspace -- --test-threads=1
sh docs/design-audit/native-cargo.sh clippy --locked --workspace --all-targets -- -D warnings
sh docs/design-audit/native-cargo.sh build --locked --release --bin they-work --example measure_tower
```

Results: formatting **pass**, workspace **470 ordinary tests passed, 4 ignored**,
strict all-target Clippy **pass**, release build **pass**. Separately, the
harness-free storage executable reports **three process cases passed** on macOS
ARM64. Empty filtered binaries and doc-test sections are not counted as tests.
The isolated fake-provider process checks use loopback sockets; no real account
or provider invocation is used.

Passing workspace cases include redraw after forward/reverse Tab, keyboard/click
equivalence, stale-pointer invalidation on resize, opaque-panel hit protection,
exact request identity and expiry, inspection without approval, preserved
reading/draft state and stable appearance across roster changes/restart.

All **115 updated goldens are color-only**: headers, dimensions and symbols
match their baseline. They include the existing compatibility views and glyph
sheets. The first run caught omitted glyph-sheet recoloring; a later parallel
run exceeded the rendering wall-clock budget under concurrent test load. Both
failed logs are retained. The serial suite passes with the original threshold,
and a separate exact timing check executes one test and passes all three
encodings. No assertion was weakened.

## Timing, measured separately

The unchanged `measure_tower` example ran against optimized baseline and
candidate builds, sequentially, after builds and capture work stopped and while
preview compilation was paused. It exercises 1/6/20 projects with 50 tasks,
80×24/120×36/192×58 and Tower/Office/inspector. Each of 27 cases records 60 inputs
after ten warm-up inputs, with motion enabled.

Largest per-case p95: **15.917 ms baseline, 16.946 ms candidate**. The largest
paired increase is 4.753 ms for the 192×58 office with six projects. Two cases
increase by more than 1 ms. This single sequential comparison on a shared
desktop does not establish a causal speed improvement or a statistically stable
regression. All measured composition p95 values remain below 150 ms.

The separate debug encoding budget reports 100 frames in 2439 ms (sextants),
1772 ms (quadrants) and 807 ms (half blocks), each below the unchanged 5000 ms
bound. Exact commands, CSVs and sequencing timestamps are in `validation/`.

These values exclude image encoding, terminal transport, OS/GPU paint and source
polling. **Input-to-visible latency in VS Code/WSL remains not tested.**

## Executable previews and actual-terminal boundary

The preview report records each Linux executable's architecture, static ELF
linkage, source revision, binary/archive hashes, toolchain, normal process exits,
Kitty/iTerm2/Sixel/no-image PTY behavior and terminal restoration. x86_64 execution
on the ARM64 Docker daemon is labelled emulated; ARM64 is daemon-native. Neither
is a physical Windows/WSL terminal.

Both preview architectures **passed**: each has no ELF interpreter or shared
library dependencies, three successful process checks, four successful protocol
or fallback PTYs, normal `q` exit and restored mouse/alternate-screen state.
Each final archive also passed checksum → extraction → help/demo/headless
execution in an isolated Linux tmpfs. The archive contains the exact previously
executed binary plus its complete local documentation bundle. Six process
smokes and eight PTYs are recorded; the extraction rehearsal repeats three
commands per architecture and is separate evidence, not six new application
behaviors. See `validation/preview-summary.json` and the retained preview report.

Run `target/audit/venv/bin/python docs/design-audit/iteration-11/test-preview.py`
for the seven helper contracts, including malformed/dynamic ELF rejection and
deterministic archive bytes. An initial underscore-only discovery glob matched
zero tests and failed; it is not counted as validation. The direct command
executes all seven successfully, including the added packaged-document check.
The first application build was
withheld because its musl linker injected a dynamic loader; its failure is
retained with the corrected build evidence.

[WSL-PREVIEW.md](WSL-PREVIEW.md) provides extraction, checksum and terminal setup
instructions. [WSL-SESSION.md](WSL-SESSION.md) remains **not tested** until the
user records actual VS Code/Ubuntu versions, doctor output, images, interaction,
resize and clean exit. Demo records cannot validate a live provider approval.
Authenticated providers, participant studies and publication gates remain open.
