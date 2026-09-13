# REL-01 closure: verified

The never-sent rejection contract passes on native macOS ARM64. The exact
recipient-bound draft can be resubmitted deliberately after an ordinary failed
write, and canonical-state loss still requires checked supervisor restart.
No production change was needed for this item.

## Candidate and evidence

The copied executable has SHA-256
`670ced3bd4662b5da5b483a17b6db91e15040865e1b934228367889f03aaf007`.
Its iteration-9 build record names `3d0ba4e`; the baseline `4097e4f` adds audit
records. All 38 recorded source hashes matched before use. The integrated
candidate `7cb26e4` changes only the Docker build and its helper regression, so
these executable results are reused for that candidate with the unchanged Rust
source and binary hash made explicit in [the manifest](manifest.json).
After capture, the contributor guide clarified the release helper's Python
3.11 minimum. Both captured and current guide hashes are recorded separately;
the application route/control contracts and executable are unchanged.

All results below were executed again during iteration 10:

| Check | Result and evidence |
|---|---|
| Pre-write and file-size injection, correction persistence, post-rename uncertainty, live fault authority | Four executed unit tests, zero failures in [admission-unit.log](admission-unit.log). Sending left on disk restarts as Uncertain without provider launch. |
| Actual Unix permissions and process file-size limit | Two executable cases, 16 passing assertions in [admission/results.json](admission/results.json). Zero provider calls before rejection; stable same-ID receipt and payload mismatch rejection; one explicit new submission. |
| Canonical disruption and concurrent clients | One executed process test in [canonical-disruption.log](canonical-disruption.log). Eight clients share the same rejected operation. Restoring canonical bytes does not clear the running-host fault; checked restart permits one explicit new submission. |
| Current recovery capability projection | One executed test in [recovery-ui.log](recovery-ui.log). Fresh storage is distinct from unsafe established storage; unsafe storage disables creation and exposes the reason. |
| Ordinary failure through the real UI | One 80×24 PTY in [not-sent/results.json](not-sent/results.json). Exact rejection and draft remain visible, zero rejected provider calls, `managed-1` remains the recipient, and one explicit new operation sends once. |
| Canonical recovery through the real UI | The [supplemental route](../docs/coverage-recovery/results.json) preserves the office process while its fixture supervisor restarts. Task actions → Reconnect resumes `managed-1` once, restores the identical visible draft, and sends no new turn or steer. Reopening the office also sends nothing. |

The test-count check sums executed results and requires the exact expected
count. A filtered test target with zero matches alone cannot pass. The first
actual admission attempt could not bind its local socket inside the sandbox;
[its failure](admission-process.log) is retained. The scoped loopback retry was
approved and [passed](admission-process-unrestricted.log).

## Critical review

The complete [ordinary-failure screen](not-sent/02-not-sent-draft-retained.png)
shows the instruction, full rejection, operation ID and explicit Send control.
This is the stronger recovery promise for a failed write with valid canonical
state, and it remains intact.

Canonical loss has a different presentation: the composer shows “Observation
only” in place of the draft while authority is disabled. The draft is **hidden,
not lost**: the same office process displayed it again after deliberate checked
reconnection, with the original recipient and no send. Connections exposes the
original error and required restart, but repeats parts of that diagnostic.
These are comprehension limitations to retain in UX evaluation; the audit did
not change the display merely to satisfy a failed visibility assertion.

Actual physical disk exhaustion, interrupted power, authenticated providers and
native Windows behavior are not tested here. RLIMIT_FSIZE is a process limit;
removing its hard limit requires restarting that fixture process. PTY replay
establishes executable behavior and terminal restoration, not physical-terminal
paint latency. WIN-01 has its own native execution gate.

Exact commands, hashes, dependency versions, 12 total PTY sessions shared with
DOC-01, and all evidence hashes are in [manifest.json](manifest.json). Raw ANSI
streams and action traces are retained as deterministic gzip files alongside
their text and PNG captures; synthetic endpoint credentials were not exported.
