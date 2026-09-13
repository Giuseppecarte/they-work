# Tower and scene navigation

The tower now displays several complete project floors. The host supplies the scene rectangle after reserving its controls and inspector; the tower never draws into those reserved areas. At an 8×16 pixel terminal cell, a body of 20, 32 or 54 rows shows two, three or five floors respectively, provided that many projects exist.

## Decisions

- Floors follow the stable project ordering. A contiguous window always includes the selected project. Resizing changes the number of visible floors, not project identity.
- Overview floors have a maximum height of eleven terminal rows and a building width of 1120 physical pixels, aligned to complete cells. A building with fewer projects is centered against a static exterior instead of stretching its people or walls. The overview keeps its separately authored 24×32 cast; entering a floor uses the detailed 48×64 cast. Detail scale is shared with the studio and stops at 2×.
- Each floor has a native project label, occupancy, attention or working count, and a visible Visit action. A click on its body selects the project; a click on a person opens inspection. These targets use the physical geometry of the rendered frame, including animated actor bounds.
- An active team requires explicit delegation edges with present endpoints and an active descendant. Session membership and forks do not create meetings. Missing or ambiguous parents do not acquire an invented lead. The world already rejects cycle-forming edges; the projection also guards traversal.
- A visible room selector switches between observed teams and independent desks. All active teams are excluded from the independent desk roster, including teams outside the selected room. A remembered team cannot override a person subsequently selected by the keyboard.
- Cosmetic names belong to stable profiles. The tower does not append a roster-dependent ordinal or rename a person when a neighbor arrives. Original task titles remain visible for the selection and in inspection. Explicitly identical aliases remain the names the user chose.
- Native text is composited after the image layer. Labels use bounded truncation and a one-column gutter rather than running into neighboring people.
- Image transport is optional. The native counterpart retains floor and person targets, team selection, original titles, current state, source coverage, and bounded pages. Meeting pages keep their lead visible when two or more people fit. Small windows reduce the number of listed people.
- A detailed office is capped at 512 physical pixels in height. If at least four terminal rows remain, a native “Latest on this floor” panel uses that space for real aliases, current states, original titles and activity details. It follows the selected person, pages the existing roster, and provides inspection targets. The inspector remains full height in its own column.

## Critique and corrections

The first capture exposed a real compositing defect: project headers had valid strings and click rectangles but still retained the image skip mask, so the rendered image hid them. The tower now explicitly releases those native cells. Render tests check this condition at every visible floor; checking string contents alone would have missed it.

A final pixel review found that reusing expansive actor-hitbox rounding for nameplates could erase ten pixels of a body. Native labels now start at the next whole cell and retain their intended one overview row or two detail rows. Actor hitboxes still expand to cover touched cells. The dedicated regression sweeps every vertical alignment within a sixteen-pixel cell.

The first implementation also reused a previous character-scale formula when deciding whether an office could split into two rooms. The split contract is checked against the real studio geometry: neither half may reduce the detailed cast scale, and a meeting must still fit its lead and a child.

The first collision treatment added an ordinal to duplicate aliases. Review rejected it because adding an earlier worker changed another person's displayed identity. Profiles now provide stable names, and explicit custom aliases are preserved exactly.

Independent review also rejected the stretched, 1536-pixel-wide floors with only two or three people. The final width cap leaves the 80- and 120-column views unchanged and gives the larger window a building silhouette. At 192 columns and an 8-pixel cell, the building begins at column 26; tests verify that its floor and worker click regions use the same translation.

Paging originally used the number of people actually present on the last page. That made PageUp move too little when the last page was incomplete. `SceneLayout.capacity` now communicates the physical capacity independently of occupancy, and the focused test compares the first and last page.

A sparse single-floor tower deliberately keeps the overview scale. It can contain substantial exterior space on a tall terminal; opening that floor provides detailed inspection. This is an explicit choice to preserve continuity between floors, not evidence that a taller window contains more work.

The first stable-scale office still stretched its tiled floor into a large empty region below the people. The final review rejected that composition: the 512-pixel cap and native work panel replace that unused space with actionable project information while keeping the same people, scale and provider facts. Tests verify its click boundaries alongside an offset inspector and the last person on a paged roster.

## Reproduce the visual review

The fixture contains twenty independent projects and fifty conversations, including two confirmed delegation families and independent workers on the first floor. It exercises the first and last project at 80×24, 120×36 and 192×58, one- and three-project towers, floor entry, inspection, native fallback, a light theme and a 32×14 terminal. The example uses the actual `Ui`, not a separately drawn mock-up.

```sh
cargo run -p theywork-render --example tower_overview -- docs/design-audit/tmp/iteration-5-tower
python3 docs/design-audit/iteration-4/export_ui.py docs/design-audit/tmp/iteration-5-tower docs/design-audit/iteration-5/evidence/tower-final
cargo test -p theywork-render --lib views::tower::
```

The export script needs Pillow and the macOS Menlo font. The repository-local audit environment can run it with `PYTHONPATH=docs/design-audit/tmp/python`. The output combines exact physical RGBA pixels, the native-text mask and Menlo glyphs. It validates scene composition and text ordering; it is not a screenshot of a physical Kitty, Sixel or iTerm2 terminal, and it does not prove their transport behavior.

The focused regression tests cover floor counts and selection after resize, inspector offsets, clipped and nonempty hit regions, the last worker in a crowded floor, family switching, independent desks, missing and ambiguous relationships, identity stability, native coverage and parent-pinned pagination. The fixture performs no provider login, paid turn, approval or network operation.

The [geometry and navigation run](tower-tests.log) passed twelve tests after the height cap. The [final label-mask regression](tower-label-mask-test.log) passed after its subsequent correction. The integration audit records the complete branch checks.

| Scene | Evidence |
| --- | --- |
| Two complete floors, overview nameplates | [80×24 tower](evidence/tower-final/tower-20-80x24-first.png) |
| Five floors and centered click geometry | [192×58 tower](evidence/tower-final/tower-20-192x58-first.png) |
| Last project remains accessible | [80×24 last floor](evidence/tower-final/tower-20-80x24-last.png) |
| Bounded office with native project activity | [192×58 inspector](evidence/tower-final/inspect-team-192x58.png) |
| No-image counterpart | [80×24 native view](evidence/tower-final/native-80x24-first.png) |
| Tiny native window | [32×14 native view](evidence/tower-final/native-32x14-last.png) |

## Performance scope

`measure_tower` emits separate rows for tower, office and inspector, each with 1, 6 or 20 projects containing fifty conversations, at the three documented sizes. Each case has ten warm-up frames followed by sixty samples. Motion is enabled at simulated times 9000–15900 ms so the shared decorative vignettes are included. Physical image dimensions are recorded; a zero dimension explicitly identifies a compact inspector with no scene image.

The interval includes keyboard dispatch, actual UI composition and native-text image masking. It excludes collection, provider responses, terminal encoding, output writes, operating-system paint and display refresh. It therefore cannot establish a 150 ms end-to-end terminal input guarantee. The earlier CSV is retained as an intermediate measurement from before the width cap and the vignette timing correction; compare configurations with care.

The final macOS release run contains 27 cases and 1,620 measured frames. Across those cases the largest p95 was **9.180 ms for the tower**, **8.100 ms for the office**, and **6.526 ms for the inspector**. The largest individual sample was 9.202 ms. Raw values and physical image dimensions are in [input-composition.csv](input-composition.csv).

After that run, the art layer received a semantic correction that gives explicit human requests priority over an older editing pose. The measured fixtures contain no `Wait` events; their geometry and exercised poses did not change. The visual fixture does contain a human request and its final captures are regenerated with that correction.

```sh
cargo build --release -p theywork-render --example measure_tower
target/release/examples/measure_tower > docs/design-audit/iteration-5/input-composition.csv
```
