# Before-publication design and usability review

The user requested another full design/usability iteration, with freedom to
improve the experience and a self-critical implementation/verification loop.
Baseline: `bfde837`, clean `audit/design-and-usability` branch. The previous
iteration's passing checks are context, not acceptance of this new work.

## Requirements

1. **Understand the company.** The tower must clearly communicate independent
   project floors, selected floor, staffing and attention. Replace misleading
   CCTV decoration with a readable hierarchy and a coherent furnished scene.
   Verify one, six and twenty floors, long/duplicate project names and mixed states.
2. **Reach the right work quickly.** A visible global finder must locate projects
   and conversations by useful names/context without memorizing floor numbers.
   Selection must resolve by identity through live changes, resize and pagination.
   Typing must not invoke unrelated keyboard shortcuts.
3. **Inspect and act with confidence.** Prioritize task title, real status,
   request/current work and the next step over internal metadata. Keep history
   distinct from current state. Make history position and time zone explicit;
   page keys and the phone must lead to the correct conversation.
4. **Connect once, understand the choice.** First-run copy must explain local
   access, folder selection and source status. Correcting a path must be usable;
   errors must focus the failing source. Remembering choices must be visible and
   optional without requiring the user to learn a special configuration flag.
   Demo and noninteractive reads must not create preferences or silently grant
   source access.
5. **Preserve the experience on resize and across presentation modes.** Exercise
   80×24, ordinary wide windows and tall windows, light/dark and color limitations.
   Overlays and text must stay readable when graphics are enabled. Preserve the
   previous character animation and terminal-restoration behavior.
6. **Challenge the finished artifact.** Inspect native PTY captures and actual
   font replays, record rejected intermediate work, execute complete workspace
   tests, strict Clippy and formatting, and make reviewable commits. Document
   any environment that cannot be visually inspected. Do not publish or merge.

## Method and limits

Each area starts with observed problems, implements a concrete proposal, then
reviews the rendered result and interaction sequence against those problems.
Focused regressions must exercise behavior rather than duplicate implementation.
Visual snapshots are refreshed only after inspection. Scratch data and fixtures
stay in `docs/design-audit/tmp/iteration-3`; only synthetic conversation content
is used in exported evidence.

Terminal.app access was denied in the earlier computer-use session. Continue
using real PTYs and explicitly labelled CoreText font replays; do not call them
native-terminal screenshots or use a different API to bypass that denial.
