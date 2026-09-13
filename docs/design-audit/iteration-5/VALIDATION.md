# Iteration 5 validation

The tested host is macOS 26.6.2 (25G83), arm64, with Rust 1.90.0,
Crossterm 0.28.1 and Ratatui 0.29. The branch is
`audit/design-and-usability`; its starting point for this iteration was
`f7a33d6`. Production changes are recorded in `aa4da4b` and `4d4be18`.
The repository-local wrapper below supplies the cached native Rust
toolchain and directs temporary files into ignored audit scratch.

## Automated checks

The final serial workspace run passed **381 tests**, with **zero failures**.
Strict [workspace Clippy](evidence/clippy.log), formatting and the [release
build](evidence/release-build.log) also passed.
Two existing collector checks were explicitly ignored because they require live
personal Codex/Claude stores. No fixture read those stores, authenticated to a
provider or sent a real model turn.

```sh
sh docs/design-audit/native-cargo.sh test --workspace -- --test-threads=1
sh docs/design-audit/native-cargo.sh clippy --workspace --all-targets -- -D warnings
sh docs/design-audit/native-cargo.sh fmt --all -- --check
sh docs/design-audit/native-cargo.sh build -p theywork-tui --release
```

The [workspace transcript](evidence/workspace-tests.log) includes 220 renderer
unit tests, native/image-layer integration, collector relationship fixtures,
control/console fixtures, 26 executable unit tests and 34 CLI checks. Exact
requests, replacement approvals, pointer geometry after resize, skipped Sixel
frames, independent families, persistent profiles and reduced motion are covered
by the respective behavioural regressions. Goldens were refreshed after visual
inspection of the native toolbar, compact views and inspector; the final suite
compared them normally, without update mode.

## Rendering and visual review

The final matrix covers tower, office and inspector; 1, 6 and 20 projects with
50 tasks; and 80×24, 120×36 and 192×58 terminals at 8×16 physical pixels per cell.
Each case includes ten warm-up frames and sixty measured frames, for 1,620 samples.
The largest p95 per view was 9.180 ms (tower), 8.100 ms (office) and 6.526 ms
(inspector); the largest individual sample was 9.202 ms.

The [raw CSV](input-composition.csv) measures keyboard dispatch, UI composition
and native masking only. It excludes collection, encoding, terminal transport,
image parsing and display refresh. These figures do **not** establish the 150 ms
end-to-end input target. The final gesture-priority correction was made after
this measurement; the measured fixtures contain no human Wait event, so that
branch is not exercised in its scenarios.

```sh
sh docs/design-audit/native-cargo.sh build --release -p theywork-render --example measure_tower
target/native-macos/release/examples/measure_tower > docs/design-audit/iteration-5/input-composition.csv
sh docs/design-audit/native-cargo.sh run -p theywork-render --example interaction_review -- docs/design-audit/tmp/iteration-5-ui
python3 docs/design-audit/iteration-4/export_ui.py docs/design-audit/tmp/iteration-5-ui docs/design-audit/iteration-5/ui
```

The exporter requires Pillow and Menlo. [UI captures](ui/ui-manifest.json) and
[tower captures](evidence/tower-final/ui-manifest.json) combine the actual
`Ui`/`TestBackend` native cells and exact physical RGBA. They are compositor
reconstructions, not terminal-window screenshots. [Artwork evidence](art/ART.md)
adds both authored grids, all twelve silhouettes, the three presets, four zones
and a complete departure/activity/return animation. The [critical review](REVIEW.md)
records defects found in the intermediate images and the resulting corrections.

## Executable and PTY boundary

The final executable SHA-256 is
`5de1411f62fe09a81b78868a4e8193e421fdacb10ab6aba8864ca57392422c8b`.

Both final PTY runs passed (four application sessions in total). The compiled
release is exercised in actual controlling PTYs at 120×36 and
80×24 with simulated provider executables. The [120×36 report](evidence/control-pty/results.json)
and [80×24 report](evidence/control-pty-80/results.json) record the binary SHA-256,
exact scoped reply, literal paths/prompts, known project selection, console
canonical-input/echo checks, mouse release/restoration, source-editor cancellation
and reopening without replay. The second launch uses `--mouse=off` and confirms
that source clicks cannot modify the pending choices.

```sh
PYTHONPATH=docs/design-audit/tmp/python python3 docs/design-audit/iteration-5/review_control_pty.py --binary target/native-macos/release/they-work
PYTHONPATH=docs/design-audit/tmp/python python3 docs/design-audit/iteration-5/review_control_pty.py --binary target/native-macos/release/they-work --stage control-pty-80 --columns 80 --rows 24
```

The local supervisor uses an authenticated loopback socket; the test needs local
socket permission. Native-console assertions prove the handoff to a fixture
running with a real controlling terminal. They do not prove compatibility with
an authenticated Claude account or that a real Codex request was accepted.
Raw ANSI streams preserve mode changes; their PNGs are cell reconstructions.

## Publication requirements still pending

- End-to-end input latency and graphics playback in real Kitty, iTerm2 and Sixel
  terminal windows, including Windows and WSL, with versions recorded.
- Authenticated provider sessions, real console attachment and provider-specific
  error/reconnect behaviour in those environments.
- Five new users completing the ten-second attention and team-comprehension
  tasks without explanation, including no-color and reduced-motion use.

These are the remaining publication gates retained in the approved scope. The
available automated checks, compositor images and fixture PTYs cannot certify
them. Terminal.app was not controlled, and no platform not exercised here is
reported as passing.
