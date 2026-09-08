# Follow-up: does the collector recover snapshot gaps?

**Yes, sometimes.** The actual Codex SQLite collector recovered the persistent parent→child relationship in all three integrated cases. It also recovered both deliveries when they remained within its initial 64-item tail. It did not recover the deliveries or historical delegation message when newer durable items put them outside that tail. This narrows the severity and scope of the initial isolated-supervisor finding.

## Three bounded cases

Each case applies `CodexSource::poll` into World first, then `snapshot_events` into that same World, matching the order in the TUI host. Source and control identities share the canonical synthetic home, and their project paths are identical. The collector is a new instance, representing startup/reopening rather than an uninterrupted background collector. The managed consumer starts at cursor 0, as a newly opened host view does. A second poll checks whether the now-initialized collector backfills the missed evidence.

The SQLite stores contain the expected two explicit final results, a typed spawn item and a durable `thread_spawn_edges` row. The oracle independently records their IDs and verifies their row positions with SQL. No result is deleted from the store to force the outcome.

| Input shape | Early record's newer items in its own thread | Collector + snapshot result | Second poll |
| --- | --- | --- | --- |
| 300 observations, 299 durable items | Spawn 149; each result 148 | Native metadata edge recovered. Zero result rows; no historical delegation record. | Still zero results |
| 3,000 observations, 2,999 durable items | Spawn 1,499; each result 1,498 | Native metadata edge recovered. Zero result rows; no historical delegation record. | Still zero results |
| 3,000 observations, mostly deltas aggregated into two durable commentary items; 5 total items | Spawn 2; each result 1 | Both results and the typed delegation message recovered; one metadata edge. | Two results remain, no duplicates |

All cases retained exactly two people in one project, proving that the two feeds joined the same identities. Both SQLite database files were byte-for-byte unchanged by collection. The notebook retained its generic partial-history notice throughout.

The managed snapshots are synthetic 256-event windows taken from each case's independent full input stream. They exercise the actual public bridge. They do **not** rerun the supervisor: that ring's behavior was already measured with the real supervisor in the separate eight-case audit. The different persistence shape in the positive case is deliberate: thousands of streamed deltas need not equal thousands of durable history rows.

## Ranking correction

- Keep the explicit continuity diagnostic candidate: a known snapshot gap still deserves an accurate explanation. Do not present it as proof that the complete app necessarily loses every semantic fact.
- Qualify delivery loss as **cold load/reopen plus older result outside the per-thread history tail**, when no other retained World evidence supplies it. The integrated reproduction still supports this narrower loss scenario.
- Downgrade the claim of missing family structure where the store has native spawn metadata: that relationship was recovered in all three cases. The historical delegation message/prompt can still be absent. A source schema without the metadata edge is not tested by these three cases.
- Recent-result recovery is a positive existing capability and must remain a regression test. The combined feeds neither duplicated deliveries nor multiplied workers.
- A continuously running collector can use its incremental cursor. These tests do not establish loss during uninterrupted collection, nor performance under a large real history. They also do not establish that any specific live provider emits the tested persistence shape.

For candidate D1, implementation acceptance should include these three dual-feed cases alongside the raw snapshot tests. Coverage must distinguish an unobserved stream interval from facts recovered through another source. A recovered relationship is usable evidence even when unrelated messages are missing; a generic gap must not invalidate a valid current approval or conceal recovered results. Any later read-only backfill design must preserve the positive deduplication behavior.

## Reproduce and inspect

```sh
python3 docs/design-audit/iteration-7/data/run_dual.py
```

The runner builds only the standalone `dual_feed` audit target, offline and locked. It uses synthetic SQLite files under the existing ignored iteration7-data scratch directory, and invokes no provider process. It stores results separately under [evidence/dual-feed](evidence/dual-feed/results.json): [metadata](evidence/dual-feed/metadata.json), [build log](evidence/dual-feed/build.log), [run log](evidence/dual-feed/run.log). The runner explicitly verifies that every file in the original eight-case evidence directory remains byte-identical.

Production code and APIs remain unchanged. This bounded run was concurrent with other lightweight/large-history discovery work, so its build/runtime is not used as a product performance benchmark.
