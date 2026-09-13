# Inspection and attention, third design pass

The test is practical: identify the conversation, understand why it needs attention, read the evidence, and know where to act. Decorative identity stays visible without displacing the request.

## Problems observed in the running application

The baseline is a native macOS PTY session against isolated synthetic Codex SQLite records. The records include an explicit approval assessor linked to a pending command, an error, a silent open turn, an active conversation, and a completed conversation. No account or live conversation was changed.

- At 80×24, the Desk title ended at `PRODUCTION CUTOV`. The metadata occupied three rows before the request; the request itself was clipped and did not appear in the history because it arrived as a current-state event.
- Phone's Attention channel still displayed the heading `STANDUP`. Its silence card claimed `Waiting; no approval detail available`. The error card described an earlier successful `cargo test` instead of the actual connection error.
- Rendering a historical edit while the worker was currently blocked replaced the edit with today's request. Grouped, oldest-first history also hid the most recent useful events.
- The Desk timeline used an unlabeled UTC clock, had no visible scroll position, and highlighted old requests with the same background as current outstanding requests.

Before: [Desk, 80×24](evidence/before/desk-request-80x24-menlo.png), [Attention, 80×24](evidence/before/phone-2-80x24-menlo.png).

## Changes and their reasoning

Desk keeps the title's original capitalization and wraps it. The first card presents current state, captured detail, and a concrete next step. Explicit requests say to review the original conversation; silence says no approval was identified; errors show the captured error. The app remains an observer, and does not imply that pressing a key approves a command.

The history includes the complete received title and thread ID at its beginning, UTC day separators, a visible line range, and a clearly separate `NOW` section. That final section preserves current requests even when the collector emitted no historical beat. Home exposes conversation context; End reaches the latest state; page keys move through evidence. Old requests retain their historical event label but do not retain an outstanding-alert background.

Phone now labels its channels Now, Attention, Edits, and Messages. Current conversations put attention first; recorded events put the newest first across workers. A selected card gets enough room for a wrapped title and detail, with Enter leading to the full inspector. Current-state cards and historical-event cards have separate semantics: historical text is never overwritten by a worker's later status. Error and silence belong to Attention without being called an approval request. Project and provider context accompany each conversation. Stable message keys allow the UI to preserve focus when the list reorders.

Tokens, branch, and decorative character remain available after the work and attention information. At narrow widths, avatar space yields to readable text.

## Verification

Focused regression coverage includes long Unicode titles, a current request after earlier recorded work, preservation of complete request text at 80×24, neutral styling for old requests, silence without an invented approval, current-message identity through reorder, error inclusion, cross-worker chronological ordering, narrow channel tabs, and rendering an edit while its worker later waits for approval.

Reproduction:

```sh
sh docs/design-audit/native-cargo.sh test -p theywork-render --lib views::desk
sh docs/design-audit/native-cargo.sh test -p theywork-render --lib views::phone
sh docs/design-audit/native-cargo.sh build --release -p theywork-tui --bin they-work
python3 docs/design-audit/iteration-3/inspect_pty.py --binary target/native-macos/release/they-work --stage inspection-after
```

The harness writes actual ANSI cells and verifies clean exit and terminal-mode restoration. The `*-menlo.png` evidence uses the iteration-2 CoreText replay script with the installed Menlo font, full-cell backgrounds, and cell clipping. These are native application output replayed in a real font, **not Terminal.app screenshots**. The other PNGs use ideal cell geometry and are only supporting evidence.

## Acceptance and limits

The final native run at 80×24 and 120×36 passed exit and terminal-restoration checks. Home exposed full conversation context; End returned to the `NOW` section. Menlo replay confirmed that `cutover` stays visible, the complete received approval command includes `customer-account-id-index`, the selected phone card is distinct from neighboring alerts, and historical edits retain their own details.

After: [Desk 80×24](evidence/inspection-final/desk-request-80x24-menlo.png), [Desk 120×36](evidence/inspection-final/desk-request-120x36-menlo.png), [Attention overview](evidence/inspection-final/phone-2-80x24-menlo.png), [Selected request](evidence/inspection-final/phone-request-selected-80x24-menlo.png), [Silence without approval](evidence/inspection-final/phone-silent-selected-80x24-menlo.png), [Recorded edits](evidence/inspection-final/phone-3-80x24-menlo.png).

During review the fixture exposed an upstream loss: a 137-character request was reduced to 120 characters without an ellipsis, ending at `customer`. The collector now preserves pending-command detail up to the 2,000-character timeline limit and marks actual truncation. The final captures above include that integrated correction. History remains bounded by the core model and is not a complete transcript; the interface calls it recorded/available history and directs action to the original conversation.

The final inspection found and corrected a generic footer that hid channel keys, unnecessary fixed-height gaps, low contrast in the Phone heading, inconsistent history backgrounds, and an avatar that could exceed a partially opened phone row. Large selected messages deliberately use more vertical space; the list counter and arrows retain access to the remaining conversations.

Validation: [Desk 5/5](evidence/inspection-desk-tests.log), [Phone 7/7](evidence/inspection-phone-tests.log), [image layer integration](evidence/image-layers-tests.log), and [strict renderer Clippy](evidence/inspection-clippy.log). The separate image-layer integration test checks Settings, Phone, and Finder followed by a return to Office, in both themes: native letters survive inside the graphics rectangle, opaque blank backgrounds cover image pixels, and stale overlay masks do not leak into later frames. That is a composition contract test, not a claim that all graphics terminals have been visually tested.
