# A warmer office for a local WSL preview

The visual candidate is `17ae09f85982b2e29c701fe47b7ce03fa34c488e`, compared with
`282722294b8b372d7e20bda4a3705f3e19b62b1b`. Start with
[WSL-PREVIEW.md](WSL-PREVIEW.md) to try the Linux executable in VS Code. This is
a local testing build; no image, public tag or release was published.

## Changes

- A shared material recipe separates walls, wood, upholstery, screen bezels,
  metallic stands and props. All four existing palette indices affect both
  standalone computers and team laptops. The three existing presets retain
  their construction and get distinct warm/cool materials.
- Navy and cream establish the native panel hierarchy. Turquoise fills identify
  primary actions and selected tabs; lilac identifies secondary controls.
  Brackets, bold labels, focus markers and explicit status wording remain.
  Light mode uses cream and dark teal. **Office palette** replaces the inaccurate
  **Legacy palette** label in its existing Advanced location.
- Provider behavior, identities, authored character grids, interaction geometry,
  motion rules, saved preferences and public APIs are unchanged.

[Material pilot and critique](materials/REVIEW.md) records the three-person
pilot before the full-cast rollout. [VALIDATION.md](VALIDATION.md) separates
compositor, automated executable PTY and actual terminal evidence.
[CRITIQUE.md](CRITIQUE.md) records remaining limitations.
[PREVIEW-VALIDATION.md](PREVIEW-VALIDATION.md) identifies both delivered Linux
archives, their checksums and the execution environments.

## Reproduce the complete comparisons

Use the pinned Rust toolchain/lockfile and the Python environment prepared by
`python3 scripts/audit.py bootstrap`. The Rust example runs only synthetic facts,
with fixed time, reduced motion and no provider stores. It emits 128 complete
screens: 48 preset/palette/seating/scale specimens and 80 workflow/depth/size
specimens, including truecolor, 256-color, monochrome and image-free rendering.

```sh
env -u NO_COLOR TERM=xterm-256color COLORTERM=truecolor \
  cargo run --locked -p theywork-render --example color_preview -- \
  target/audit/iteration-11/after
```

For the baseline, extract the baseline revision into an isolated ignored
directory and copy only `examples/color_preview.rs` and `examples/ui_inventory.rs`
from the candidate into its render crate. The latter changes only the visibility
of existing fixture helpers. Build there with a separate Cargo target directory,
then run the same example to `target/audit/iteration-11/before` in this checkout.
Never overlay candidate production code onto the baseline.

```sh
target/audit/venv/bin/python docs/design-audit/iteration-11/compare.py \
  target/audit/iteration-11/before target/audit/iteration-11/after \
  target/audit/iteration-11/comparison
```

The comparison checks identical native text, hit targets and physical image
geometry, then replays full screens with licensed bundled fonts. Underlying
color-dependent block encodings are artwork, so they are not mistaken for text
changes. Decoded RGB hashes identify the compared pixels. PNGs are compositor
replays, not screenshots from VS Code, Windows or an authenticated provider.

The committed visual specimens and comparison register are under `visual/`.
Raw regenerated cells, RGBA, PNGs, binaries and archives stay under ignored
`target/audit/iteration-11/`. Historical evidence is preserved.
Captured compiler and terminal logs retain their original whitespace and line
endings through narrowly scoped Git attributes; their hashes cover those bytes.

`python3 docs/design-audit/iteration-11/verify_evidence.py` checks retained files
and source hashes. Add `--require-previews` in the original build checkout to
also require the generated archives and native comparison binaries. Missing
generated files in a clean checkout are reported explicitly.

## Preview builds

`preview.py` freezes committed Cargo/crate sources, builds static musl executables
for both Linux architectures, checks their ELF identity, runs the executable
checks and verifies archive bytes before producing checksums. Its Docker work
is opt-in, uses local images and never pushes. See `preview.py --help` and the
retained preview report for the exact command and architecture evidence.

The WSL user does not need the build tools. [WSL-SESSION.md](WSL-SESSION.md)
starts with every real-terminal observation marked **not tested**. REL-02,
participant studies and existing publication gates are separate from this pass.
