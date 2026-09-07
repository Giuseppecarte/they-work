# Project notebook review

The notebook separates attention, recorded deliveries, changes since entry, and task families. This review found and fixed four cases where the first implementation could mislead a user:

1. An absent recipient was displayed as “user.” It now reads “Not recorded”; known unavailable endpoints are labelled explicitly. Native evidence, turn/item identity, and source limitations remain in the readable detail.
2. A child output was omitted if its transcript was absent, even when the recorded recipient was present. The known recipient now supplies project context without changing the sender's identity. Retired workers' outputs and activity history remain readable. Enter only targets a current seat; it does not silently dismiss the notebook for an unavailable task.
3. Attention acknowledgement depended on the worker's latest activity timestamp. Human-request markers now use the native collaboration event identity, so token updates, other activity beats, messages, and source heartbeats cannot make the same request unread again. A new request receives its own marker. Marking seen only updates local review memory and never approves, answers, or completes a provider request.
4. A session member sorting before its root could appear as an independent root, and membership-only families could not collapse. The forest now starts from recorded family roots, retains explicit relationship kinds and missing endpoints, and folds membership branches. Duplicate nodes and repeated family traversals are suppressed. Details list all recorded incoming relationships; the visual projection is not a claim of unrecorded immediate parentage.

The Changes channel honors the entry baseline before the latest persisted visit timestamp. Refreshing the panel does not move that baseline. Retired history is included, and ordinary talking beats do not duplicate native collaboration output. Source coverage distinguishes unknown, partial, stale, and unavailable observations in the footer and details. Narrow layouts preserve four channel shortcuts and Escape; detail scrolling clamps to wrapped content and resets only when the selected record changes.

## Verification

Eleven focused notebook tests pass on macOS aarch64. They exercise local marking through `Ui::handle_key`, request identity stability versus other observations, absent/retired senders, unknown recipients, entry/restored baselines, nested delegation, missing parents, session membership, forks, folding, coverage, selection preservation, and bounded review memory. The rendering test checks actual Ratatui TestBackend buffers at 28×12, 40×16, 80×24, and 120×36, reaches the last line with PageDown, and verifies control-sequence sanitization and visible channel/Escape controls. These are buffer checks, not physical terminal screenshots.

- [Focused tests](evidence/workboard-tests.log): 11 passed.
- [Renderer strict Clippy](evidence/workboard-clippy.log): all targets passed with warnings denied.

Reproduce using an installed Cargo or the repository-local toolchain described in [DATA.md](DATA.md):

```sh
cargo test -p theywork-render --lib views::workboard --offline
cargo clippy -p theywork-render --all-targets --offline -- -D warnings
```

Remaining limits are explicit: local history can be incomplete; a delivery without any known actor or recipient cannot be assigned a project; discarded history beyond the core bounds is unavailable. Seen markers do not claim that a task or request is resolved. This review does not claim a live provider connection, a native graphical terminal check, or Windows execution.
