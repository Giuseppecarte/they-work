# Installing they-work

Run the office directly in your terminal on macOS, Linux, Windows, or WSL.
Docker is optional. A native process reads only the local sources you select;
Docker additionally enforces read-only data mounts and disables runtime networking.

## Install this checkout now

The changes on this branch have not been published as native release assets.
The older `v0.1.0` release is Docker-only. To run this version today, build from
this checkout with Rust 1.90 or newer and a C compiler (SQLite is bundled).
Install Rust from [rustup.rs](https://rustup.rs).

| Platform | C compiler prerequisite |
| --- | --- |
| macOS | Xcode Command Line Tools: `xcode-select --install` |
| Debian/Ubuntu/WSL | `sudo apt install build-essential` |
| Windows | Visual Studio Build Tools, “Desktop development with C++” workload |

From the root of this checkout, these commands work in a shell or PowerShell:

~~~sh
cargo install --locked --path crates/theywork-tui
they-work --demo
~~~

The binary goes in Cargo's user bin directory. Follow rustup's PATH instructions if your current
terminal does not yet recognize `cargo` or `they-work`; opening a new terminal
usually applies them. Press `q` to leave the demo.

## Native release installers

After a release with native assets is published, a reviewed local copy of the
installer downloads just the executable for your OS and CPU, verifies its
SHA256 checksum, and installs it without administrator access:

~~~sh
sh scripts/install.sh
# Optional: sh scripts/install.sh --version vX.Y.Z --install-dir "$HOME/.local/bin"
~~~

On Windows PowerShell:

~~~powershell
powershell -ExecutionPolicy Bypass -File .\scripts\install.ps1
# Optional: .\scripts\install.ps1 -Version vX.Y.Z -InstallDir C:\Tools\they-work -NoPath
~~~

The POSIX installer uses `curl`, `tar`, and either `sha256sum` or macOS's
`shasum`. It installs into `~/.local/bin`, or `--install-dir`, and prints a
working full-path launch command if that directory is outside PATH. The Windows
installer uses built-in PowerShell/.NET, installs into
`%LOCALAPPDATA%\Programs\they-work`, and adds that directory to your user PATH
unless `-NoPath` is passed. It updates the current process PATH too.

Installers are also attached to future GitHub Releases, alongside these assets:

| Platform | Release archive targets |
| --- | --- |
| Linux and WSL | `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl` (`.tar.gz`) |
| macOS Intel and Apple Silicon | `x86_64-apple-darwin`, `aarch64-apple-darwin` (`.tar.gz`) |
| Windows x64 and ARM64 | `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc` (`.zip`) |

You may instead download an archive and `SHA256SUMS` from the same
[release page](https://github.com/Giuseppecarte/they-work/releases), verify it
with `shasum -a 256` / `sha256sum` / `Get-FileHash`, then extract the executable
to a directory on PATH. Checksums detect a corrupt download; they are not code
signing or notarization. The release workflow builds and executes each native
target before publishing; until that workflow actually runs, those targets are
configured coverage, not verified releases. Real terminal/font behavior still
requires testing on that terminal.

To update, rerun the same installer. A failed download or checksum leaves an
existing installation untouched. To uninstall, remove the executable and, on
Windows, its installer-added user PATH entry. Source records are never removed.

## Choose your conversation sources

Start with `they-work`. On first launch, **Connect your team** lets you choose
Codex, Claude Code, both, or an empty tower. No account login, API key, browser
authorization, or subscription is needed. It reads local conversation titles,
messages, and tool activity. Approve requests in the original coding app.
Press `d` to explore the demo without reading conversations.

Use `↑` / `↓` to choose a source, `Space` to turn it on or off, and `e` to edit
its **app data folder** (`.codex` or `.claude`, not a project folder). While editing,
use arrow keys, Home/End, Backspace/Delete, or `Ctrl+U` to clear the path. `Enter`
applies the path; `Esc` cancels the edit. Press `Enter` again to connect. If a
folder is missing, the screen selects the source that needs repair.

**Remember on this computer** is selected by default. Turn it off with `Space`
on that row, or press `m`, for a temporary session. Saved choices are left
unchanged. Launch with `--no-save` to disable preference writes for the whole
session. Source switches and paths are saved when you confirm Connect; appearance,
characters, room palettes, and the selected floor are saved during interactive use.
Demo mode, `--once`, `--doctor`, and `--headless` do not save preferences or create
settings folders. Demo mode also skips reading saved preferences.

Later, simply run `they-work` again. Use `c` inside the app or `they-work --setup`
to change sources and repair folders. Settings live in:

- macOS/Linux/WSL: `$XDG_CONFIG_HOME/they-work`, or `$HOME/.config/they-work`.
- Windows: `%APPDATA%\they-work`, falling back to the home `.config\they-work`.

An absolute `--config-dir <folder>` overrides this location; relative paths and
`~` are also normalized. The directory is created only when saving. No transcript
content is copied into settings. To reset the saved connection, remove only
`connections.json` from this folder and launch again; source records remain intact.

For an explicit launch, including nonstandard data folders:

~~~sh
they-work --sources codex --codex-home /absolute/path/to/.codex
they-work --sources claude --claude-home /absolute/path/to/.claude
they-work --sources all --doctor
they-work --sources all --once
~~~

`--doctor` explains source discovery and readability. `--once` prints each project
and worker; both can run over SSH or in a pipe. Without saved source choices or
`--sources`, these commands explain how to choose; `--doctor` checks only folder
locations. No conversations are read until sources are chosen. A home override
alone does not grant permission. Command-line choices apply to that run; use the
connection screen to remember them. An unavailable source does not prevent the
other source from working.

In the office, `c` opens source selection, `v` changes camera, `w` / `W` changes
or resets the selected worker's character, and `o` / `O` changes or resets the
selected room palette. `0` opens the tower, `Tab` / `Shift+Tab` cycles floors,
and `!` jumps to a worker needing attention. Each repository gets one floor;
its Git worktrees remain together, including with `--project <path>`.

In WSL, install the Linux binary. Windows-side data can be selected explicitly:

~~~sh
they-work --sources codex --codex-home /mnt/c/Users/YourName/.codex
~~~

The source directory must be readable by your own account. Never change live
store permissions just to hide a diagnostic failure. SQLite opens main stores
read-only, but may coordinate through an existing writable `-shm` sidecar;
missing required sidecars are not created. Transcript content, paths, and
messages appear on screen, so the terminal has the same sensitivity as that data.

## Docker from a checkout

Requires Docker and GNU Make, with Docker Desktop/the daemon running. These
commands compile the checked-out version and do not need Rust installed locally:

~~~sh
make demo
make run
~~~

The demo mounts nothing. `make run` mounts existing source homes read-only,
skips missing homes, runs with your UID/GID, drops capabilities, and disables
network access. Add arguments with `make run ARGS="--doctor"` or
`make run ARGS="--sources codex"`. Override host locations with
`THEYWORK_CODEX_HOST` or `THEYWORK_CLAUDE_HOST`. Paths containing spaces work.

The default read-only Docker container cannot save settings. Turn off
**Remember on this computer**, or use `--no-save`. To
retain choices, mount one settings directory read-write and pass its container
path with `--config-dir`. Merely passing the flag does not grant write access.
For scripts without saved choices, include `--sources`, for example
`make run ARGS="--sources all --doctor"`.

## Docker without a checkout

The existing `v0.1.0` image is a Linux/amd64 release. It does not contain the
changes in this branch. Other host architectures need Docker emulation for
that historical image. For a demo that reads no host data:

~~~sh
docker run --rm -it --network none --read-only --cap-drop ALL \
  --security-opt no-new-privileges -e TERM -e COLORTERM -e TERM_PROGRAM \
  ghcr.io/giuseppecarte/they-work:v0.1.0 --demo
~~~

For that release's live-data launcher, download and verify the exact historical
installer before running it. The pinned checksum is independent of the download.
The doctor step stops before the interactive office if store inspection fails.

<!-- verified-docker-bootstrap -->
~~~bash
(
  set -e
  installer=$(mktemp)
  trap 'rm -f "$installer"' EXIT
  if ! curl -fsSL https://raw.githubusercontent.com/Giuseppecarte/they-work/v0.1.0/docs/install.sh -o "$installer"; then
    echo "Installer download failed; nothing was executed." >&2
    exit 1
  fi
  if command -v sha256sum >/dev/null 2>&1; then
    verify_sha256() { sha256sum -c -; }
  else
    verify_sha256() { shasum -a 256 -c -; }
  fi
  if ! printf '%s  %s\n' '2a665a28b75d9fa22f07b7ae8aa686a3bfd6e263309eb45b522cd8a9221fa2d4' "$installer" | verify_sha256; then
    echo "Installer verification failed: truncated or modified download; nothing was executed." >&2
    exit 1
  fi
  sh "$installer" --doctor
  sh "$installer"
)
~~~

This legacy launcher pulls `latest` unless you set
`THEYWORK_IMAGE=ghcr.io/giuseppecarte/they-work:v0.1.0`. `--doctor` and `--once`
work without a TTY. Host paths must exist on the Docker daemon's host; a remote
Docker daemon cannot read your local laptop's files.

## Troubleshooting

- **No conversations:** run `they-work --setup`, choose sources, then use
  `--doctor` for the exact paths and next steps. Use `--demo` to preview the office.
- **Command not found:** use the installer's printed absolute command, or add its
  directory to PATH. For Cargo installs, reopen the terminal after rustup setup.
- **Docker unavailable:** start Docker Desktop/the daemon, or use a native build.
- **Native asset missing:** no native release has been published for that tag;
  build the checkout. A failed installer never reports successful installation.
- **Gaps or broken glyphs:** open Settings (`s`) and compare the pixel encodings.
  Terminal on macOS defaults to half blocks; some fonts do not contain sextants
  or leave gaps around quadrant glyphs. Remove an old `THEYWORK_ENCODING`
  override to restore automatic selection. Inspect `--doctor` in that terminal.
- **Tiny window:** increase terminal size; compact layouts retain worker status.

See [CONTRIBUTING.md](CONTRIBUTING.md) for verification commands and
[the installation audit](docs/design-audit/INSTALLATION.md) for actual test evidence.
