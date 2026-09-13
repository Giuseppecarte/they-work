# Iteration 9 acceptance record

Engineering changes are implemented locally. **DATA-01, DATA-02 and PERF-01 have
executed bounded acceptance evidence. WIN-01 remains pending its selected native
Windows x64 and ARM64 runtime gate.** No physical-terminal or authenticated-provider
certification is claimed.

| Evidence layer | Executed result | Limit |
| --- | --- | --- |
| Integrated native macOS workspace | 465 ordinary tests passed, zero failed, four ignored; three additional harness-free native fake-provider scenarios passed. | The later coverage Back-button test adds one directed case; final affected checks are recorded separately, not added as independent full-suite runs. |
| Shared coverage and retention | Seven new core cases passed, plus the existing core suites. Quiet-project retention, 20/>512 projects, duplication, original project attribution and historical-state safety are covered. | In-memory observation retention, not a durable transcript store. |
| DATA-01 | 14 new tests passed: three lineage, nine reconciliation/SQLite, two actual fake-provider process cases. Strict control Clippy passed. | Synthetic provider protocol and disposable stores. |
| Coverage interaction | Three final directed tests passed, including duplicate-source counts, all six sizes, hidden-action safety and Back/Esc equivalence. Short-panel current-request test also passed. | Ui/TestBackend composition and input routing, not physical terminal paint. |
| PERF-01 | Paired/unpaired before/after replays each processed 128,000 tool starts; outcome oracles passed. Collector integration suites passed in the workspace run. | Debug builds and synthetic workloads; memory improvement includes a measured polling cost. |
| WIN-01 local | Four shared storage tests, one live-fault test, one canonical-disruption process case and three native macOS process cases passed. Strict native and Windows cross-Clippy passed. | Windows source cross-compilation is not Windows execution. |
| Visual matrix | 96 complete exports passed route, hit-bound and effective-color checks; eight full replays visually inspected with bundled fonts. | Remaining 88 exports have no separate visual verdict; all terminal transport is not tested. |
| Final formatting and lint | Pinned workspace formatting and strict Clippy pass. | Logs and source hashes are retained with the validation manifest. |
| Native macOS release build | Optimized binary built successfully; `--demo --no-save --once` exited normally. | Synthetic data only; binary hash is recorded in the manifest. |
| Native Windows x64 / ARM64 | **Not tested.** Jobs enforce six Windows replacement tests, four shared storage tests, one live fault test and four native process cases per architecture. | The remote push awaits explicit destination approval after automatic review rejected it. |

The first unrestricted workspace attempt was run inside a sandbox which refused
the synthetic loopback listener. That is retained as an environment limitation;
the approved local loopback run is the native workspace result above. Its first
integration run exposed the S-fixture/canonical-disruption mismatch, which was
corrected and rerun before the passing integrated run. Earlier failures are not
silently counted as passing evidence.

See [validation/manifest.json](validation/manifest.json) for commands, current
source hashes, platform/toolchain details and retained log hashes. Per-item
manifests identify their own measured source versions. The [visual manifest](visual/manifest.json)
records the final environment, frame hashes and individual review verdicts.

## Acceptance still requiring external execution

The pending action is pushing `audit/design-and-usability` to
`https://github.com/Giuseppecarte/they-work.git` and inspecting the actual x64 and
ARM64 CI results and retained artifacts. Automatic approval review rejected that
upload because it requires explicit approval for the external destination.
An approval question is pending. No push, merge or release publication occurred.

Authenticated-provider trials, physical Windows/macOS/Linux/WSL terminal
recordings, input-to-visible latency, physical-power-loss testing and participant
sessions remain **not tested**. They are not substituted by compositor timing,
cross-compilation, process-kill tests or synthetic requests.
