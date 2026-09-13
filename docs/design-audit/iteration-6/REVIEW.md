# Iteration 6: make every detail explain the work

The main interaction is now a work brief beside a furnished office: identify the
worker, read the current observation, and choose an action supported by that
connection. Activity, Team and Details preserve deeper evidence without making
technical identifiers the opening screen. Instructions remain attached to their
recipient inside the panel, including during expansion and resizing.

## Four reviewed stages

1. **Foundations.** The [screen inventory](INVENTORY.md) and
   [component ledger](COMPONENTS.md) define the review scope. Semantic focus
   removes duplicate keyboard stops for a character, computer and nameplate.
   Freshness, source failures and real Floor/Team/People scopes are explicit.
   Shared native colors, buttons, headings and opaque backgrounds supply a
   consistent base for forms and the scene.
2. **Complete pilot.** Three workers were reviewed as independent desks and as a
   shared team, at both authored character scales. The [work-panel pilot](WORK-PANEL.md)
   paired an observed edit, a synthetic current request and a recorded result. The
   [workstation pilot](art/ART.md) checked the person, hands, keyboard, monitor,
   chair and desk as one composition before extending the cast.
3. **Rollout.** All twelve characters use complete workstations. The three room
   constructions and four decoration zones remain distinct. Search and trays
   open the same brief or an exact retained record. Settings, Advanced,
   Connections, source selection, new-task forms and the recorded-update inbox
   use the revised navigation and copy. Appearance changes preview locally and
   require Apply; Cancel restores the previous preferences.
4. **Critical review.** Full screenshots, recorded motion, synthetic providers
   in executable PTYs, semantic input tests and the compositor matrix were
   checked separately. Review findings below were corrected and the affected
   checks repeated. [Validation](VALIDATION.md) records the exact tested scope,
   versions, binary hashes and remaining publication gates.

## Defects found after the implementation appeared complete

| Finding | Correction and reason |
|---|---|
| Monitors clipped one ponytail; screen motifs escaped small glass; a headphone band looked disconnected | Complete workstation clipping/occlusion tests cover the twelve silhouettes, both grids, desk variants and shared tables. Screen artwork is clipped; the band joins both cups. |
| The error-text color missed the contrast target | Adjusted semantic text colors and checked built-in light/dark pairings. Targets are 4.5:1 for text and 3:1 for essential boundaries, not a certification claim. |
| Light text styling was applied over a sampled dark image background | Native nameplates and signs now use the theme's panel background. Source/status markers and selected identity remain separate. |
| Opaque overlays inherited bold styling from a previous heading | Reset the underlying cells before applying the new style. The regression checks symbols, background and inherited modifiers. |
| A narrow panel hid Expand when preceding actions used the whole row | Measure and wrap all action rows before allocating reading space. Task actions and Expand/Collapse remain visible. |
| Current requests were followed by redundant “No current activity” text | Prioritize the exact request and omit contradictory empty paragraphs. Historical requests retain their explicit historical label. |
| Opening requests through F4 carried an unrelated send receipt | Route the shortcut through the same request-opening method and clear unrelated notices. A regression and PTY assertion cover the transition. |
| A phone summary described silence despite an explicit wait; a stale Finder preview sounded current | Explicit recorded waits take precedence, stale activity says Last recorded, and clipped text ends with an ellipsis. |
| The old request screenshot route pressed a shortcut disabled while composing | Corrected the fixture to click Review request and assert the exact request heading. Also aligned its worker Wait event and managed capabilities with the simulated request. Earlier screenshots were not evidence of that surface. |
| New-task fields lacked labels, showed two carets, or clipped the unavailable reason | Added field labels, one caret on the focused field, concise reasons and responsive hints. The project picker states the displayed range and total. |
| Short forms placed their actions at the bottom of very tall windows | Bound short form/editor heights while keeping deep request reading available. |
| Advanced was unreachable by mouse at 32 columns; old small cameras overlapped artwork and text | Show all ordinary settings in the compact form. Older cameras use the functional compact roster when space cannot support them, preserving their saved camera choice. |
| More and small forms repeated or truncated contextual guidance | Keep one contextual footer, concise compact hints and a visible return action. |
| Attention, Team, search and native rosters omitted observation age; long identities could obscure state | Reserve identity, state and observation-age space through shared native age formatting. Missing observation time stays unknown. Recorded deliveries retain their own event age; search project counts use the same observation-aware attention rule as the tower. |
| The legacy top camera's project sign overlapped its native alert | Reserve the alert's geometry and omit only an intersecting decorative sign; the native project heading remains. |
| Image-free compatibility cameras painted unreadable block art; old light-theme plates used a dark sampled background | Use the compact native roster without images while retaining camera preferences. Explicit semantic plate backgrounds keep light-theme labels readable. |
| A narrow request hid the reading shortcut; panel scroll counts had no unit | Keep PageUp/PageDown in the compact request footer and name panel scroll positions as lines. |
| The tower's one-project context used plural wording | Use “1 project” for both the single-project and single-worker scenarios; recheck their complete frames. |

## Result and practical limits

The design explains substantially more work at the point of inspection. A
question is visibly different from a stale observation, and a recorded delivery
has an author and evidence rather than an inferred completion claim. Laptops,
monitor stands, keyboards and seated hands make team tables understandable.
Names, clothes and motion remain decorative; they never change provider inputs.

The 32×14 view is deliberately an emergency work brief/roster. Complete work tabs
need more space and resume with their selected context when enlarged. Legacy
cameras retain limited artwork. Very large teams paginate without shrinking the
people indefinitely. Retained history cannot recover data a source did not
expose or records already evicted by its retention limit.

The [local gallery](evidence/inventory/index.html) separates automated route/bounds
results from hash-bound visual verdicts. Compositor PNGs replay native cells and
RGBA; PTY PNGs replay executable ANSI. Neither is a terminal-window screenshot.
The original art sequence records a full outbound/action/return cycle, rather
than presenting a sprite sheet as motion validation.

Built-in palette targets follow W3C guidance for [text contrast](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html)
and [non-text contrast](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html).
These are design targets for the supplied colors; terminal font rendering, user
color overrides and assistive technology behavior require separate validation.

Authenticated provider sessions, physical macOS/Linux/Windows/WSL terminal
recordings, end-to-end input latency and five-person comprehension testing remain
**not tested** publication gates. Local implementation and fixture evidence do
not establish those outcomes. The iteration does not claim the project is ready
for a public release on every platform.
