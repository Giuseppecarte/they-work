# Component review ledger

This is the component-level companion to the [screen inventory](INVENTORY.md).
`pass` means the stated local contract was inspected or tested, within the linked
evidence. It does not mean terminal accessibility certification or user testing.
The final screen gallery separately records hash-bound visual verdicts. A route
or bounds check alone is not a visual pass.

| Component and states | Contract reviewed | Local verdict and evidence |
|---|---|---|
| Global navigation, full and overflow | Complete names; one navigation row; More exposes hidden destinations; no renaming Attention | Pass: inventory Tower/More across six sizes; semantic focus regression |
| Context and footer | Selected project/work surface; current source failure; one contextual footer; visible return | Pass: source-error, Help, Settings and compact inventory cases |
| Buttons and focus | Native labels and brackets; Unicode width; one semantic focus stop per action; multiple physical click regions | Pass: shared component, complete redraw/Tab cycles, workstation click and resize regressions |
| Text, notices and opaque panels | Contrast targets; prior glyphs/modifiers cannot leak through; warnings remain readable | Pass: built-in palette contrast and opaque-cell reset regressions; light/native inventory |
| Tower floor and team selectors | Stable floor, family and person identity; each pager operates on its visible collection | Pass: nested/family/last-person tests and directed final-floor captures |
| Nameplates and selection | Selected worker stays identified with names hidden; status differs from selection; light background remains legible | Pass after correction: light-plate regression, names-off and monochrome captures |
| Work brief header and Now | Real title, verb/object, observed age, source coverage, exact request before routine activity, result author | Pass: three-worker panel pilot and integrated Now captures |
| Activity and reading state | All retained records, results filter, record anchor across redraw/resize/new input, explicit return to latest | Pass: WorkBrief/Inspector tests; results and Activity captures |
| Team and provenance | Confirmed parents, session membership and forks distinguished; missing participants are explicit | Pass: retained relationship fixtures and Team captures |
| Details and capabilities | Technical data behind Details; unavailable controls explain their limitation | Pass: Details, disconnected and external-task fixtures |
| Panel sizing and actions | Side panel 40–64 columns only with readable scene geometry; expansion retains task/section/draft; wrapped actions remain visible | Pass after correction: 40-column Open/Expand/Stop/Reconnect regression and inline PTYs |
| Emergency work brief | At 32×14 retains identity, coverage, work/request and safe next action; fuller tabs resume on enlargement | Pass for this explicit compact behavior; complete tabs are unavailable at this size |
| Instruction field and receipts | Fixed recipient; explicit send; Enter newline; F7 preserves draft; uncertain sends not repeated | Pass: control fixtures and executable simulated-provider PTYs |
| Exact request view | One set of current choices; historical/replaced requests cannot be answered; inspection never approves | Pass: exact request, resize and replacement regressions; executable approval fixture |
| Attention, deliveries and changes | Observation-aware categories; local seen/reviewed markers; exact record opening; record/line scope named | Pass: workboard regressions and screen inventory |
| Search results and preview | Real title plus alias; state/age reserved; activity/results searchable; old activity labeled Last recorded | Pass after correction: Finder tests and long-title/no-results captures |
| New task and project picker | Instruction field labeled; unavailable creation explains why; project range and total visible | Pass after correction: forms inventory and simulated-provider PTY |
| Connections and login entry | Local source scope, provider/control availability and official login; no separate account | Pass for local UI and fake-provider fixtures; authenticated login **not tested** |
| Source chooser and folder editor | Eight focus stops, visible current choice, reversible path edits, explicit Connect/Demo/Back, temporary mode | Pass: nine [executable PTY cases](SOURCES_PTY.md), six sizes and three color variants |
| Settings and Advanced | Ordinary controls first; legacy limitations named; all six settings reachable at 32 columns; saved choices preserved | Pass after correction: compact Settings captures and preference regressions |
| Character and zone editors | Immediate affected-zone preview; mouse/keyboard edits; Apply commits, Cancel restores | Pass: preview/cancel regression, character/design captures and authored zone gallery |
| Recorded-update compatibility inbox | Four channels; native controls; observed waits take precedence; original edits/results retained | Pass after correction: Phone wait/source tests and compact/full captures |
| Other compatibility cameras | Existing preferences/routes remain; inspection opens the actual worker; limited art stated in Advanced | Pass for compositor routes and compatibility tests; physical terminal behavior **not tested** |
| Person/computer/desk composition | Both authored grids; three-person pilot before twelve; landscape monitors/laptops, connected supports, keyboard hands, chairs, readable faces | Pass: [art review and gallery](art/ART.md), clipping and twelve-character occlusion tests |
| Rooms and decoration | Three constructions and four zones; clear doors/routes; reduced window/grid contrast; stable selection | Pass: material/zone gallery, integrated captures and geometry tests |
| Animation and observed gestures | Complete outbound/action/return; no invented events; at most two gags; human request interrupts decoration | Pass: complete art sequence and deterministic/reduced-motion/source-health tests |
| Image presentation and resizing | Integer scaling; last-presented click geometry; bounded pending frames; native overlays above graphics | Pass for compositor/encoder/PTY tests; physical Kitty/iTerm2/Sixel transport across platforms **not tested** |

The critical screen review corrected issues in components that already passed
automated bounds tests: inherited bold text, dark backgrounds under light-theme
nameplates, a contradictory phone summary and a narrow panel hiding Expand.
Their earlier captures are evidence of defects, not acceptance of the final UI.

Publication gates remain separate: authenticated Codex/Claude sessions, real
macOS/Linux/Windows/WSL terminal recordings, end-to-end input latency and five
new-user comprehension sessions are **not tested** here.
