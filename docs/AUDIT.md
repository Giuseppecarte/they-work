# Reproduce a bounded UI and data audit

From a clean checkout, these two commands reproduce one complete tower screen and one synthetic collector fixture. They do not start Codex or Claude, read personal conversations, or change historical audit evidence.

Prerequisites: Git, the Rust **1.90.0** toolchain pinned in `rust-toolchain.toml`, a C compiler/linker for bundled SQLite, and **CPython 3.12** with `venv`/`pip`. macOS needs the Command Line Tools; Linux needs a C build toolchain. The orchestrator may run under a newer Python, but the isolated replay environment must use 3.12.

```sh
rustup toolchain install 1.90.0 --profile minimal --component clippy --component rustfmt
python3 scripts/audit.py bootstrap
python3 scripts/audit.py smoke
```

If `python3` is a different minor version, select an installed Python 3.12 explicitly:

```sh
python3 scripts/audit.py bootstrap --python /path/to/python3.12
python3 scripts/audit.py smoke
```

`bootstrap` is the network step. It creates `target/audit/venv`, installs binary Pillow wheels from a version and hash lock, and runs `cargo fetch --locked` for the workspace. It installs no global Python package or font. `--cargo /path/to/cargo` optionally selects Cargo; the local bootstrap marker remembers that executable for `smoke`. Rustup/Cargo environment overrides remain available, but no machine-specific runtime path is built into the scripts.

`smoke` uses `--locked --offline`, `CARGO_NET_OFFLINE=true` and `RUSTUP_AUTO_INSTALL=0`. After a successful bootstrap, disconnecting the network must not prevent it from building and running. A missing dependency or changed lock produces an actionable error; it does not silently download or weaken the lock. To prepare a different machine or fresh Cargo cache, run bootstrap there first.

The smoke runs exactly these two cases:

1. `ui_inventory` → `tower-image-80x24`: actual `Ui` output at 80×24 cells, 8×16 pixels per cell, timestamp 1,000 ms, motion disabled. Cargo's JSON artifact stream supplies the executable path. The PNG covers the **entire 640×384 viewport**, including native text, backgrounds, toolbar, footer and the RGBA scene. The runner checks the expected route, image mode, one-case count, nonempty hit regions and their bounds.
2. `codex_updates_same_timestamp_items_and_retains_removed_deliveries_without_writing_stores` in `theywork-collect/tests/collaboration.rs`: the existing synthetic SQLite fixture checks same-timestamp updates, retained deliveries and unchanged store bytes. Cargo must report exactly one passing test; a zero-test filter fails the smoke. Its temporary stores are redirected beneath the run directory and cleaned by the existing fixture.

Each invocation creates a new directory under `target/audit/smoke/`. For a named run, use `--output target/audit/smoke/my-review`; existing directories are rejected. Output outside `target/audit` is rejected.

| Output | Purpose |
| --- | --- |
| `index.html`, `tower-image-80x24.png` | Complete, unscaled compositor replay to inspect |
| `raw/*.txt`, `*.cells.json`, `*.rgba`, `*.hits.json` | Native text, cell masks, original image bytes and real interaction bounds |
| `logs/` | Exact commands, Cargo artifact output, replay and fixture results |
| `metadata.json` | Source/binary/output hashes, revision and dirty state, dependency versions, font/license hashes, geometry and explicit verdict scope |

Open `index.html` in a browser or inspect the PNG directly. `status: pass` means the automatic reproduction contract passed. `visual_review` and `terminal_transport` remain `not-tested`: a successful command does not certify aesthetics, mouse delivery, Kitty/Sixel transport, or a user's terminal. Native Windows reproduction is outside this bounded CI gate; macOS and Linux jobs live in [the native workflow](../.github/workflows/native.yml).

The replay uses unmodified **DejaVu Sans Mono 2.37**, regular and bold, at 13 pixels with Pillow's BASIC layout. The bundled [license](../scripts/audit_assets/fonts/LICENSE.txt) and [upstream/hash manifest](../scripts/audit_assets/fonts/manifest.json) travel with the fonts. Font hashes and native glyph coverage are checked; there is no system-font fallback. [Pillow's wheel lock](../scripts/audit_assets/requirements.lock) and [wheel provenance](../scripts/audit_assets/dependencies.json) pin version 12.3.0 for CPython 3.12. FreeType's actual version is recorded because platform wheels may differ. Repeated runs in the same pinned environment should produce identical PNG bytes; cross-platform PNG identity is not promised. Geometry, native cells, RGBA and hit records are the shared comparison contract.

To test the runner's negative paths without Cargo builds or network access:

```sh
target/audit/venv/bin/python -B scripts/test-audit.py
```

The scripts do not update goldens or write into dated audit directories. Older full inventories, terminal recordings and performance probes retain their original prerequisites and evidence limits; this entry point intentionally reproduces one complete screen and one meaningful data fixture.
