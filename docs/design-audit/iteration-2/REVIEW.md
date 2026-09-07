# Second visual audit

The original completion claim did not survive the user's real resize screenshots.
This iteration changed the pixel encoder, native-size animation frames and room
composition, then rejected two intermediate versions after inspecting their
output. Source connections and installation remain as implemented in the first
audit; this revision runs from the same `audit/design-and-usability` branch.

## What the review caught

| Visible defect | Cause and correction | Evidence |
| --- | --- | --- |
| Teeth and gaps in straight edges | Two sextant masks were omitted; transparent samples could inherit the text foreground; Menlo's block outlines do not fill its line box. All representable masks now round-trip, transparency reveals the background, and Apple Terminal defaults to lower half blocks. | [Font and Unicode audit](raster.md) |
| Pink skin, washed-out materials | The 256-color conversion assumed an evenly spaced cube. It now uses the actual six levels and preserves every fixed palette entry. | [22 encoder tests](evidence/canvas-color-tests.log) |
| Giant title and mostly empty office | Layout followed the viewport and a fixed grid rather than occupied desks. Rooms now size occupied rows, bound their height, and keep the sign within a small title band. | [Room decisions](OFFICES.md) |
| Clouds, rug and floor outside their boundaries | Clouds lacked clipping; separate floor polygons left gaps; duplicate scanline intersections left a horizontal crack. Each boundary was repaired and tested. | [Intermediate iso](evidence/review2/apple-terminal-1-iso-192x58.png), [repaired iso with Menlo](evidence/review3/apple-terminal-1-iso-192x58-menlo.png) |
| Motion disappeared | Compact and mini idle frames had identical body pixels. Eight poses are authored on each native grid, with independent phases; motion-off freezes ambient art too. | [Motion audit](MOTION.md) |
| Crowded small isometric rooms | Small windows still attempted ten people. They now page at three, five or ten; PageUp/PageDown and Home/End work inside a floor. | [Final native navigation](evidence/final-navigation/results.json) |
| Small, squeezed inspector portrait | Full quadrant portraits did not obey the horizontal aspect rule used by miniature workers. Portraits now share that rule and get a larger, centered inspection area. Idle details are labelled as the last update. | [Inspector with Menlo](evidence/review3/surface-desk-192x58-menlo.png) |

## Visual acceptance

The native PTY exercise resizes the same running process through 80×24, 120×32,
192×58, 240×70 and 110×80, with one, three and twenty conversations in each of
the three office views. The Apple Terminal environment selects half blocks and
256 colors automatically. Tower, desk, phone, settings, help and sources are
also exercised at three sizes. All four sessions exit successfully and restore
terminal settings: [63 captured states](evidence/review3/results.json).

A further 54 states exercise explicit quadrants and sextants at small, wide and
tall sizes: [results](evidence/other-encodings/results.json). These images
reconstruct ANSI cell geometry. They establish what the running executable
emitted, not whether every font can render the chosen glyphs.

Four full native cell buffers were separately replayed through CoreText and the
installed Menlo font. The replay uses actual glyph outlines, not rectangular
substitutes. Inspection found continuous floors and windows, readable signs,
contained furnishings, distinct workers and a usable inspector. The paired
`.font.json` files record zero LastResort glyphs. These are labelled font replays,
not Terminal.app screenshots.

![Three-worker side office, actual Menlo font replay](evidence/review3/apple-terminal-3-side-192x58-menlo.png)

The final navigation executable adds page-key hints after those scene captures;
it has the same room and sprite rendering. The final PTY check reaches the last
of twenty workers, changes window size three times, opens that same worker's
inspector and returns using Home/PageDown/PageUp. Its [recording script](check_navigation.py)
also asserts clean exit and terminal restoration.

The Linux check exposed two portrait tests that still inherited their canvas
encoding from the host. Their comparisons now select explicit encoding and
color depth; head-crop fidelity is checked at all three encodings. This was a
test-fixture correction, with no change to the runtime renderer.

Final macOS ARM64 and Linux ARM64 workspace runs each pass **221 tests**, with
two live-store tests explicitly ignored. Strict workspace Clippy and formatting
pass on both environments. The revised portrait fixtures also pass their native
focused check. Logs: [macOS tests](evidence/native-tests-final.log),
[macOS Clippy](evidence/native-clippy-final.log),
[Linux tests](evidence/linux-tests-final.log),
[Linux Clippy](evidence/linux-clippy-final.log),
[portrait fixtures](evidence/portrait-fixtures.log).

The native motion check passes **116 scenarios and 1,392 sampled frames**:
all 58 motion-off cases produce one stationary room frame, while every motion-on
case changes the clothing mask between three and twelve times. All reported
worker states remain correct. The 62 recorded sessions exit cleanly and restore
terminal settings. This includes small windows, 256-color conversion and top/side
views: [motion results](evidence/motion-summary.json).

The combined test, artifact hash and terminal results are recorded in
[verification.json](evidence/verification.json).

## Reproduce

```sh
sh docs/design-audit/native-cargo.sh build --release --bin they-work
python3 docs/design-audit/iteration-2/review_pty.py --stage review3 --encodings apple-terminal --surfaces
python3 docs/design-audit/iteration-2/review_pty.py --stage other-encodings --encodings quadrants sextants --sizes 80x24 192x58 110x80
python3 docs/design-audit/iteration-2/check_navigation.py
python3 docs/design-audit/iteration-2/check_motion.py
./scripts/cargo test --workspace
./scripts/cargo clippy --workspace --all-targets -- -D warnings
./scripts/cargo fmt --all -- --check
```

The macOS harness uses `pyte` and Pillow from the ignored audit runtime and
synthetic conversation databases confined to `docs/design-audit/tmp`. It never
needs private thread contents. Follow [raster.md](raster.md) to reproduce the
CoreText font replays. Intermediate scene versions are retained to make the
rejected defects inspectable rather than replacing every image with the final
result.

## Limits

The supplied user screenshots are the native Terminal.app evidence. Computer
Use did not permit access to that app, so no alternative capture method was
used to bypass that restriction. The installed Menlo replay is stronger font
evidence than the original ideal reconstruction, but it does not establish the
user's exact font, line spacing or Terminal.app's internal glyph handling.

Windows, WSL, Kitty, Sixel and iTerm2 graphics sessions were not visually
inspected in this iteration. Their existing transport tests and CI definitions
are not substitutes for native visual runs. A small character terminal has
less facial detail than a wide window; selecting a worker opens the larger
portrait and complete task title. No release was published, and no claim of
universal visual perfection follows from the passing checks.
