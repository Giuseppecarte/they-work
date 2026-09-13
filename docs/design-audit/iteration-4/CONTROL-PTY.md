# Control through the real terminal application

This audit launches the compiled `they-work` executable in a real macOS PTY,
types into its controls, and checks the provider requests produced by the
application. It complements the backend's process tests with the complete
UI → host worker → detached supervisor → provider route.

The provider executables are local Python fixtures. Their exclusive PATH,
HOME, Codex home, Claude home, settings and temporary files are under
`docs/design-audit/tmp/iteration-4-control-pty`. The test environment does not
inherit account credentials or provider-session variables. No installed
provider, login, real account, model turn or displayed command is executed.
The fixture's detached host is terminated during cleanup.

## Recorded behavior

- `n` opens New Task. Bracketed paste enters the project and instruction; F5
  reaches `thread/start` with the exact directory and `turn/start` with the
  exact native thread ID and instruction. The UI shows a confirmed receipt.
- F4 shows the exact `test-only-command`, its project/task origin and native
  `managed-1` identity. Pressing the explicit single-request choice sends
  exactly `{"id":"approve-1","result":{"decision":"accept"}}`. Nothing
  executes that command. The provider's resolution removes the pending request.
- A second task creates two explicitly delegated children. The office has four
  distinct people: the first independent task, the second parent and its two
  children. Team shows the actual parent/child hierarchy. Children have observed
  identities without acquiring managed-control ownership.
- A controlled Kitty capability response exercises the graphics path. Keeping
  the two delegated turns active creates a MEETING room; a responsive split
  adds adjacent DESKS only when its people remain readable.
  Their PNG comes from actual emitted Kitty payloads, including the current
  `f=100` PNG encoding, rather than calling the renderer directly.
- Selecting Claude and submitting a new instruction opens the native
  foreground-console path. The fixture receives the exact argument
  `native return; literal $(text)` after `--`, with the chosen working directory
  and source home. It observes a real TTY with canonical input and echo enabled.
  Enter returns to the office panel. The instruction was not interpreted by a
  shell.
- Quitting restores the original terminal settings. Reopening the same saved
  workspace finds the same detached host/provider. The provider log still has
  one `app-server` invocation and two `turn/start` requests: reopening adds no
  instruction and does not replay either task.

The [fallback results](evidence/control-pty-final/results.json) and
[graphics results](evidence/control-pty-graphics/results.json) record the binary
SHA256, assertions and terminal restoration. Their adjacent `provider.jsonl`
and `invocations.jsonl` contain only the local fixtures' requests and arguments.
Both final runs passed against binary SHA256
`b90c70cb5185fc2f6ba5209b0aad20ad2c5ac574c9d02b296205a8709bbd6591`.

The [final full-room pixels](evidence/control-pty-graphics/04-team-office-art.png)
and [adjacent-room pixels](evidence/control-pty-graphics/04-team-adjacent-art.png)
were visually reviewed. The smaller meeting capacity shows the parent and one
child at a time; the native label says `MEETING 1/2` and Team lists both children.
This harness checks the complete family in Team, not traversal of every meeting
page. The [resolved Team view](evidence/control-pty-final/05-team-board.txt)
confirms that the first worker is no longer waiting.

## Criticism and evidence limits

The first end-to-end run found a real state bug: an approval could resolve in
the provider and disappear from pending requests while its worker remained
`waiting` and the office retained a blocked marker. The old activity remained
after `Wait(None)`. The core correction passed the final harness option
`--expect-resolved-state`, which requires that the worker no longer appears
waiting in the real Team view.

The first harness also expected full child names on small desk labels. That
assertion was wrong: the designed labels preserve distinct suffixes while
eliding long names. The corrected test checks four people and two distinct
labels, then checks complete child identities in Team. This was not an
application failure.

The graphics decoder initially expected raw Kitty RGBA (`f=32`); the production
encoder now sends PNG (`f=100`) for these frames. The harness now decodes both
formats. That mismatch was in the audit tooling, not a missing image emission.

Ordinary PNGs here reconstruct native ANSI cells with Menlo. In the graphics
case they show the native text layer; `*-art.png` files show the separately
decoded transmitted artwork. They are not screenshots of a real Kitty window,
and do not prove that a terminal composites either layer correctly. The
[before image](evidence/control-pty-graphics-before/04-team-office-art.png)
exposed a narrow/tall layout defect: splitting the floor constrained people to
the smallest scale and left excessive wall above them. The final responsive
check requires the selected full meeting at 132×42, adjacent rooms at 152×24,
and the same selected parent after returning to 132×42.

The Claude case exercises the official foreground-console handoff interface
using a fake executable whose help omits `--bg`. It does not verify real Claude
authentication, its background supervisor, live attachment, a real approval
dialog, or Windows console behavior. Those limits remain explicit in
[CONTROL.md](CONTROL.md) and [PLATFORM.md](PLATFORM.md).

## Reproduce

Python needs `pyte` and Pillow, as used by the earlier terminal audits. Build
the executable first; the script itself neither builds nor installs anything.

```sh
python3 docs/design-audit/iteration-4/review_control_pty.py \
  --binary target/native-macos/release/they-work \
  --stage control-pty-final --expect-resolved-state
python3 docs/design-audit/iteration-4/review_control_pty.py \
  --binary target/native-macos/release/they-work \
  --stage control-pty-graphics --graphics --expect-resolved-state --check-responsive
```

The test needs permission for a PTY and the local supervisor's loopback socket.
The graphics responder is controlled test input; it never launches a browser or
changes the user's terminal preferences.
