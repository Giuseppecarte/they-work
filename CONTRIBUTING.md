# Contributing to they-work

they-work is intentionally small. The binary polls local agent data, folds
observations into a shared world model, and renders that model as a terminal
office. Keep those responsibilities separate so the read-only promise remains
easy to inspect.

## Crate layout

| Crate | Responsibility |
| --- | --- |
| theywork-core | Domain model for offices, workers, activities, events, and the deterministic demo world |
| theywork-collect | Read-only Claude Code and Codex sources that turn local data into core events |
| theywork-render | In-memory canvas, sprites, animations, overlays, and views; it performs no I/O |
| theywork-tui | The binary: argument parsing, polling, terminal setup, and wiring |

<code>theywork-core</code> is the contract. Both <code>collect</code> and
<code>render</code> depend on it, and neither depends on the other. A collector
should emit a core event when the model needs new information; a renderer
should consume the existing world model rather than reaching into a source.

## Project-selection contract

The user-facing startup, project-switching, persistence, and setup-check
behavior is specified in [`docs/project-selection.md`](docs/project-selection.md).
The CLI owner wires `--project`, `--config-dir`, and `--check`; the collector
owner supplies normalized project identities and the first-scan counts. Keep the
default path read-only and make every additional write require the explicit
config-directory opt-in described there.

## Build and test

Rust is not required on the host. <code>./scripts/cargo</code> builds
<code>docker/Dockerfile.dev</code> on first use, mounts the repository at
<code>/src</code>, and runs Cargo as the invoking user:

~~~bash
./scripts/cargo fmt --all -- --check
./scripts/cargo clippy --workspace --all-targets -- -D warnings
./scripts/cargo test --workspace
make build
make demo
~~~

<code>make fmt</code> formats files in place. <code>make fmt-check</code> is
the non-mutating version for checking a clean tree, and <code>make check</code>
runs formatting, strict Clippy, and the workspace tests. The release image is
built from <code>docker/Dockerfile</code>; its dependency layer copies the
manifests and stub sources before the real sources, so ordinary source edits
can reuse the registry and dependency layers.

## Collector safety rule

Collectors may only open their configured transcript or database paths for
reading. Do not add write APIs, file creation, deletion, renaming, network
access, process spawning, or code that follows a source symlink out of its
configured tree. If a new source cannot satisfy that rule, it does not belong
in <code>theywork-collect</code>.

Keep tests focused on this boundary. The container supplies an additional
runtime boundary with no network, a read-only root filesystem, dropped
capabilities, no-new-privileges, non-root execution, and read-only agent
mounts.

## Adding a sprite

1. Add the sprite data and its dimensions in
   <code>crates/theywork-render/src/sprite.rs</code>.
2. Add it to <code>SpriteSet</code> and give it a descriptive name.
3. Reuse the existing transparent-pixel and nearest-neighbor scaling helpers
   instead of drawing directly into the terminal buffer.
4. Put the sprite in a view or animation where it communicates state, then
   cover clipping and small-terminal behavior with a renderer test.

The deterministic demo world in <code>theywork-core/src/demo.rs</code> is a
useful place to preview a sprite without reading anyone's files.

## Adding a view

1. Create a focused module under
   <code>crates/theywork-render/src/views/</code> and keep it pure: accept a
   <code>World</code>, draw into the supplied frame/canvas, and perform no I/O.
2. Reuse the shared header, footer, panel, status, and color helpers so small
   terminals degrade consistently.
3. Add the view to the presentation state and navigation in
   <code>crates/theywork-render/src/lib.rs</code>. Keep the data source
   unaware of the view.
4. Document its keys in the help overlay and test normal, tiny, and empty
   worlds with ratatui's test backend.

If a view needs new information, extend the core event/model contract first.
Do not make <code>render</code> depend on <code>collect</code>.

## Pull requests

Keep changes narrow, explain user-visible behavior, and run the commands above.
The GitHub Actions workflow repeats formatting, strict Clippy, the full test
suite, and the release image build on pushes and pull requests.

## Releasing the image

Create and push a semantic version tag such as `v1.2.3`. The tag-only
[`release workflow`](.github/workflows/release.yml) builds `docker/Dockerfile`
and publishes both `ghcr.io/giuseppecarte/they-work:v1.2.3` and `latest`. It
has `packages:write` only in that tag-triggered workflow; CI for branches and
pull requests remains read-only. The no-checkout install command and image
pinning rules are in [`docs/release.md`](docs/release.md).

## Reviewing art

Generate the deterministic review bundle with:

~~~bash
make shot
make shot VIEW=top LIGHT=1
~~~

This writes one SVG per surface (`floor.svg`, `guard-office.svg`, `desk.svg`,
and `phone.svg`), a selected `shot.svg`, and an `index.html` under
`docs/shots/`. The exporter consumes the renderer's fixed-time golden-frame
serialization, so the cells shown here are the same cells that the golden test
compares. CI uploads this directory as the `they-work-shots` build artifact.

Goldens answer “did any cell change?” and should be regenerated only when an art
change is intentional. Regenerate the checked-in dark/light, normal/small set
with:

~~~bash
THEYWORK_UPDATE_GOLDEN=1 ./scripts/cargo test -p theywork-render --lib golden::tests::snapshots_match_checked_in_goldens
~~~

Shots answer “does the art look good?” and are the files to open during review.
Keep both in the review loop.

## CI boundary

CI runs on a Linux GitHub-hosted runner with no configured Claude or Codex home.
The collector acceptance suite therefore exercises its fixtures and skips its
live-machine smoke check when those homes are absent; it does not prove a
particular user's transcript or database layout. CI also does not cover
Windows/WSL bind-mount behavior, terminal-specific key handling and dimensions,
or pulling the public release image from its registry. The checked-in
goldens cover deterministic rendering; the interactive demo remains a manual
terminal check.
