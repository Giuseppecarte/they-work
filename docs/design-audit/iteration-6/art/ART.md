# Stage 6: readable workstations

A workstation now depicts a person using a complete computer. Detail uses an 84-pixel slot; overview uses 44. The authored characters remain 48×64 and 24×32, enlarged by integers. Three detail stations still fit a 640×304 canvas at 2×, the space available in an 80-column graphics view.

The [previous integrated office](../../iteration-5/ui/office-80x24.png) shows the starting problem: a shared table without individual computers. This iteration treats the computer, working hands and chair as one authored station.

## Pilot and corrections

The [detail pilot](pilot/compare-detail.png) and [overview pilot](pilot/compare-overview.png) compare three deliberately different silhouettes: headphones, chef and dinosaur. Each has an individual desktop and a shared-table laptop. The pilot was inspected before expanding the cast review to twelve.

The monitor is landscape, with a contrasting bezel, a visible stem and a base touching the desk. The keyboard is a separate plane with recognizable keys. A chair back, seat pan, pedestal and casters sit behind the person. Working and screen-reading poses bend the knees and reach over the keyboard; the foreground hand layer crosses the tabletop. Human requests retain the frontal raised hand.

The review produced concrete corrections:

- Overview screen motifs initially extended beyond a small screen's glass. Each motif now paints into a clipped local raster before composition. The exhaustive clipping test covers ten categories, eight phases and four glass dimensions.
- The first three people did not expose the runner's wide ponytail. The twelve-person occlusion test found a monitor crossing that silhouette at source pixel (46,27). Moving the computer to the right preserved the whole upper silhouette without changing slot capacity. Both desktop and laptop variants were checked.
- On a dark workshop wall, the headphone band's open right side read as a disconnected bar. The final art joins both sides to the ear cups at both grids.
- Windows competed with screens. Reflections, frames and floor seams now have lower contrast; there are at most a few windows across a room. Workshop and laboratory construction remains distinct.

The pilot PNGs preserve the earlier three-person review. The gallery is regenerated from the final recipes. The captions use ASCII separators so a missing label glyph cannot contaminate the visual comparison.

## Final artwork evidence

| Evidence | Review target |
|---|---|
| [Individual detail](gallery/cast-individual-detail.png), [individual overview](gallery/cast-individual-overview.png) | All twelve costumes seated at desktops, keyboard contact, monitor support and readable silhouettes. Rows contain costumes 0–2, 3–5, 6–8, 9–11. |
| [Shared detail](gallery/cast-shared-detail.png), [shared overview](gallery/cast-shared-overview.png) | Every team participant has a laptop; the table remains continuous and faces remain visible. |
| [Studio dark](gallery/materials-studio-dark.png), [studio light](gallery/materials-studio-light.png) | Warm surface, restrained glazing and three visible Desk choices at a shared table. |
| [Workshop dark](gallery/materials-workshop-dark.png), [workshop light](gallery/materials-workshop-light.png) | Pegboard and workbench materials; same equipment and hand geometry. |
| [Laboratory dark](gallery/materials-laboratory-dark.png), [laboratory light](gallery/materials-laboratory-light.png) | Cool counters and glass; dark computer casing remains distinct against the light room. |
| [Screen categories](gallery/screen-categories.png) | Row-major: editing, command, reading, searching, thinking, automatic wait, human input, error, quiet, unavailable. These abstract motifs contain no invented log lines, percentages or results. |
| [Complete route](gallery/outbound-action-return.gif) | Departure, decorative activity and return to the same station, 129 frames at nominal 125 ms. |
| [At station](gallery/route-000.png), [outbound](gallery/route-012.png), [activity](gallery/route-048.png), [return](gallery/route-116.png), [back](gallery/route-128.png) | Full-RGB checkpoints from the same route. Empty chairs, keyboards and monitors remain in place while the decorative actors move. |

The retained source grids have twelve independently authored outlines. Small accessories lose detail in overview, but each person and computer remains a distinct silhouette. Screens are supporting category cues; native text and inspected source evidence carry the exact facts. The small automatic-wait dots are deliberately quiet, while human input has an amber mark and the raised hand.

## Interaction and state contract

`SeatLayout.workstation` contains physical-pixel rectangles for `computer`, `keyboard`, `chair`, `selection` and `actor_home`. `paint_region` translates every one alongside the person and native nameplate. The complete station can point to the same worker even while a decorative actor is in the aisle. The UI owns selection markers, keyboard focus and native labels; the art never substitutes color for that semantic state.

The layout test checks three stations at 80 columns, 2× scale, containment and all five translated anchors. Artwork tests compare the actual foreground raster against every upper silhouette: twelve costumes, two authored grids, three equipment variants, individual/shared furniture, working/human poses and two typing phases. Visible working hands must overlap the keyboard at each phase. Monitor-support tests require continuous stem pixels reaching the desk. Shared Desk variants must produce different art.

Observed failures and human requests preserve their factual priority while the source is current. If a source has a nonzero observation time and is stale or unavailable, its last recorded activity does not drive live gestures, collaboration cues or decorative movement. The character sits quietly, the screen shows unknown coverage, and the model still retains the request/error/activity for native inspection. A regression covers old editing, error and human request; it also checks unavailable frames remain unchanged as time advances. An unobserved source (`observed_at == 0`) retains the existing legacy/demo behavior.

The previous absolute-time simulation, stable identity, bounded caches, two-vignette limit and reduced-motion behavior remain in place. A shared floor still receives one simulation plan. No new provider capability, real task result or animation queue is introduced.

Final focused verification: [29 passing artwork/layout/simulation tests](checks/focused-tests.log). The [gallery export](checks/gallery-export.log) completed. Strict Clippy was attempted during concurrent UI integration and stopped on an unfinished `InspectRecord` match outside the artwork; it is recorded as an intermediate attempt, not a passing check. The integrated release check owns the final strict result.

The [80-column room](../evidence/inventory-review2/office-auto-image-80x24.png), [tower](../evidence/inventory-review2/tower-image-80x24.png) and [offline-source state](../evidence/inventory-review2/directed-source-offline-80x24.png) were independently inspected. Three stations remain readable; native labels begin below complete bodies and keyboards. Noa's human request has the raised hand. The offline view instead leaves characters quiet. These integration PNGs precede the final headphone-arc detail; they validate UI placement, while the gallery validates the final authored pixels.

## Reproduce

From the repository root, with Rust and Pillow:

```sh
cargo run -p theywork-render --example workstation_pilot -- docs/design-audit/tmp/iteration-6/pilot
python3 docs/design-audit/iteration-6/art/export.py docs/design-audit/tmp/iteration-6/pilot docs/design-audit/iteration-6/art/pilot
cargo run -p theywork-render --example workstation_gallery -- docs/design-audit/tmp/iteration-6/gallery
python3 docs/design-audit/iteration-6/art/export.py docs/design-audit/tmp/iteration-6/gallery docs/design-audit/iteration-6/art/gallery
cargo test -p theywork-render --lib living_office::
```

The examples export exact PPM canvas pixels and geometry/timing metadata to ignored scratch. The Python exporter performs no sprite resizing. [Gallery manifest](gallery/manifest.json), [geometry](gallery/geometry.txt) and [motion timing](gallery/motion.csv) record the output. GIF timing alternates 120/130 ms because its format uses centiseconds; GIF colors are a palette approximation and the PNGs retain RGB.

These are Rust compositor exports, not live terminal screenshots. They prove art geometry and pixel composition; native label integration, terminal transport, performance and input tests belong to the integrated UI review. Terminal.app was not controlled. The graphics assets are not compressed into block glyphs: the compact fallback continues to use native text.
