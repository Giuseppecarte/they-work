# PERF-01 closure verification

**Verified for the planned synthetic engineering envelope.** All eight replays passed, covering 1,024,000 tool starts, all 512,000 expected paired outcomes and 8,940 collection samples. No new correlation-limit or attribution failure was reproduced. This does not establish physical-terminal response or authenticated-provider behavior.

## Frozen sources and reproducibility

The measured collector and shared core are from `4097e4f706faff924fbe71fb075ec469909746bf`. The existing exact ignored resource test is used unchanged for the 32-call cases. The missing 1/8/128 envelope is run from an isolated copy of that revision with six test-only substitutions: a checked overlap parameter, derived batch count, call loop, sequential ID calculation, expected paired entry count, and recorded overlap. Production files are identical to the frozen revision.

Every case contains 50 workers and exactly 128,000 tool starts. Overlap is 1, 8, 32 or 128 calls per worker before collecting results. This produces 2,560, 320, 80 or 20 batches. The paired case writes a matching result phase; the unpaired case deliberately never supplies results. Calls alternate the same Bash and Edit observations, with the same native IDs and input bodies across the envelope. The fixture only writes generated transcripts under ignored `target/audit/`; it does not execute the recorded commands or access a provider account.

Run the collector library check before preparing, so the current exact-test binary can be retained. Preparation requires the existing local Rust 1.90.0 audit toolchain and dependency cache and uses the repository lockfile offline:

```sh
mkdir -p target/audit/iteration-10/perf
sh docs/design-audit/native-cargo.sh test --locked -p theywork-collect --lib claude -- --nocapture > target/audit/iteration-10/perf/focused-tests.log 2>&1
python3 docs/design-audit/iteration-10/perf/replay.py prepare
python3 docs/design-audit/iteration-10/perf/replay.py run --mode paired --overlap 32
python3 docs/design-audit/iteration-10/perf/replay.py run --mode unpaired --overlap 32
```

Repeat the two `run` commands with overlaps 1, 8 and 128, sequentially, then run `python3 docs/design-audit/iteration-10/perf/replay.py summarize`. To validate reuse on a later candidate, first run `python3 docs/design-audit/iteration-10/perf/replay.py match-source --candidate <commit>`; it refuses reuse if any of the 24 core/collector/build inputs changed. The runner refuses existing output paths, checks that exactly one test executed, and rejects missing RSS evidence. For another repetition, use a fresh checkout and its fresh `target/audit/` tree; do not delete or overwrite historical evidence. `/usr/bin/time -l` is macOS-specific, and process inspection must be available for the test's `ps` samples. Python 3.12 or later is required for safe archive extraction.

## Completed envelope

Paired peaks were 50 / 400 / 1,600 / 6,400 entries for overlaps 1 / 8 / 32 / 128, respectively. Peak owned strings were 2,250 / 15,400 / 61,600 / 246,400 bytes. Every result phase returned entries, lookup capacity and the ordering index to zero; no paired budget loss, ambiguity or oversize notice occurred. Each unpaired run plateaued at 8,192 entries and 315,392 owned-string bytes, ended with exactly 119,808 retirements, and produced zero invented outcomes.

| Mode / overlap per worker | Poll samples | Poll p95, ms | Sampled peak RSS, KiB | Process peak RSS, KiB | CPU user / system, s | Process elapsed, s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| paired / 1 | 5,120 | 16.58 | 6,560 | 30,016 | 50.21 / 88.34 | 201.05 |
| unpaired / 1 | 2,560 | 13.43 | 10,320 | 20,560 | 23.02 / 41.09 | 77.93 |
| paired / 8 | 640 | 50.65 | 7,184 | 9,536 | 23.29 / 44.94 | 81.87 |
| unpaired / 8 | 320 | 49.94 | 10,704 | 10,704 | 10.49 / 22.40 | 38.80 |
| paired / 32 | 160 | 99.49 | 11,904 | 11,904 | 17.49 / 35.24 | 56.38 |
| unpaired / 32 | 80 | 87.39 | 15,536 | 15,536 | 8.84 / 18.31 | 28.31 |
| paired / 128 | 40 | 524.72 | 52,336 | 52,336 | 19.03 / 30.78 | 52.04 |
| unpaired / 128 | 20 | 466.23 | 41,968 | 41,984 | 9.79 / 17.75 | 29.84 |

Full sample distributions are retained as lossless `.json.gz` files alongside exact test logs and macOS time reports. [RESULTS.json](RESULTS.json) contains numeric summaries; [MANIFEST.json](MANIFEST.json) identifies hashes, commands and reused source evidence. All 24 relevant core/collector/build inputs also match integrated candidate `7cb26e45e13de5fa9bfa3a3be7c2eeb59a1011c1`, allowing these frozen measurements to be reused for that candidate.

## Acceptance and measurement boundaries

The focused Claude checks passed **14 tests**, with one intentionally ignored resource replay. The complete collector library check passed **27 tests**, including those 14, with the same one ignored replay. These are overlapping test selections, not 41 distinct tests.

Those tests cover per-cursor and aggregate count/byte pressure, duplicate and conflicting starts, oversize IDs, Unicode string capacity, deterministic retirement, late-result attribution, background launch acknowledgements, reset and removal cleanup, old/new worker identity, healthy-poll loss persistence, and unchanged provider-store bytes. The resource probe separately checks source bounds and lookup/order agreement at every collection phase. Paired result phases must return entries, lookup capacity and order length to zero without losses. Unpaired cases must finish at 8,192 entries with exactly 119,808 retirements and no invented outcomes.

Source `poll` time excludes fixture generation and RSS sampling. Whole-process CPU and elapsed time include the Rust test's fixture and sampling overhead; CPU is reported as separate user and system seconds. Sampled RSS is in KiB and is measured after each poll; the macOS process high-water RSS is independently reported in bytes. Owned-string accounting is not total heap or RSS. Percentiles use nearest rank.

The probe retains all sample records and serializes the complete result after its last RSS sample. That measurement storage grows with sample count, unlike the bounded pending store, and contributes to process high-water RSS. Compare store entry/byte counters for the correlation limit; do not interpret process high-water RSS as a measurement of that store alone.

The replay cases are sequential, but run on a shared development machine alongside unrelated verification work. Filesystem caches and desktop load are not isolated. These are debug builds. A smaller fresh timing sample does not invalidate iteration 9's recorded 175.26 ms unpaired poll p95 or demonstrate an optimization: the production correlation code is unchanged. The original pre-change measurements remain historical comparison evidence, not a simultaneous controlled performance baseline.

No fresh two-hour soak, Windows/Linux RSS curve, heap profile, authenticated traffic, real-user overlap distribution, physical-terminal response, or 150 ms UI acceptance claim is made. The numeric limits remain conservative engineering policy, not a demonstrated description of typical users.

## Critical review

The planned count/byte and attribution acceptance criteria now have complete 1/8/32/128 synthetic coverage. Normal paired observations remain intact, overload remains bounded, and late or ambiguous records do not acquire invented attribution. The helper and test-only envelope extension are audit artifacts; production APIs, persistence and collector behavior were not changed.

Larger batches still cost more source-processing time. At 128 overlapping calls per worker, one phase delivers 6,400 new records across 50 workers; fresh poll p95 was 524.72 ms paired and 466.23 ms unpaired. These debug, shared-host results deserve visibility even though they do not measure or invalidate the separate 150 ms input-to-visible target. No performance improvement is claimed against earlier runs. An optimization investigation would need an explicit question about optimized source-update delay under sustained large batches; it is not necessary to redefine this bounded-correlation finding as a whole-application speed guarantee.

The remaining uncertainty is environmental and behavioral generalization: real users, authenticated records, other operating systems and longer application lifecycles remain outside this verification. Existing evidence for those areas must not be inferred from these passing replays.
