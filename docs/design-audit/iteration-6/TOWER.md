# Tower semantics and interaction

The visual office remains a view of recorded projects and conversations. This
iteration fixes mismatches between that view and the controls around it.

## Implemented contracts

- Floor counters use the same observation precedence as the inspector. Known
  unavailable and stale workers are counted separately from current work and
  attention; automatic/team/process waits are labelled as waiting.
- Selection has its own `>` marker. Activity symbols no longer stand in for
  selection: human requests use `!`, follow-up uses `?`, stale observations use
  `~`, unavailable sources use `-`, and recorded errors use `×`.
- Names off hides optional scene aliases. The selected worker and a worker with
  a recorded human question or approval remain named. Native inspection retains
  original task titles and source coverage.
- `TowerLayout.navigation_hint` describes the actual displayed scope: Floor,
  selected Team or Desks, visible People and a page only when that group has more
  people. It does not divide the complete project roster by unrelated room seats.
- Computer, keyboard, chair, stable workstation and moving actor rectangles all
  inspect the same real worker ID. The native plate remains the final hit region
  for that action so semantic keyboard focus can stay on readable text.
- Native signs and plates use the semantic panel background. Light-theme text
  does not inherit a dark sampled artwork colour; label masks keep the same
  geometry and cannot expand over an actor.
- Without image density, every saved office projection uses the functional
  native roster. Isometric, Top and List preferences remain saved for later
  image-capable rendering; selecting the last person or clicking a row still
  opens that exact task. This avoids illegible blocks and lost labels in the
  character-only compatibility path.
- Compact rows and the tall office's Latest list reserve separate space for
  observed state and source-observation age. Long aliases yield space to those
  facts; unknown age remains explicit. The original task title stays on the next
  line, and the row still inspects its original worker ID.
- Empty views distinguish no sources, an initial scan, source errors, a project
  filter with no results and a selected source with no conversations. Their
  visible actions open existing connection/task flows. The filtered message
  explains `--project`; it does not claim a source button removes that boundary.

The host owns semantic focus, toolbar overflow, inspector panels, global health
and the final footer. The artwork owns physical station geometry and animation.
The tower consumes their public contracts instead of duplicating UI state.

## Recorded messages

The Phone is now a static native inbox using the same project context, selection,
buttons and coverage language as other panels. Now and Attention show observations;
Edits and Messages show recorded history. An old edit keeps its original file and
change count even when the worker later asks for approval. Offline and stale
observations cannot become a current attention request, and their activity is
explicitly prefixed `Last recorded`.

An explicit recorded wait uses its shared wait explanation, rather than the old
silence heuristic. Long visible records end with an ellipsis when their complete
text does not fit. The underlying history remains unchanged for inspection.

All four channels have pointer and keyboard targets, including two short rows at
32 columns. Every visible record opens its actual worker identity. The phone does
not answer a request or synthesize a conversation; review remains in the task
panel. Source coverage remains visible alongside the record, including at 32×14.

## Validation scope

Focused tests check observation precedence, monochrome selection with names off,
the empty-state cause/action pairs, real family boundaries, pagination, clipped
hit regions and the existing integer-scale geometry. The new [inventory](INVENTORY.md)
uses actual UI inputs and redraws, and keeps automated, visual and real-terminal
results separate. The image-free compatibility regression passed all 27
combinations of three saved projections, three character encodings and three
viewport sizes. It exercises End, a presented mouse target, and the resulting
inspector identity. The separate image-capable isometric paging regression also
passed. Full-suite and screenshot results are recorded in the final inventory;
unrun cases are not declared successful here.
