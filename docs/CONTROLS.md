# Tasks, teams and provider connections

Use the native `they-work` binary for controls. Observation works without a
they-work account. The normal Docker launcher is an observation environment:
its data mounts are read-only and networking is disabled.

## Connect and sign in

`c` and `C` open Connections with sources, control availability and official login.
Choose **Local sources** to select the folders the office may observe, or the
named provider's login control to open its official flow. Merely opening this
panel does neither. They-work never asks for a provider password or stores
its own copy of a login token. Install the official native CLI on PATH; `--doctor`
reports its version and whether background Claude attach is available.

On Windows, controls require native `.exe` clients. An npm `.cmd` launcher is
not executed through a shell with the contents of your instruction. In WSL use
the Linux clients. Reading a Windows data folder from WSL does not grant a
connection to the process running on Windows. Cross-machine control is outside
this version's scope.

## Start or continue work

- `n` opens a new task. Tab selects provider, project and instruction; arrows
  change provider. Type or paste the instruction, then press `F5` to send.
- `m` opens an instruction composer inside the work panel. The displayed recipient stays attached
  to the draft even when the roster changes. Enter inserts a newline; `F5`
  sends a Codex instruction. During an active managed turn this steers the
  selected turn, with its expected turn ID.
- `F7` expands or collapses the work panel while retaining the draft. Enter
  inserts a newline; it never sends the instruction.
- **Task actions → Stop task** interrupts only the selected active managed
  Codex turn. **Reconnect** reconnects a saved managed task without sending an
  instruction or restarting its previous turn.
- **Review request** opens an exact unresolved request. Number keys choose a
  displayed decision. Structured questions accept text, Tab moves between
  fields, and `F5` submits answers to that request's ID. Merely inspecting the
  worker never decides a request.

Secondary task shortcuts do not run while composing. Leave the draft with Esc
and use the work panel's explicit actions; the recipient-bound draft is retained.

A receipt distinguishes confirmed, rejected and uncertain outcomes. An uncertain
send is never retried automatically. Keep the operation ID and inspect the task
before intentionally sending anything again. A request that expires is disabled;
an arriving replacement requires deliberate selection. Controls stay disabled
when the terminal is too small to show their contents.

**Not sent: local state could not be saved. Restore storage access and submit
again.** means the live supervisor rejected the operation before contacting the
provider. The recipient and draft stay available. Restore storage access and
submit deliberately; refreshing the rejected receipt does not resend it.
If a supervisor restart instead reports **Uncertain**, follow that receipt:
an incomplete local save may have left a durable intent, so the application
does not infer a safe automatic retry from the earlier storage error.

Codex tasks created here belong to a local supervisor. Closing the office leaves
accepted work running. Reopening the office connects to that supervisor. If the
supervisor itself restarts, tasks remain disconnected until explicit Reconnect, and a
new instruction is still required to start work. External Codex histories remain
observable: reading or resuming them does not establish ownership of their
currently running process.

Managed Codex control requires **Remember on this computer**. With `--no-save`
or Remember disabled, the office stores no preferences or private supervisor
state. Observation and the native Claude console remain available; the office
explains the persistence requirement before creating a managed Codex task.

## Claude's official console

New Claude tasks launch through the official client. A version advertising
background mode uses its official supervisor; older versions open a foreground
conversation. For an existing compatible background task, Open conversation (or `F3` while composing) verifies the native roster and attaches to that exact session.

The office leaves raw mode and its alternate screen while the official console
owns the terminal. Talk, interrupt and approve there. Exiting or detaching restores
the office. They-work does not emulate the console, inject keystrokes into another
window, or automatically resume a task that is no longer running. Foreground or
otherwise unattached external sessions explain that they must be opened at their
origin. Claude's normal login and account restrictions still apply.

## Read the office

`b` opens the notebook: `1` attention, `2` deliveries, `3` changes since your last
visit, `4` team. `g` opens the team tree directly. Arrows select entries; Enter
opens the worker, and PageUp/PageDown scroll the recorded detail. Left/right
collapse or expand team branches.

Approval requests, questions, automatic review, errors, waits for collaborators
and unavailable information are named separately. `r` only marks an attention
entry seen or a delivery reviewed. It does not answer a question, grant permission
or complete a task. Deliveries require recorded result evidence; finishing a
turn is not project completion. The changes view states that historical coverage
can be incomplete.

**Since your visit** uses the previous visit to that project as its baseline.
Entering a floor advances its visit time even without opening a record or
marking it seen. **Deliveries** retains available result records and their
separate local reading markers. Visiting, marking seen, reviewing a request and
sending a provider decision are distinct actions; none proves that a person
actually read or accepted a result.

Meeting rooms represent confirmed delegation. Session membership with an unknown
immediate parent and conversation forks have their own labels. Missing parents
and participants remain unknown instead of being invented. Messages and deliveries
show recorded provenance; decorative coffee, walking and costumes have none.

Task lists keep source observation age separate from the task title. “Age unknown”
means no observation time was recorded; a recent UI refresh does not make old work
current. Search and the tower use the same rule for current attention counts.

Advanced retains older camera preferences. Without image support, or below their
minimum usable size, those cameras show the native task roster. The saved camera
returns when the terminal can display it. Text encoding choices remain available
for compatibility surfaces that draw pixel characters.

## Local storage and compatibility

Connection and appearance preferences remain in the existing settings directory.
`notebook.json` stores reading markers and visit times. Legacy explicit wardrobe
choices migrate when their old native ID has one unambiguous owner. New choices
use provider, source and native identity, so adding or removing a colleague does
not change a worker's outfit.

Managed Codex state lives in a private, source-specific `control/` subdirectory.
It contains conversation-related events, ownership metadata and receipts, with
bounded history. It is not an account credential store. Do not remove it while
you still need to inspect or control its running tasks.

The [current acceptance record](design-audit/iteration-8/VALIDATION.md) separates fixture
and PTY evidence from actual provider and platform testing. Schema changes that
are not understood remain unavailable; the office does not fabricate a working
control for them.
