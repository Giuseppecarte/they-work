# Iteration 4 — implemented work and remaining acceptance

The accepted plan is implemented on `audit/design-and-usability`: source-scoped
people and observed teams, a continuous graphical tower, local attention and
delivery markers, managed Codex controls, and official Claude console handoff.
This is a locally testable implementation, not a certification of every terminal,
provider version or workflow. No release was published and no branch was merged.

The three most consequential defects fixed were missing or falsely related
subagents; commands or acknowledgements that could target a different task after
the selection changed; and artwork/text compositing that hid native labels or
shrunk people until their features disappeared.

## What changed and why

| User need | Implemented behavior | Evidence and limitations |
| --- | --- | --- |
| Understand delegation | Provider + source + native identity; confirmed parent edges, session membership, forks, directed messages and final results remain distinct. Missing people remain unknown. | [Data review](DATA.md), collector fixtures and managed connection fixtures. Local transcript formats can change; coverage stays partial. |
| See an actual team | Active delegation families occupy a meeting room; independent tasks retain desks. The lead remains visible while members paginate. Nested relationships remain inspectable in the tree. A worker is counted once. | [Graphical exports](ART.md), actual UI buffer tests at 1, 6 and 20 projects. The room depicts recorded family membership, not an invented discussion. |
| Act on the correct task | `m` pins the recipient; `n` scopes drafts to provider and project. `F5` sends with a receipt, `F2` interrupts an exact managed turn, `F4` answers a concrete current request. Old or unsupported requests cannot silently become a new approval. | [Control review](CONTROL.md), UI input tests and the native executable PTY fixture. External histories provide observation, never execution authority. |
| Keep work alive | A private source-bound Codex supervisor outlives the view. Reopening discovers it. Restarted hosts do not replay prompts; `F6` reconnects remembered history explicitly. | Detached-host/restart tests. Managed Codex creation requires remembered local settings; `--no-save` explains the restriction. |
| Use the normal Claude login and console | Capability detection chooses official background mode or foreground fallback. A verified native roster permits attach. The scene releases the terminal and restores it on return, including signal handling. | [Platform and native-console evidence](PLATFORM.md). Real installed provider/account sessions still need acceptance. |
| Supervise many floors | `b` separates attention, deliveries and changes since entry; `g` shows team trees. Local seen/reviewed marks never resolve provider work. Evidence and incomplete coverage are visible. | [Notebook review](WORKBOARD.md), including absent senders, repeated notifications and narrow layouts. History is bounded, not a permanent transcript archive. |
| Recognize people and read the office | Twelve original 48×64 masks, integer scaling, stable identity, contact shadows, coherent materials, a shared elevator, large signs and native labels. Max two deterministic decorative situations per floor; reduced motion freezes them. | [Art review and captures](ART.md). Outfits may repeat; they do not change an agent's instructions. Other projections retain their compatible compact art. |
| Install locally across platforms | Native packaging and installers, source/login diagnostics, observation-only Docker, explicit Windows native-CLI and WSL boundaries. | [Platform review](PLATFORM.md). Packaging and compile checks do not imply Windows/WSL runtime certification. |

## Critical review and corrections

The review did not accept the first passing unit-test result as completion.
Independent inspection and running the executable exposed these problems:

1. **Native labels disappeared or contained colored stripes.** The image canvas
   left temporary skip flags and block glyphs underneath text regions. Labels
   now clear both the flags and symbols before native rendering. Reconstructed
   80×24, 120×36 and 192×58 exports use the real final mask, without repairing
   the screenshot afterwards.
2. **Small rooms traded faces for empty walls.** People now retain integer 2×
   art at the standard small graphical geometry; team members paginate with a
   pinned lead. Larger rooms use 4×. Adjacent rooms share one elevator shaft.
   A final real-executable graphics capture exposed the same problem when a
   tall floor was divided in half. Splitting now requires enough aligned width
   to preserve the intended character scale and fit the lead plus a participant.
   Otherwise the selected room uses the floor width and colleagues remain
   reachable through ordinary worker navigation and Team.
3. **An automatic review looked like a human approval.** Wait reasons, hand
   gestures, labels and notebook categories now distinguish people, processes,
   teammates and unavailable information. A stationary native alert remains
   visible during decorative movement.
4. **The request panel could approve from incomplete or foreign item text.**
   Details are joined by thread, turn and item identity. Accept is unavailable
   without the concrete command or complete matching file changes. Unknown
   schemas and expired requests remain unavailable. A global request displays
   its own project/task origin.
5. **An acknowledgement could erase another draft.** Pending sends retain the
   original target and text. A confirmed receipt clears only that draft.
   Provider/project changes also retain independent new-task drafts. Typing
   cannot activate global office shortcuts.
6. **Leaving for the native console could accumulate polling events.** The
   collector pauses, drains its bounded pending batch, and restarts with its
   existing cursors. Provider controls remain owned by their supervisors.
7. **A resolved approval still looked blocked in the real executable.** The
   PTY fixture answered the exact native request and observed an empty pending
   queue, but the model retained its old waiting activity. Explicit wait
   resolution now clears that presentation without inventing new tool activity;
   later or simultaneous pending requests retain priority.
8. **A blank delivery panel wasted its scarce rows and team names lost project
   context.** The notebook gives a single result more detail space, prefixes
   All floors entries with their project, and advertises only available marks.

## Verification

Integration host: macOS 26.6.2 (25G83), arm64, Rust 1.90. Tests use synthetic
provider data, isolated source homes and offline provider stubs. No real model
turn or provider login was performed by the verification harnesses.

The reproducible final commands are:

```sh
sh docs/design-audit/native-cargo.sh test --locked --workspace --offline -- --test-threads=1
sh docs/design-audit/native-cargo.sh clippy --locked --workspace --all-targets --offline -- -D warnings
sh docs/design-audit/native-cargo.sh fmt --all -- --check
sh docs/design-audit/native-cargo.sh build --release --bin they-work --locked --offline
python3 docs/design-audit/iteration-4/review_control_pty.py --stage control-pty-final
```

The PTY replay requires the existing isolated `pyte` and Pillow audit dependencies
described in the script's base harness. The repository-local Cargo wrapper is
optional; installed Cargo can run the same arguments. Process fixtures require
local loopback sockets and PTYs, never a remote service.

| Final integrated check | Result |
| --- | --- |
| [Workspace tests](workspace-tests.log) | 340 passed, zero failures, two opt-in personal-store checks ignored. Includes 182 renderer tests, golden views, collector fixtures, local runtime controls and CLI integration. |
| [Strict workspace Clippy](clippy.log) | All targets passed with warnings denied. |
| [Formatting](fmt.log) | `cargo fmt --all -- --check` exited 0. |
| [Release binary](release-build.log) | Optimized macOS arm64 executable built successfully. |
| CLI smoke | [Help](cli-help.log), [demo `--once`](cli-demo.log) and [disconnected doctor](cli-doctor.log) each exited 0. |
| [Complete executable PTY route](CONTROL-PTY.md) | Both fallback and controlled Kitty passes succeeded: exact request/reply, parent/two children, responsive room composition, resolved approval cleared, native-console return, terminal restoration and reopen without replay. |

[Machine-readable verification](verification.json) records the executable hash
and check scope. Earlier lane reports retain their original scope and counts;
they must not be added together as independent tests. The Linux checks preceded
the final portable UI/state corrections; their log scope is recorded separately
in [PLATFORM.md](PLATFORM.md).

The workspace contains pre-existing wall-clock render-budget checks. Two parallel
runs failed the unchanged five-second limit while sharing CPU with other tests
(5.99 s and 5.34 s). The first failure is retained in
[workspace-tests-contended.log](workspace-tests-contended.log). Canonical Make
and CI test commands now run test cases serially, with the same thresholds.
This avoids measuring unrelated concurrent test load as renderer cost.

[Input-to-composition measurements](input-composition.csv) cover 50 tasks across
1, 6 and 20 projects at 80×24, 120×36 and 192×58. Each cell has 10 warm-up frames
and 60 recorded samples using the release compositor; the largest measured p95
is 15.502 ms. This measures the office key handler, draw and native-background
composition only. It excludes graphics encoding, terminal output, OS painting
and physical display, so it does **not** establish the complete 150 ms target.
Separate Kitty/iTerm2/Sixel encoding measurements are in [ART.md](ART.md).

## What still prevents a finished-product claim

- **Actual provider acceptance:** authenticated Codex app-server and Claude
  foreground/background sessions with current installed versions, including
  real tool approvals, disconnects and native console restoration. Fixture
  success verifies our contract, not every evolving provider schema. General
  MCP elicitation forms remain unsupported in the UI beyond decline/cancel.
- **Terminal and platform acceptance:** real graphical playback and recordings
  in Kitty, iTerm2 and Sixel terminals on macOS, Linux, Windows and WSL. The
  PNGs replay real UI buffers; PTYs run the actual executable. Neither is an
  emulator-window screenshot. Terminal.app control was not authorized during
  this work and was not used. Windows runtime, MSVC and the full native release
  matrix remain unexecuted locally; Windows GNU compilation is narrower evidence.
- **Full input latency:** measure event-to-visible response p95 on those actual
  terminals with their versions, fonts, cell sizes and graphics transports.
- **Five first-time users:** the ten-second attention test and unaided explanation
  of parent, subtasks and results have not been conducted. Until then, discoverability
  of `b`, `m`, `n` and the meeting metaphor is a hypothesis. At 80 columns native
  names are abbreviated and require selection for their full title.
- **Persistent-history expectations:** recorded coverage is bounded and can be
  incomplete. A result is a turn output, not a project-complete claim. Costume
  repetition and truncated names can still require explicit inspection.

These are release acceptance limits, not passing checks. The implementation can
be tried locally with `./target/native-macos/release/they-work --demo` and the
controls are documented in [CONTROLS.md](../../CONTROLS.md).
