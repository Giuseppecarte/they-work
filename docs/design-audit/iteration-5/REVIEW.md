# Interactive tower: implementation and critical review

Baseline: `f7a33d6` on `audit/design-and-usability`. This iteration implements the
approved local design scope. It does not certify a public release or real
provider sessions. No real model turn or external message was sent for testing.

## What changed

The tower has complete, contiguous floors at an overview scale. Entering a
project opens its detailed office; the selected project and conversation remain
bound to their real IDs. Overview characters are authored at 24×32 and office
characters at 48×64. The main detail scale is fixed at 2×: widening a window
makes room for more desks and decoration instead of magnifying the architecture.
The overview width is bounded in very wide windows. Detailed offices are capped
at 512 physical pixels high; taller windows show a native summary of actual floor
activity beneath them. Zoom is an immediate,
interruptible view change without fractional interpolation or queued transitions.

Three presets supply distinct materials and four configurable zones. Conversation
profiles contain a fictional name, outfit and animation style; defaults derive
from stable identity, and edits are local preferences. Existing wardrobe and
palette selections continue to load. Neither these choices nor ambient scenes
modify provider instructions, task state, relationships or transcripts.

The native toolbar exposes Tower, Attention, Deliveries, Find, New task and
Connections. Characters, plates, floor controls, team selectors and the toolbar
have targets shared with keyboard activation. Tab focuses controls; arrows
navigate items; Enter activates; Escape returns. PageUp/PageDown move between
floors when the scene has focus. The inspector occupies 40 columns when the
remaining scene can keep 2× figures, and otherwise uses the screen.

Attention starts across all projects, with a visible floor filter and separate
human-response and follow-up sections. Recorded team events use plain sentences,
with original text retained and technical provenance disclosed separately.
Incomplete, stale and unavailable observations remain explicit. A worker with a
pending request opens its inspector and then the exact request, without answering
it. Control availability comes from the verified connection, including reasons
when observation is the only available capability.

Mouse capture defaults on, can be changed in Settings or with `--mouse=off`, and
is released for an official console and on exit. The sources chooser also has
mouse controls. A source selection is applied by its Connect button. Pointer
coordinates resolve against the last successfully presented frame; resize
invalidates that map. Sixel pacing does not acknowledge a skipped frame, and
user input or changed available actions bypass animation pacing.

## Critique and corrections made during implementation

1. **The first detailed office still enlarged everything with the window.**
   Its side inspector left native plates far below their characters. Fixed the
   detail scale and placed plates immediately beneath the desks. Extra height
   provides circulation and rest space rather than taller windows.
2. **Five wide floors looked like strips of windows.** Bounded overview width,
   centered the building and retained an exterior margin. This preserves the
   overview sprites and floor counts rather than silently switching scales.
3. **The first native floor headers disappeared behind their image layer.**
   Corrected the native mask before accepting the screenshot. Visible header
   geometry and pointer targets are checked together. A later pixel review also
   caught labels masking the feet: plates now align down to whole text rows while
   actor hitboxes still cover their full bounds.
4. **The last page changed navigation capacity.** Visible occupants and maximum
   seat capacity are now separate; an incomplete final page cannot alter the
   size of a navigation step. Large families retain the responsible agent.
5. **Default names were disambiguated by roster position.** Removed that ordinal.
   Default name and initial now derive from identity alone; explicit names are
   respected. Identical chosen aliases still lead to distinct real task IDs and
   real titles in the inspector and search.
6. **A narrow inspector could hide Review request or scroll away coverage.**
   Reserved visible space for the current observation and first action, including
   a 40×12 panel with a two-line title. Unavailable observations do not retain an
   unqualified Working/Ready label.
7. **The project picker could confuse identical project names.** It now retains
   and displays the selected full path and deduplicates by path.
8. **A replacement approval must not inherit an old button click.** Operational
   hit targets carry exact request IDs and responses; current request choices,
   readability and capabilities are checked again before dispatch. Seen/reviewed
   markers remain local bookkeeping.
9. **The native fallback initially had no pointer targets.** Added a compact view
   with the same real identities, family boundaries, states, coverage and bounded
   pagination. The compatible isometric and top views remain available.
10. **The first request view repeated global and local footers.** Removed the
    duplicate global footer and gave that space to the request/composer panel.
11. **A human question could retain the previous editing pose.** The raised-hand
    pose now follows the explicit human wait reason even if the last activity is
    Editing. Failures retain priority; automatic review, child waits and process
    waits never create a human-request gesture.

## Evidence and how to interpret it

- [Artwork and complete sequences](art/ART.md): authored resources, pilot critique,
  integer scales, presets, movement destinations and return paths.
- [Tower and navigation](TOWER.md): 1, 6 and 20-project layouts, up to 50 tasks,
  independent families, offsets and bounded pagination.
- [Inspector and controls](UX.md): shared observation language, exact request
  identity, global attention, original event text and project selection.
- [Interactive UI captures](ui/ui-manifest.json): actual `Ui`/`TestBackend`
  native cells and physical RGBA composited with Menlo. These are compositor
  reconstructions, not terminal-window screenshots.
- [Control PTY evidence](evidence/control-pty/results.json): the compiled binary
  running under a real PTY with simulated providers. It checks input, literal
  project/prompt handling, one scoped reply, native console handoff, mouse
  restoration, exit and reopening without replay. It does not authenticate to
  Codex or Claude.

Reproduction commands and final measured results are recorded in
[VALIDATION.md](VALIDATION.md). Rendering latency excludes terminal transport,
terminal image parsing and display refresh; it must not be presented as measured
end-to-end input latency on Windows Terminal, Kitty or iTerm2.

## Remaining publication gates

Actual terminal-window captures and complete sequences on macOS, Linux, Windows
and WSL still need their terminal versions recorded. Authenticated provider
sessions need direct validation. Five new users have not yet performed the
10-second attention and team-comprehension tasks. The current fixtures and
compositor checks do not establish those results. These gates were explicitly
outside this implementation stage and remain prerequisites for publication.
