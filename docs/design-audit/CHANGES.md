# Implemented changes

Work is on `audit/design-and-usability`, created from `main`. Commits separate installation, navigation, rendered references, data correctness and the host connection flow. No merge, push or release publication was performed.

## Product behavior

- **Connections before collection:** a local source screen, editable folders, provider switches, all-disconnected mode, `--setup`, `--sources`, `--codex-home` and `--claude-home`. `c` opens it again. Demo reads no conversation records. No remote account or API credential is requested.
- **A persistent tower:** selecting a floor changes presentation; `--project` alone limits input. Git worktrees with available pointer metadata share their primary project's floor. Twenty-project views page and preserve selection. Floors keep stable identities and numbers; `!` opens the next worker needing attention.
- **Recognizable workers and rooms:** authored 24×34, 14×20 and 7×10 character maps replace fractional squeezing. `w`/`W` selects or resets a conversation's fictional character. `o`/`O` selects or resets a project's room palette. Portraits, floor workers and camera feeds share those choices.
- **Useful inspection:** known questions and approval requests require attention immediately. A timeout is labelled as uncertain attention, without an invented pending command. Desk timelines, phone summaries, status counts and shortcuts use consistent state semantics.
- **Remembered choices:** `--config-dir` saves sources, appearance, character choices, room choices and the real selected floor. Explicit startup camera and theme options set absolute values. Automatic terminal capability choices are not frozen into settings.
- **Reliable live data:** current root-level Codex databases take precedence over migration leftovers; legacy and mixed paths remain supported. Empty Codex installations are watched for their first session. A bounded checkpoint detects Claude tail rewrites when mounted filesystems round timestamps. Reconnection neither replays history nor loses the last queued batch.
- **Native distribution:** POSIX and PowerShell installers verify checksums; source installs and release packaging produce a single executable. The CI/release matrix covers Linux, macOS and Windows x64/ARM64. Docker remains available. Native assets are prepared on this branch and are not yet published.

## Review-driven corrections

The first working version was challenged again. That review found replayed collector history, a dropped in-flight batch, lost configuration across demo transitions, fictitious demo paths being saved as real projects, incorrect CLI toggles, quoted-tilde configuration paths, key-release double actions, and a small-terminal default with too much empty space and no artwork. These were corrected and covered by targeted tests or native PTY reproduction. The compact office review also caught a diagonal sign colliding with its windows; compact projections now use a clean horizontal title strip.

Running outside the original Linux environment also exposed WSL path case-folding, native Windows profile/metrics assumptions, tests that depended on a system temporary directory living outside any Git repository, and tests that implicitly scanned private host stores. Those assumptions were removed or made explicit.

## Deliberate choices

The application remains a read-only observer. Its connection screen grants access to local records; it does not impersonate provider authentication or approve commands. Character quirks are decorative and do not alter a model's behavior. Room customization uses four coherent material palettes instead of unrestricted pixel editing. A floor directory with office feeds is used instead of a literal animated skyscraper. The settings preview is compact; the full room is immediately visible in the floor view.

Fallbacks are judged at actual terminal-cell budgets. Tiny displays prioritize readable controls and status over preserving a composition that cannot fit. Source metadata cannot prove every approval state, and missing worktree metadata is not replaced with guesses about repository identity.

## Verification and reproducibility

Final Linux ARM64 and macOS ARM64 runs each passed **201 tests**, with two live-store tests explicitly ignored. Workspace Clippy with warnings denied and the formatting check passed. Twenty-one installer and Docker-launcher regression tests passed. The native PTY harness also passed setup, inspection, customization, persistence, all-disconnected mode, source cancellation, demo round-trips and terminal restoration after normal exit and SIGTERM. Machine-readable results are in [verification.json](evidence/verification.json).

Run from the repository root:

```sh
./scripts/cargo test --workspace
./scripts/cargo clippy --workspace --all-targets -- -D warnings
./scripts/cargo fmt --all -- --check
python3 scripts/test-native-install.py
python3 scripts/test-install.py
```

The first Docker build needs `THEYWORK_CARGO_NETWORK=bridge ./scripts/cargo fetch --locked`; subsequent checks run with the offline cache. If native Rust is present, the wrapper uses it. Audit toolchains and temporary fixtures are kept in ignored repository directories.

- [Workspace tests](evidence/workspace-tests.log), [strict Clippy](evidence/clippy.log).
- [Native macOS tests](evidence/installation/native-tests-final.log), [native Clippy](evidence/installation/native-clippy-final.log).
- [Native connection/navigation/persistence/cleanup results](evidence/native-pty/results.json) and [reproduction script](check_pty.py).
- [Native installation and release verification](INSTALLATION.md).
- [Visual measurements and before/after frames](VISUAL.md), [navigation regressions](USABILITY.md).

`check_pty.py` runs the compiled native binary against a synthetic Claude fixture. It requires `pyte` and Pillow in `docs/design-audit/tmp/python` (`python3 -m pip install --no-cache-dir --target docs/design-audit/tmp/python pyte pillow`). Its screenshots reconstruct ANSI cells with Menlo; they are not screenshots of Terminal.app. `native-cargo.sh` reuses the isolated audit Rust installation when present. The ordinary project does not depend on that audit toolchain.

The platform and human-test limits are stated in [FINDINGS.md](FINDINGS.md). A passing encoder or a configured CI runner is not reported as a successful native graphics session.
