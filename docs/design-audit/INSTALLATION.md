# Installation audit — 2026-09-07

The three changes that matter most are a native executable that does not require
Docker, an explicit local source-selection entry point, and release gates that
execute each supported native target. This branch implements those paths. It
does not publish releases, and the documentation says so before giving an
installer command.

## Reproduced findings and fixes

1. **The front door required Docker for a small terminal application.** The
   previous README only offered a Linux/amd64 image. Following the development
   path on this Mac initially found no native Cargo; Docker existed, but the
   sandbox could not reach its socket. An escalated read-only `docker version`
   confirmed Docker Desktop 4.56.0 / Engine 29.1.3 on Linux/arm64. Native
   `cargo install --locked --path crates/theywork-tui` is now the documented
   source install; POSIX and PowerShell release installers are included.

2. **Fresh Docker tests did not bootstrap dependencies.** Running the required
   `./scripts/cargo test --workspace` with an empty Cargo cache attempted DNS
   inside `--network none` and failed. [The exact output](baseline-tests.log)
   preserves this first attempt. `make check` already ran the required fetch,
   but invoking the audit's isolated test command did not. Documentation now
   states the `make fetch` prerequisite clearly. The wrapper also diagnoses a
   missing toolchain or stopped Docker daemon immediately, and prefers native
   Cargo when installed. `THEYWORK_TOOLCHAIN=docker` preserves the offline lane.

3. **Local Docker launch and the downloaded launcher had different behavior.**
   The old Makefile unconditionally bound both source homes with unquoted paths;
   missing homes could be created by Docker and spaces broke arguments. The
   shared launcher now quotes arguments, mounts only existing directories using
   `--mount`, and accepts `make run ARGS="--doctor"` without a TTY. Its demo path
   now mounts no source directory even when those directories exist. Ten
   executed regression tests cover these boundaries and the historical download
   and checksum failures.

4. **The installer assumed GNU checksum tools.** The historical bootstrap and
   new POSIX installer now accept macOS's `shasum -a 256`. The installer tests
   remove `sha256sum` from PATH and execute the fallback. Other tests exercise
   both OS/CPU asset names, paths with spaces, missing releases, duplicate and
   incorrect checksums, archive traversal, and symlink rejection. Failed
   downloads/checksums preserve a previous executable. Nine tests pass locally.

5. **Distribution had no native platform gate.** The new reusable workflow
   builds, runs workspace tests and strict Clippy, then executes and packages
   each native binary on Linux x64/ARM64, macOS Intel/ARM64, and Windows
   x64/ARM64. Linux releases use musl; macOS release builds request a minimum
   deployment target of 11.0. Runner names were checked against the
   [GitHub-hosted runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
   The release workflow waits for those gates before publishing its dual-CPU
   container and native release attachments. YAML syntax was checked locally;
   the remote workflow has not been run from this branch.

6. **Native execution exposed tests hidden by a Linux-only workflow.** A full
   native test run initially failed because a path assertion expected original
   case after normalization, and a live-store smoke test assumed an existing
   Claude directory must contain an active worker. The baseline Docker run also
   exposed assumptions about temporary directories outside a Git checkout and
   same-size file replacement. These were sent to the collector owner with
   reproductions. The initial native failure log redacts the live source's
   identifier/timestamps; fixture-only tests should not depend on personal data.

7. **Package metadata and Docker locking were misleading.** The Cargo repository
   URL pointed at `OWNER`. It now points at the actual repository. The release
   Dockerfile no longer retries a locked failure with an unlocked build or
   silently hides the original build error.

## What was actually executed

Host: macOS 26.6.2 (25G83), Apple Silicon, Apple clang 21.0.0, with Xcode Command
Line Tools. Rust 1.90.0 was downloaded from `static.rust-lang.org`; the
`rustup-init` SHA256 was checked before execution. `--no-modify-path` and
repository-local `CARGO_HOME`, `RUSTUP_HOME`, `TMPDIR`, and build/install roots
kept the toolchain and all temporary files inside the checkout. No user shell
configuration was edited.

| Execution | Result / evidence |
| --- | --- |
| Docker baseline with empty cache | Failed DNS as expected; [baseline](baseline-tests.log) |
| Explicit networked dependency fetch | Passed after updating the lockfile for current source changes; [fetch](evidence/installation/fetch-updated.log) |
| Native macOS release build | Passed, 53.82 s including an empty native build cache; [build](evidence/installation/native-build.log) |
| Documented native Cargo install, with repository-local `--root` | Passed; [install](evidence/installation/native-install.log) |
| Installed binary `--demo --once` | Passed; [output](evidence/installation/native-installed-once.log) |
| Native strict workspace/all-targets Clippy | Passed; [Clippy](evidence/installation/native-clippy.log) |
| Native release archive and help/once/headless smoke checks | Passed; [package](evidence/installation/native-package.log) |
| Release installer against that actual native archive | Checksum, extraction, installation into a path with spaces, and launch passed; [replay](evidence/installation/native-release-install.log) |
| Nine native shell installer regression tests | Passed; [tests](evidence/installation/native-installer-tests.log) |
| Ten Docker launcher/bootstrap regression tests | Passed; [tests](evidence/installation/docker-installer-tests.log) |
| Workflow YAML parsing | Passed; [syntax](evidence/installation/workflow-syntax.log) |

The archive replay substitutes a local `curl` fixture for GitHub downloads,
because this branch's native assets are not published. It is not evidence of
public release availability. That distinction is also explicit in `INSTALL.md`.

To reproduce with the installed audit-only toolchain:

```sh
sh docs/design-audit/native-cargo.sh build --locked --release --bin they-work
sh docs/design-audit/native-cargo.sh clippy --locked --workspace --all-targets -- -D warnings
sh docs/design-audit/native-cargo.sh install --locked --path crates/theywork-tui --root docs/design-audit/tmp/installed
python3 scripts/package-release.py --target aarch64-apple-darwin --binary target/native-macos/release/they-work --out-dir docs/design-audit/tmp/native-dist
python3 docs/design-audit/replay-native-install.py
python3 scripts/test-native-install.py
python3 scripts/test-install.py
```

`native-cargo.sh` is an audit reproduction helper, not an end-user installer.
Its ignored Rust toolchain must exist before running it. Normal installations
use the documented system Rust toolchain or a published native archive.

## Remaining limits and self-review

- The POSIX installer and the macOS native binary were executed. Windows
  PowerShell tests are implemented and wired into both Windows CI jobs, but no
  Windows runtime was available here. Windows, Linux musl, and macOS Intel
  release artifacts remain unverified until those jobs actually run.
- The native installers cannot download assets that have not been published.
  They fail with a source-build instruction instead of reporting success. The
  source installation is usable now; no fictional release version is presented
  as available.
- This installation lane exercised native executable output and headless
  rendering, not the appearance of every terminal emulator. Interactive visual
  evidence belongs to the broader audit; macOS execution does not prove Kitty,
  iTerm2, Windows Terminal, SSH, or WSL graphics behavior.
- Checksums detect corruption, not a compromised release publisher. Signing,
  notarization, Homebrew, Winget, and package-store publication were not added;
  they would imply distribution work that was not performed in this audit.
- Local source selection replaces the requested login-like step. The app reads
  selected local records; it does not need remote credentials. Persistence
  remains an explicit `--config-dir` choice, documented for native and Docker.
