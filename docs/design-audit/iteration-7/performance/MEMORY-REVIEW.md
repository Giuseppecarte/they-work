# Late-soak memory step: bounded source review

**A per-file correlation map can grow with every unmatched tool call.** This is separate from World's bounded activity history. The active soak supplies exactly the kind of stream that grows it: unique `Edit`/`Bash` tool-use IDs without matching tool results. Several RSS steps align with plausible hash-table capacity transitions. The accumulation is confirmed by source; attributing the measured bytes to rehashing remains a causal hypothesis, because this review did not inspect the process heap or its map capacities.

This review only read source and existing resource rows, through elapsed 6,110.198 s. It did not touch, stop, instrument or add load to the running application. It does not declare the two-hour run finished or passed.

## Confirmed state lifetime

- [`ClaudeSource::FileCursor.pending_tools`](../../../../crates/theywork-collect/src/claude.rs#L509) is a `HashMap<String, PendingTool>` with no count limit, expiry or per-turn pruning. [`parse_assistant`](../../../../crates/theywork-collect/src/claude.rs#L1290) inserts an owned ID and activity for each eligible tool use.
- [`pending_tool_kind`](../../../../crates/theywork-collect/src/claude.rs#L1393) includes Bash, Edit/Write/NotebookEdit and Task/Agent. **Read is not inserted.** This is not growth for every transcript event.
- [`parse_user`](../../../../crates/theywork-collect/src/claude.rs#L1346) removes an entry when a matching `tool_result` arrives. Completing a turn does not clear this map. A cursor reset clears it; replacing/removing its active file drops the cursor. As long as a transcript remains active and receives distinct unmatched calls, its entry count has no declared bound.
- Each entry owns the tool ID and an activity; displayed input detail is truncated, but that bounds individual detail length, not entry count. `HashMap::clear` on reset can also retain allocated capacity until the cursor is dropped; a cleared count does not by itself prove RSS immediately falls.

The other inspected state does not grow once per tool-use ID in this fixture:

| Structure | Code boundary and relevance |
| --- | --- |
| World worker history | [`remember`](../../../../crates/theywork-core/src/model.rs#L262) limits it to 64 beats. These unmatched tool uses emit current `Acted` activity, not completed-tool history beats; they do not each fill another permanent World row. |
| World collaboration / relationships / retired workers | 512 collaboration records, 2,048 relationship keys and 512 retired workers. The soak has no delegated children or typed collaboration messages. |
| Active file cursors, identities, names and office cache | Pruned against active files/projects; this fixture keeps 50 active files and 20 projects rather than continually creating workers. Each cursor still owns its independent pending-tool map. |
| Claude delegation maps | Explicit 4,096-entry cleanup; no Task/Agent events in this workload. That limit does not apply to `pending_tools`. |
| Incomplete JSONL line and rewrite checkpoint | 1 MiB maximum line buffer and 4 KiB checkpoint per cursor. Completed-line buffers can retain their high-water allocation, but line length is not growing here. |
| Poll event vectors | Temporary per-poll observations, not an accumulating ID ledger. A backlog/rewrite can increase one batch, but that is not established as the cause of this sampled step. |

## Why this fixture accumulates pending entries

[`soak.py`](soak.py) seeds 200 Read calls per active worker, then appends one assistant `tool_use` every writer tick in the sequence Edit, Bash, Read. The IDs are unique (`call-{worker}-{ticks+1000}`). It never writes a `tool_result` for them. The initial Read calls contribute zero pending-map entries.

For a cursor that has consumed all written records, the expected pending count after writer tick `t` is:

```text
pending_per_worker(t) = t - floor(t / 3)
```

The same schedule runs for all 50 workers. At tick 2,699, this yields 1,800 entries per worker, 90,000 total. These counts are derived from the fixture and insertion/removal code, **not** sampled from process memory; a poll may lag the writer. The approximately 30-second resource interval is coarser than that lag and than the individual allocation operations.

The fixture therefore represents an **unmatched-tool / incomplete-result stream**, not repeated normally completed Read/Edit/Bash cycles. That is a valid stress condition for an observer of partial trails, but its resource curve must not be generalized to paired tool-use/result traffic.

## Specific capacity transition consistent with the step

The largest step in the reviewed CSV prefix is:

| Elapsed seconds | Writer ticks | Derived pending entries per worker | Sampled RSS |
| ---: | ---: | ---: | ---: |
| 5,537.700 | 2,684 | 1,790 | 18,080 KiB |
| 5,567.807 | 2,699 | 1,800 | 30,688 KiB |

That interval crosses 1,792 entries per cursor. The increase is 12,608 KiB (12.31 MiB). A 2,048-bucket table with a 7/8 occupancy limit has usable capacity 1,792, making growth to the next table size a concrete candidate. Because all 50 maps receive the same tool schedule, their growth can cluster in the same sampling interval.

Earlier steps also bracket that capacity family:

| Candidate usable capacity | Derived count before→after | CSV seconds before→after | RSS KiB before→after |
| ---: | ---: | --- | --- |
| 112 | 107→117 | 330.923→361.097 | 13,904→14,976 |
| 224 | 224→234 | 692.193→722.232 | 15,200→17,664 |
| 448 | 448→458 | 1,383.742→1,413.746 | 10,624→14,624 |
| 896 | 895→905 | 2,768.613→2,798.804 | 14,336→17,408 |
| 1,792 | 1,790→1,800 | 5,537.700→5,567.807 | 18,080→30,688 |

The locally cached hashbrown 0.15.5 source uses a 7/8 occupancy rule in `raw::bucket_mask_to_capacity`. This corroborates the capacity hypothesis; it is **not** proof of the exact allocator/table implementation linked into the production standard library. No new capacity probe or Cargo invocation was performed. Page residency, allocator reuse and other transient allocations can affect RSS, so this review does not assign all 12.31 MiB to those maps. In particular, the earlier RSS decreases show that sampled residency is not the same thing as retained logical entries.

## Decision implication

There is now a specific follow-up worth prioritizing: define a bounded lifetime for unmatched tool correlations while preserving truthful outcomes and late-result handling. The 64-beat local history guarantee does not currently bound this collector map. Raising the history limit or changing graphics would not address it.

If selected for follow-up, use a controlled paired-versus-unmatched replay with the same unique IDs and body sizes, recording total pending length/capacity through audit-only instrumentation and RSS separately. Include a late result and a cursor reset/removal. That distinguishes normal completion, missing results and retained allocator capacity. It need not become another broad soak. No such workload was run here, and no additional workload is required to complete this discovery review.

**Not tested:** a paired `tool_use`/`tool_result` workload, direct pending-map length/capacity sampling, or heap attribution. The current curve establishes none of those results.

The smallest production policy to evaluate is a count **and** owned-byte budget for pending correlations, with a source-wide aggregate budget so multiplying active files cannot bypass it. Select numerical limits only after measuring representative paired and overlapping calls; this review supplies no justified default count or MiB value. Keep only the fields needed to correlate an outcome. On budget pressure, evict the oldest unresolved entry deterministically by local insertion order, continue observing current activity, and publish a bounded correlation-loss counter/reason. A single oversize correlation should not bypass the byte budget or stop the entire source. Account for table capacity separately when checking the memory bound: string-byte accounting alone is not RSS or full heap usage.

Acceptance should require that matching results release their accounting; repeated/duplicate IDs do not grow it; late results for evicted IDs never acquire another tool's activity or an invented success; and correlation loss remains explicit without retaining an unbounded tombstone set. Cursor reset/removal must release its contribution to the aggregate budget. Verify the count/byte boundaries, normal paired recovery and overload recovery using the same record IDs, while preserving read-only source behavior. Budget eviction, correlation recovery and late results must never replay a command, resume a turn or issue a provider mutation. Those are future acceptance tests, not claims about the active soak.

A future eviction policy must report insufficient correlation rather than fabricate a successful tool outcome when a late result arrives. The existing runtime oracle can still finish successfully on worker count, polling errors and exit status, but those checks do not establish bounded pending state or a memory plateau. Label this accumulation finding separately from the final soak completion status.
