# Visual audit — authored pixels, camera distance and identity

The first changes worth making were preserving faces in character terminals,
making every project reachable, and making approval requests legible without
interpreting an animation. The navigation and source audits cover the latter two;
this document records the visual work and its limits.

The user-supplied visual references are preserved as
[`references/characters.jpeg`](references/characters.jpeg),
[`references/tower.jpeg`](references/tower.jpeg), and
[`references/company-signs.webp`](references/company-signs.webp). The more
specific existing surface boards remain under `docs/references/`.

## Reproduction and evidence

The `before-*-sextants.png` files in `evidence/` were rendered
from the original checked-in golden frames before changing them. They use the
fixture timestamp and 160×48 terminal cells. These are terminal-buffer
reconstructions, **not photographs of a running terminal**. The SVG exporter
draws block patterns as exact subcell rectangles; it therefore cannot reveal missing
glyphs, terminal font metrics, image protocol failures, or input latency.

The final navigation sorts floors by a stable project order, so the default
office fixture is now `beta-platform`, where the baseline selected `they-work`.
These full-screen pairs compare composition and navigation; they are not a
pixel-diff experiment on the same worker. Native-grid regression tests isolate
the character scaling change.

To regenerate the final frames after updating the goldens:

```sh
THEYWORK_UPDATE_GOLDEN=1 ./scripts/cargo test -p theywork-render golden::tests::snapshots_match_checked_in_goldens
python3 docs/design-audit/capture_visual.py
python3 docs/design-audit/capture_visual.py --encoding quadrants --views office desk cameras
python3 docs/design-audit/capture_visual.py --encoding half-blocks --views office desk cameras
python3 docs/design-audit/capture_visual.py --size small --views office cameras desk phone settings help
python3 docs/design-audit/capture_visual.py --stage before --revision 378b4db --views office cameras desk phone
```

The capture helper uses Google Chrome, accepts `--chrome` or
`THEYWORK_SVG_RASTERIZER`, and keeps its temporary profiles inside this audit.
It closes only the browser process it started after the PNG is complete: the
local desktop browser kept running after writing `--screenshot`, which made the
original screenshot script wait indefinitely. The helper also reconstructs
quadrant/half-block patterns as rectangles: unlike the shared exporter's
sextant path, those paths originally relied on browser glyph metrics and
introduced artificial gaps. PNG files are versioned; the large SVG intermediates
are generated locally and ignored. The committed golden frames and capture
script preserve the exact pixels/text needed to reproduce them. `--revision`
reads the original baseline without changing the working tree.

## Findings and changes

### The source art was correct, but its pixels did not survive the floor

`docs/design/v2/gen.py` declares 24×34 figures with integer enlargement.
`cast.py` spends individual pixels on eyes, a nose, mouths and hair. The floor
instead resampled those 816 source pixels into these budgets:

| Rendering mode | Before: source → destination | After: authored source → destination |
| --- | --- | --- |
| Half blocks | 24×34 → 7×10 | 7×10 → 7×10 |
| Quadrants | 24×34 → 7×10 | 7×10 → 14×10, integer column doubling |
| Sextants | 24×34 → 14×20 | 14×20 → 14×20 |
| Image protocol floor | 24×34 → 72×102 | 24×34 → 72×102 |

The old half-block path retained only 70 samples, 8.6% of the authored grid.
The sextant path retained 280 samples, 34.3%. Increasing source detail could
not repair that reduction. The quadrant worker was also half as wide in screen
cells as the half-block worker, although both used the same 7×10 destination.

Characters now have compact 14×20 and miniature 7×10 pixel maps. Both deliberately
place eyes, hands, clothing and feet at their own resolution. All three maps
share the wardrobe palette and six look choices. Quadrants double miniature
columns because they pack two columns into the width of one half-block column.
Floor rendering asserts integer enlargement; full detail is retained where it
fits. Tiny forced views clip a miniature rather than sample away its features.

The same selection is used by the desk, phone head crops, settings preview and
guard rooms. The desk now uses its whole avatar area instead of imposing an
additional smaller box. Phone crops remain crops of the matching character
resolution, rather than a separate unrelated avatar.

The first review of the final 80×24 screenshot caught a remaining product
failure: automatic projection still chose a plain list, so the normal terminal
size had no office workers at all. Auto now draws a compact top-down floor at
80×24 and a one-row side office at 60×18. Smaller windows, or explicit List mode,
use the text list. Side offices paginate one row; top-down nameplates follow the
actual floor row edges instead of overwriting the middle of a worker. Image
density gets proportional character/desk budgets in those compact projections.
`evidence/visual-small-office-test.log` verifies all five names and rendered
worker clothing in half-blocks, quadrants and sextants at 80×24.
That screenshot review also exposed a sloping legacy title drawn over windows
in the compact projection. It was replaced with horizontal pixel lettering on
a clear band; the large isometric sign remains. Selected compact desks retain
their `!` or `×` marker instead of hiding it behind the selection arrow.

### Guard alerts used the wrong coordinate system in image mode

`guard_scene::draw` divided marker pixels by the Unicode encoding dimensions,
even when its canvas had a physical image density such as 10×20 pixels per cell.
A blocked marker could therefore land beyond its camera tile. It now divides
by the canvas's actual pixels per cell. A dedicated native-density regression
checks that returned positions stay inside a 53×19-cell camera.

### Appearance was generated, but users could not choose a character

The desk now names a decorative character and its fictional quirk. `w` cycles
six wardrobes; `W` restores the original generated look. The selection follows
the conversation ID across the floor, desk, phone, camera wall and preview.
Navigation/preferences integration persists that choice. These quirks do not
claim anything about the coding model's actual abilities or work. Shirt colour
still identifies the source; skin choice remains independent of the decorative
persona. Accessory colour that exactly matched the warning amber is now a cool
neutral.

### Office customization must follow the project, not the window

The source already had four guard-room palettes, but selected them automatically
and did not apply the same palette to the full floor. The office palette is now
a saved choice per office ID: Sandstone, Midnight, Orchard or Arcade. `o` cycles
the palette, `O` restores Auto, and the settings panel exposes the choice.
Auto uses the same stable project hash in both views. This is separate from the
global paper/noir light setting. Exact room material remapping leaves worker
clothing, skin and warning colours alone. The preview uses the same material
selection. Tests verify four distinct floors in both light modes and unchanged
worker/alert colours.

### Stale activity must not look like an approval request

The desk previously titled any blocked status `WAITING ON YOU`, including a
worker inferred blocked only because it had not produced recent activity. It
now reserves that wording for an explicit waiting activity. A timed-out worker
shows `NEEDS ATTENTION`, asks the user to check the original conversation, and
says that no approval was identified. Old command detail is not presented as a
new approval request.

### Two suspicions in the brief were already fixed

The baseline manager and worker already used the same 24×34 character system.
The differing visible height comes from the worker being seated behind a desk.
They still use the same source dimensions at every new resolution.

The baseline `canvas.rs` already recognised `WT_SESSION` independently of
`TERM_PROGRAM`, and already had a regression test. No second detection patch
was necessary. Passing that test is not proof of a real Windows Terminal font
or graphics session.

### A sprite bounds bug was visible only when reading the implementation

`Sprite::pixel(width, 0)` previously read the first pixel of the next row.
It now rejects either out-of-range coordinate before indexing. The test covers
both edges; renderer consumers can safely crop without accidentally wrapping.

## Fidelity by surface

| Surface | Gap against the supplied direction | Assessment |
| --- | --- | --- |
| Office floor | The large sign, room plate, desks and workers survive. Decorative detail is coarser than a bitmap game; arbitrary sprite shrinking was the avoidable loss. | Fixed the structural character loss. Kept adaptive list/side/top-down views for windows without room for isometric scenery. |
| Guard office / tower | Separate rooms and status totals match the concept; miniature workers used to be narrow resampled streaks. More rooms cannot all remain readable on one screen. | Native miniatures plus the navigation lane's floor list and pagination. Paging is a medium constraint, not a missing room. |
| Desk | Live text and approval instructions dominate as intended. The old portrait was unnecessarily reduced. | Full avatar budget and a selected decorative persona. Conversation details remain the primary content. |
| Phone | The channel panel and attention stripe follow the reference. The bitmap header and tiny decorative props cannot carry the same detail at 80 columns. | Kept text channel labels and status wording; native head crops improve identity without spending more rows. |
| Settings | The reference promises a complete office preview and several named office palettes; the original implementation only showed a worker and tiled backdrop. | The four named palettes now apply per office and persist alongside navigation settings. The preview deliberately shows a character, desk and selected room materials within the available terminal space. Close settings to inspect the full office and chosen projection. |
| First run / sources | The reference's entry choice did not explain actual local transcript access. | The installation/source lane replaces ambiguity with explicit local source controls. The selected character system does not require account login. |
| Help | A design board is not useful if key labels disappear in a small terminal. | The navigation lane makes the binding list adaptive and includes source, tower and character controls. |

## Self-critique and limits

The references combine two different targets: Tiny Tower's readable small
characters, and a high-resolution isometric city with large signage. A normal
character terminal cannot reproduce both at screenshot resolution. Faces need
an authored small representation, while room decoration needs fewer marks.
Keeping the image protocol as the only quality target would repeat the original
mistake. The new maps deliberately change the art at each camera distance.

Per-cell two-colour quantisation still merges nearby skin, hair and outline
colours. Passing source-pixel tests establishes that features are drawn, not that
every font/terminal user can distinguish them. The sextant screenshots establish
composition and text legibility under the exporter's assumptions. No Windows,
WSL, Linux desktop, Kitty or terminal image protocol session was visually tested
in this lane. No first-time human usability session was available.

The camera wall is useful for attention across projects; the desk is better for
reading work. Isometric props alone never identify whether a worker is blocked.
Status text and markers must remain explicit. A decorative personality must not
be mistaken for evidence of a model's behaviour.

A second full room inside settings was deliberately not added: at common
terminal sizes it would compete with the controls and require another smaller
character representation. Its material/character preview is simpler than the
design board. The final composition places the preview desk in front of the
worker rather than leaving them at opposite ends of the panel.

The final renderer test run passed 97 tests including the golden frames;
`evidence/visual-final-tests.log` and `evidence/visual-final-clippy.log` record
the full renderer checks. `evidence/visual-sprite-tests.log` records
the later targeted character tests; the final workspace checks are recorded by
the main audit. Regression checks cover native dimensions, all six wardrobes,
same-person head crops, manager dimensions, reset behaviour and image-marker
coordinates. `evidence/visual-stale-work-test.log` covers the inferred-timeout
approval regression. Golden regeneration still requires visual inspection; accepting a
new hash alone is not a design review.
