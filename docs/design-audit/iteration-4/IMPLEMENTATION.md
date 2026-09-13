# Iteration 4 — teams, control and a living tower

This iteration implements the accepted plan for a graphics-first office, real
delegation relationships, explicit task controls and local review markers.
The baseline is commit `41101df` on `audit/design-and-usability`.

## Product decisions

- Projects remain independent floors. Independent conversations retain desks;
  confirmed delegation families can occupy project meeting rooms.
- Recorded state is authoritative. Expressive decorative animation cannot
  create messages, approvals, completion or collaboration evidence.
- Codex uses a managed app-server connection for integrated controls. External
  transcripts do not confer control over their active process.
- Claude uses the unchanged native client and its existing login. Talking and
  approving transfer the terminal to the official console and then return.
- Character-only terminals retain an accessible compact view. Graphical scenes
  target Kitty, iTerm2 and Sixel with protocol-specific pacing.
- Reading/review markers are local preferences; acknowledging an alert never
  resolves the provider request.
- Closing the viewer does not cancel managed work. Restarting a host must not
  replay a prompt or resume generation automatically.

## Implemented areas

| Area | Evidence report | Acceptance |
| --- | --- | --- |
| Identities, relationships, collectors, demo | DATA.md | Fixture coverage, graph correctness, source boundaries |
| Managed runtime, native client handoff | CONTROL.md | Fake-provider integration, no duplicate or stale commands |
| Original sprites, room composition, animation | ART.md | Rendered scenes, persistent identity, performance |
| Notebook, controls UI, host integration | REVIEW.md | End-to-end paths and input/output inspection |

Changes are committed in reviewable pieces. [The integration review](REVIEW.md)
records reproduced defects, corrections, final verification and acceptance limits.
An implementation is not by itself a passing real-provider or real-terminal test.

## External acceptance still required

Native GUI recordings on each advertised terminal and evaluation with five new
users require those environments and participants. Synthetic PTYs and image
reconstruction are complementary evidence, not substitutes. This run must not
claim those checks passed unless they actually took place.
