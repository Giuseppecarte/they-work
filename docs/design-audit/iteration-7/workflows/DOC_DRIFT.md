# Documentation versus executable navigation

Read-only audit. `INSTALL.md` was not edited. Claims below refer to the recorded
binary in [REPORT.md](REPORT.md); a destination panel is not proof that every
provider action within it works. These observations come from actual PTY inputs
plus targeted code inspection, not assumptions based on old screenshots.

| Current instruction | Actual destination / behavior | Consequence and bounded correction |
| --- | --- | --- |
| `INSTALL.md:134`: `c` opens source selection. | `c` opens Connections; Local sources is a subsequent control. [Capture](evidence/docs-c-connections.png). | Name the intermediate panel and route. A person looking for source toggles has one more choice than documented. |
| `INSTALL.md:85`: `C` is “for official login.” | `C` also opens Connections. It does not immediately run login. [Capture](evidence/docs-uppercase-c-connections.png). | Describe provider-specific login inside Connections; preserve the distinction between local observation and official controls. No login action was invoked in this audit. |
| `INSTALL.md:134`: `v` changes camera. | `v` opens Advanced / compatibility with Camera selected; further input is needed. [Capture](evidence/docs-v-advanced.png). | Explain `v` → Camera → choose, or point to the visible Settings path. Do not imply a one-key cycle. |
| `INSTALL.md:136`: Tab / Shift+Tab cycles floors. | One Tab from the tower retains the selected floor and advances UI focus. The main UI handles forward/reverse semantic focus. [Capture](evidence/docs-tab-focus.png), [full input trace](evidence/reorientation.trace.json). | Teach Tab as focus navigation; describe the visible floor selector and actual floor-selection commands separately. This affects keyboard learnability, not just wording. |
| `INSTALL.md:87`: `d` explores the demo, adjacent to office controls. | `d` in an office opens Office Design. [Capture](evidence/docs-d-design.png). The source chooser has its own demo action. | Qualify the first-launch context. The audit does **not** conclude that the source chooser's demo shortcut is broken. |
| `INSTALL.md:134–136`: `w/W` character and `o/O` palette shortcuts. | Code retains legacy aliases; current authored customization also has Character and Design surfaces. No alias route was exercised in this bounded walkthrough. | Do not call them dead shortcuts. Prefer current visible surfaces in a later documentation update and verify aliases separately before removing promises. |
| `INSTALL.md:99`: selected floor is saved. | Host code contains `persist_selected_office`; this audit did not test an exit/relaunch at a selected floor. | No mismatch established; keep the claim as unverified by this particular route rather than assuming it is false. |

`docs/design-audit/GOAL.md` describes the original app as read-only. That remains
historical audit context, not a current product capability contract. The current
product distinguishes local observation from optional provider controls. Do not
rewrite the historical goal to erase that evolution; current installation and
task-control instructions should state it consistently.

Suggested validation for a documentation-only follow-up: a fresh reader follows
each route at 80×24 with keyboard only, reaches the named destination, returns
without an unintended action and explains which data is local. Capture the
action sequence. A search-and-replace of key names alone would miss modal
context: in this walkthrough Esc from Advanced first returned to Settings, so a
subsequent shortcut was swallowed until the overlay was actually closed. The
final trace explicitly resets context between independent shortcut checks.
