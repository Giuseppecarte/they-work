# Global finder execution review

The finder was exercised against twenty independent projects and sixty local
conversations through the native binary. The fixtures use their own configuration
and SQLite sources under `docs/design-audit/tmp/iteration-3-finder`; no real
conversation records are read or changed.

## Executed behavior

- `/` opens the project directory. Cancelling preserves the eighth floor that
  was selected before search. Ctrl+K also opens the same finder.
- Typing `qcs?` stays inside the query. It does not quit, connect sources, open
  settings, or show help. A no-results explanation remains visible.
- Searching `codex` returns sixty conversations. PageDown and PageUp move
  through results and restore the first page.
- A bracketed paste of `20-billing-service account` returns one conversation.
  Enter opens `Build account settings` in project `20-billing-service`, verified
  by the inspector's `Thread: person-57` context. The silent-turn explanation
  belongs to that target. Escape from another finder preserves this desk.
- Home, End, Left, Right, Delete, and Backspace edit the query. A long Unicode
  paste scrolls horizontally and keeps the cursor inside the 80-column window.
- At 28×10 the finder asks for more space; Enter cannot open the valid result
  hidden by this fallback.
  Resize back to 80×24 preserves the query and restores a usable result area.
- At 80×24, 120×32, and 110×80 the exact result retains its title, project,
  provider, state, selected row, path context, and next-step keys.
- The light appearance was selected through Settings, then the finder searched
  attention. It returned ten conversations: five failed and five silent.
- Normal exit returns zero and restores the PTY's terminal attributes.

## Visual critique

The result title and project context are separated clearly. A project badge
cannot be confused with a conversation state; the selected row has an arrow and
background. Search keeps the same dimensions while typing, so an exact match
leaves empty space. This is an intentional stability tradeoff; it did not hide
the selection or next action at any tested size.

The first light-mode review exposed a concrete contrast defect. The title and
key hints used cyan `#58d6e8` on beige `#e6dfd0`, a relative luminance contrast
ratio of 1.30:1. Selected project context used `#8a8299` on `#cbc0aa` at 2.03:1;
the attention badge was 2.22:1 on the panel. Actual Menlo/CoreText replay also
looked washed out, confirming this was not caused by the block rasterizer.
The light text palette was corrected and then measured from a new native PTY
capture. The [initial measurements](evidence/finder-first/contrast.json) and
[final measurements](evidence/finder-final/contrast.json) are recorded alongside
the images.

| Text role | Before | After |
| --- | ---: | ---: |
| Title and key hints | 1.30:1 | 6.50:1 |
| Selected project context | 2.03:1 | 4.56:1 |
| Attention badge | 2.22:1 | 6.46:1 |
| Selected failure badge | 3.30:1 | 4.89:1 |

The corrected palette was visually checked with Menlo/CoreText as well as the
ordinary cell replay. Two smaller defects were also fixed and recaptured:
`1 match` now uses the singular, and the footer announces `PgUp/Dn page`.
A final independent review also found dark terminal-default cells below the
light directory, outside the panels. The whole view now paints its background
before drawing components. The final PTY run asserts that the exposed area has
the same light background as the header, and the full Menlo frame was reviewed
again. This defect had been missed while concentrating on the overlay text.

The final finder run passed every behavior assertion, all five sampled
light-text contrast checks, and the exposed-background check; see
[results.json](evidence/finder-final/results.json).

## Reproduction

```sh
sh docs/design-audit/native-cargo.sh build -p theywork-tui --release
python3 docs/design-audit/iteration-3/review_finder.py --stage finder-final --font
```

`review_finder.py` sends keys, resize events, and bracketed paste through a real
PTY. Its assertions use the emitted terminal state. PNGs are replays of those
cells, not screenshots of a terminal application. The `-menlo.png` images replay
captured cells through installed Menlo and CoreText, including real fallback
fonts; their `.font.json` files record the fonts and unsupported symbols.
The simpler pixel replay uses Menlo without font fallback, so its CJK query
image can show missing-glyph boxes even though the underlying query and cursor
are correct. The shared CoreText helper also clips double-width glyphs to one
cell; it was used only for the ASCII finder scenes. Unicode behavior is verified
by emitted text and cursor state, not by claiming font fidelity from that replay.
Windows/Linux native windows and graphics-protocol terminals were not exercised
by this finder subtask.


Reviewed evidence:

- [Initial light contrast, Menlo](evidence/finder-first/finder-light-120x32-menlo.png)
- [Final light contrast, Menlo](evidence/finder-final/finder-light-120x32-menlo.png)
- [Exact result at 80 columns, Menlo](evidence/finder-final/finder-exact-80x24-menlo.png)
- [Exact result in a tall window](evidence/finder-final/finder-exact-110x80.png)
- [Correct conversation opened](evidence/finder-final/finder-opened-desk-120x32.png)
- [Provider search after PageDown](evidence/finder-final/finder-provider-page-80x24.png)
- [Tiny terminal fallback](evidence/finder-final/finder-tiny-28x10.png)
