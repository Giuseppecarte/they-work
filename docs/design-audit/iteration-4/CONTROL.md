# Local control implementation and audit

The new `theywork-control` crate owns provider processes and control authority.
The renderer receives plain state and explicit commands; it does not write to
transcripts or infer permission to control a conversation from its title, path,
or session ID.

## Implemented contract

`ControlClient::connect_or_spawn` starts the current executable with the internal
`--control-host <private-config-path>` entrypoint. The host owns one Codex
App Server over piped JSONL. It is detached from the terminal on Unix and Windows;
closing a client or the office leaves its accepted tasks running. A file lock
prevents two hosts owning one state directory. Each directory is bound to one
canonical `CODEX_HOME`, also passed explicitly to the provider process.

Startup only loads local state. It launches no provider and submits no turn.
Saved tasks are disconnected until an explicit reconnect loads their history.
`ControlClient::saved_snapshot` discovers that durable ownership without starting
a host, so the UI can offer reconnect after a machine restart.
Only a subsequent user instruction starts work. Closing a view, restarting the
host, losing an acknowledgement, and opening historical data never replay an
instruction.

The public operations are start, send, steer with `expectedTurnId`, interrupt a
specific turn, reply to a specific pending request, and explicitly reconnect a
previously managed task. Every mutation carries a caller-generated operation ID.
The host saves intent before sending and records confirmed, rejected, or uncertain
outcomes. Reusing an ID returns its existing receipt; changing the action under
that ID is rejected. There is no automatic resend. An uncertain thread creation
retains any native thread ID already returned by the provider.

The local listener binds only loopback. Its random 256-bit credential is stored
in an owner-only file, never in process arguments. Unix checks ownership and
rejects symlinks, with directories `0700` and files `0600`. Windows checks the
current user's SID, rejects reparse points, and applies a protected owner-only
DACL. Requests are bounded to 1 MiB; snapshots and saved state to 16 MiB. The
event history and pending-request count are bounded. No web listener or remote
provider attachment is implemented.

## Approval and identity boundaries

Only explicit server requests enter the pending queue. Command and file replies
are limited to one-request accept/decline/cancel; session grants and execution
policy amendments are rejected. Permission grants must be a subset of the
requested permissions and apply to the current turn. Structured questions and
elicitation replies retain native IDs. Unknown request formats remain blocked
and visibly unsupported. Nothing auto-approves or treats silence as consent.

A sent reply remains pending until provider resolution. A turn change,
completion, disconnect, or connection generation change invalidates stale requests.
Replacing a dead provider transport happens only during an explicit new-task or
reconnect action; other managed conversations remain disconnected.
Identity is provider + canonical source home + native thread ID, matching the
collector. Only tasks created by this host have durable control ownership.
Explicit spawn items can create subagent people and relationships, but do not
grant those children direct controls. Unrelated internal thread notifications
do not become people. The UI can open the controlling parent instead.

`snapshot_events` bridges roster, activity, native delegation/message/result
events, and human requests into the core model. Collaboration IDs match the
collector's turn/item identifiers, so later transcript discovery does not add a
second person or duplicate an already delivered result. Source freshness is the
poll observation; activity timestamps remain the provider-event observations.

## Native Claude console

`NativeProvider` checks the official executable's version and help output, then
uses `claude agents --json` for a fresh active roster. Attach requires a matching
source and full session ID, background kind, a live-process PID in that roster,
and a nonfailed/nonstopped job. Historical sessions are not resumed implicitly.
New sessions use official `--bg` when advertised; otherwise they open the native
foreground console. Login is the official `auth login` or Codex `login` command.
`CLAUDE_CONFIG_DIR` / `CODEX_HOME` is explicit for every command.

The TUI suspends its alternate screen and raw mode before `NativeCommand::run`.
The command inherits the real terminal. A signal guard protects the parent while
the native child retains Ctrl-C handling, and restores the original parent
handlers afterwards. There is no VT emulator, SDK account, API-key login, shell
command concatenation, `respawn`, or automatic recovery turn. Arguments after
`--` preserve even prompts beginning with CLI-like flags as user text.

Windows detection accepts native `.exe` providers. Npm `.cmd` launchers are not
run through a shell with user instructions; users need the official native CLI
for these controls. Temporary settings mode keeps persistent managed control
disabled; the UI explains that remembered access is required.

## Verification and criticism

All process tests use the repository's offline Python fixtures. They never start
a real Codex/Claude turn, log in, change an account, or control an existing task.
The test suite covers:

- Detached startup, one-host reuse, client closure with ongoing work, literal
  project paths, native source identity, steer and interrupt correlation.
- Lost acknowledgement, duplicate operation IDs, restart with no provider
  launch or turn replay, and explicit history-only reconnect.
- Wrong IPC credentials, source mismatch, owner permissions, stale approvals,
  refusal of session-wide decisions, and external IDs with no control authority.
- One parent and two delegated people, stable result identities and no child
  authority; immediate completion and provider disconnect races.
- Native capability fallback, literal metacharacter arguments, stale/stopped
  Claude jobs, cross-source/subagent attach rejection, and Ctrl-C in a real PTY
  returning to the parent with its original signal handler restored.

Autocriticism found and fixed a completion race that could redraw an already
completed turn as working, a reply retry window before provider resolution,
ambiguous prompt-leading flags, incorrect source-freshness defaults, and signal
inheritance that could terminate the office during native handoff.

Reproduce with `sh docs/design-audit/native-cargo.sh test -p theywork-control`,
then strict Clippy for the native target and `--target x86_64-pc-windows-gnu`.
The local sandbox requires permission for loopback sockets and PTYs; the offline
suite passed with that permission. Logs are adjacent to this document.

Windows compilation and strict Clippy passed. Windows runtime ACLs, console
events, and detached-process behavior have not been executed on Windows. Actual
provider/account compatibility, real approval dialogs, native graphics console
restoration, and paid-turn outcomes remain untested. This audit does not claim
cross-platform runtime certification or support for every provider version.

## Protocol sources checked

The [official Codex App Server reference](https://learn.chatgpt.com/docs/app-server)
specifies initialization, JSONL transport, thread/turn methods, expected-turn
steering, server request correlation, and provider-specific schema generation.
App Server is evolving; rejected or unknown methods are surfaced, not replaced
with transcript writes or unverified control paths. This implementation uses
the stable request fields and does not enable experimental API capabilities.

The [official Claude agent-view reference](https://code.claude.com/docs/en/agent-view)
documents native background sessions, their supervisor, JSON roster, attach,
and source-home isolation. Feature detection is per installed binary; the
documented background research preview began in v2.1.139 and later releases
changed its behavior. Neither a detected version nor a successful fake test is
proof that every real installation supports a particular action.
