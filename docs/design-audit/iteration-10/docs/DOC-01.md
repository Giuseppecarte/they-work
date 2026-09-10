# DOC-01 closure: verified

The current application navigation, persistence and provider-control contracts
match the executed macOS keyboard routes at 80×24. No application route/control
wording or production correction was required. The new audit probe covers the
coverage reader and canonical-storage recovery route absent from iteration 8.

Final review separately clarified the contributor prerequisite: the release
helper regression requires Python 3.11 or later for standard-library `tomllib`.
The prepared audit interpreter is Python 3.12.14 and already satisfies it. The
manifest records both captured and current `CONTRIBUTING.md` hashes and the exact
documentation delta; this change does not invalidate the application PTY routes.

## Executed routes

The unchanged iteration-8 `check_routes.py` completed all five groups, comprising
eight executable PTY sessions. [Results](routes/results.json) and compressed
action traces retain each input and resulting screen:

| Group | Observed behavior |
|---|---|
| First run | Remember defaults on; editing then Enter retains field context; F5 connects; settings appear only after Connect; the selected project is saved. |
| Navigation and reopening | The saved project returns. Both `c` and `C` open Connections, Local sources opens the chooser, `v` opens Advanced, and Esc returns through Settings. Tab/Shift+Tab focus controls; office PageUp/PageDown changes floors. Design, Character and Attention routes match the documentation. |
| Compatibility shortcuts | Two `wo` sessions apply stable costume/palette changes; `WO` clears their overrides. These shortcuts are distinct from appearance editors. |
| No-save | Source, palette and selected-floor changes leave all four existing settings-file hashes unchanged. |
| Temporary and demo | Remember off creates no settings; reopening asks again. The chooser's `d` starts the demo without creating settings. |

The supplemental [check_coverage_recovery.py](check_coverage_recovery.py) runs
three additional PTY sessions and records [its results](coverage-recovery/results.json):

- Attention, Deliveries, Since your visit and Team all expose coverage through
  `h`, retain a visible Back control, and return through Esc to the selected
  record or empty state. Enter and `r` cannot act on an obscured record; reading
  coverage leaves the settings hashes unchanged.
- A lost canonical state produces a truthful pre-send rejection and a reachable
  Connections error explaining restoration and host restart. Restoring its bytes
  does not resend. Keeping the office open while restarting only its supervisor,
  then selecting Task actions → Reconnect with Tab/Enter, restores the same draft
  and recipient. One `thread/resume`, zero additional `turn/start` or `turn/steer`.
- Closing and reopening the office does not repeat work. Every successful PTY
  session exits normally and restores terminal attributes.

The mouse Back button's equivalence to Esc and the full reading-position
invariant are covered separately by one [executed compositor test](coverage-back.log).
The PTY probe is keyboard-only; it does not claim mouse execution.

## Contract review

`INSTALL.md` remains authoritative for navigation, source choice and persistence;
`docs/CONTROLS.md` remains authoritative for provider capabilities and decisions.
The old project-selection proposal explicitly identifies itself as historical.
The polling implementation traverses sources sequentially in a background
thread, then waits one second. Documentation accurately avoids claiming fresh
observations every second.

The documented distinction between an ordinary save failure and unsafe missing
canonical state survives execution. The former retains a visible draft and
permits an explicit new send in the running host; the latter requires checked
restart. Visiting, marking seen, reviewing and approving remain separate actions.
The routes open login controls without activating an actual login flow.

## Critical review and probe corrections

Seven complete screens were visually inspected: ordinary pre-send rejection;
Connections; Advanced; canonical-loss composer; canonical-loss Connections;
reconnected draft; and Deliveries coverage. The request recipient, explicit
controls, rejection and coverage units remain legible. Other retained captures
support executable assertions and were not individually visually reviewed.

The recovery composer hides its draft behind “Observation only” while controls
are unavailable. The [reconnected screen](coverage-recovery/canonical-reconnected-draft-preserved.png)
proves preservation, but the fault screen does not explain that the draft is
retained. Connections also repeats the recovery reason. Those limitations remain
visible in the evidence and should be part of UX-02's unchanged investigation.
No participant comprehension claim is made.

Initial probe failures are retained, not recast as application failures:

- `coverage-recovery.log`: the first probe expected mixed-case “Coverage”, while
  the rendered heading is uppercase.
- `coverage-recovery-final.log`: the heading correctly scrolls away; the probe
  now identifies the persistent coverage footer and uses Home for a top capture.
- `coverage-recovery-checked.log`: the initial visible-draft assertion failed
  during canonical loss. The probe now reports visibility separately and proves
  preservation through checked reconnection rather than assuming it.
- `coverage-recovery-verified.log`: the probe attempted F6 while composing.
  Secondary shortcuts are deliberately disabled there, as CONTROLS documents.
  The passing route uses the visible Task actions → Reconnect controls instead.

The final [pass log](coverage-recovery-final-pass.log) and all earlier result
objects remain separate. [The manifest](../control/manifest.json) records exact
commands, actual test counts, candidate reuse, source hashes and dependencies.
Replays use Python 3.12.14, Pillow 12.3.0, prepared pyte 0.8.2 / wcwidth 0.8.3 and
the macOS Menlo font; these historical PTY dependencies are separate from the
portable reproduction smoke. Physical terminals, authenticated providers,
Windows routes and participants remain not tested by this report.
