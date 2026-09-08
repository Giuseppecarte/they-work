# Contributor and evidence maintenance audit

The documented native verification path works once its stated prerequisites are
available: an independent checkout passed `make check` in 128.56 seconds with
418 tests passed, zero failed, three ignored, formatting checked, and strict
Clippy clean. The checkout remained unchanged. This is one macOS run, not a
first-time installation study or a Linux/Windows verification result.

## Clean-checkout experiment

| Step | Result | Meaning |
| --- | --- | --- |
| Local `git clone --no-hardlinks --single-branch --branch audit/design-and-usability` | 1.81 seconds; exit 0 | Independent checkout and Git objects; no network-clone claim. |
| Follow `make check` using the host's default PATH | Exit 2, with actionable Rust/Docker prerequisite guidance | Neither native Rust on PATH nor a usable Docker daemon was available. This is an environment limitation, not a failed application test. |
| Supply the pinned native Rust toolchain, force offline dependency resolution | Missing cached `winapi 0.3.9`; exit 2 | The existing dependency cache was incomplete. Offline setup from this cache was not possible. |
| Permit Cargo dependency retrieval and local fake-provider loopback; repeat in the same untouched checkout | Exit 0 in 128.56 seconds | Clean build target, existing dependency cache supplemented by downloads; not an empty-cache timing. |

The baseline is commit `748d371f9769e83798f29a52fffeb90410afc3bd`, macOS
26.6.2 arm64, Rust 1.90.0. The command executed the contributor entry point,
including the process fixtures, rather than a selected subset of tests.
The three ignored tests remain ignored; their presence is not additional
coverage. Local TCP permission was needed by the fake-provider tests. No account,
personal conversation store, or authenticated provider was involved.

Evidence: [clone](clone.json), [default prerequisite failure](default-make-check.log),
[offline cache failure](native-make-check.log), [successful run metadata](network-native-make-check.json),
[full successful log](network-native-make-check.log). The commands and native
environment are reproducible with [check_checkout.py](check_checkout.py).

## Confirmed documentation drift

Three kinds of documentation are being treated as current contracts even though
their behavior has changed:

1. `CONTRIBUTING.md` links `docs/project-selection.md` as the behavioral
   specification. That older document says source selection is not persisted
   without `--config-dir`; current first-run Remember and `--no-save` behavior
   must instead be explained consistently with the actual preferences flow.
2. The `Source` trait documentation describes polling once per frame to keep the
   program single-threaded. The host now uses a background poller, with a
   one-second poll interval and sequential source polling. This can lead a
   contributor to implement a source against an obsolete latency assumption.
3. Installation and help instructions still describe controls that now open
   another surface. The executed examples and screenshots are in
   [workflow documentation drift](../workflows/DOC_DRIFT.md).

The smallest useful change is to update the authoritative entry points and mark
historical design specifications as historical. A contributor should be able to
follow the source-selection instructions, then explain the actual ownership of
polling, without reconciling contradictory documents. A full architectural
rewrite is not supported by this evidence.

## Audit reproducibility and storage

Iteration 6's exact screenshot recipe invokes `native-cargo.sh`, which expects
an ignored local toolchain, and uses a Python image library plus a particular
macOS font for rasterized terminal cells. Those resources do not arrive in a
clean checkout. The existing validation document identifies the toolchain as a
local convenience, but there is no single portable bootstrap for reproducing
the evidence. This limitation concerns audit reproduction, not the native
application's runtime dependencies.

At the baseline there are 3,192 tracked files and 93,795,192 bytes of working-tree
content. `docs/design-audit/` accounts for 2,893 files and 65,578,401 bytes: about
69.9% of tracked bytes. Git reports 53.50 MiB of loose objects and 5.82 MiB of
packed objects on this checkout. These measurements establish storage
concentration; they do not establish a slow network clone or a contributor's
subjective burden. See [repository metrics](repository-metrics.json).

For new audits, retain the generator, dependency recipe, hashes, small expected
logs, representative complete screens, and a manifest. Keep generated stores,
toolchains, build products, and redundant captures outside tracked evidence.
Offer a portable font fallback and record the font actually used. Do not rewrite
Git history or discard old evidence as part of a documentation cleanup.

## Responsibility boundaries

The existing core/collector/control/renderer separation remains useful. The
renderer does not need to acquire provider permissions to fix the reproduced
data or control problems. The largest files alone do not justify refactoring.
The concrete seam worth changing is the explicit delivery of observation-window
coverage from control through core to presentation, as specified in
[the data brief](../data/BRIEFS.md). Control receipt persistence belongs in the
control layer; UI copies of ledger policy would make the reported false
capabilities harder to maintain.

## Not tested and next evidence

- A genuinely new contributor following the instructions without help.
- An empty-cache network checkout/install on Linux, Windows, or WSL.
- Docker execution, because a usable daemon was unavailable.
- The exact cost of downloading audit assets from the public repository.

The study kit can capture first-run documentation friction, but developer build
friction needs a separate contributor rehearsal only if the documentation and
bootstrap changes are selected. These limits do not invalidate the successful
native verification run.
