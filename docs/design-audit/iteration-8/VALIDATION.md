# Acceptance results

Evidence was captured on 2026-09-09. A passing compositor, executable PTY or local
registry test is not a passing physical-terminal or participant test. Records
identify binary/index hashes and candidate source state; a referenced commit
alone is not claimed to describe a clean build of a shared working tree.

| Scope | Executed evidence | Result | Practical limit |
| --- | --- | --- | --- |
| Workspace before the final receipt layout fix | `make check` through pinned native Cargo | 422 tests passed, 0 failed, 3 ignored; formatting and strict Clippy passed | [Log and metadata](validation/workspace-check.json); overlapping checks must not be counted as new independent trials. |
| Affected renderer/TUI suites after that fix | Pinned Cargo tests for both crates, formatting, workspace all-target strict Clippy | 321 tests passed, 0 failed, 1 ignored; formatting and Clippy passed | [Final commands](validation/post-receipt-check.json). Two new tests inspect real rendered cells/controls across 80-column, 40-column inline/full/new-task, and compact geometry. |
| Pre-send storage and restart boundaries | Three injection tests, eight concurrent process clients, EACCES and RLIMIT_FSIZE executable fixtures | Passed; zero unintended provider dispatches | [Control report](control/REL-01.md). Physical disk exhaustion and native Windows injection not tested. |
| Existing control behavior | Concurrency, controls, uncertain response, provider-gated crash, post-send persistence failure | 14/14 retained checks passed | [Regression results](control/regression/results.json); fake providers. |
| Visible recovery | Final 80×24 keyboard PTY, directory-permission failure, storage restoration, explicit resubmit | Full reason and draft visible; zero automatic resends; one correct-recipient resubmit; normal exit | [Results](docs/evidence/not-sent/results.json), [before/after discussion](docs/REPORT.md). ANSI replay, no physical display. |
| Release logic | Deterministic helper/verifier tests | 20/20 passed | [Log](release/evidence/unit-tests.log). |
| Multi-architecture product image | Kitty, iTerm2 and native-roster fallback PTYs on amd64 and arm64 | 6/6 passed; quit sent and exit 0 | AMD64 emulated; ARM64 native to the Linux Docker daemon. [Report](release/evidence/verification.json). |
| Registry recovery | Disposable local registry, verified multiarch index including attestations | 9/9 scenarios passed, exact digest retained | [Scenarios](release/evidence/rehearsal/results.json). Native archives were checksum fixtures; GitHub-publication failure was an injected outcome. |
| Reproduction | Two prepared independent candidate checkouts; one full tower frame and one exact collector test in each | Both passed; 13 runner tests per OS; no tracked-file changes | [Summary](repro/summary.json). Linux arm64 container used `--network none`; macOS used an explicitly supplied pinned toolchain/cache. |
| Complete-frame visual review | 640×384 full tower captures with native cells and authored image | Pass at the bounded 80×24, 8×16 condition; identical decoded RGB across Mac/Linux | Compression bytes differ by platform. No frame latency, transport or broader UI-matrix certification. |
| Documentation | First run, Connections, Advanced, Tab, both office page keys, aliases, Remember, no-save, reopening | Five scenario groups passed on final binary | [Results](docs/evidence/results.json); keyboard only, 80×24, fake stores. |
| Study tooling | Five actual launcher PTYs; three presentations and two semantic variants; manifest guards and resize | Preflight passed on macOS | [Preflight](study/PREFLIGHT.md) names its earlier binary. Those paths are unaffected by later receipt wrapping; participant baseline remains unfrozen. |
| UX-01/UX-02 | No new-user sessions, diaries or follow-ups | **Not tested** | [Status](study/status.json). No prototype qualifies; no evidence-supported no-change conclusion exists. |

## Unperformed acceptance and release gates

Public GHCR authentication/tag writes, remote GitHub Actions/artifact retention,
actual native archive download/install across platforms, and GitHub Release API
recovery were not performed. Local registry success does not close those checks.
No real provider account or physical terminal recording was used. Windows/WSL
receipt and study sessions, physical-terminal input p95, assistive technology and
the five-person comprehension target remain not tested.

The bounded smoke deliberately does not repeat iteration 6's full UI matrix,
long-running workload or all historical audit recipes. The late receipt change
received relevant geometry, renderer/TUI, Clippy and executable-PTY checks; no
unrelated UI redesign or speculative UX prototype was added.

Generated development output stays in ignored `target/audit/`. Selected evidence
is retained here; original iteration 1–7 files are unchanged. Fixed-width text,
ANSI streams and bundled fonts retain their original bytes. One release build
log is normalized for readable progress lines with both hashes disclosed in its
manifest. [EVIDENCE.json](EVIDENCE.json) inventories this iteration's retained files.
