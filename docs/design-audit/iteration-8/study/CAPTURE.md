# Owner-operated terminal capture

No physical terminal has been controlled for this study preparation. The owner
or participant performs these steps manually in a terminal they choose. The
PTY evidence elsewhere is not proof that the same font, transport or input
works in that terminal window.

The preparation helpers are separately preflighted on macOS (see [PREFLIGHT.md](PREFLIGHT.md)) and reuse its Menlo font
and Unix PTY audit dependencies. They are not validated study launchers for
native Windows, Linux or WSL. The protocol and manual capture checklist apply
there, but a moderator must first prepare and verify an equivalent isolated
fixture on that platform. Do not classify a research-tool setup failure as a
participant failure or use it to claim the product cannot run there.

1. Record app binary SHA-256, OS, terminal name/version, font/size, rows/columns,
   zoom, color mode, motion preference and graphics capability. Capture
   `they-work --doctor` from the **isolated fixture launcher** below; do not
   accidentally inspect the participant's default sources.
2. Run `prepare_session.py P01` with Python including Pillow and the existing
   audit `pyte` dependency. It writes only inside
   `docs/design-audit/tmp/iteration-8-study/study-P01` and prints a launcher
   path. Slots P01–P05 are supported. Use that same directory for every condition
   so source identity and characters stay stable. `--reset` explicitly replaces
   only this generated fixture for a fresh session.
3. Freeze [baseline.json](baseline.json) after the candidate checks and physical
   dry run, recording its binary hash and source commit; see [GATES.md](GATES.md).
   `--preflight` bypasses that lock only for engineering preparation and must not
   be used for participant sessions. Invoke the printed launcher with `--condition tower`, `compact` or `reduced`.
   It restricts HOME, config, source paths and provider executables to the
   generated fixture. Tower/reduced use normal terminal capability detection;
   compact requests half-blocks and receives the native roster. If the terminal
   cannot show images, mark the graphics condition unavailable. Never falsify a
   terminal's capability reply in a participant session.
4. Keep the participant's usual readable size. Save a normal screenshot of the
   whole application window, including the title bar or terminal identity if
   useful, before the first task, at a breakdown and after returning. Do not
   resize or change font mid-task without logging it. Record whether captures
   are operating-system screenshots or ANSI replays.
5. If the participant agrees to video, use their operating system's own capture
   controls. The moderator does not remotely drive the window. Otherwise log
   timestamps/actions and take stills. Screen recording is not required for a
   valid session. Avoid recording unrelated windows or personal conversations.
6. For the return task, run `prepare_session.py P01 --add-result` in the
   moderator's own terminal while the participant is in Alpha. This updates a
   synthetic Beta record; it neither sends a message nor starts a provider task.
7. Exit the app with `q` from its main view. Record any failure to restore normal
   input, copy/paste, scrolling or cursor. Preserve the local fixture's
   `invocations.jsonl` if the optional fake action task was used; it identifies
   exactly which stub was called. Never attach authentication files.

Example preparation from the repository root:

```sh
python3 docs/design-audit/iteration-8/study/prepare_session.py P01
python3 docs/design-audit/iteration-8/study/prepare_session.py P01 --add-result
```

The first command prints an executable Python launcher and precise invocation.
Its `--doctor` option runs the existing app's diagnostic mode with the same
isolated source/config environment. The optional approval task uses the local
Codex-shaped stub; it is not evidence of authenticated provider interoperability.

Capture files belong under a participant-approved location inside the audit
folder when added to this repository. Keep consent records separately and do not
commit identifying participant information. A missing screenshot or unavailable
graphics condition stays missing; do not substitute an expert replay and call
it participant evidence.
