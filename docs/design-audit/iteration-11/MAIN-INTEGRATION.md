# Main integration and the testing baseline

The `codex/office-preview` branch contains the accumulated application work and
reconciles it with `origin/main` at `2ac0c49528081360ca2efdcee69ab369329ccc23`.
The original audit history remains on `audit/design-and-usability`.

The README installs the current checkout. The published `v0.1.1` screenshots,
their evidence and their capture tools are retained as historical evidence;
that published image does not contain the new office.

## CI is included in the same PR

After the owner enabled workflow permission, the CI definitions were restored
under `.github/workflows/`. The definitions under
[deferred-workflows/](deferred-workflows/) remain a historical record of the
permission-related split; the active workflow directory is authoritative.

CI runs the normal verification suite, Cargo wrapper contracts, native builds
for six targets, the macOS/Linux reproduction smoke and Windows storage
contracts. The release workflow runs only for `v*.*.*` tag pushes. It verifies
both container architectures and native archive checksums before public image
tags change. Ordinary branch and PR pushes do not publish releases.

The application-only candidate `92c26ba` passed both its
[push CI](https://github.com/Giuseppecarte/they-work/actions/runs/34773134733) and
[PR CI](https://github.com/Giuseppecarte/they-work/actions/runs/34773175922).
The restored native matrix must be assessed from its own run on the new commit;
those earlier checks did not execute the native jobs. Public release execution
and real-terminal certification remain separate from a passing CI build.
The [native CI review](CI-RESTORATION.md) records failures exposed by the first
matrix, their corrections and the distinction between debug correctness and
the isolated optimized frame-budget check.

## Use main as the application baseline

Review and merge the preview PR into main, then update a clean checkout:

```sh
git switch main
git pull --ff-only origin main
cargo install --locked --path crates/theywork-tui
they-work --demo --no-save
```

The source build requires the Rust toolchain and native compiler described in
[INSTALL.md](../../../INSTALL.md). For the prepared Linux binaries and the
VS Code/Ubuntu WSL settings, use [WSL-PREVIEW.md](WSL-PREVIEW.md). Each successful
native CI job uploads an archive for that run's source commit. Download from
the intended branch or main run; merging does not publish a public release.
The original local visual-preview archives remain bound to their earlier
candidate. Record the binary revision when reporting a test.

After checking the demo, quit and run `they-work --setup` to choose existing
local sources. Use `they-work --doctor` directly in the WSL terminal to record
actual image support. Compositor and automated test results do not establish
real-terminal or authenticated-provider acceptance.

## Evidence and limits

The first pushed preview (`8ba2b28`) failed its GitHub workspace tests. The
authenticated log identifies two causes: a fixture compared unquantized RGB
while using terminal-dependent color detection, and parallel tests exceeded
the existing 5-second encoding budget (5,911 ms). The fixture now explicitly
chooses truecolor. `scripts/cargo test` defaults to one test thread when
`CI=true`, with explicit environment and command-line overrides preserved;
Docker receives the same setting. No production rendering code or performance
threshold was changed for these fixes.

Local integrated results on macOS ARM64 with Rust 1.90.0: formatting, strict
all-target Clippy and release build pass; the locked workspace suite executes
471 passing tests and 4 ignored tests. The harness-free storage executable
separately reports three passing process cases. Release helper tests pass
(23 tests), and the new Cargo-wrapper contracts pass (10 process tests).
The color fixture also passes with `TERM=dumb` and color overrides unset.
All regenerated golden files match the preview bytes. Remote CI on the new
commit must be checked separately; the original failed run is retained as
failure evidence, not replaced by the local pass.

Integration results are recorded under `main-integration/`. The iteration-11
visual evidence remains bound to its original candidate and is not relabelled
as evidence for a new commit. The merge adds adapted regression coverage for
the upstream authored-character sizing fix and preserves recorded release
verification summaries.

Native Windows jobs, authenticated-provider sessions, the five-person study and
physical-terminal latency remain separate gates. Main can serve as a local
testing baseline without claiming publication certification.
