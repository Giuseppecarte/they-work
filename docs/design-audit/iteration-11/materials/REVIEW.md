# Material pilot and rollout

The office now separates warm surfaces, colored upholstery, silver supports and dark screen glass. This is a material pass on the existing authored geometry; faces, costumes, gestures, status motifs and interaction anchors retain their previous definitions.

## Recipes

| Preset | Dark wall / light wall | Desktop | Distinguishing construction and accent |
| --- | --- | --- | --- |
| Studio | `#9b7462` / `#f0d2b4` | Honey `#dcaa67` | Existing glazed windows and wood shelves; turquoise seats and coral supplies in palette 1. |
| Workshop | `#8b7d74` / `#e6d7c3` | Warm plywood `#c2a47a` | Existing pegboard and drawer option; plum storage and teal equipment. |
| Laboratory | `#658a9c` / `#d5eaf0` | Mint laminate `#c1dcd2` | Existing clean panels and benches; silver fittings and lilac seats in palette 1. |

Saved zero-based palette indices remain 0–3. The accent families are teal `#36a8ab`, blue `#5d97d4`, green `#84a557` and violet `#a27fc5`. Each changes architectural trim, chairs, desk edges, computer-case details and props. Walls and flooring depend on preset and light mode so large surfaces stay quiet. Laboratory seats use lilac, blue, mint and rose to remain distinct against its cool construction.

The internal `materials` recipe is shared by rooms, chairs and complete computers. Bezels are pale in both modes, stands are metallic blue-gray, screen glass and its observed-activity motifs are unchanged. A colored case edge is outside the screen. No decorative material changes current state, request authority or selection.

## Pilot and evidence

The existing `workstation_pilot` was captured from baseline `282722294b8b372d7e20bda4a3705f3e19b62b1b` before editing. It rendered three people, both individual desks and shared laptops, all three workstation variants, and both authored scales: twelve frames. The initial environment inherited `NO_COLOR`; those temporary outputs were replaced with explicitly normalized truecolor captures before comparison.

```sh
env -u NO_COLOR TERM=xterm-256color COLORTERM=truecolor \
  sh docs/design-audit/native-cargo.sh run -p theywork-render \
  --example workstation_pilot -- target/audit/iteration-11/materials-before
```

The same command with `materials-pilot` produced the first updated pilot. Individual and shared 640×304 scenes were visually compared before the full cast gallery was reviewed. Both `geometry.txt` files match byte for byte, including every recorded computer, keyboard, chair, home and selection rectangle. The retained PNGs show the whole authored room, but deliberately contain no native terminal labels; the separate application inventory supplies full UI evidence.

After the pilot, `workstation_gallery` generated all twelve people in both scales and seating arrangements, preset/light specimens and the existing decorative sequence. Four full cast specimens and the Studio light, Workshop dark, Laboratory dark/light sheets were visually inspected. The final Lab revision makes default upholstery clearly lilac and slightly darkens glazing. The full decorative sequence was generated but not frame-by-frame reviewed in this material subtask.

## Tests

- Existing workstation screen clipping, connected supports, face/raised-hand occlusion and keyboard reach checks: 3 passed on the initial pilot.
- Focused `living_office` suite: **31 passed**, including the two new tests. This suite predates only the final Lab color adjustment. The unrelated integration binaries selected zero tests and do not count as passing cases here.
- Final exact palette test after the Lab adjustment: **1 executed, 1 passed**. It checks 72 preset/light/scale/arrangement/variant groups, each with four palettes: 288 workstation compositions. Every group has four distinct chair, computer and desk-edge crops, identical occupied-pixel masks and unchanged hit anchors.
- The new whole-scene test checks 96 combinations across all presets, palettes, light modes, scales and seating arrangements. Worker IDs, costumes, actor/nameplate/workstation geometry, room signs, elevators and capacity match the baseline combination.

Raw logs and source/evidence hashes are retained beside this document. Final application-wide tests belong to the integrated validation report; no timing or physical-terminal result is inferred from these compositor tests.

## Critique

The strongest improvement is immediate material separation: people no longer share a gray-green field with chairs, desks and monitors. Computer bezels, dark glass and connected silver supports remain legible; laptops show a distinct colored lid and keyboard deck. Colored chair backs remain visible around the seated bodies without covering faces. Studio's muted terracotta wall keeps skin and clothing brighter, while the light version reads as apricot.

The three presets keep their structural identity rather than becoming uniform recolors. Narrow tower rooms necessarily show less upholstery than the office view, and the preview still uses the existing large native nameplate budget. Pale costumes lose some silhouette contrast against the light wall; faces and raised hands remain readable in reviewed specimens and pass the existing occlusion checks. This pass does not certify visual comprehension or VS Code/WSL rendering: those need the user's actual terminal session.
