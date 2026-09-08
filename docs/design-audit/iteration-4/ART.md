# Living office artwork and composition

The graphical scene now has an original twelve-character cast, authored at
48×64 pixels. The previous 24×34 sprites remain the compact character fallback.
The new artwork is in `crates/theywork-render/src/living_office/art.rs`; it is not
an enlargement or recolouring of those older sprites. The twelve costumes are
headphones, chef, explorer, gardener, astronaut, artist, wizard, rocker,
bookworm, runner, hard hat and dinosaur. These are decorative costumes, not
claims about a conversation's role or capabilities.

## Visual changes and critique

- [Cast at integer 3×](evidence/art/cast.png): separate crowns, hats, tails,
  backpacks, ponytail, cape and clothing cuts. Faces have stepped contours,
  restrained edge shadows, skin highlights and individually drawn features.
- [Office, 80×24 pixel geometry](evidence/art/office-3-80x24.png): wall reveals,
  city windows, upper-left illumination, wood planks, contact shadows, furniture
  layers and a continuous elevator shaft. The fascia and nameplates are blank
  in this art-only export because their contents are native terminal text.
- [Meeting room](evidence/art/meeting-6-120x32.png): a connected shared table,
  documents and individual seating. The parent stays on every member page.
- [Decorative activity](evidence/art/gags-6-120x32.png) and
  [pose samples](evidence/art/poses.png): walking, reading, coffee, stretching,
  watering plants and a paper plane. Actors move into a clear foreground aisle.
- [Six-second motion sample](evidence/art/decorative-motion.gif): 48 authored
  frames at eight frames per second. GIF alternates 120/130 ms durations to
  preserve the six-second timeline despite its centisecond precision.
- [Observed interactions](evidence/art/interactions.png): separate folder-out,
  envelope and folder-in gestures for delegation, message and returned result.

The first visual inspection found monitors covering the edge of faces and a
narrow, tall room shrinking its occupants. Monitors now sit beyond the face
silhouette. Character scale follows available height; narrower rooms paginate
instead of reducing all people. The full inspector can use integer 4× art.
Coffee and stretching poses were revised so their hands connect to the props.

The first integrated UI exports also exposed two native-layer defects outside
the art module: text drawn with Paragraph retained Canvas's temporary skip flag;
then clearing only the flag preserved old block glyphs in empty label cells.
Both were reported with buffer coordinates and reconstructed PNG evidence for
the integration owner to fix. The reproduction pipeline preserves the real
native mask; it does not repair missing labels in a screenshot after the fact.
The corrected [80-column inspector](evidence/ui/office-80x24.png) uses integer
2× people, a pinned lead and one child per page with an explicit page counter.
The previous three-at-1× composition still gave too much space to the wall;
readable people now take priority over fitting every participant at once.
At [192 columns](evidence/ui/office-192x58.png),
the cast enlarges to integer 4× and labels include both identity and real state.

## Scene API and facts

`Studio::paint` paints a whole physical canvas. `Studio::paint_region` composes
contiguous floors into a subrectangle of a shared canvas. Only the completed
outer canvas is presented as an image. `SceneLayout` returns translated actor
hitboxes, stationary nameplates, project fascia, elevator bounds, visible
members, page counts and the number of active decorative vignettes.

`Canvas::set_image_cell_size` explicitly selects negotiated physical geometry;
sextant dimensions cannot accidentally become the graphic scene resolution.
Undersized surfaces return `graphics: false` so the host can use compact text.
Every character uses integer enlargement. A character's appearance depends
only on its ID and its explicit wardrobe choice. Removing or reordering its
neighbours cannot change its face; finite costumes may legitimately repeat.

`MeetingGroup` accepts only explicit parent/member IDs supplied by the host.
The scene neither discovers nor invents delegation. It deduplicates members,
does not replace an absent parent with the first child, keeps a present parent
pinned and follows the selected member across pages.

`SceneCue` is an optional host-supplied map of source-observed interactions.
Delegating, messaging and delivering select different props; waiting for a team
uses a reading pose. Real human requests and failures take precedence over
these cues. No decorative routine can produce a semantic cue or message text.
The host controls recency; the scene does not infer events from costumes.
Adjacent rooms set `show_elevator: false` to use an ordinary glazed wood door,
sharing the primary tower shaft instead of inventing a second elevator.

Decorations use a fixed clock, stable IDs and deterministic cycles. At most two
actors per office receive a vignette. They never mutate activity, lifecycle,
messages, requests or relationships. Reduced motion removes the vignettes and
freezes the art clock while facts remain live. Automatic review, child waits
and process waits do not use the raised-hand pose, including in the old compact
sprites. Only explicit human approval/input or an explicit legacy unknown wait
uses that gesture. Silence alone does not.

## Resource and transport limits

Studio retains at most 512 authored poses and eight room backgrounds, with one
reusable region buffer. It has no frame-output queue. Integer blits reuse one
RGBA write borrow and convert source colours once rather than once per output
pixel. The host remains responsible for pacing and dropping stale frames.

Kitty transmission now selects lossless PNG when smaller than raw RGBA; noisy
images retain raw encoding. The PNG and raw paths both round-trip through the
independent capture decoder. Identical frames still produce no transmission.
The placement, native layer order, image ID and removal policy are unchanged.

For the actual 960×512 office export, a release build measured 115,933 bytes per
Kitty frame versus 2,621,440 bytes of raw base64 (about 22.6× smaller). iTerm2
used 115,727 bytes and Sixel 33,590 bytes. Encoding took approximately 0.39 ms,
0.38 ms and 5.76 ms respectively on this machine. These are encoding-only
measurements, not terminal latency or playback claims. Exact results and scope
are in [encoding-metrics.json](evidence/art/encoding-metrics.json).

## Reproduction and acceptance boundaries

```
sh docs/design-audit/native-cargo.sh run --release -p theywork-render --example living_gallery -- docs/design-audit/tmp/iteration-4-art --motion
python3 docs/design-audit/iteration-4/export_art.py docs/design-audit/tmp/iteration-4-art docs/design-audit/iteration-4/evidence/art
env -u NO_COLOR THEYWORK_COLOR=truecolor sh docs/design-audit/native-cargo.sh run -p theywork-render --example living_ui -- docs/design-audit/tmp/iteration-4-ui
python3 docs/design-audit/iteration-4/export_ui.py docs/design-audit/tmp/iteration-4-ui docs/design-audit/iteration-4/evidence/ui
sh docs/design-audit/native-cargo.sh run --release -p theywork-terminal-image --example measure_art -- docs/design-audit/tmp/iteration-4-art/office-6-120x32.ppm
```

The Python exporters require Pillow and use Menlo from macOS for annotations
and native-cell replay. PPM, RGBA and cell JSON intermediates stay under ignored
audit scratch. The committed PNG manifests record dimensions and SHA-256.

The integrated [UI exports](evidence/ui/ui-manifest.json) come from the actual
Ui, TestBackend, physical PixelFrame and native mask at 80×24, 120×36 and
192×58. They are Menlo/Pillow reconstructions, not screenshots of a real
terminal emulator. Terminal.app was not controlled. Kitty/iTerm2/Sixel live
playback and Windows/WSL visual acceptance remain separate release checks.
The same exporter exercises `b` then `2` for deliveries and `g` for the team
tree at 80×24 and 120×36, including native-only overlays after the image scene.

Focused tests cover all twelve distinct source masks, stable identity,
deterministic skipped-frame behaviour, the two-vignette limit, reduced motion,
real waiting semantics, physical geometry, member pagination, translated
regions, bounded caches and lossless Kitty transfer. Golden character snapshots
remain the fallback checks; they do not substitute for these image tests.

Validation at the artwork freeze: 15 scene tests, 15 terminal-image tests,
179 renderer tests with the pending golden regeneration explicitly skipped,
and strict Clippy for both crates across all targets passed. Logs are in
[art-focused-tests.log](evidence/art/art-focused-tests.log),
[transport-tests.log](evidence/art/transport-tests.log),
[art-tests.log](evidence/art/art-tests.log), and
[art-clippy.log](evidence/art/art-clippy.log).
