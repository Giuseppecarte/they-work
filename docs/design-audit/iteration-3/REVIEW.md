# Third design and usability review

This iteration follows `bfde837` on `audit/design-and-usability`. It addresses
the user's request for a more complete design and easier daily operation before
publication. No push, merge, or release publication is part of this work.

## Decisions tested against the running application

| Problem | Final behavior | Evidence and self-critique |
| --- | --- | --- |
| A wall of feeds did not explain independent projects; navigation required remembering floors | Numbered floor directory and selected furnished office, with staffing, attention, pagination and visible overflow | [Tower](TOWER.md), one/six/twenty projects and one-worker floor |
| Finding a conversation meant visiting each project | `/` or Ctrl+K searches names, paths, providers, branches and states; Enter resolves the chosen live identity | [Finder](FINDER.md), 20 floors/60 conversations, paste, resize, Unicode editing and exact target |
| The desk clipped task context and current requests; Phone confused historical events with current state | Wrapped task title, state/request/next step first; UTC history position and paging; Now/Attention/Edits/Messages with stable selection | [Inspection](INSPECTION.md), explicit request, silent turn, error, work and completed task |
| Connection setup required CLI knowledge to remember choices and made path repair awkward | Local source selection, cursor-based folder editing, explicit errors, default settings location and optional remembering | [Onboarding](ONBOARDING.md), restart/temporary/demo/cancel/no-save/corrupt settings and changed launch directory |
| A pending command lost its suffix before layout | 2,000-character current requests with visible truncation; short captions remain bounded | [Source detail](SOURCE_DETAILS.md), SQLite and Claude regressions, native inspector evidence |
| Images covered native labels and help, and clearing an old image erased freshly drawn text | Compose image backgrounds and native text in a defined order, preserve wide cells and input cursor | [Graphics](GRAPHICS.md), three encoders, overlay transitions and compiled PTY runloop |
| The light theme reused faint colors and left black gaps outside short panels | Dedicated readable text/status tokens and a painted full-window background | [Finder contrast](evidence/finder-final/contrast.json), actual buffer measurements and Menlo replay |
| The final demo smoke test showed billions of fictitious tokens and never demonstrated a current request | Bounded sample usage plus a clearly fictional approval and an example error, alongside animated work | [Demo smoke output](evidence/demo-once.txt) and a regression using real epoch timestamps |

The first pass in each area was rejected where execution showed a defect.
Notable second-pass findings were contradictory last-activity copy, unlabelled
hidden workers, an invisible Phone heading, nonuniform history backgrounds,
silent request truncation, a relative source path that changed meaning after
restart, and light colors with contrast as low as 1.30:1. These were corrected
before accepting the final evidence. Live identity tests also cover a result
leaving the tower between selection and the next frame.

## Verification

Final command results and artifact identity are recorded in
[`evidence/verification.json`](evidence/verification.json). Required commands:

The final native and Linux workspace runs each pass **258 tests**, with zero
failures and two opt-in live-store tests ignored. Strict Clippy passes on both
toolchains, formatting is clean, and the native release passes `--help`, the
fictional `--once` smoke, and the compiled graphics PTY exercise. The recorded
SHA-256 binds that last PTY run to the executable offered for local testing.

```sh
./scripts/cargo test --workspace
./scripts/cargo clippy --workspace --all-targets -- -D warnings
sh docs/design-audit/native-cargo.sh test --workspace
sh docs/design-audit/native-cargo.sh clippy --workspace --all-targets -- -D warnings
sh docs/design-audit/native-cargo.sh fmt --all -- --check
```

Linux commands run in the repository's Docker toolchain; native commands run
with Rust 1.90 on macOS ARM64. Two collector tests are deliberately ignored by
the standard workspace command because they require explicit opt-in to read
live personal agent stores; no ignored test is counted as passing. Visual goldens are
updated deliberately after reviewing the new layouts, never as a substitute
for running the binary.

The native PTY scripts in this directory use isolated synthetic source folders
and configuration. They check key sequences, exact inspected workers, clean
exit and restored terminal modes. Existing sprite/animation and allocation
regressions remain in the full suite. The previous iteration's broad motion
survey is historical supporting evidence, not a new run of those 116 cases.

## Visual evidence and limits

PNG files are reconstructions of actual PTY output. `*-menlo.png` files replay
those cells through installed CoreText/Menlo fonts. They are **not terminal
window screenshots**. The old replay script clips a two-cell CJK glyph to one
cell, so Unicode acceptance uses captured text and cursor position rather than
claiming that defective raster proves the terminal's appearance.

The graphics PTY responds to capability queries with controlled Kitty replies.
It proves negotiation, actual compiled runloop output order, retained native
headings and terminal restoration. It is not an interactive Kitty application.
No Windows/WSL, Linux graphical terminal, Kitty or iTerm2 visual session was
available. Terminal.app access was declined earlier and was not bypassed.

The app observes bounded local records. It cannot create or approve a provider
request, and a quiet turn is not proof of an approval. The inspector directs
action to the original conversation. A directory is used for the tower instead
of allocating the terminal to a literal continuously scrolling skyscraper.
Search maintains a stable popup size while typing; an exact match therefore
leaves some intentional blank space. Earlier portraits in a multi-portrait
view use character rendering; the final canvas receives image presentation.

## Try the integrated native build

```sh
./target/native-macos/release/they-work --demo
./target/native-macos/release/they-work --setup
```

The first command uses fictional data. The second opens local source selection.
`0` shows the tower, `/` finds a task, Enter inspects, `c` changes sources, `?`
opens help, and `q` quits. The native executable is local build output, not a
published release. Other platforms can use the source installation described
in the repository's installation guide and their native compiler/toolchain.
