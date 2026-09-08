# Reliability discovery, iteration 7

The production release was exercised without modifying production code or APIs.
Nine isolated scenarios produced **26 passing checks and three failed checks**,
grouped into **two reproduced product problems**. Each problem occurred again in
a fresh isolated run. A failure here is a discovery result, not a reason to
change the expected outcome until the implementation passes it.

## What ran

- macOS arm64, executable SHA256
  `b2c529653f5b9d6a2ee93cd84cb87772ee8e1dee96b431ac9b645f3bfd33de84`.
- The unmodified release's `--control-host` process, its actual local JSON IPC,
  durable files, timeouts and provider transport. The provider peer is an offline
  Python fixture that records protocol messages and controls acknowledgement
  timing. It never runs a model or the displayed command.
- The unmodified POSIX installer and packaging script. The archive contains the
  real release executable; `curl` alone is replaced with a local-file download
  boundary. This does not exercise HTTP, TLS, GitHub, signing or OS download
  quarantine. Updating means replacing an existing installation with the same
  build, not proving migration between product versions.
- Actual filesystem permission denial (`EACCES`) and a 4 KiB file-size quota
  (`EFBIG`). No host disk was filled. Actual `ENOSPC` and physical power failure
  were not tested.

All homes, projects, endpoints and provider logs were generated beneath
`docs/design-audit/tmp/iteration-7-reliability`. The process environment does not
inherit provider credentials. Owned host/process groups are stopped on cleanup.
No personal source store, account, provider task, public release or terminal
window application was accessed. Loopback permission was required.

## Outcomes and independent expectations

Expectations are declared in `run_reliability.py` before any case executes. They
are checked against independently recorded provider messages, installed binary
hashes, operation receipts and persisted/reloaded state.

| Scenario | Outcome |
|---|---|
| Real archive installation, launch, replacement while running | Passed; installed SHA matches the release and the already running process exits normally. |
| Download failure, bad checksum, truncated archive, unwritable destination | Passed; each preserves the previous executable byte-for-byte. |
| Eight concurrent requests with one operation ID | Passed; exactly one `thread/start` and one `turn/start`; changed payload under that ID is rejected. |
| Stop and Decline below the history bound | Passed; the same fixture protocol and reply shape execute exactly once. This rules out an invalid request fixture as the reason for REL-02. |
| Missing acknowledgement, duplicate request and explicit reconnect | Passed; uncertainty survives restart, no turn is replayed, reconnect sends only one `thread/resume`. |
| Host killed after provider receives a turn, before acknowledgement | Passed; restart changes the saved Sending receipt to Uncertain and does not replay. |
| Permission failure before durable intent | Sends nothing, but subsequent same-ID retry after restoring write access returns Sending. **REL-01.** Restart permits one execution. |
| File-size quota before durable intent | Passed for the scoped safety expectation: no provider command, explicit write error, recovery through restart without quota executes once. |
| Write failure after provider acknowledgement | Passed; caller receives an error, restart exposes uncertainty, and duplicate operation ID does not replay the accepted turn. |
| Operation ledger reaches 10,000 entries | The boundary is reached as expected; both Stop and Decline are rejected despite live capabilities. **REL-02.** |

The ledger experiment seeds 9,997 synthetic historical receipts, alongside one
receipt created through the actual host. Reconnect and a new turn then reach
10,000. It does not claim to have observed 10,000 real user operations or their
frequency. The pending approval and active turn are real fixture protocol events.

## Confirmed findings

**REL-01 — A pre-send persistence failure leaves an impossible pending receipt.**
The first attempt returns `Permission denied`, and the provider log contains no
thread or turn request. After restoring writes and waiting two seconds (longer
than the configured 1.5-second provider timeout), the same operation ID returns
`Sending / Awaiting provider acknowledgement`; no provider exists to acknowledge
that operation. The entry is inserted before the durable intent write and is
retained when that write fails (`supervisor.rs:256–271`). The failed operation
therefore requires a new operation ID or host restart to make progress. The
experiment demonstrates this retry state; its indefinite character follows from
the existing-receipt return path, not a long-duration measurement.

**REL-02 — Historical receipt capacity removes controls from current work.**
At exactly 10,000 receipts, the snapshot still advertises `interrupt=true` and
`reply=true` for an owned active turn with one supported approval. Both actions
return `Operation ledger is full; keep the existing state and choose a new
control directory`. Neither reaches the provider; the request remains unresolved
and the turn active. The common ledger guard runs before all mutation types
(`supervisor.rs:252–255`). Frequency in actual use is unknown, but the loss of
current controls is confirmed. This is the strongest implementation candidate;
see [the bounded brief](IMPLEMENTATION-BRIEF.md).

## Source-inspected boundaries, not runtime passes

`inspect_static.py` records source hashes and ordering in
`evidence/static-boundaries.json`.

- **Windows state replacement:** the Windows path removes the prior file before
  renaming the staged file (`security.rs:74–81`). The missing-state interval is a
  confirmed code path. Ownership/receipt loss after a crash in that interval is
  a hypothesis requiring a Windows fault test; Windows was **not tested** here.
- **Release publication:** `release.yml` pushes the version and `latest` image
  tags before the post-push smoke test. Native release assets depend on that
  test. The order is confirmed; an externally visible partial release was
  **not tested**, because this audit did not publish anything.
- Windows installer/PATH behavior, Windows ACL and console runtime, real release
  downloads, cross-version migration, authenticated providers and physical power
  failure remain **not tested**.

## Evidence and reproduction

- [Primary results](evidence/results.json), [run log](run.log), and per-case
  `*-calls.json` / `*-provider.jsonl` preserve the actual requests and responses.
  IPC endpoint credentials are not exported.
- [Independent reproduction](evidence/reproduction/results.json) repeats REL-01
  and REL-02 from fresh directories; [log](reproduction.log).
- [Installer runs](evidence/installer-runs.json),
  [package smoke output](evidence/package.log), and
  [installed executable output](evidence/installed-demo-once.log).

From the repository root, provide an already-built native release:

```sh
python3 docs/design-audit/iteration-7/reliability/run_reliability.py \
  --binary target/native-macos/release/they-work
python3 docs/design-audit/iteration-7/reliability/inspect_static.py
```

Use `--cases permission ledger --evidence <repo-local-output>` for the two
reproductions. The installer case is intentionally scoped to macOS arm64; do not
label another platform verified by running its archive under a mocked OS name.
The script records the executable hash and each result instead of reporting a
blanket green status when a discovery expectation fails.
