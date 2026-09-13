# Iteration 5 — inspection, requests and task navigation

The office now exposes people and their real tasks together. Decorative names
are searchable alongside source titles; inspection retains the original title,
provider, observed state, recent work, reported outcomes, token usage and family.
A narrow panel preserves these distinctions without pretending the character is
the provider's actual identity.

## Changes

- `presentation.rs` shares language for human requests, automatic review, waits
  for other agents/processes, missing recipients and source coverage. Unavailable
  or stale observations take precedence over present-tense working/ready labels.
- `views/inspector.rs` renders in a supplied rectangle. The coverage line stays
  fixed above its scrollable history. Reported command exit codes, file changes
  and compact token counts remain available. Observation-only tasks explain that
  instructions belong in the original conversation. Buttons follow the current
  control capabilities; a current request gets first position even at 40×12.
- `views/control.rs` distinguishes new work, task controls, requests and
  connections. Visible buttons support Tab/Shift-Tab and Enter, with the existing
  shortcuts retained. A focused button cannot accidentally edit an unseen draft.
  The project picker preserves the exact known folder, displays its selected
  path (with a leading ellipsis when necessary), and removes duplicate paths.
  Connections includes the local source chooser.
- Approval hit targets carry the exact request ID and response, then revalidate
  them against the current request. Changing/expiring a request invalidates an
  old click. Hidden response buttons cannot be invoked by their numeric shortcut
  after the panel has been rendered. Input forms and controls stop accepting
  actions below their supported geometry.
- `views/workboard.rs` defaults to attention across all floors, with an explicit
  floor filter. Human questions/approvals are distinguished from follow-ups.
  Enter on a current human request opens its request review; marking seen remains
  a local reading marker. Deliveries use plain-language event descriptions,
  preserve unknown recipients, and hide native IDs behind an explicit expansion.
  Related or retired workers retain their recorded deliveries and relationships.
  Opening Team for a particular worker selects that person and expands its
  ancestors, rather than silently selecting the first roster entry.
- `views/finder.rs` searches character names and original task titles and routes
  each mouse target to the exact floor or worker. Automatic review is not indexed
  as a request for human approval.

## Integration contracts

Each stateful view exposes `hit_regions()` for its last rendered frame.
`ControlPanel::activate(Command)` validates action identity/capability before a
host operation. The main UI owns mouse routing and presentation acknowledgement;
these views do not run a provider. `select_project(String)` accepts known paths;
`show_requests(Option<WorkerId>)` scopes a review without converting it into a new
work form. `Command::Sources` routes to the local chooser.

The inspector returns hit regions from
`draw(frame, area, &InspectorContext { world, worker_id, profiles, status, now,
scroll })`. Finder and notebook expose `refresh_with_profiles`; notebook
`scope_all` is true by default and `select_key(&str)` uses a stable entry key.
`focus_worker(WorkerId)` runs after `show(Team)` and selects its row after refresh.
Marking uses an exact notebook key, not the currently highlighted row index.

## Verification and critique

The focused suite passes **41 tests**: control 12, notebook 17, finder 6,
inspector 3 and presentation 3. The [test log](evidence/ux-tests.log) includes the
final small-height inspector regression after the other focused suites.
[Strict renderer library Clippy](evidence/ux-clippy.log) passed.

Both final PTY runs tested release SHA-256
`5de1411f62fe09a81b78868a4e8193e421fdacb10ab6aba8864ca57392422c8b`.

The native macOS PTY exercise uses the actual release executable and generated
fake provider programs under an isolated repository-local HOME. It performs no
login, invokes no installed provider and uses no personal conversation store.
The fake native console receives a real controlling terminal. The harness does
not execute the command displayed by the fake approval request.

The [120×36 PTY results](evidence/control-pty/results.json),
[80×24 PTY results](evidence/control-pty-80/results.json) and their protocol logs
record the release hash and establish:

1. Clicking Start task sends the literal project and instruction once.
2. REVIEW REQUEST exposes the command and the exact request; clicking its visible
   choice sends exactly one `approve-1` response with `decision: accept`.
3. Selecting a known project preserves its exact directory for a second task.
4. Native-console handoff receives canonical input and echo with mouse capture
   released. The office captures mouse input again after the console returns.
5. Connections opens Local sources by click. Clicking a source title changes its
   pending choice; clicking its folder opens the editor. Cancel restores the
   original folder, and Back leaves saved source choices byte-for-byte unchanged.
6. Both exits restore terminal modes. Reopening with `--mouse=off` emits no mouse
   capture enable sequence, ignores source title/folder clicks, reuses the
   provider process and replays no task.

Screenshots are ANSI-cell reconstructions from the PTY, not physical-terminal
screenshots. The raw streams preserve the actual mouse enable/disable sequence.
The first harness attempt ran into the sandbox's loopback bind restriction;
a later harness correction selected the action button rather than duplicate
read-only text in the request body. Neither issue was treated as a product bug.

During critique, the fixed coverage warning, usage/history omissions, short-panel
request priority, duplicate project paths and ambiguous picker names were fixed.
The initial PTY also exposed redundant local/global footer hints; the UI
removed the duplicate footer before the final captures. The source folder editor
now explicitly says Enter applies and Esc cancels instead of showing the previous
folder's readiness while a new path is being edited. This exercise does not certify remote provider services,
Windows terminal input, screen-reader behavior, or physical graphics protocols.

Reproduce from the repository root (requires the existing Python rendering
packages documented by the iteration-2 harness):

```sh
python3 docs/design-audit/iteration-5/review_control_pty.py \
  --binary target/native-macos/release/they-work
```

The supervisor binds an authenticated loopback socket, so an environment that
forbids local socket binding must grant that test capability. The fixture logs
and generated homes remain under `docs/design-audit/`; fixture processes started
by the harness are stopped at the end.
