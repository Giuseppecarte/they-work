# Work panel, iteration 6

The work panel answers what a task is doing from recorded evidence. It does not infer a goal, progress percentage, or a finished result from an idle status.

## Pilot and critique

The first pilot used three synthetic workers: Avery editing the retry policy, Morgan asking for a timeout decision, and Casey returning documentation. Nine TestBackend buffers covered each worker at 80×24, 40×36 and 64×36. The initial images are in `evidence/panel-pilot/`; the revised set is in `evidence/panel-pilot-after/`. These PNGs replay cell buffers with Menlo, not a physical terminal window.

The review found three avoidable distractions: a current request was followed by “No current activity”; a request without history showed a second empty-state paragraph; and a short record incorrectly suggested scrolling. The revised pilot removes those paragraphs and labels fully visible content “All shown”. Observed verbs and paths, the latest actual result and its author, and the current question fit in the pilot without opening secondary details. A later narrow-panel review found that a long Open conversation action could hide Expand; the action area now measures and wraps every visible action before sizing the reading area, with explicit Task actions and Collapse labels. A 40-column test covers all native capabilities together. The emergency 32-column layout keeps identity, state, coverage, current work or question, and exact Review/Back controls.

## Shared reading model

- Now prioritizes the exact current request, then the latest observed action and object, an actual returned result with its author, and the recorded team.
- Activity exposes every record retained by the core: tool beats with outcomes, delegation, messages, requests and results. Historical requests are labeled as history and never acquire approval buttons.
- Team preserves recorded delegation, forks and session membership. It explains when immediate parentage or a transcript is unavailable.
- Details holds source provenance, project path, branch, tokens and native identifiers. Source warnings remain visible outside the scrolling body.

`work_brief::retained_records` supplies the same stable record keys to the panel and Finder. Notebook deliveries/changes open `InspectRecord(worker, key)` rather than opening an unrelated summary. Finder searches retained file paths, messages and results and opens the matching record; stale or disconnected preview activity is explicitly labeled Last recorded; a title-only match opens Now. Notebook reading markers remain local and never resolve provider requests.

Reading is anchored by record identity and wrapped line. New records do not move an older record out from under the reader. A new-activity counter offers Latest explicitly. Removed records produce an expiry notice rather than silently attributing another record to the old position. This covers retained history only; it cannot recover records that a provider did not expose or that the core has already evicted.

## Instructions and requests

The inline composer reuses the existing ControlPanel draft and provider capability checks. It pins the task recipient, permits expanding with F7 without losing the draft, and exposes Send, Open conversation and Back as applicable. Hidden F2/F4/F6 task actions cannot run while composing. Acknowledgements stay with their task/request instead of appearing on a subsequently opened task. A native PTY review caught the legacy F4 entry path retaining a previous instruction receipt in REVIEW REQUEST; it now uses the same request-opening path, and both a focused regression test and the PTY harness assert the receipt is absent.

Current requests have their own REVIEW REQUEST heading and exact request identity. Normal choices appear once as buttons; long choices retain their full meaning in the scrollable body while the button carries the original response. Expired/replaced requests require deliberate reselection. New Task labels its instruction field and explains why creation is unavailable; its project picker shows the displayed range and total. Only the focused input draws a caret. Connections and New Task cap their heights at 26 and 36 rows, so their controls stay near the form in a tall terminal; requests keep their full reading area. Narrow hints and unavailable-action explanations use complete short phrases.

## Verification

`evidence/work-panel-rollout-tests.log` records focused checks: 16 control, 7 Finder, 19 notebook, 6 inspector and 3 WorkBrief tests passing. The manual pilot exporter is ignored by default and was invoked explicitly into the audit directory. Tests cover exact request replacement, local-only reading markers, unknown and retired actors, source freshness, file/result search, cross-section record keys, all retained history, reading anchors and the compact request view. Earlier failures are retained in the log with the corrected reruns; they included an intentional expired-request fixture and the navigation assertion updated from generic task opening to exact record opening.

The native PTY harness is `review_control_pty.py`. It completed at 120×36 and 80×24 against binary SHA256 `fbef32cc8ed5bf78aef76949d7ec6061569e6442df711f809c91d67a87d73178`. Both runs passed seven interaction flows: literal task creation with one active caret; exact approval with no unrelated instruction receipt; project selection; inline F7 expansion/collapse and one `turn/steer` to the original `managed-2` task; native console return; source path editing/cancel; and mouse-off reopening without replay. Each run exited twice with code 0 and restored terminal modes. New Task, current request and inline composer PNGs were visually reviewed at both sizes.

Evidence is in `evidence/control-pty/results.json`, `evidence/control-pty-80/results.json`, their logs, ANSI streams and replay PNGs. These were actual macOS PTYs with isolated homes and locally generated fake providers. The loopback host required sandbox escalation. No real provider, personal configuration or terminal-window application was used; PNGs replay terminal cells. The output confirms control routing and terminal behavior, not a live provider compatibility claim.

The subsequent display-only delta labels zero/default token data “Token usage unavailable” and shortens the project-picker footer so F7 back stays visible. The token assertion passed in the final focused check; the Details and picker image deltas are reviewed separately. The PTY evidence remains labeled with the exact binary it exercised.

The notebook reserves separate status and age columns. Attention and Team derive age from `SourceCoverage.observed_at`, use an explicit unknown label when missing, and keep relationship text after the state. Deliveries and Changes use event age. Long-title coverage verifies that a fresh wait/metadata event does not manufacture a fresh source observation. Reading hints explicitly identify line positions and narrow request panels retain their PgUp/Dn hint.

The final panel review covers 42 hash-specific images: four tabs at six image sizes, light/native/no-color variants at 80×24, and expanded/actions/results states at 80×24 and 192×58. All 42 were visually inspected and passed within their stated limits; the four 32×14 routes intentionally show the emergency summary. The final capture used an explicit color environment and 8×16 cell geometry. Verdicts in `panel-reviews.json` are reusable only for byte-identical PNGs. These are Ui cell/pixel reconstructions, not physical-terminal transport tests.
