# Linux preview validation

**Ready for the owner's WSL trial.** Both delivered archives contain the exact
statically linked Linux executables that passed the container checks below.
Application source is frozen at `17ae09f85982b2e29c701fe47b7ce03fa34c488e`.
The packaging recipe and documentation are recorded separately by hash in the
[final manifest](preview/manifest.json); documentation corrections did not rebuild
or change either executable.

| Check | x86_64 | ARM64 |
| --- | --- | --- |
| Correct ELF architecture | pass, machine 62 | pass, machine 183 |
| No ELF interpreter or shared-library dependencies | pass | pass |
| `--help`, demo `--once`, bounded demo `--headless` | 3/3 pass | 3/3 pass |
| Kitty, iTerm2, Sixel, image-free native roster PTYs | 4/4 pass | 4/4 pass |
| `q` exits normally; mouse and alternate screen restored | 4/4 pass | 4/4 pass |
| Truecolor foreground/background in recorded PTY output | 4/4 pass | 4/4 pass |
| Checksum → extract archive → run packaged binary | pass | pass |
| Runtime execution | Docker emulation | ARM64 Docker daemon native |
| Actual Ubuntu WSL + VS Code terminal | **not tested** | **not tested** |

The driver was macOS ARM64, Docker Desktop 4.56.0 / Engine 29.1.3, with an ARM64
Linux 6.12.54 daemon. Runtime containers were Debian 12. Builds used Rust 1.90.0
and musl compiler packages 1.2.3-1. Full toolchain output and runtime OS/architecture
are retained per platform. No provider directories were mounted. Runtime networking
was disabled; root filesystems were read-only. The extraction rehearsal used a
temporary in-container filesystem and a read-only archive mount.

The PTYs simulated 160×48 cells at 8×16 pixels. They verify probe handling,
graphics transmission, native fallback content and process restoration. They do
not prove a terminal displayed the images, certify Windows/WSL, authenticate a
provider, or measure the 150 ms input-to-visible target. The image-free recordings
contain task titles, statuses, project/team context and navigation, plus explicit
RGB foreground and background output.

## Delivered files

Local archives are intentionally ignored build output in
`target/audit/iteration-11/dist/`. Each includes the executable, MIT license,
manifest, quick start, WSL guide, blank session record, opt-in VS Code settings
and the INSTALL/CONTROLS contracts. The packaged WSL guide's contract links point
to those bundled files. Contributor and historical audit references inside the
contract documents refer to the source checkout.

| Archive | Bytes | SHA256 |
| --- | ---: | --- |
| `they-work-x86_64-unknown-linux-musl.tar.gz` | 3,102,600 | `c87ffe9484b6f319f683d2b6ce77f7170b1c693e71d2c7a888f6c13983717ae0` |
| `they-work-aarch64-unknown-linux-musl.tar.gz` | 2,866,486 | `05d5288483e0175ef7490d79892afceb149f53b72e19ad472a8c0e0ceec82523` |

[Retained checksums](preview/SHA256SUMS) and [extraction results](preview/archive-extraction.json)
identify the final archives. The executable hashes remain independent of the
documentation bundle: x86_64 `e02b43e3d90b64a5201cb790d7f9aaa690659a35b701ec6e6b76899940ecdb6b`;
ARM64 is recorded in the final manifest.

## Reproduction and evidence

In a clean checkout with Python 3.12+ and Docker available, the audit helper
builds only the committed Cargo/crates source snapshot. Dependency preparation
needs network access. It never pushes an image or publishes a release:

```sh
python3 docs/design-audit/iteration-11/test-preview.py
python3 docs/design-audit/iteration-11/preview.py --commit 17ae09f85982b2e29c701fe47b7ce03fa34c488e --run
python3 docs/design-audit/iteration-11/verify-extraction.py
```

The helper refuses to overwrite existing evidence or archives. Use a fresh
checkout for a repeat, or deliberately retain old output elsewhere first.
`--repack --output <passing-build-directory>` rebuilds only the archive bundle,
after checking the executable still matches its successful architecture and
execution record. Seven helper contracts pass, including rejection of dynamic
interpreters, shared libraries, wrong architectures and truncated ELF headers,
plus archive content/mode and bundled guide dependencies.

[Evidence index](preview/evidence.json) hashes 33 retained files. Per-platform
folders contain build/toolchain logs, CLI output, PTY results and extraction
logs. Complete raw PTY bytes are gzip-compressed; decompressed hashes match each
result's `frame_sha256`. [PTY byte checks](preview/pty-byte-checks.json) independently
confirm RGB output and absence of image transmissions in the no-reply fallback.

## Critical review and corrections

The first x86_64 attempt failed the static-ELF gate. Debian's `musl-gcc` linker
specification added `/lib/ld-musl-x86_64.so.1` as an interpreter even though Rust
reported `crt-static`. Delivering it would have required a musl loader on the
user's WSL installation. The corrected recipe uses musl-gcc for bundled SQLite
and Rust's normal self-contained target linkage. The [original failure](preview/first-attempt/report.json),
[linker diagnosis](preview/first-attempt/linker-diagnostic.log) and successful
small linker probe remain available. The gate was preserved; neither final
executable needs that loader.

Review then found the first archive bundle omitted the referenced blank WSL
session record and left source-relative INSTALL/CONTROLS links. The final bundle
includes those documents and localized guide links. Binaries were unchanged;
checksums were regenerated, and both final archives passed extraction and
execution again. Initial archives remain in ignored
`target/audit/iteration-11/dist-initial-doc-bundle/` as superseded packaging evidence.

The actual WSL experience remains the next check. Follow [WSL-PREVIEW](WSL-PREVIEW.md)
and record the installed environment in [WSL-SESSION](WSL-SESSION.md). Static
linkage removes the glibc-version obstacle, but simulated protocols cannot establish
how VS Code's renderer, font, terminal resizing or input timing behave on the
owner's machine.
