# Design and usability findings

The newest pass is [iteration 6](iteration-6/REVIEW.md). Its critical review
corrected hidden current work, duplicate focus stops, stale-looking-live activity,
unfurnished teams, unrelated request receipts, narrow action clipping, inherited
styles and light-theme nameplate contrast. Its [inventory](iteration-6/INVENTORY.md)
records every surface and keeps unperformed checks explicit.

The [validation record](iteration-6/VALIDATION.md) separates local compositor and
executable PTY evidence from physical terminal, authenticated-provider and new-user
acceptance still required before publication.

## Previous review: fourth iteration

The [fourth-iteration implementation review](iteration-4/REVIEW.md) supersedes
the earlier product boundary: the accepted plan adds explicit managed controls,
real delegation teams, a living graphical tower and local review markers.
Its priority findings are missing subagent identity/relationships, ambiguous
control targets and requests, and undersized art with hidden native labels.
The real-executable PTY also reproduced a resolved approval remaining blocked.
Corrections and evidence are linked in that review. Actual-provider sessions,
physical terminal recordings and five-user acceptance remain explicitly pending.

## Previous review: third iteration

The three priorities were locating work across many projects, explaining the
actual reason for attention, and making source setup persist without a special
CLI flag. The [third-iteration review](iteration-3/REVIEW.md) supersedes earlier
completion judgments below. Its evidence includes the running native binary,
synthetic local stores, terminal restoration, and explicitly labelled font and
protocol replays.

The review found and corrected: a CCTV-style tower with poor project hierarchy;
no global conversation search; clipped task titles and misleading historical
Phone messages; awkward path editing and no ordinary first-run persistence;
pending requests truncated silently before rendering; native text obscured by
terminal images; low light-theme contrast and unpainted background gaps. Each
reproduction and rejected intermediate result is documented in the linked review.

Earlier reports remain below as historical evidence, including their own test
counts and limits. They are not claims about the final third-iteration screens.

## Reopened after native-terminal feedback

The user's resize screenshots disproved the first visual completion judgment.
The source and installation fixes below remain valid, but the floor still had
font-dependent gaps, disproportionate composition and motion that disappeared
at smaller sprite sizes. The second iteration is tracked in
[the review](iteration-2/REVIEW.md), [font evidence](iteration-2/raster.md)
and [motion verification](iteration-2/MOTION.md). Earlier screenshots and test
counts below describe the first iteration, not final acceptance of the redesign.

The three priorities for this iteration are continuous pixels with the actual
terminal font, proportionate rooms at different occupancy and window sizes,
and visible character motion without inventing worker activity.

The first three things to fix were:

1. **Let the owner choose sources, and read the current stores.** Starting the program silently read both providers; a real Codex installation appeared empty because it retained an old database under `sqlite/` beside the current one.
2. **Preserve character features at the terminal's actual resolution.** Detailed 24×34 art was sampled into 14×20 or 7×10 boxes. Adding detail to the source could not make those sampled faces legible.
3. **Make every project reachable without moving its floor number.** The selected project could disappear beyond the tab bar; twenty feeds were squeezed into unusable tiles; launching inside a project excluded the other floors entirely.

All findings below were addressed unless explicitly identified as a design decision or an untested environment. The branch is `audit/design-and-usability`; it has not been merged or pushed to main.

## Consequences, reproduction and verification

### P1 — Real conversations were missing, and source selection came too late

Run `they-work --doctor` against a Codex home containing old `sqlite/state_5.sqlite` and current root-level `state_5.sqlite` and `thread_history_1.sqlite`. Initially the collector required both databases under `sqlite/`; fixing only the history path still found the obsolete roster: **127 unarchived threads, zero recent**. Choosing current root databases found **1,221 unarchived threads, 15 recent** in the native diagnostic at capture time. These counts are observations of a changing store, not fixture expectations.

Expected: the current conversations should appear, with explicit source selection before interactive collection. Implemented: a connection screen, provider switches, editable folders, `--sources`, and root-first database resolution with legacy and mixed-layout support. Disabled providers are excluded from both polling and diagnostics. `--setup` repairs malformed source settings; valid choices are retained. Connecting with every provider disabled is supported. No remote login or API key is appropriate for a local observer.

Evidence: [initial partial-layout diagnostic](evidence/live-doctor.log), [correct current-store diagnostic](evidence/live-native-doctor.log), and [native connection capture](evidence/native-pty/01-connections.png). `codex_desktop_split_database_layout_is_collected`, `native_codex_root_database_layout_is_collected`, `migrated_codex_root_wins_over_leftover_legacy_database`, and `disabled_provider_is_not_inspected_even_when_its_store_is_broken` reproduce the boundaries without private records.

### P1 — Picking an office discarded the rest of the tower

Launch from a directory that has agent conversations, or restore a remembered project. Previously the host restricted its collectors to that path. Returning to the guard view could never show the other projects. A view preference had become a collection filter.

The default now collects all selected sources and restores the selected floor by identity. Only an explicit `--project` restricts collection. Native Git worktrees resolve the `.git` and `commondir` pointers to their primary project, including when `--project` is used. Unmounted worktrees without available Git pointers retain their recorded path; no remote-repository guess is made.

Reproduce with `launching_inside_one_project_keeps_the_other_floors` and `related_git_worktrees_share_the_primary_project_floor` in the CLI suite.

### P1 — A request for input did not immediately mean attention

Create an `Activity::Waiting` event and render it at its event timestamp. Previously status depended on an open turn becoming silent for three minutes; a known request could appear running or idle. World aging could also replace the explicit waiting activity with idle.

Known requests now become blocked immediately and retain their detail. Turn completion clears the request. A long-silent open turn remains an attention hint, but its desk says **NEEDS ATTENTION** and explicitly says no approval was identified. Approval is performed in the original application. The phone's blocked channel uses the same distinction; timeout alone is not labelled an approval request.

Reproduce with `explicit_request_needs_attention_immediately_and_completion_clears_it` and `stale_work_is_attention_without_an_invented_approval_request`. The [native desk capture](evidence/native-pty/04-desk.png) shows an actual parsed synthetic Claude question.

### P1 — New installations and rewritten transcripts could remain stale

Connect an existing empty Codex home, then create its first databases. The old source factory installed no collector, so the running program never noticed. Empty homes are now watched while known database symlinks remain rejected.

The existing Claude rewrite test failed on Docker Desktop's bind mount. Two timestamps a millisecond apart were both reported with zero nanoseconds. Size and modification time therefore did not detect replacement. A bounded 4 KiB checkpoint now verifies the previously consumed tail before continuing. This detects recent-tail rewrites without hashing multi-gigabyte transcripts on every poll; an in-place edit confined to older content with unchanged identity, size, timestamp and tail is outside this incremental reader's guarantee.

Evidence: [measured timestamps and original failure](evidence/claude-rewrite.log). Reproduce with `an_empty_codex_installation_is_watched_for_its_first_conversation` and `claude_recovers_from_home_gap_deletion_and_same_size_replacement`.

### P1 — Source reconnection replayed history during the first implementation

The critical review of this branch found that recreating a collector against an existing world replayed its first scan. It also found that pausing could leave a final batch in a discarded channel. The host now joins and drains the collector, reuses its cursors on cancellation, and retains the already-scanned collectors when connecting. The regression `pausing_collectors_preserves_the_last_batch_and_does_not_replay_it` verifies both boundaries.

The same review caught loss of configuration context when visiting demo mode. Source and appearance settings now survive that transition; demo floor names cannot overwrite the last real project.

### P2 — Pixel sampling defeated the character design

The half-block path retained only 70 samples out of 816 authored pixels; sextants retained 280. Manager and worker crops also allocated different amounts of the same destination box to their heads. Image-mode alert coordinates were divided by Unicode cell density rather than the actual image density.

The renderer now uses authored full, compact and miniature maps with integer enlargement, consistent portraits, and the correct image coordinate system. Six selectable characters and fictional quirks are stored by conversation identity. Four room palettes are stored by project identity and shared by the floor, tower and settings preview. These choices change the representation, not the coding agent's real behavior.

See [visual measurements, comparisons and reproductions](VISUAL.md). There is no claim that terminal characters reproduce a raster reference pixel-for-pixel.

### P2 — Navigation and status summaries failed at scale

Render twenty projects at 80×24 and select the last one. The old grid allocated only two or three rows to each tile and the tab bar only showed its beginning. Numbered floors also changed when attention priority changed. An idle-only office had a green dot.

Feeds now page at usable sizes, the selected tab remains visible, and floor numbers follow stable project order. The tower includes working/idle/waiting/failed totals and an attention shortcut. Escape ascends from desk to floor to tower. Ordinary 80×24 terminals retain office artwork; a list remains available explicitly and as a fallback below the useful artwork budget.

Evidence: [twenty floors at 80×24](evidence/tower-20-floors-80x24.txt) and [navigation audit](USABILITY.md). This verifies reachability and legibility of labels, not a timed comprehension result from a new user.

### P2 — Saved appearance made explicit CLI options wrong

Restore a light theme and top-down projection, then run `--light`, `--dark`, or `--view side`. The initial implementation toggled keys instead of setting values. It could invert light, ignore dark, and keep the wrong camera after `c` became the source shortcut.

CLI overrides now update explicit preference values before applying them. Environment colour restrictions remain authoritative. A quoted `--config-dir "~/..."` is expanded once, before every settings read or write. Directories requested for persistence are created.

Evidence: [before](evidence/installation/cli-preferences-review.log), [after](evidence/installation/cli-preferences-fixed.log), and `explicit_camera_and_light_override_saved_preferences_absolutely`.

### P2 — Installation assumed a Docker and platform setup that users may not have

Follow the original README: native Windows and macOS had no binary installation path; the published image was amd64-only. A source build also required a networked cache-fill step before offline tests. Native compilation exposed Linux-only process metrics and Windows user-profile assumptions.

Native installers, checksum verification, packaging and a six-target CI/release matrix are implemented. The repository-local macOS ARM64 toolchain built, installed, packaged and executed the program. Installer tests cover missing releases, checksum failures, bad archives, failed pulls and destination preservation. The currently published `v0.1.0` has no native assets: its real HTTP 404 was tested and the installer failed without creating a destination. Publication was not part of this branch operation.

See [installation evidence and platform limits](INSTALLATION.md). Windows runner definitions are verified against official documentation; they are not evidence of completed Windows runs.

### P2 — Platform assumptions weakened tests and project identity

Running the suite natively on macOS exposed case-folding of Linux paths transported through WSL UNC syntax. It also revealed a test that read the host's private stores by default and assumed that an installed Claude home must have active conversations. WSL Linux paths now retain case, Windows canonical drive paths normalize consistently, and both live-store tests require explicit opt-in. The ordinary suite uses fixtures only. Native execution also exposed a golden fixture inheriting `NO_COLOR` from the host: the fixture now resets its environment-lock flags after selecting its fixed rendering settings, while production continues to honor `NO_COLOR`.

Temporary test directories inside a Git checkout exposed another grouping bug: an unrelated ancestor repository could override Claude's recorded project boundary. The recorded boundary now stops that upward search while a nearer valid Git root still wins.

## Surface-by-surface fidelity judgment

| Surface | Judgment and decision |
| --- | --- |
| Floor | Structural defects fixed: native-grid figures, proportional heads, readable fallbacks, custom room material, prominent project sign. A character terminal cannot retain every 24×34 facial pixel; smaller artwork is deliberately authored for it. |
| Guard office / tower | Reworked as a floor directory with paged live feeds. It expresses independent projects without making a literal scrolling skyscraper consume the navigation budget. The security-room scene remains available. |
| Desk | Identity, character quirk, state, current request and timeline are visible together. Actual approval execution remains in the source application. |
| Phone | Portrait identity and attention semantics checked and corrected. It remains a read-only digest of recorded events, not an invented chat between workers. |
| Settings | Scrollable options, stable character/floor choices and persistence implemented. The compact preview illustrates the selected material and character; the complete room and projection are visible on closing settings. This smaller preview is an intentional terminal adaptation. |
| First run | Replaced the interactive diagnostic wall with source selection before collection. Plain noninteractive diagnostics remain available through `--doctor` and `--once`. |
| Help | Scrolls and wraps by terminal cells, explains states and where to respond, and exposes the new controls. Very small terminals prioritize controls over art. |

## What remains unmeasured

The code was built and executed on macOS ARM64 and Linux ARM64 in Docker. Native PTY interaction, terminal-mode cleanup, fixture data, installed executable behavior, image encoders and deterministic rendered frames were tested. [PTY results](evidence/native-pty/results.json) distinguish graceful exit from SIGTERM exit; the terminal comparison excludes macOS's transient kernel `PENDIN` bit.

Computer Use explicitly refused access to `com.apple.Terminal`, so there is no Terminal.app visual claim. The supplied PNGs are labelled buffer/ANSI reconstructions. No Windows Terminal, WSL GUI, Kitty or iTerm2 graphics session was available. No independent newcomer was timed. These are test limits, not passing results or a future-feature roadmap.
