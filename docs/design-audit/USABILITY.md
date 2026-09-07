# Tower navigation and customization audit

The highest-impact navigation defects were hidden projects, changing floor
numbers, and missing guidance for a worker that needs attention. The audit
combined baseline layout and key-path inspection with executed regression
cases and terminal captures. The fixes below were verified on this branch.

## Reproductions and decisions

| Reproduction | Before | Implemented behavior |
| --- | --- | --- |
| Open the tower with 20 projects at 80×24; select the last project. | The baseline layout calculation gives seven rows in a 19-row body. Individual tiles therefore have two or three terminal rows, leaving zero or one row inside the borders. The tab bar only displays its beginning, so the selected project disappears. | The tower pages its feeds at a minimum useful size. The selected feed is always drawn. The top bar follows selection and displays `20/20`; `PgUp`, `PgDn`, `Home`, and `End` work across all projects. |
| Let a worker in another project request approval. | Attention ordering changed every floor's number, so numeric project shortcuts changed meaning while the app ran. | Floors retain the world's canonical project-path order. A global status summary and `!` jump directly to the next worker requiring attention. Selection is retained by project and conversation identity across snapshot changes. |
| Inspect a worker, then press Escape twice. | The second Escape did nothing: the floor could not return to its parent view. | Escape follows desk → floor → tower. `0` always opens the tower; Enter descends. |
| Look at a quiet office in the project bar. | Every office without a failure or block used a green dot, including offices where nobody was working. Camera “busy” counts mixed active work with waiting. | Idle offices have muted dots. Tower and feed summaries separately count working, idle, waiting, and failed workers using the same status function as the desk. Selected floors also use a textual `>` marker. |
| Open settings at 24×10 and move to the last option. | The last options were clipped and could be changed without seeing them. The screen had no editing or closing instructions. | Options scroll with the selection. The preview is omitted when there is no useful space. Editing, closing, and source-selection shortcuts remain visible. The redundant light/theme controls are one theme control. |
| Open help on a small terminal. | Help was truncated, could not scroll, and only listed keys; it did not explain what waiting meant or where approval happens. | Help wraps to actual terminal cell widths, scrolls, explains the project/conversation mapping and all states, and directs approvals back to the source application. |
| Use a project title containing wide CJK characters. | Truncation counted Unicode scalars instead of terminal cells, so text exceeded the intended width. | Elision measures displayed cell width and retains valid text. Regression coverage includes 200-character names and CJK characters. |
| Customize the office, exit, then open it in another terminal. | Settings were session-only. Persisting the detected encoding blindly would also force one terminal's capabilities onto another. | `RendererPreferences` exposes serializable user choices to the host. Only explicitly selected colour/encoding values are stored; automatic capabilities are detected again. Explicit environment overrides remain authoritative. |
| Give a conversation a recognizable character. | Wardrobes were derived from conversation IDs with no user choice. | In the desk, `w` cycles six characters and `W` restores the original. The choice is keyed by conversation ID, saved with preferences, and used by every portrait and office view. Desk labels identify each character's fictional quirk; these choices do not change the coding agent's behavior. |
| Customize one project without changing another. | Room materials were fixed or chosen automatically; the owner could not choose them. | `o` cycles four room palettes and `O` restores automatic selection. Settings exposes the same choice with a live preview. Saved palette choices use project IDs; floor and tower feed share the result. |
| Receive key-press and key-release events for the same key. | The release could toggle an overlay a second time, closing it immediately. | Release events are ignored. A regression verifies that they cannot toggle the phone, request sources, or quit. |

At widths of at least 110 columns the tower adds a scrolling floor directory.
Smaller terminals devote the space to the selected page of office feeds. The
selected project's full path is exposed in the tower's second footer row when
it fits. `c` opens the host's local-source connection screen from any view;
`v` changes the office projection. This keeps sources discoverable without a
separate remote account or a misleading authentication requirement.

## Evidence and reproduction

The renderer's test backend exercises actual layout, rendering, state changes,
and key handling, without requiring a particular installed terminal font.
These are terminal buffers, not claims of a human usability test.

```sh
./scripts/cargo test -p theywork-render --lib
THEYWORK_UPDATE_GOLDEN=1 ./scripts/cargo test -p theywork-render --lib
```

The second command deliberately refreshes checked-in golden frames and these
audit captures. Review generated changes before accepting them.
Plain-text captures omit trailing blank cells; the golden frames preserve the
complete terminal grid.

- [20 floors at 80×24](evidence/tower-20-floors-80x24.txt)
- [20 floors at 160×48](evidence/tower-20-floors-160x48.txt)
- [20 floors at 240×70](evidence/tower-20-floors-240x70.txt)
- [Last customization option at 24×10](evidence/settings-24x10-last-option.txt)
- [Help at 24×10](evidence/help-24x10-first-page.txt)
- [Scrolled help at 24×10](evidence/help-24x10-scrolled.txt)
- [Renderer regression results](evidence/renderer-tests.log)
- [Renderer strict lint results](evidence/renderer-clippy.log)
- [Tower visual capture](evidence/after-cameras-sextants.png)
- [Customization visual capture at 80×24](evidence/after-settings-sextants-small.png)

The recorded renderer run passed every test, including the golden frames. Strict
Clippy completed with warnings treated as errors. The core, collectors, and
terminal host have additional workspace checks recorded in the audit's main
verification evidence.

Relevant regression cases include
`twenty_floors_keep_the_selected_project_and_page_visible`,
`attention_shortcut_opens_the_worker_in_a_different_project`,
`small_settings_scroll_to_every_option_and_help_stays_navigable`,
`saved_preferences_round_trip_without_persisting_automatic_terminal_choices`,
`character_choice_stays_with_the_conversation_and_can_be_reset`,
`room_palette_changes_only_the_selected_project_and_survives_restore`,
`key_releases_do_not_toggle_overlays_or_send_host_commands`,
`navigation_descends_and_ascends_through_the_view_hierarchy`, and
`short_path_never_splits_utf8_and_respects_tiny_widths`.

## Critique after implementation

The initial revision still estimated help's wrapped height from character
counts. That could make its final lines unreachable at narrow widths. Help now
wraps explicitly before clamping its scroll position. A second revision added
textual selection markers because highlighting alone disappears in no-colour
terminals. Persisting automatically detected pixel modes was also rejected:
capability detection must remain local to the terminal running the program.

Reviewing the customization PNG exposed a desk placed far below its worker
and a compact character centered inside a taller invisible box. The preview
now uses the native sprite's exact integer dimensions, anchors its feet at
the floor, and places the desk immediately in front. Reviewing the 80×24
office also rejected a text-list default that removed the product's main
visual experience. Automatic mode retains a compact visual office at 80×24;
the list remains available explicitly and for smaller terminals.

A native run with `NO_COLOR` exposed a separate verification defect: the
golden fixture forced truecolour but retained the environment-lock flag, so
settings rendered `truecolor (env)` instead of `truecolor`. The fixture now
fixes both values and lock flags explicitly. Production still honors the
user's environment settings.

The tower is a directory with live office feeds, not a literal animated
skyscraper. That decision preserves a readable office view at normal terminal
sizes while making project boundaries and navigation explicit. Character
quirks are fictional decoration. No worker's real reasoning, behavior, or
approval state is inferred from those traits.

The smallest buffers (1×1 and 4×4) are checked for safe rendering, not useful
interaction. A 24×10 terminal provides compact navigation, scrollable help,
and settings; the office artwork needs more room. No timed test with an
independent newcomer was available, so the “under ten seconds” comprehension
claim remains unmeasured. Font rendering and keyboard behavior in native
Windows Terminal and WSL sessions were not exercised in this lane. Synthetic
buffer coverage is not evidence that those native sessions passed.
