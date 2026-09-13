# Motion audit

The host was already drawing at roughly 10 frames per second, and new renderer
preferences already enabled motion. The authorized read of
`~/.config/they-work/appearance.json` returned `ENOENT`; no user preference was
changed. The frozen appearance had a concrete rendering cause: compact and mini
Idle animations contained identical pixels. Several other small activity poses
also changed little or nothing. The old full Idle pose changed a tiny decoration
beside the head rather than moving the person.

Each activity now has eight cached animation frames on each authored character grid: full
24×34, compact 14×20, and mini 7×10. Shoulders, hands, eyes and head position move
on those grids. Idle people breathe, blink and look around; they do not type or
acquire an active turn. Reading, searching, contemplation, conversation, approval
and failure retain their own gestures and timing. Idle walks have alternating
feet and arms, with their path and stationary intervals controlled by the room.
Neighboring thread IDs receive different animation phases so workers do not all
move together.

The pose follows `status_at(real_time)`. A silent open turn uses contemplation;
it never gets an Idle pose merely because the most recent tool activity expired.
A completed turn remains Idle, even while its character moves. The presentation
clock is separate: reduced motion fixes character and ambient poses at time
zero, while real source events and status transitions continue to update.

Verification is deliberately stricter than comparing two timestamps. The sprite
suite requires at least four different body poses for every activity, both
provider palettes and all three native grids. It also sends actual worker frames
through sextants, quadrants and half blocks in true color, 256 colors and
monochrome; those terminal cells must change over time, and must remain identical
when reduced motion is enabled. Worker status is asserted independently.

`check_motion.py` runs the compiled application in real macOS PTYs against
synthetic local conversation databases. Its 80×24 through 192×58 captures sample
Idle, active command execution and an open turn without tool use. It measures
the worker shirt colors separately from the whole room, excluding UI clocks.
For the Apple Terminal environment it unsets `COLORTERM`, sets
`TERM_PROGRAM=Apple_Terminal` and confirms indexed ANSI output. An independent
exhaustive search of the fixed xterm cube and gray ramp identifies the expected
shirt colors. The mask then requires nearby face or hand skin; blue windows can
share a shirt's 256-color index and must not count as a moving person.
At least two distinct clothing masks and three room frames are required when
motion is on; the entire room must remain still when motion is off. A successful
exit must also restore terminal settings. Its PNGs and GIFs reconstruct emitted
ANSI cells; they do not claim native font rasterization or Windows/Linux visual
execution.

Self-review caught two flaws in the first expanded test run. At 80 columns the
valid status phrase ends at “all desks”; testing its missing “live” suffix was a
false failure. The harness now checks the visible Idle/Running status and saves
the observed header. Its exit check also keeps draining ANSI while waiting,
because a full PTY output buffer can otherwise block the final screen cleanup.
Only the affected small cases were repeated. The 256-color runs were repeated
with the stricter skin-adjacent mask. These fixes changed audit code only.

Reproduce with the repository-local native toolchain and the existing audit
Python dependencies (`pyte` and Pillow):

```sh
sh docs/design-audit/native-cargo.sh test -p theywork-render --lib sprite::tests
sh docs/design-audit/native-cargo.sh build --release -p theywork-tui
python3 docs/design-audit/iteration-2/check_motion.py
python3 docs/design-audit/iteration-2/check_motion.py --stage motion-small --sizes 80x24 120x32
python3 docs/design-audit/iteration-2/check_motion.py --stage motion-apple256 --environment apple256 --sizes 80x24 192x58
python3 docs/design-audit/iteration-2/check_motion.py --stage motion-apple256-views --environment apple256 --sizes 192x58 --states idle --encodings half-blocks quadrants --views top side
```

The sprite suite passed 15 tests; see [sprite-tests.log](evidence/sprite-tests.log).
The renderer also passed [strict Clippy](evidence/render-clippy.log). Full workspace
validation is recorded separately in the iteration review.

All **116 PTY scenarios passed**, representing 1,392 sampled frames and 62
recorded successful exits with terminal settings restored. Every motion-on case
changed the character mask; all 58 motion-off cases produced exactly one room
frame. All source-state assertions passed. See the machine-readable
[summary](evidence/motion-summary.json).

| Environment and camera | Sizes | Scenarios | Distinct clothing frames with motion |
| --- | --- | ---: | ---: |
| True color, isometric | 160×48, 192×58 | [36](evidence/motion/results.json) | 3–12 |
| True color, isometric | 80×24, 120×32 | [36](evidence/motion-small/results.json) | 6–12 |
| Apple Terminal 256 colors, isometric | 80×24, 192×58 | [36](evidence/motion-apple256/results.json) | 6–12 |
| Apple Terminal 256 colors, top and side | 192×58 | [8](evidence/motion-apple256-views/results.json) | 12 |

The isometric groups cover all three encodings and Idle, command execution and
quiet open turns, each with motion on and off. The top/side checks cover Idle
with half blocks and quadrants in both motion settings. This adds the small,
palette and camera boundaries without repeating every camera combination.

Compare the native 256-color [Idle animation](evidence/motion-apple256/idle-quadrants-192x58-on.gif)
with its [reduced-motion capture](evidence/motion-apple256/idle-quadrants-192x58-off.gif),
or a [quiet open turn](evidence/motion-apple256/quiet-quadrants-192x58-on.gif).
The [top view](evidence/motion-apple256-views/idle-quadrants-top-192x58-on.gif)
and [side view](evidence/motion-apple256-views/idle-quadrants-side-192x58-on.gif)
also retain visible character motion.
