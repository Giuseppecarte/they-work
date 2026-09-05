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
The CLI exposes `--project`, `--config-dir`, and the non-rendering `--doctor`
diagnostic; the collector owner supplies normalized project identities and
first-scan counts. Keep the
default path read-only and make every additional write require the explicit
config-directory opt-in described there.

## Build and test

Rust is not required on the host. <code>./scripts/cargo</code> builds
<code>docker/Dockerfile.dev</code> on first use, mounts the repository at
<code>/src</code>, and runs Cargo as the invoking user:

~~~bash
make fetch
./scripts/cargo fmt --all -- --check
./scripts/cargo clippy --workspace --all-targets -- -D warnings
./scripts/cargo test --workspace
make build
make demo
python3 scripts/test-install.py
~~~

<code>make fmt</code> formats files in place. <code>make fmt-check</code> is
the non-mutating version. <code>make check</code> fetches the locked
dependencies with explicit network access, then runs the formatting check,
strict Clippy, and the workspace tests offline. The release image is
built from <code>docker/Dockerfile</code>; its dependency layer copies the
manifests and stub sources before the real sources, so ordinary source edits
can reuse the registry and dependency layers.

The Cargo container runs with Docker's <code>--network none</code> by default.
<code>make check</code> handles the fresh-checkout bootstrap. Before running
individual Cargo commands in a fresh checkout, populate the ignored
<code>.cargo-home</code> cache once with an explicit networked fetch:

~~~bash
THEYWORK_CARGO_NETWORK=bridge ./scripts/cargo fetch --locked
~~~

CI caches that directory by lockfile and toolchain pin and performs that
networked bootstrap only on a cache miss. Set <code>THEYWORK_CARGO_NETWORK</code>
only when intentionally refreshing the dependency cache.

<code>cargo fmt --all</code> crosses crate boundaries. For a focused change,
use <code>./scripts/cargo fmt -p &lt;crate&gt;</code> (and add
<code>-- --check</code> when checking) so an unrelated crate is not
reformatted by accident.

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
then pulls the published digest through a 160×48 Kitty-capable PTY, checking
the baked UTF-8 locale, a graphics transmission, and quadrant-specific output
when the terminal does not answer the graphics probe. It has `packages:write`
only in that tag-triggered workflow; CI for branches and pull requests remains
read-only. Re-shoot the README stills from that published digest after the
check passes. The no-checkout install command and image pinning rules are in
[`docs/release.md`](docs/release.md).

## Reviewing art

The renderer is a pixel canvas whose resolution is the terminal size. A design
drawn at desktop resolution does not survive scaling down to 80 columns. The
primary review target is the common size recorded in the normal goldens; the
degraded golden remains beside it so the smaller view is still reviewable. A
screenshot without its terminal size attached cannot be judged.

When a board under `docs/design` changes, run `scripts/render-design.sh` first.
CI runs that same source-to-reference step before `make shot`, so the sheet
does not quietly review an older generated reference.

Generate the deterministic review bundle with:

~~~bash
make shot
~~~

This writes one SVG per surface (`floor.svg`, `guard-office.svg`, `desk.svg`,
and `phone.svg`), matching dark/light PNG and SVG variants, selected
compatibility files, and an `index.html` contact sheet under `docs/shots/`.
The contact sheet puts the intended-design reference beside the dark and light
render for every surface and shows the fixed demo timestamp. The exporter
consumes the renderer's fixed-time golden-frame serialization, so the cells
shown here are the same cells that the golden test compares. Its PNG path uses
Google Chrome or Chromium to rasterize the exact SVG that it just wrote; set
`THEYWORK_SVG_RASTERIZER` to an executable path when it is not on `PATH`. The
PNG path does not redraw cells: it checks that the SVG text matches the frame,
captures that SVG, and checks the resulting PNG dimensions before writing the
output. This is the round-trip guard against the PNG and SVG paths drifting.
CI uploads this directory as the `they-work-shots` build artifact.
The exporter reads the normal golden as the primary frame and the small golden
as the degraded frame. It verifies that each group has one shared terminal
size, then requires any image-frame manifest to use that same primary size.
Encoding-specific files use names such as
`office.dark.normal.sextants.golden`; their metadata carries the same encoding
in the depth field. Legacy unqualified goldens are treated as `half-blocks`.
The contact sheet labels every dark and light panel with its terminal size and
encoding, and each SVG title carries the same metadata for review outside the
sheet. It discovers every encoding with a complete dark/light primary/degraded
set, so the current complete ladder is `sextants`, `quadrants`, and
`half-blocks`; partial sets are reported and left out until the missing frames
exist.
`docs/shots/` is gitignored, so a fresh clone has no contact sheet or rendered
frames until `make shot` runs. Generate the bundle before opening the sheet;
the CI artifact is the copy retained outside the working tree.
Only surfaces with rendered output appear in the contact sheet. The six
additional design-only boards stay in `docs/references` until a matching
renderer surface exists, because there is nothing to compare against yet.
The exporter searches `THEYWORK_SVG_RASTERIZER`, `google-chrome`, `chromium`,
and `chromium-browser` in that order. If none is available, it fails with
`cannot rasterize SVG: install Google Chrome/Chromium or set
THEYWORK_SVG_RASTERIZER to its executable`.

The optional graphics-protocol panels are separate from the cell exporter.
They appear only when `make shot IMAGE_FRAME_DIR=/path/to/dump` receives a
complete renderer-backed image-frame dump. The dump root contains
`manifest.json`, whose required shape is:

~~~json
{
  "version": 1,
  "source": "renderer-pixel-frame",
  "timestamp": 192000,
  "viewport": {"columns": 160, "rows": 86, "cell_width": 10, "cell_height": 10},
  "frames": [{
    "surface": "floor",
    "theme": "dark",
    "png": "floor-dark.png",
    "width": 1600,
    "height": 860,
    "packets": {
      "kitty-direct": "floor-dark.kitty-direct.bin",
      "sixel": "floor-dark.sixel.bin",
      "iterm2": "floor-dark.iterm2.bin"
    }
  }]
}
~~~

It must include every rendered surface (`floor`, `guard-office`, `desk`, and
`phone`) in both themes. The timestamp and cell viewport must match the primary
character goldens. The 160×86 / 10×10 example therefore labels its PNG
`1600×860`. The exporter verifies every PNG header against the declared
physical dimensions and terminal cell geometry, checks that each protocol
packet is present and non-empty, copies only the verified PNGs into
`docs/shots/`, and places them beside the primary character-cell panels. It
rejects a partial dump, a viewport mismatch, an external path, or a synthetic
encoder sample. The protocol packets remain binary evidence in the supplied
dump; the contact sheet does not render terminal control bytes as text.

For a single renderer-owned BMP or PNG capture, use:

~~~bash
make shot IMAGE_FRAME=/path/to/renderer-frame.bmp
~~~

The exporter converts the captured 32-bit BMP to a sheet PNG and adds it only
to the matching dark floor comparison. A 1600×860 renderer source frame is
paired with a 160×86 character canvas; it is visual evidence of the source
image, not a claim that a particular terminal played the packet successfully.

Reference images belong in [`docs/references`](docs/references/README.md) as
`floor.png`, `guard-office.png`, `desk.png`, or `phone.png` (JPEG, WebP, and
SVG are also accepted). Replace a supplied reference there and rerun `make
shot`; the exporter never overwrites that directory.
The extra supplied boards are documented as design-only references and are
not added as contact-sheet rows without corresponding renderer output.

Goldens prove nothing changed. Shots let a human judge whether it is any good.
Neither substitutes for the other, and a golden regenerated against broken
output makes the breakage permanent. Regenerate the checked-in dark/light,
normal/small set only when an art change is intentional, with:

~~~bash
THEYWORK_UPDATE_GOLDEN=1 ./scripts/cargo test -p theywork-render --lib golden::tests::snapshots_match_checked_in_goldens
~~~

Shots answer “does the art look good?” and are the files to open during review.
Keep both in the review loop.

Visual review means running `make shot`, opening `docs/shots/index.html`, and
comparing every rendered output with its intended-design reference. Reading a
diff is not a substitute for that comparison.

## CI boundary

CI runs on a Linux GitHub-hosted runner with no configured Claude or Codex home.
The collector acceptance suite therefore exercises its fixtures and skips its
live-machine smoke check when those homes are absent; it does not prove a
particular user's transcript or database layout. CI also does not cover
Windows/WSL bind-mount behavior, terminal-specific key handling and dimensions,
or pulling the public release image from its registry. The checked-in
goldens cover deterministic rendering; the interactive demo remains a manual
terminal check.
