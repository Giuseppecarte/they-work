# Try the colorful office in VS Code + Ubuntu WSL

This is a local preview of iteration 11, not a published release. For the
current branch or merged main, open its successful [CI run](https://github.com/Giuseppecarte/they-work/actions/workflows/ci.yml)
and download `native-x86_64-unknown-linux-musl` or
`native-aarch64-unknown-linux-musl` from **Artifacts**. Unzip the downloaded
artifact to obtain the `.tar.gz` and `.sha256` files. Check the run's source
commit so you know which version you are testing; artifact downloads require
GitHub sign-in.

The original visual-preview archives remain in
`target/audit/iteration-11/dist/`; their `PREVIEW.json` records that earlier
source revision, architectures, executable hashes and smoke results.
No Rust or Docker installation is needed in WSL to run either extracted executable.

## 1. Enable the terminal image option

Open your project in a **WSL: Ubuntu** VS Code window, then open its integrated
terminal. The bottom-left remote indicator should identify WSL; `uname -s`
in the terminal should print `Linux`. See the
[VS Code WSL instructions](https://code.visualstudio.com/docs/remote/wsl).

Opt in by merging these properties into **Preferences: Open User Settings (JSON)**.
Keep your existing settings. A copy is provided in `vscode-settings.json` beside
this guide; the application does not edit your VS Code settings.

```json
{
  "terminal.integrated.enableImages": true,
  "terminal.integrated.gpuAcceleration": "auto",
  "terminal.integrated.customGlyphs": true
}
```

VS Code documents terminal images as opt-in. Custom glyph rendering helps box
and block characters when the GPU renderer is available. Support still depends
on the installed terminal and its active renderer; the office probes capability
instead of assuming that this setting guarantees images.
[Image support](https://code.visualstudio.com/docs/terminal/advanced#_image-support),
[terminal rendering](https://code.visualstudio.com/docs/terminal/appearance#_gpu-acceleration).

## 2. Extract the matching Linux archive

Copy the archive and its `.sha256` file into a folder in WSL. Check `uname -m`:

| WSL result | Archive |
| --- | --- |
| `x86_64` | `they-work-x86_64-unknown-linux-musl.tar.gz` |
| `aarch64` or `arm64` | `they-work-aarch64-unknown-linux-musl.tar.gz` |

For x86_64, run the following in the folder containing both downloaded files:

```sh
sha256sum -c they-work-x86_64-unknown-linux-musl.tar.gz.sha256
mkdir -p "$HOME/they-work-preview"
tar -xzf they-work-x86_64-unknown-linux-musl.tar.gz -C "$HOME/they-work-preview"
cd "$HOME/they-work-preview"
./they-work --help
```

For ARM64, substitute `aarch64` for `x86_64` in both filenames. Stop if the
checksum fails. Extracting into this separate folder leaves any installed
version intact. The executables use statically linked musl, avoiding a dependency
on a particular Ubuntu glibc version. Actual WSL execution remains a separate
user-run check; container smoke tests are not Windows/WSL certification.

## 3. Check detection and run the demo

Run the doctor **directly in the integrated terminal** so it can probe the
terminal, then copy or photograph the output. A redirected or piped invocation
does not provide the same graphics evidence.

```sh
./they-work --doctor
./they-work --demo --no-save
```

Record `terminal_color`, `terminal_graphics`, `cell_pixels`, and `terminal_frame`
from the doctor output. `--doctor` without selected sources can report source
selection guidance; it still reports terminal diagnostics. Automatic image
detection remains enabled. With no image support, the native roster and work
panel remain available with the same task facts and controls.

If the environment contains `NO_COLOR`, it overrides color selection even when
you request truecolor. Remove it for this invocation only:

```sh
env -u NO_COLOR ./they-work --doctor
env -u NO_COLOR ./they-work --demo --no-save --color true
```

Use `--color 256`, `--color none`, or `--light` for comparison. The Settings
screen includes reduced motion. `--mouse off` leaves mouse selection to the
terminal; the default enables application clicks.

## 4. First-session checklist

Start near **80×24**, then try **120×36** or larger. Use the live terminal's
`stty size` before launching to record rows and columns.

1. Identify the worker who needs a response. Check that amber request wording
   remains clear against the warmer room and colorful controls.
2. Click a worker, computer or nameplate to open the work brief. Repeat using
   arrows and Enter; Tab and Shift+Tab should follow the visible controls.
3. Read the fictional question in **Now** or **Activity** and identify its
   worker. The demo has recorded requests, without live provider decisions.
   A connected, answerable request later exposes **Review request**; merely
   inspecting its worker does not approve it.
4. Return with Esc, enter the tower with `0`, select another floor, and return.
   `d` opens Office Design. Settings → Advanced contains **Office palette**;
   the four choices color office details, while the preset sets its materials.
5. Resize while a work brief is open. Check task identity, request visibility,
   selection and clipped text in both the side and full-width panel.
6. Press Esc to leave any panel, then `q` from the main view. Verify the shell
   prompt and normal terminal text selection return.

The demo reads no provider stores, skips saved preferences, and saves no
preferences. Its requests and characters are simulated. It is suitable for
checking the UI before granting source access; it does not validate authenticated
provider decisions.

## 5. Connect real sources after the demo

Quit the demo and launch `./they-work --no-save` for a temporary observation
session. On first launch, choose the local sources the office may read. From the
office, `c` or `C` opens **Connections**; **Local sources** chooses the provider
data folders. No separate they-work login is needed for observation.

`--no-save` keeps saved preferences unchanged and disables managed Codex task
creation because its supervisor requires durable local state. For a later
persistent session, launch `./they-work`, leave **Remember on this computer**
enabled, and choose **Connect** explicitly. Use the official provider login
controls only when you want to create or control tasks.

In WSL, controls use the Linux provider clients and their normal login. Reading
a Windows-side `.codex` or `.claude` directory only establishes observation;
it does not connect controls to a Windows process. Available actions and their
reasons appear in the work panel. See [INSTALL](../../../INSTALL.md) and
[CONTROLS](../../CONTROLS.md) for the authoritative persistence and provider
authority contracts.

## Evidence to bring back

Use `WSL-SESSION.md` to record your VS Code version from **Help → About**, Ubuntu
version from `cat /etc/os-release`, WSL kernel from `uname -r`, architecture,
font, terminal size, doctor output, settings and checklist results. Include a
complete screenshot of both the office and work panel. Mark observations you
did not perform **not tested**. We have not measured input-to-visible latency
in your terminal; compositor timings and simulated PTYs do not establish the
150 ms target there.
