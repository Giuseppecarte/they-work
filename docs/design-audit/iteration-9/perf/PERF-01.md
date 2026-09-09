# PERF-01: bounded Claude tool correlation

**Implemented and exercised with synthetic provider-shaped records.** The observer now retains at most 256 pending correlations / 256 KiB of owned string allocations per cursor, and 8,192 / 8 MiB per Claude source. These are engineering limits, not a measured claim about typical user concurrency. They do not bound every allocation made by the collector or application.

## Behavior and compatibility

The source owns one correlation store, scoped by cursor and exact native tool ID. An ordered index records local insertion order, independently of native timestamps. Pressure retires the oldest unresolved entry in the affected cursor, then the source if necessary. The index contains only fixed-size cursor tokens and ordinals; the native ID is owned once. Removing an entry synchronously removes its order entry. Sparse lookup tables shrink, and empty tables are dropped.

The byte counter uses native-ID allocation length plus retained activity `String::capacity()`. It includes every dynamically owned string in the lookup and order structures; the order index owns no strings. Inline entry/index storage, allocator metadata, and active-cursor bookkeeping remain separate, bounded by their corresponding entry/file counts. Lookup capacity and order length are exposed only to the test probe and measured separately from RSS. No public collector-control API or provider-store schema changes.

A single oversize ID is omitted from pending storage while its observed activity is still emitted. Identical duplicate starts do not grow state or refresh retirement age. A changed complete tool-block fingerprint marks the existing ID ambiguous rather than overwriting its attribution; its bounded slot is removed by a result or ordinary retirement. The fingerprint is an in-process consistency check, not a cryptographic integrity claim. Full inputs/results are not retained to perform that check.

Matching results release their accounting. A late result without retained context does not acquire an invented command, success, child actor, or delivery classification. Normal untracked Read results are not counted as budget loss. Independently recorded child transcripts continue through the existing collaboration path. Background launch acknowledgements keep a bounded pending correlation until an explicit terminal result; an acknowledgement still does not create a delivery.

`ToolCorrelationCoverage` supplies saturating eviction, oversize, ambiguity and reset-discard counts, an epoch and observation time. Counts describe observation limitations, not distinct lost results. Rewriting a cursor publishes its old worker's reset loss before starting a new epoch. The shared coverage merge retains prior loss for the same worker; a replacement worker does not inherit it. Normal healthy polls retain these facts, and source loss remains separate from freshness/availability. Cursor removal or source clearing releases the aggregate accounting without an unbounded historical-ID map.

## Before/after replay

Each run contains **128,000 identical tool starts across 50 workers**, with 32 calls per worker written before each collection phase. Calls alternate Bash and Edit, preserving IDs and inputs between versions. The paired case then appends matching recorded results; the unpaired case never does. Both use 80 batches. This is an accelerated bounded replay of the incomplete-correlation mechanism, not another two-hour lifecycle soak.

| Measurement | Paired before | Paired after | Unpaired before | Unpaired after |
| --- | ---: | ---: | ---: | ---: |
| Peak retained entries | 1,600 | 1,600 | 128,000 | 8,192 |
| Peak owned string bytes | 61,600 | 61,600 | 4,928,000 | 315,392 |
| Peak lookup capacity | 4,736 | 2,800 | 179,200 | 22,005 |
| Final lookup capacity | 4,151 | 0 | 179,200 | 14,351 |
| Recorded outcomes | 128,000 | 128,000 | 0 | 0 |
| Budget retirements | not supported | 0 | not supported | 119,808 |
| Sampled peak RSS, KiB | 10,944 | 12,400 | 32,960 | 16,656 |
| Poll p95, ms | 73.822 | 92.155 | 57.114 | 175.258 |
| Whole replay, seconds | 46.248 | 52.554 | 24.232 | 30.313 |

The candidate paired run preserved every expected outcome, with zero retirement/oversize/ambiguity. Every paired result phase returned both lookup capacity and order length to zero. Under unmatched pressure, count and owned bytes plateaued within limits; the final retirement count is exactly `128000 - 8192`. The independent outcome oracle expects zero completed tools in that workload, rather than treating worker counts alone as correctness.

These are **debug Rust test processes on macOS**, not optimized application throughput or terminal responsiveness. RSS was sampled with `ps` after each poll; it is not heap attribution or a whole-process memory guarantee. Runs were sequential, but the desktop/development environment and filesystem cache were not isolated. Initial sandboxed runs could not sample RSS; their numeric records are retained separately, and the measured runs used the same frozen binaries with local process inspection available. Baseline pending behavior came from `77a9b785dd25d27884b7fe2f81548d538f53a35a`; additive default coverage fields were already present in the integration checkout. Exact test-binary and measured source hashes are recorded in [RESULTS.json](RESULTS.json).

## Acceptance coverage

The focused tests cover cursor and source count/byte limits independently, Unicode allocation capacity, oversize IDs, simultaneous cursor pressure, deterministic retirement, repeated and conflicting IDs, ordinal rebasing, synchronous accounting/index cleanup, late results, unchanged provider-store bytes, healthy-poll loss persistence, reset/reconnect, replacement-worker identity, and a background acknowledgement followed by actual completion.

The resource probe additionally asserts all candidate count/byte bounds after every collection phase, equal lookup/order entry counts, no normal paired losses, and the exact 128,000-start/128,000-or-zero-outcome oracle. The collector's existing collaboration and collector suites remain the integration regression checks; the iteration-wide validation records their final execution.

**Not tested here:** authenticated Claude traffic, real-user concurrency distributions, heap profiling, Windows/Linux RSS curves, physical-terminal feedback, or a new two-hour soak. The visual p95 target of 150 ms remains separate. The measured unpaired debug poll p95 is above 150 ms and is reported openly; it is a source-processing measurement, not an input-to-paint measurement.

## Reproduce and review

Use the pinned Rust toolchain and lockfile. With the native audit toolchain installed, run one exact ignored test for each mode, using a fresh output directory:

```sh
THEYWORK_PERF_MODE=paired THEYWORK_PERF_OUTPUT="$PWD/target/audit/perf-paired" \
  sh docs/design-audit/native-cargo.sh test -p theywork-collect --lib \
  claude::correlation_tests::correlation_resource_replay -- --exact --ignored --nocapture
THEYWORK_PERF_MODE=unpaired THEYWORK_PERF_OUTPUT="$PWD/target/audit/perf-unpaired" \
  sh docs/design-audit/native-cargo.sh test -p theywork-collect --lib \
  claude::correlation_tests::correlation_resource_replay -- --exact --ignored --nocapture
```

A standard installed toolchain can use `cargo` in place of the native wrapper. The current probe validates a fresh path under the repository's canonical `target/audit` before generating fixtures. Its path guard was tightened after the frozen resource runs; measured source snapshots identify the earlier guard, and the workload/accounting semantics are unchanged.

For the baseline, use an isolated checkout of the revision above, copy [baseline-probe.rs](baseline-probe.rs) to the collector's `src/claude_correlation_tests.rs`, and add this test-only declaration inside `claude.rs`: `#[cfg(test)] #[path = "claude_correlation_tests.rs"] mod correlation_tests;`. Then run the same exact test/modes. The retained baseline probe reconstructs the measured workload/accounting semantics; it is not a byte-identical source snapshot of the initially unformatted probe. Do not modify a provider home or rewrite historical iteration-7 evidence.

## Critique

The overload mechanism is now bounded and its limitations are visible without inventing outcomes. The normal paired envelope lost no evidence. The tradeoff is measurable: the extra ordering, full-block consistency fingerprint and retirement work increased debug poll times, and the paired RSS sample rose modestly. The current oldest-entry lookup scans at most the cursor's 256 retained entries; the global index itself stays exact and bounded. These results justify a bounded observer, not a claim that memory, speed or real-terminal behavior is universally solved. Any further optimization should answer the specific question of optimized-build overload cost before changing the retention or attribution policy.
