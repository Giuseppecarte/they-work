# DATA-01 — disclose stream gaps and retain unresolved observations

The managed connection now reports known provider-event gaps and retains unresolved actor observations within a bounded runtime cursor. Independently collected history remains available. A stream gap does not count missing results, and a historical request does not become an actionable request.

## Implemented behavior

- Control snapshots add optional version-1 `EventWindow` metadata: lineage ID, first retained sequence, last assigned sequence, and unknown prior lineage. The supervisor still retains 256 provider events. Incoming notifications and incoming provider requests are both counted.
- The TUI uses `reconcile_snapshot` and commits its returned cursor after applying the batch. The old `snapshot_events` function remains a compatibility projection; it does not promise deferred recovery.
- Unknown actors are retained until their exact native identity and source appear. The queue contains at most 256 observations and 1 MiB of serialized event data, including identity/correlation fields. Oldest sequence retires first. One oversized observation cannot expel the existing queue. Repeated snapshots neither enqueue again nor increment loss counters.
- Exact missing counts are cumulative within one lineage. Details retain the most recent 16 coalesced sequence ranges, with a count of omitted ranges. Deferred retirement is separately counted. An empty, old, legacy or malformed window does not fabricate an exact missing count.
- New/recovered lineage windows and deferred events contribute recorded collaboration, relationships and history-only beats. They cannot replace current activity with an old beat, complete a newer turn or reactivate an old request. Supported current roster state is projected afterward; the current pending-request snapshot is projected last.
- Stream coverage is structured separately from freshness and source capability. Core preserves it across independent collector updates; aggregate presentation counts a shared source lineage once. Cancelling Connections preserves the cursor when the World was not replaced.

## Why recovered hosts start another lineage

A live snapshot can expose sequences before the supervisor's next batched save. A crash can therefore leave an older tail on disk than a client has already consumed. Keeping the same lineage after restart would silently reuse those live sequence numbers.

Every startup that loads existing state now rotates the observation lineage and persists it before the host is exposed. Retained native records remain available for historical replay; earlier continuity is unknown. Healthy reconnects to the same host preserve its lineage. No per-event synchronization, sequence reservation protocol, receipt migration or automatic provider replay was added.

## Acceptance evidence

[Recorded checks](evidence/results.json) retain commands, environment, source hashes and logs for:

| Check | Result | Boundary |
| --- | --- | --- |
| Sequence lineage tests | 3 passed | Pure producer bookkeeping, serialization, restart and overflow |
| Reconciliation/data tests | 9 passed | Bounded replay, missing identities, both budgets, range details and three real SQLite collector cases |
| Provider process tests | 2 passed | Actual local supervisor plus offline JSON-RPC provider, including stale-disk recovery |
| Strict Clippy | Passed | Control crate, all targets, no dependency lint claims |

The process fixture primes cursor 1, then emits 300 or 3,000 provider events. It reports precisely 44 unavailable events (2–45) or 2,744 (2–2,745), keeps two workers and one pending request, and records no provider requests caused by reconciliation or restart. One counted incoming event is the fixture's approval request. A separate test restores disk at sequence 1 after exposing sequence 301: the next host uses another lineage and does not claim to know the exact lost unsaved tail.

The three cold SQLite cases use the prior independent fixture recipe. For 300 and 3,000 durable-item streams, persisted relationship metadata survives but the two old results outside the collector's 64-item per-thread tail remain unavailable. In the 3,000-delta/five-row case, both results and the delegation record survive; the stream gap remains separately visible. Repeated collection does not duplicate facts or invent backfill. Both SQLite files are byte-identical before and after each case.

The first attempt to run a supervisor test inside the normal sandbox could not bind a local socket (`Operation not permitted`). It was rerun with loopback access; this is an execution-environment limitation, not a provider failure.

## Critique and remaining limits

The change makes observation loss inspectable and recovers still-retained unknown-actor facts. It does not provide a complete transcript, a durable event journal, or an exact number of lost semantic results. After an actual host restart, conservative unknown continuity is preferable to a falsely precise count based on potentially stale disk.

The queue and range details are bounded. The surrounding pre-existing supervisor roster/event payload/storage limits are not redesigned here. Authenticated Codex sessions, PTYs, physical terminals and platform certification are not established by these tests. Full-UI composition evidence is recorded separately, and does not certify terminal transport or human understanding.

## Complete-screen composition fixture

`cargo run -p theywork-render --example coverage_inventory` exports 96 complete Ui surfaces under ignored `target/audit/iteration-9/captures/`. The fixture uses fixed time 1,000, reduced motion, both 8×16 and 10×20 cell geometries, six viewport sizes, and additional 80×24 light and image-free monochrome cases. It covers Inspector Now/Details, Attention, Deliveries, Changes and the coverage reader using actual mouse/keyboard routes, redrawing after each input. Cells, native text, physical RGBA and hit geometry are exported alongside a manifest.

The generated source data deliberately combines 300 missing provider-stream events shared by two Codex tasks, two local retention evictions, Claude tool-correlation loss and one simulated current request. These figures are separate units. The complete export passed all 96 route and hit-bound checks. Its separate visual review belongs to the integrated audit; the exporter marks visual review and terminal transport **not tested**. Compact Details is selected at 80×24 before resizing to 32×14; that frame truthfully represents the emergency brief and does not claim tab availability at the smaller size.
