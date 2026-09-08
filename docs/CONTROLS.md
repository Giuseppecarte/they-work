# Tasks, teams and provider connections

Use the native `they-work` binary for controls. Observation works without a
they-work account. The normal Docker launcher is an observation environment:
its data mounts are read-only and networking is disabled.

## Connect and sign in

`c` chooses the local providers and data folders that the office may observe.
`C` shows control availability and opens the official provider login with `1`
(Codex) or `2` (Claude). They-work never asks for a provider password or stores
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
- `m` opens the selected task's controls. The displayed recipient stays attached
  to the draft even when the roster changes. Enter inserts a newline; `F5`
  sends a Codex instruction. During an active managed turn this steers the
  selected turn, with its expected turn ID.
- `F2` interrupts the selected active managed Codex turn. It does not stop
  unrelated workers or the entire supervisor.
- `F4` shows unresolved requests from that connection. Number keys choose a
  displayed approval decision. Structured questions accept text, Tab moves to
  the next question, and `F5` submits the answers to that request's ID.
- `F6` reconnects a saved, previously managed Codex task after its host or
  provider connection stops. Reconnection loads its history; it does not send
  an instruction or restart the previous turn.

A receipt distinguishes confirmed, rejected and uncertain outcomes. An uncertain
send is never retried automatically. Keep the operation ID and inspect the task
before intentionally sending anything again. A request that expires is disabled;
an arriving replacement requires deliberate selection. Controls stay disabled
when the terminal is too small to show their contents.

Codex tasks created here belong to a local supervisor. Closing the office leaves
accepted work running. Reopening the office connects to that supervisor. If the
supervisor itself restarts, tasks remain disconnected until explicit `F6`, and a
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
conversation. For an existing compatible background task, `F3` (or Enter in its
control panel) verifies the native roster and attaches to that exact session.

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

Meeting rooms represent confirmed delegation. Session membership with an unknown
immediate parent and conversation forks have their own labels. Missing parents
and participants remain unknown instead of being invented. Messages and deliveries
show recorded provenance; decorative coffee, walking and costumes have none.

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

The [implementation audit](design-audit/iteration-4/REVIEW.md) separates fixture
and PTY evidence from actual provider and platform testing. Schema changes that
are not understood remain unavailable; the office does not fabricate a working
control for them.
