# UI inventory and review contract

This audit separates a successful route, geometric checks, visual review and real
terminal behaviour. One of them cannot substitute for the others. Fixtures are
local and synthetic; no provider login, real model turn or personal store is used.

## Finite matrix

`crates/theywork-render/examples/ui_inventory.rs` declares 31 surfaces:

- Tower; office Auto, Side, Isometric, Top and List.
- Inspector Now, Activity, Team and Details.
- Notebook Attention, Deliveries, Since your visit and Team; Finder.
- Connections, New task, Task controls, Request and Project picker; Sources.
- Character and office Design; Settings, Advanced settings and Help.
- Phone Now, Attention, Edits and Messages; More.

Each surface is scheduled at 32×14, 80×24, 120×36, 192×58, 110×80 and 240×70.
Each also has native, light and no-color variants at 80×24. That is 279 base
entries. Thirty directed entries cover no sources, scanning, a source error, a
project-filter empty result and an ordinary empty result at every size. Sixty
entries cover one project, one worker, the last floor/person, names off, stale
and disconnected observations, different wait reasons, no search results and
long Unicode titles. Six further entries cover expanded inspection, extra
actions and result filtering at 80×24 and 192×58. Total: **375 scheduled entries**.

Sources is owned by the executable. Its nine entries remain `not-tested` in the
renderer manifest and require the isolated executable/PTY evidence; rendering a
Connections panel is not a substitute for testing the source chooser.

At 32×14 the inspector intentionally becomes an emergency work brief with identity,
observed state, source coverage and the next review action. Full tabs require a
larger window. Non-Now tab cases are opened at 80×24 before resizing; the manifest
calls the rendered surface `compact-work-brief` and records this limitation. It
does not claim four complete tabs or their controls fit in the emergency layout.

## What the generated manifest says

The example uses the actual `Ui`, keyboard events and presented pointer targets.
Every input is followed by a redraw and acknowledgement of the presented frame.
It records native text, native cell styles/masking, physical RGBA and a JSON
manifest. Missing expected routes and out-of-bounds targets are recorded as
`defect`, rather than silently substituting another surface. A successfully
rendered case initially remains `not-tested` for visual review. Automatic route
and geometry results are recorded separately.

A reviewer must inspect the resulting frame before recording visual acceptance.
An exported image is a reconstruction using native glyphs plus compositor pixels,
not a terminal-window screenshot. Real image transport, text selection, operating
system input latency, authenticated provider behaviour and novice comprehension
remain separate acceptance work.

```sh
sh docs/design-audit/native-cargo.sh build --release -p theywork-render --example ui_inventory
env -u NO_COLOR -u THEYWORK_COLOR -u THEYWORK_PIXELS target/native-macos/release/examples/ui_inventory docs/design-audit/tmp/iteration-6-inventory
PYTHONPATH=docs/design-audit/tmp/python python3 docs/design-audit/iteration-6/export_inventory.py docs/design-audit/tmp/iteration-6-inventory docs/design-audit/iteration-6/evidence/inventory
```

An optional second argument filters case IDs, for example `80x24` or
`directed-source-offline`. This permits a small pilot before regenerating the
complete matrix. The normal exporter requires Pillow and Menlo.
An optional third argument selects `8x16` (the default) or `10x20` cell pixels.
The captured metadata, image dimensions and native reconstruction retain that
actual geometry. The additional 10×20 acceptance gallery is separate from the
375-entry 8×16 manifest.

The example controls its rendering modes; remove inherited colour overrides from
the audit command only, leaving product behaviour unchanged. Its first pilot
exposed `NO_COLOR=1` inherited from the host, which made nominally coloured text
monochrome. The corrected manifest records effective colour and encoding to make
that mismatch inspectable.

`export_inventory.py` publishes a searchable local gallery, native text, hit
targets and the combined manifest. Its optional `--reviews` file maps case IDs
to `{ "sha256": "…", "status": "pass|defect|not-tested", "note": "…" }`.
An accepted review applies only to the exact reviewed image hash; a changed frame
returns to `not-tested` instead of inheriting a stale verdict.
Repeat `--reviews` for independent review files. Reviews for different image
hashes do not override one another; conflicting verdicts for the same current
image retain the more conservative result.
A byte-identical PNG can reuse an inspected image's visual verdict. The manifest
names that reviewed case in `visual_review_reused_from`; route and hit bounds
remain recorded independently for each case. Similar-looking images with
different hashes never inherit this approval.
`--reuse-images` updates the verdicts/gallery without repainting PNGs; it rejects
missing images or a native capture newer than the exported image. This is useful
while independent reviewers finish their verdicts on an unchanged final export.

## Review requirements

- Identity: selected project, worker, request and record survive redraw, resize,
  scrolling and changes to neighbouring data.
- Input: each semantic control is reachable once in a complete Tab/Shift-Tab
  cycle; pointer hitboxes may have multiple physical regions. Enter and click
  perform the same action. No input operates on an unseen replacement request.
- Text: no native label masks an actor, hides its action, or relies solely on
  colour. Focus and status remain separate in monochrome. Truncation is explicit.
- Scope: labels distinguish floor, selected team/desks, displayed people and
  actual pages. A different team is not another page of an unrelated roster.
- Observation: offline/stale data never increases a current-working count. A
  quiet turn, automatic review and a human request have distinct labels.
- Space: 32×14 is a functional emergency layout. Normal layouts retain clear
  next actions, with overflow discoverable rather than silently omitted.
- Preferences: names, theme and motion visibly affect the intended surface;
  names off preserves selected and request identities. Compatibility options
  are identified as such rather than presented as equivalent modern views.
- Recovery: each empty/error/loading state explains its cause and next step;
  closing overlays returns to the selected context without submitting work.

The executable evidence and the final reviewer verdict are recorded separately
from this inventory contract. An untested entry is never counted as passing.

## Evidence and corrections

The [local gallery](evidence/inventory/index.html) contains the full captured
matrix and per-image verdict. [Generation metadata](evidence/inventory-generation.json)
records the release exporter hash, render geometry and exact command. The
[earlier findings](evidence/before/before.json) preserve six concrete failures
from the audit rather than replacing their evidence with the corrected frames.
The [legacy findings](evidence/legacy-before/before.json) also preserve the
overlapping Top title, low-contrast Isometric plates and illegible character-only
projection before their bounded corrections.

Visual inspection found and corrected native labels that inherited dark artwork
backgrounds in the light theme, the Phone's contradictory explanation for an
explicit request, its unusable narrow layout and the inspector's former empty
emergency panel. It also found an audit error: the Request route opened a
composer after a shortcut changed. The fixture now clicks the exact worker's
Review action and requires the `REVIEW REQUEST` heading. Its managed capability
and recorded human wait agree with each other; fixture data is never passed off
as a live provider observation.

The single-font Menlo reconstruction cannot certify CJK font fallback. The
long-title fixture retains its Unicode data and bounded cells, but missing-glyph
boxes in this replay remain explicitly `not-tested` for glyph appearance. Sparse
single-floor towers deliberately retain the overview scale and exterior space;
Visit is the route to larger office detail.

## Final recorded result

The final export from source `5ea7f77` contains **360 pass, zero defect and 15
not-tested entries**. All 366 rendered cases passed their independent route and
hit-bound checks. The 15 exceptions are exactly the nine executable-owned Sources
placeholders and the six CJK glyph cases described above. Each remaining case has
an exact-hash visual verdict, including explicitly identified reuse of an
identical PNG. No changed image inherited an old approval.

The final wording correction, “1 project”, changed twelve single-project and
single-worker frames; all twelve were reopened and approved. The complete
workspace check preceded this copy-only correction. The subsequent
[13 interaction acceptance tests](evidence/final-copy-tests.log),
[format check](evidence/final-copy-format.log) and
[release rebuild](evidence/final-copy-build.log) passed. Final executable and
exporter hashes are in the generation metadata above.

The separate [10×20 gallery](evidence/geometry-10x20/index.html) records the
additional geometry checks with the binary actually used. The
[composition report](COMPOSITION.md) documents 27 timing cases and explains why
they cannot certify real-terminal input latency. Executable, platform and
remaining user-validation gates are tracked in [VALIDATION.md](VALIDATION.md).
