# REPRO-01: portable, bounded reproduction

The new [audit guide](../../../AUDIT.md) and `scripts/audit.py bootstrap` / `smoke` entry points reproduce one complete screen and one existing collector fixture. No production renderer, collector or control API changed. The dated iteration 1–7 harnesses and evidence were left untouched.

Both independent candidate checkouts passed. [Machine-readable results](summary.json) link exact full-frame hashes and the retained logs. Automatic metadata deliberately remains `visual_review: not-tested`; the subsequent visual inspection records are separate and tied to the actual PNG hash.

| Measurement | macOS arm64 | Linux arm64 container |
| --- | --- | --- |
| Python / Pillow / FreeType | 3.12.14 / 12.3.0 / 2.14.3 | 3.12.14 / 12.3.0 / 2.14.3 |
| Rust and Cargo | 1.90.0 | 1.90.0 |
| Complete screen | 640×384, 80×24 at 8×16 | Same |
| Actual UI interaction regions / native cells | 48 / 668 | 48 / 668 |
| Exact collector fixture per smoke | 1 passed | 1 passed |
| Runner contract tests | 13 passed | 13 passed |
| Repeated PNG within environment | Identical | Identical |
| Complete compositor visual inspection | Pass; also independently reviewed | Pass |
| Offline transport restriction | Sandbox network restricted; Cargo offline and rustup auto-install disabled | Docker `--network none`, Cargo offline and rustup auto-install disabled |
| Physical terminal transport | Not tested | Not tested |

The [macOS screen](macos/tower-image-80x24.png) and [Linux screen](linux/tower-image-80x24.png) have identical decoded RGB pixels, native cells, RGBA scene, text and hit records. Their PNG files differ because the wheels use different compression libraries: macOS reports `1.3.1.zlib-ng`, Linux `1.2.13`. The replay therefore records both PNG and decoded RGB hashes. This observed equality does not promise identical rasterization on every supported OS or dependency patch.

The selected data case is the unchanged `codex_updates_same_timestamp_items_and_retains_removed_deliveries_without_writing_stores` integration test. It exercises actual `CodexSource`/`World` against generated SQLite databases, updates same-timestamp items, retains removed deliveries, and checks store bytes. The runner requires exactly one reported passing test. The fixture's temporary root is redirected into the ignored run directory; no real provider or personal store is involved.

The candidate checkouts were created from revision `5fa7d55d89af9c407ea95327d8080322e2bef13e`, with the working candidate diff and new audit files applied. They had no copied `target`, ignored toolchain installation or historical runtime environment. macOS received a pre-existing Rust installation/cache through explicit `CARGO_HOME`, `RUSTUP_HOME` and `--cargo`; bootstrap still ran the locked dependency fetch. Linux started with the [recorded build environment](Linux.Dockerfile), installed Python/Cargo dependencies into its own fresh checkout, and then ran with networking disabled. The retained [macOS metadata](macos/metadata.json) and [Linux metadata](linux/metadata.json) include commit, dirty state, all Rust source hashes, actual dependency versions and artifact hashes.

To repeat from a checkout containing the candidate commit:

```sh
git clone --no-hardlinks /path/to/they-work /path/to/fresh-checkout
cd /path/to/fresh-checkout
python3 scripts/audit.py bootstrap --python /path/to/python3.12
python3 scripts/audit.py smoke
target/audit/venv/bin/python -B scripts/test-audit.py
```

For the Linux test, the audit image was built from `Linux.Dockerfile`; the checkout was mounted at `/repo`. Bootstrap used `CARGO_HOME=/repo/target/audit/cargo-home`. Both the retained smoke and runner tests used `docker run --network none`. The image identity is retained in `summary.json`. Git's `safe.directory=/repo` was supplied only to this isolated container through `GIT_CONFIG_COUNT/KEY_0/VALUE_0`, because bind-mounted checkout ownership differs from the container user.

Two harness issues were corrected before retaining the passing result. First, `Ui::diagnostics()` reports its character fallback encoding even with image density active; the gate now checks the real image-mode case, its physical RGBA rectangle and native overlay. Second, an initial shared Git clone had an absolute alternate-object path that was inaccessible inside Docker. The final candidate clones were disassociated from that object store before the passing runs. Neither issue required product changes.

The 13 runner tests cover missing bootstrap/Python/Cargo, Cargo JSON executable discovery and ambiguous artifacts, zero or wrong fixture selection, output overwrite boundaries, forced render overrides, exact route/hit bounds, full-frame and opaque native composition, deterministic replay, font/license integrity, missing glyphs and malformed image geometry/bytes. The macOS/Linux CI jobs run this same bounded gate; those GitHub-hosted jobs were wired here but not remotely dispatched. No Windows, real terminal transport, full UI matrix or performance claim is added by REPRO-01.

## Integrated candidate check

After the receipt layout fix, both prepared checkouts received the final Rust
sources and runner tests. Their incremental offline smoke again passed exactly
one capture and one fixture; all 13 runner tests passed in each environment.
Every recorded source hash matches the integrated candidate. The retained
[macOS integrity record](macos/integrity.json) and
[Linux integrity record](linux/integrity.json) compare tracked diffs and status
before and after those commands and report no changes. Each final PNG is
byte-identical to its earlier same-platform capture, so the earlier complete-frame
comparison remains applicable. The Linux final frame was also inspected independently.
