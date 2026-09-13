# Native text above terminal images

## Reproduction and cause

The host marked every cell in the canvas rectangle as skipped, then transmitted
the image after Ratatui's text output. Names drawn over the room, Help, and the
new finder were consequently absent from the native layer. Sixel and inline
image erasure could also remove text already drawn in a previous image's area.
The ordinary character snapshots did not exercise this presentation order.

## Correction

The canvas temporarily marks its image cells while a frame is composed. Opaque
native widgets clear that mask, including their blank background cells. The UI
captures this native layer after theme conversion and removes every temporary
skip marker before returning. Only the last canvas gets image presentation;
earlier portraits remain ordinary character cells. Each UI frame clears the
previous canvas destination, so a tiny/empty view cannot resurrect stale art.

The host clears the previous image and its text, draws the replacement image,
then paints native text and restores the input cursor. Native panel backgrounds
are also filled into the image; the terminal retains responsibility for font
glyphs. Wide-character continuation cells are skipped during native repaint.
Unchanged images are retained, and Sixel pacing yields immediately when native
overlay contents change. The native layer is repainted after image operations
because image erasure may affect any cells in the former image rectangle.

Kitty uses `z=-1`, as its [official graphics protocol](https://github.com/kovidgoyal/kitty/blob/master/docs/graphics-protocol.rst)
specifies negative placement indices for images below text. This is distinct
from negative animation-frame timing values in the same protocol.

## Verification and limits

- A host test composes Help with Kitty, Sixel and iTerm2, verifies that native
  `HELP` bytes follow the image payload, checks cursor restoration, and confirms
  that an unchanged frame does not retransmit image data.
- An independent renderer integration test visits Settings, Phone and Finder,
  then returns to the office in dark/light mode. It verifies every native letter
  in the image area, opaque background pixels, and restoration of the room mask.
- A separate regression covers nameplates, opaque Help blanks, and tiny views
  with no current image rectangle.
- The [native PTY harness](review_graphics_pty.py) runs the compiled release's
  actual loop with `--demo --no-save`, answers its capability probe with Kitty
  direct support and 8×16 cells, then opens Help and Finder. The
  [recorded run](evidence/graphics-pty/results.json) passed: each heading followed
  its image transmission, remained in four subsequent native frames, and the
  process exited successfully with terminal modes restored. Results include the
  binary and transcript hashes. Reproduce with
  `python3 docs/design-audit/iteration-3/review_graphics_pty.py` after a native
  release build; large raw ANSI payloads remain under ignored audit scratch.

These checks validate composed buffers and encoded bytes. They do not establish
how a particular physical terminal emulator renders graphics. Terminal.app
access was declined earlier; no alternate API was used to bypass that decision.
Windows Terminal/WSL, Linux graphical terminals, Kitty and iTerm2 visual sessions
remain unmeasured. The native PTY protocol exercise uses a controlled capability
responder; it is not a real Kitty window or a visual acceptance test.
