# Main integration and the testing baseline

The `codex/office-preview` branch contains the accumulated application work and
reconciles it with `origin/main` at `2ac0c49528081360ca2efdcee69ab369329ccc23`.
The original audit history remains on `audit/design-and-usability`.

The README installs the current checkout. The published `v0.1.1` screenshots,
their evidence and their capture tools are retained as historical evidence;
that published image does not contain the new office.

## Include the deferred CI work

The active `.github/workflows/` files intentionally match the current main
branch. GitHub accepted this application branch with the existing credential.
The complete follow-up definitions are retained under
[deferred-workflows/](deferred-workflows/): `ci.yml`, `native.yml` and
`release.yml`. This location does not activate them as GitHub Actions workflows.

To include all of the work, copy those three files to `.github/workflows/` in a
follow-up branch and review the diff. Commit and push with a credential allowed
to update workflows, or make the equivalent edits through GitHub's web editor.
Merging the application PR does not add permissions to an existing token.
Recheck for intervening workflow changes before replacing any file.

Do not create a `v*.*.*` tag until that follow-up is integrated and verified.
The old release workflow publishes public image tags before verification and
passes only `--image`; the current verifier requires `--commit` and `--report`
and verifies both Linux architectures. The old invocation will fail.

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
VS Code/Ubuntu WSL settings, use [WSL-PREVIEW.md](WSL-PREVIEW.md). Those archives
were built from the earlier visual candidate; they are not rebuilt or published
merely by merging a PR. Record the binary revision when reporting a test.

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
