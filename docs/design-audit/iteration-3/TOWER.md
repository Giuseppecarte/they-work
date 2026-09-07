# Tower composition and inspection, iteration 3

The previous tower hid the project hierarchy inside a camera wall. A large
terminal showed a tiny floating isometric island, workers standing on desks,
and large empty panels. `REC` and a monotonic timestamp looked like surveillance
controls without answering which project needed attention. These defects were
visible in the [80-column](../iteration-2/evidence/review3/surface-tower-80x24.png)
and [192-column](../iteration-2/evidence/review3/surface-tower-192x58.png) PTY
captures before changing the implementation.

## Implemented composition

The tower now keeps a numbered directory of projects alongside a focused floor.
Each separated band is one independent project; the selected band has an arrow,
stronger background, and matching number in the room title. Large terminals put
the directory on the left. At 80 columns the directory sits above the room.
Project names remain in canonical path order: a worker needing attention cannot
move the project or change its number. Page navigation uses the actual visible
directory capacity, and the selected identity survives resizing.

The focused floor is a furnished side cutaway with a shared wall and floor,
windows, chairs, monitors, complete desks, cups, and recognizable workers.
Geometry uses native sprite resolutions and integer scaling. The selected room
uses that project's saved palette and the workers' saved characters. It shows
up to five representative workers, prioritizing attention without changing the
stored worker order. The team count always describes the complete project;
`+N more` explicitly identifies conversations omitted from the small roster.
Enter opens the full floor where every conversation can be inspected.

Working, idle, help, and failure counts appear at the tower level and beside each
project. Help and failure counts come first when a short row must truncate.
The roster displays task titles and distinguishes `Latest`, `Last update`, and
`Reason`. It shares the inspector's status explanation: inferred silence says
that no approval was identified, instead of displaying an old message as the
reason for attention. Project paths retain their distinguishing end when short.
`/ find`, `! attention`, Enter, paging, and source connection keys are visible.

## Critique and corrections after execution

- The first new composition still stretched one-floor panels to the full
  terminal height. Panel heights now follow the real rows and roster. The final
  page of a twenty-floor directory no longer reserves seventeen empty slots.
- Narrow row summaries moved horizontally with project-name length. They now
  share a fixed column; priority alerts survive truncation. Wide summaries use
  an ellipsis when the complete counts do not fit, instead of cutting a word.
- A solitary worker received a very wide, shallow window. Room width, roster
  height, and window proportions are now bounded independently of terminal
  expansion.
- A three-person room at 80 columns could list only one task without saying so.
  Its header now says `1 of 3 listed (+2 more)`, so the three visible characters cannot be mistaken for a five-person team.
- A silent worker could show its last completed message beneath `NEEDS HELP`.
  The tower now shares the inspector's current-state explanation, with explicit
  `Reason:` wording. Recorded messages stay distinguishable as historical data.
- The path was technically inside its border but visually crowded it. A spare
  column now separates the path from the right edge.

## Reproduction and evidence

Run from the repository root on macOS with the local audit toolchain:

```sh
sh docs/design-audit/native-cargo.sh build -p theywork-tui --release
python3 docs/design-audit/iteration-3/review_tower.py --stage tower-final
python3 docs/design-audit/iteration-3/review_tower.py --stage tower-final-solo --floors 1 --workers-per-floor 1
```

The harness launches the actual native binary in a PTY and feeds synthetic local
SQLite conversations through the normal collector. It covers one, six, and
twenty independent projects at 80×24, 120×32, and 192×58, using Home, End, PageUp, and PageDown to
exercise the real navigation after every resize. Resize also asserts that the
selected project is preserved. Page keys are skipped for a one-floor tower. The twenty-floor case has sixty
conversations, including current work, idle tasks, failures, and turns with no
recent activity. A separate case checks one project with one worker. Exit and
terminal-restoration results are saved alongside each set.

These PNGs reconstruct captured ANSI terminal cells with Menlo for text and
explicit block geometry. They are evidence of the emitted layout and colors,
not screenshots of Terminal.app or proof of font behavior on Windows/Linux.
The same limitation applies to the before images. No new claim is made about
an unexecuted native terminal platform or graphics protocol.

Final review images:

- [Twenty floors, 80×24, final page](evidence/tower-final/tower-20-80x24-last.png)
- [Twenty floors, 192×58, final page](evidence/tower-final/tower-20-192x58-last.png)
- [Six floors, 120×32](evidence/tower-final/tower-6-120x32.png)
- [One project, one worker, 192×58](evidence/tower-final-solo/tower-1-192x58.png)

Verification includes canonical numbering through attention changes, one-column
page capacity, final-floor reachability at all three sizes, short-path Unicode
width, narrow alert visibility, omitted team counts, prioritization of attention
representatives, opaque bounded room pixels across all character encodings,
native image marker coordinates, and whole-room reduced-motion freezing.

The final native check passed 68 view tests and strict renderer Clippy; see
[tower-checks.log](evidence/tower-checks.log). The PTY results record successful
navigation, exit status zero, and restored terminal attributes for
[one/six/twenty floors](evidence/tower-final/results.json) and
[the solitary-worker case](evidence/tower-final-solo/results.json).
