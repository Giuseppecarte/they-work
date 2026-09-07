# Room composition and resizing

The five supplied screenshots showed a camera whose title and empty floor grew
with the terminal while people and furniture stayed fixed. A nominal two-row
grid created empty cubicles even when only one conversation existed. This
iteration replaces that geometry in `views/office.rs`.

## Composition

- **Top view:** a shared floor, a shallow wall with the project sign, occupied
  workstation rows, plants, and a lounge when a small team has sufficient space.
  The final partial row is centered. Workstations have chairs, monitors, desk
  surfaces, legs and a cup. There are no per-person room outlines.
- **Side view:** a wall with bounded city windows, a common floor line, complete
  seated workstations, a bookcase and a plant. Labels sit below the corresponding
  desk rather than at an unrelated screen edge.
- **Isometric view:** a room footprint appropriate to one, three or larger teams,
  with furniture and integer-scaled figures sized from the same tile geometry.
  Small terminals page the scene at three or five people instead of packing ten
  overlapping figures into the room. Larger rooms retain the ten-desk capacity.
- **Tall windows:** top and side rooms have a bounded height and are centered.
  Extra screen height does not create empty worker rows or enlarge the sign.
- **Signs:** cyan lettering replaces the universal warning-red sign. Lettering
  has bounded native tiers, at most four terminal rows and approximately twelve
  percent of the available scene height. The complete project name remains in
  the textual header when a very small scene cannot carry pixel lettering.

Source selection, character choices, room palettes, inspection, overflow pages
and navigation retain their existing behavior. PgUp/PgDn and Home/End now also
operate on floor workers; the page step follows the active resized layout.
Nameplates remove shared prefixes
only when necessary to preserve distinguishing suffixes. Identical titles get a
stable identifier suffix; the inspector retains the complete title.

## Criticism applied during implementation

The first native PTY reconstruction was substantially clearer but still failed
visual review: clouds escaped their windows, independently rasterized floor tiles
left black gaps, and the lounge rug crossed the isometric floor boundary. Those
were source defects, not font problems. Clouds now clip to the window interior,
the floor is filled as one polygon, and the rug fits inside its tile.

A subsequent continuity test exposed duplicate scanline intersections at polygon
vertices, which left a horizontal gap even after removing the separate tiles.
The scanline fill now deduplicates the intersections. The dark room palette also
needed lighter seams: darkening both samples collapsed them to the same ANSI-256
color and made the floor disappear in native terminal output.

The second review exposed overlapping people in the explicitly selected
isometric view at 80×24 with twenty conversations. Projection-specific paging
now preserves the scene's readable capacity and keeps the last worker reachable.

Idle excursions are bounded to a person's workstation and use identity-based
timing. Pauses use the idle pose; travelling uses a walking pose. The reduced
motion setting freezes figures, sky, and manager position together while status
continues to use the real event clock.

## Verification and evidence

Office tests cover one, three and twenty workers at 80×24, 120×32, 192×58,
240×70 and 110×80 in all three character encodings. They check occupied rows,
figure bounds, label placement, last-page reachability, duplicate names, actual
blocked-manager avoidance, continuous odd-sized floors, window clipping, indexed
floor contrast and complete reduced-motion frames.

The reproducible native executable exercise is in [review_pty.py](review_pty.py).
Initial and intermediate evidence is under [evidence/before](evidence/before),
[evidence/preview](evidence/preview) and [evidence/review2](evidence/review2).
These are PTY/ANSI reconstructions; the font audit separately renders the actual
glyphs using CoreText. They are not screenshots captured from Terminal.app.
Intermediate captures deliberately preserve the defects found during review.
