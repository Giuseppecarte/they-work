# DOC-01: instructions checked against the executable

The current navigation and persistence contract is [INSTALL](../../../../INSTALL.md).
Provider authority, requests and receipts belong to [CONTROLS](../../../CONTROLS.md).
The [contributor guide](../../../../CONTRIBUTING.md) links both, and the former
project-selection proposal is explicitly historical. No source scheduling,
provider API or preference schema was changed by the documentation correction.

## Corrected instructions and executed routes

[Keyboard PTY results](evidence/results.json) identify the final native binary
and its five passing scenario groups. The walkthrough uses 80×24, isolated
synthetic stores and real keyboard input bytes. Every launched UI exited normally
and restored its terminal attributes.

| Former ambiguity | Current contract and evidence |
| --- | --- |
| `c` immediately selects sources; `C` logs in | Both open Connections; Local sources and official login are separate actions. Both routes were exercised. |
| Enter always connects after editing a source path | Enter acts on the focused field and reopens it; F5 or focused Connect connects. Executed from first launch. |
| Tab changes floors | Tab and reverse Tab traverse visible semantic controls after redraw. Number keys and office PageUp/PageDown change floors. Both page directions were executed. |
| `v` switches a camera directly | It opens Advanced with Camera selected; arrows change the value and Esc first returns to Settings. |
| A writable explicit config directory is always required | Native Remember uses the normal settings location; a configured override selects another location. Saving starts after Connect. |
| Appearance and source preferences persist at the same moment | Connect saves source choices; normal exit saves appearance. Floor and reading state may be written during use. |
| Temporary mode clears or overwrites saved preferences | Remember off leaves saved choices unchanged; `--no-save` preserved all four existing settings-file hashes. A new temporary session created no settings and asked again on reopening. |
| `w/W` and `o/O` open appearance editors | They cycle/reset legacy costume and palette overrides. Actual saved values were checked across three launches. The editors use `a` in a brief and `d` in an office. |
| Sources are fresh every second or polled per frame | Sources are traversed sequentially on the background thread, then the host waits one second. The Source trait comment now matches the host. This statement is supported by code inspection, not a terminal timing guarantee. |

The same candidate supports the documented [release recovery](../../../release.md)
and [bounded reproduction](../../../AUDIT.md) commands. Their registry and
compositor evidence is recorded in separate lanes, not counted as navigation PTYs.

## Receipt comparison and correction

The supervisor's first correction preserved the draft correctly, but visual
inspection found the last words of the recovery instruction clipped at 80×24.
The [before frame](evidence/not-sent/before-wrap/02-not-sent-draft-retained.png)
shows that defect. The [after frame](evidence/not-sent/02-not-sent-draft-retained.png)
shows the full reason, recipient, retained draft and explicit Send/Back controls.

The composer now allocates wrapped notice rows before draft space. Operation
metadata yields before the outcome when rows are scarce. Inline, expanded and
new-task composers share this handling. Insufficient geometry gives an explicit
enlarge/read hint; it does not silently enable hidden controls. Exact request
decisions retain their existing view and authority checks.

[The final fake-provider PTY](evidence/not-sent/results.json) asserts the complete
sentence in actual screen cells, zero provider calls for the rejected draft,
no resend merely because storage recovers, and exactly one new operation to the
same recipient after explicit submission. Its exit is 0 and terminal attributes
are restored. The original storage error remains in supervisor state. The full
renderer/TUI acceptance follows in [validation](../VALIDATION.md).

Two early harness expectations were corrected: a selected compatibility camera
accurately says “Compatibility view,” and receipt JSON uses `Rejected`/`Confirmed`
with capitals. Those oracle errors were not product defects. The retained early
harness diagnostic images are labeled separately from the real clipping defect.

## Reproduction and limits

```sh
target/audit/venv/bin/python -B docs/design-audit/iteration-8/docs/check_routes.py
target/audit/venv/bin/python -B docs/design-audit/iteration-8/docs/check_not_sent.py
```

These legacy PTY recipes need Unix PTYs, Pillow, pyte 0.8.2 and macOS Menlo through
the preserved iteration 7 helper; they are not the portable audit smoke. Both
accept `--binary`. They use generated fixtures under ignored audit scratch and
stop only their owned fake-provider hosts. No authenticated provider, personal
source store or physical terminal was used. The no-send permission failure is a
real local directory-permission failure, not a disk-exhaustion measurement.

The receipt is now readable in the inspected scenario; this does not establish
that new users understand the decision vocabulary. That remains UX-02's separate
participant gate.
