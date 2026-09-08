# Stage 5 — readable people in a living building

The change is structural: the tower has its own authored 24×32 cast; the room uses 48×64 art at a bounded integer scale. Larger windows no longer enlarge faces or separate a person's name from their desk. The reference's useful qualities are a clear silhouette, a small readable gesture and a room that supports the people. Adding more pixels to the previous large front-facing portraits would not achieve those qualities.

## Pilot, criticism and correction

The three-person pilot deliberately used contrasting outlines: headphones, a chef's hat and a dinosaur hood. [The first cast sheet](pilot-three-cast.png) compares detail, small authored art and its native grid. It was reviewed before extending the overview cast to twelve. It established readable clothing and a hand above the head, but the original single direction did not give a person a clear way to address the viewer.

[The final orientation pilot](front-and-three-quarter-pilot.png) adds a separately constructed frontal face and torso at both grids. Rest, human requests and errors face the viewer; directional work, collaboration gestures and walking use right/left three-quarter poses. Costumes and identity stay the same. The raised-hand pose is reserved for actual human input/approval (and explicit legacy waiting); automatic review, a child or a process does not acquire a raised hand.

The initial room review exposed disconnected table segments and a large gap between bodies and native names in tall windows. [The retained room pilot](pilot-studio-detail.png) shows the correction before the frontal-face pass: tables form one continuous shared surface, and foreground hands/books cross the tabletop. The detail scale is capped at 2×, the authored wall at 112 logical pixels and windows at 33. The native name/status rectangle starts two logical pixels below the actor's complete bounds. Extra room height creates an aisle rather than extending the wall. UI integration separately caps a single-room presentation; the art does not decide how much screen space the inspector deserves.

The first route export also exposed a concrete error: global spacing pushed the watering actor far from the plant. Destinations now match the shelf, refreshments and plant; two vignettes cannot occupy the same facility. A lower route travels through the shaft-side corridor before crossing below all native names, then reverses to the same desk. Compact rooms use a lateral route because a complete actor cannot fit under their native labels.

The preserved `pilot-*` PNGs record those intermediate reviews. Running the current pilot example uses the corrected recipes; final results are the files below.

## Final evidence

| Evidence | What to inspect |
|---|---|
| [Twelve authored characters](cast-twelve-authored-grids.png) | Twelve different outlines at both grids; 2× detail, 2× overview and 1× overview are labelled. No fractional shrinking. |
| [Frontal and three-quarter pilot](front-and-three-quarter-pilot.png) | Headphones, chef and dinosaur turn without changing identity. Human hands remain above the head. |
| [Gesture sheet](gestures-at-double-size.png) | Working, reading, delegation, delivery, message, human help and coffee retain clear props at both sizes. Semantic gestures require source events; coffee is decorative. |
| [Studio](room-studio-detail.png), [workshop](room-workshop-detail.png), [laboratory](room-laboratory-detail.png) | Different construction: glazing/books, pegboard/workbench, glass/counters. Bodies and native-label geometry are unchanged. |
| [Studio overview](room-studio-overview.png), [workshop overview](room-workshop-overview.png), [laboratory overview](room-laboratory-overview.png) | Same room grammar on the smaller authored grid. |
| [Entrance](zone-entrance.png), [desks](zone-desks.png), [meeting](zone-meeting.png), [rest](zone-rest.png) | Three visible choices per zone; each sheet fixes all other choices and keeps the same three people. |
| [Complete route animation](outbound-action-return.gif) | 129 frames cover departure, a decorative activity and return. Source time is 8.000–23.999 s with 125 ms nominal cadence. The GIF alternates 120/130 ms because GIF stores centiseconds. |
| [At desk](route-000.png), [outbound](route-012.png), [activity](route-048.png), [return](route-116.png), [back at desk](route-128.png) | Full-RGB samples from that same sequence; these avoid GIF palette approximation. |

The room/art images intentionally have blank native text slots. They are exact Rust canvas exports with explanatory labels, **not screenshots of a terminal**. Full integrated UI evidence is maintained in the neighbouring `evidence/tower-final` directory. This art review does not claim that Kitty, Sixel or iTerm rendered these PNGs in a live window. Terminal.app was not controlled.

## Behaviour and acceptance evidence

- Detail art stays at 2× for 960×512 and 1536×848 canvases; a 528 px room uses 1×. The public `detail_scale(width,height)` is shared with room splitting so that adding a meeting room cannot silently shrink its cast. Overview 640×128 retains 24×32 characters at 2× plus a real 16 px native label row.
- Scene pagination reports physical capacity independently from the last page's occupancy. A selected last child is visible with its pinned lead. Existing stable identity/remove/add tests remain in place; identical costume choices are allowed and uniqueness across an arbitrary roster is not promised.
- Front/right/left artwork differs for all twelve costumes on both grids. Existing outline tests confirm twelve authored silhouettes per grid. These are independent recipes, not sampled detail frames.
- One per-office simulation plan is shared by its rooms, with at most two decorative vignettes. Calm/curious/energetic changes the decorative choice. A secondary activity can use another free facility to avoid two people occupying the same spot. Planning is deterministic under frame skipping/reordering; it never changes `Worker` facts.
- Reduced motion removes decorative travel and fixes the art phase. New real requests still update. Current human requests and failures override decorations and collaboration cues.
- Every 125 ms sample of a complete lower-aisle sequence was checked against the nameplate rectangles returned by Studio. None overlaps. Integration review additionally caught the host expanding an overview plate from one native row into two when rounding its position; that fix belongs to the tower renderer and must be verified in the final UI captures. The return ends at the original bounds. All three styles over 100 cycles were additionally checked for facility collisions.
- Each preset and each of the four zone choices produces different visible art without changing worker IDs. Frame caching remains bounded at 512 combinations; background caching at 8 rooms; there is no animation frame queue.

Focused verification: [22 artwork tests](tests.log), [frontal-grid check](front-tests.log), and [facility collision regression](facilities-tests.log). The facility regression was added after the main focused run; it adds one test. Renderer Clippy is recorded in [clippy.log](clippy.log). Workspace, cross-platform and real PTY checks belong to the release verification record.

## Reproduce

From the repository root, with Rust and Pillow installed:

```sh
cargo run --release -p theywork-render --example studio_gallery -- docs/design-audit/tmp/iteration-5-gallery
python3 docs/design-audit/iteration-5/art/export.py docs/design-audit/tmp/iteration-5-gallery docs/design-audit/iteration-5/art
cargo test -p theywork-render --lib living_office
cargo clippy -p theywork-render --all-targets -- -D warnings
```

`studio_gallery` writes PPM frames and timing/count metadata into ignored audit scratch. The exporter copies the pixels at 1:1, adds captions in reserved margins and produces the GIF. It writes dimensions, hashes and timing to [manifest.json](manifest.json); [motion.csv](motion.csv) records actual frame times, scale and vignette count. No AI bitmap assets or external art downloads are involved. The pilot is separately reproducible with `--example studio_pilot` and the same output-directory argument.

## Limits retained deliberately

The small overview sacrifices fine costume stitching and facial shading to keep independent authored silhouettes. The decorative simulation does not claim that a conversation literally made coffee, left a desk or completed work. Source events, native statuses and inspector evidence remain the authority. Real native font/image alignment and protocol behaviour require the integrated transport tests, beyond an art canvas export. The fallback uses compact functional UI; these graphics-primary assets are not squeezed into terminal block glyphs.
