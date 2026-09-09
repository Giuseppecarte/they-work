# Contributing to they-work

they-work is intentionally small. The binary polls local agent data, folds
observations into a shared world model, and renders that model as a terminal
office. Keep observation separate from explicitly authorized provider controls.
The collectors remain read-only, and the renderer performs no I/O.

## Crate layout

| Crate | Responsibility |
| --- | --- |
| theywork-core | Domain model for offices, workers, activities, events, and the deterministic demo world |
| theywork-collect | Read-only Claude Code and Codex sources that turn local data into core events |
| theywork-control | Private local supervisor, provider RPC authority and native-console handoff |
| theywork-terminal-image | Negotiated graphics transport and bounded image encoding |
| theywork-render | In-memory canvas, sprites, animations, overlays, and views; it performs no I/O |
| theywork-tui | The binary: argument parsing, polling, terminal setup, and wiring |

<code>theywork-core</code> is the contract. Both <code>collect</code> and
<code>render</code> depend on it, and neither depends on the other. A collector
should emit a core event when the model needs new information; a renderer
should consume the existing world model rather than reaching into a source.

## Observation and control contracts

[INSTALL.md](INSTALL.md#choose-your-conversation-sources) is the current
user-facing contract for source consent, navigation and persistence.
Native interactive sessions offer **Remember on this computer** in the normal
settings directory; `--config-dir` overrides that location, rather than granting
permission by itself. Remember disabled or `--no-save` leaves the session
temporary. The standard Docker runtime still needs an explicit writable settings
mount to save. Keep source records read-only in every mode.

`--project` restricts the input to one normalized repository; choosing a floor
only changes the view. The collectors supply normalized project identities and
first-scan counts. The [Source trait](crates/theywork-core/src/source.rs) documents
the polling contract: the host traverses sources sequentially in a background
thread, publishes a batch, then waits one second. This is independent of frame
rendering; a slow read extends the traversal and delays the other sources.

[The control contract](docs/CONTROLS.md) defines provider ownership, requests,
receipts and private supervisor state. Reading a transcript never grants
authority over an external execution. The earlier
[project-selection proposal](docs/project-selection.md) is historical, not an
alternative current contract.

## Build and test

Use native Rust 1.90 with a C compiler and Python 3 for the offline process
fixtures, or Docker. Python is a development dependency, not an end-user runtime
requirement. `./scripts/cargo` prefers
local Cargo and falls back to `docker/Dockerfile.dev`. Set
`THEYWORK_TOOLCHAIN=native` or `THEYWORK_TOOLCHAIN=docker` to choose explicitly.
The Docker image runs as the invoking user, mounts the checkout at `/src`, and
keeps Cargo's cache in `.cargo-home`. Temporary build/test files stay in `target/tmp`.

~~~sh
make check
make native  # local Rust: build target/release/they-work
make install # local Rust: install into Cargo's user bin
make demo    # optional Docker runtime; no source mounts
python3 scripts/test-install.py
python3 scripts/test-native-install.py
~~~

`make check` fetches locked dependencies, then checks formatting, strict Clippy,
and workspace tests. The canonical suite uses `--test-threads=1` because its
wall-clock frame-budget checks must not compete with other expensive render
scenarios. No timing thresholds are relaxed. Provider tests use offline fakes
and need local loopback sockets and PTYs; they never start a real model turn.
Docker commands run without network access by default;
`make fetch` explicitly enables network access to populate the dependency cache.
Native Cargo follows its normal network policy. To run individual Docker tests
on a fresh checkout, first run `THEYWORK_CARGO_NETWORK=bridge ./scripts/cargo fetch --locked`.
CI forces Docker for that verification lane and separately builds, runs tests,
and smoke-tests release binaries natively on Linux, macOS, and Windows (x64/ARM64).

The release workflow packages each tested native binary with `LICENSE` and
SHA256 checksums, builds a Linux/amd64 and Linux/arm64 candidate index, verifies
that immutable digest on both platforms and checks native archive hashes before
promoting version/latest. It then attaches native installers and archives to the
tagged GitHub Release. This workflow is a release gate, not evidence that an unrun
platform has passed. No releases were published as part of the design audit.

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

1. Add high-resolution character art in
   <code>crates/theywork-render/src/living_office/art.rs</code>; keep compatible
   compact sprites in <code>crates/theywork-render/src/sprite.rs</code>.
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

Create and push an authorized version tag such as `v1.2.3`. The tag-only
[`release workflow`](.github/workflows/release.yml) builds once to a unique
candidate reference. It verifies that immutable multi-architecture digest with
explicit platform runs and normal PTY exits, checks all six native archives and
retains a publication intent before advancing version and `latest`. Candidate
tags may be visible in the public package; they are unpromoted artifacts.

Promotion reuses the verified index without rebuilding it. It is not atomic
across two image tags and the GitHub Release. A failure can leave a verified
version published while `latest` or native release publication is incomplete;
follow the [recovery runbook](docs/release.md), not an automatic rollback. The
workflow serializes promotion but does not prevent external publishers or impose
semantic version order. Branch and pull-request CI does not publish packages.

Run the deterministic release checks without registry writes:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-release-image.py
```

For the explicit-platform verifier, disposable-registry rehearsal, native
checksum contract and authorized publication recovery commands, use
[`docs/release.md`](docs/release.md). Simulated capability replies and emulation
are recorded as such; neither is physical-terminal or native ARM certification.

## Reviewing art

For a small reproducibility check from a clean checkout, follow
[the audit guide](docs/AUDIT.md):

~~~sh
python3 scripts/audit.py bootstrap --python /path/to/python3.12
python3 scripts/audit.py smoke
~~~

Omit `--python` when `python3` already uses Python 3.12. Bootstrap prepares the
declared dependencies; smoke runs offline afterward and produces one complete
screen plus one bounded collector-fixture result under ignored `target/audit/`.
The manifest identifies the actual binary, dependencies, font and capture
method. This is a compositor replay, not physical-terminal or user validation.
The larger legacy review bundle below remains available for its existing
compatibility targets; it is not a prerequisite for the small audit smoke.

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

The Docker verification lane runs on a Linux GitHub-hosted runner with no configured Claude or Codex home.
The collector acceptance suite therefore exercises its fixtures and skips its
live-machine smoke check when those homes are absent; it does not prove a
particular user's transcript or database layout. The native matrix checks Windows builds and fixture behavior; CI does not cover
WSL bind-mount behavior, terminal-specific key handling and dimensions,
or pulling the public release image from its registry. The checked-in
goldens cover deterministic rendering; the interactive demo remains a manual
terminal check.
