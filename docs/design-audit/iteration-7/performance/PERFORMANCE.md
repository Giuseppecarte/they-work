# Resource, history and response measurements

The measurements use the actual macOS arm64 executable where stated, and a
small path-dependent source probe where isolated collector timing is needed.
They do not establish physical-terminal latency. The 150 ms p95 input-to-visible
feedback target remains unverified on an actual emulator.

## Large historical stores

Each case contains 20 projects, 50 active conversations, and 1,000 older
conversations. One active transcript grows to the stated size through synthetic
tool-result bodies of about 32 KiB each. The collector starts with no in-memory
cache; the OS page cache is uncontrolled and likely warm after fixture creation.
The two-hour headless process was concurrently active.

| Largest transcript | CLI startup to completed `--once` | CLI peak RSS | Fresh source poll | Quiet poll p95 | One-record append poll p95 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 10 MiB | 91.8 ms | 33.1 MiB | 94.6 ms | 8.12 ms | 6.99 ms |
| 100 MiB | 195.6 ms | 39.9 MiB | 179.5 ms | 7.57 ms | 7.03 ms |
| 1,000 MiB | 1,403.2 ms | 109.0 MiB | 1,168.0 ms | 6.02 ms | 6.12 ms |

CLI startup and fresh source timing come from different processes. They are not
components to subtract or add together. The CLI uses the production release
profile; the probe uses its standalone release profile and records a separate
binary hash. Each startup is one sample, while each quiet/append p95 uses 60
samples with the nearest-rank estimator. CLI peak RSS comes from macOS
`/usr/bin/time -l`, whose maximum-resident-set-size unit here is bytes.

All three cases exited successfully and retained 50 workers in 20 projects.
The larger initial scan is measurable; it is not a violation of the interactive
150 ms target, which measures a different boundary. The repeated tail polls are
fast in these fixtures. Before changing the collector, test a concrete remaining
question: does a large initial or replaced file delay another source's discovery
or make the real UI unresponsive? Large bodies, many small events, network
filesystems, and slow disks are different workloads.

Raw results, all 60-sample distributions, commands and resource output are in
[large-history/results.json](large-history/results.json) and adjacent files.
The byte-exact [source used for measurement](source-used-for-measurement.txt)
is retained and hashed in that metadata. The maintained `src/main.rs` received
Rust formatting afterward; no measured binary or numerical result was replaced.

## Two-hour workload

The real process completed **7,200.109 monotonic seconds**, exited zero, and
retained exactly 50 workers and 20 projects throughout its reported min/max
counts. It performed 7,039 polls with zero polling errors. A fresh final
`--once` probe also returned 20 projects/50 workers and exit zero. The generator
appended 174,200 observations over 3,484 writer ticks; the final synthetic store
contained 89,548,320 bytes. See [the final result](two-hour/result.json) and
[headless counters](two-hour/headless.txt).

| Resource observation | Measured value |
| --- | ---: |
| Usable `ps` samples, approximately 30 seconds apart | 240; zero missing |
| Highest sampled RSS | 31,888 KiB / 31.14 MiB |
| Median RSS in the first ten steady minutes (after minute 1) | 14,984 KiB / 14.63 MiB |
| Median RSS in the final ten sampled minutes | 31,688 KiB / 30.95 MiB |
| Mean one-core utilization from cumulative CPU delta | 1.71% |
| Sampled `ps` percent-CPU p95 | 5.1% |

The CPU delta was 122.9 seconds over 7,194.686 sampled monotonic seconds.
**RSS did not plateau over this entire fixture**; the late increase motivated
PERF-01 below. These are sampled resident pages, not an allocation profile.
The headless executable's own RSS/CPU fields were unavailable on this host;
the table comes from the separately recorded `ps` samples, not those fields.

The application reported 7,200.019 elapsed seconds and the wrapper observed exit
at 7,200.109 seconds. This measures automatic deadline completion, not
keyboard-to-cleanup shutdown latency. UTC wrapper timestamps span approximately
7,294.580 seconds including outer probes. The clocks/boundaries were not
calibrated against each other, so that difference is retained as measurement
uncertainty without attributing it to a specific cause. Both the application's
counter and the wrapper's monotonic measurement meet the two-hour duration.

[Resource samples](two-hour/resources.csv) and the generated
[summary](two-hour/summary.json) preserve the calculations.
[progress.json](two-hour/progress.json) remains the last periodic running
checkpoint; the final result and summary above are authoritative for completion.

The fixture appends one synthetic Read/Edit/Bash observation to each of the 50
active conversations about every two seconds. The displayed Bash text is never
executed. The actual application runs with explicit synthetic Claude/config
paths, `--no-save --headless --exit-after 7200s`. The headless loop exercises
collection and World folding, not graphics, terminal encoding or image queues.
Its reported frame count is a loop count, not displayed animation throughput.

The generator supplies tool starts without matching `tool_result` records.
This is an incomplete-result stress stream, not normal paired tool traffic.
A [late memory review](MEMORY-REVIEW.md) found an unbounded per-file pending-tool
correlation map; its derived allocation thresholds correlate with the sampled
RSS steps. This is now PERF-01 in the roadmap. The lifecycle oracle can pass
while that bounded-state concern remains open. No heap profile or paired-tool
measurement was performed, and resource stability is not claimed.

The oracle requires process exit zero after at least 7,200 seconds, zero polling
errors, 50 initial/final workers, and a final independent `--once` probe reporting
20 projects and 50 workers. CPU and memory are observations rather than hidden
pass thresholds. Thirty-second sampling cannot establish absolute RSS peaks or
rule out shorter spikes. Even a stable two-hour result cannot prove indefinite
resource bounds, and does not exercise operation-ledger growth.

## Response boundaries

Input-to-semantic-feedback, encoded-image completion and source-to-panel delay
must be reported separately. The PTY probe records bytes received by the test
reader, with simulated terminal capabilities; it cannot see the operating
system or emulator paint the display. Physical display latency requires the
owner-run procedure in [study/CAPTURE.md](../study/CAPTURE.md), terminal/version
metadata, and frame-rate uncertainty. Neither the headless scan nor iteration
6's compositor timing substitutes for that measurement.

The [bounded PTY probe](pty/README.md) completed 25 input and 25 source-update
samples in each of two modes, using the same source path and synthetic facts.
Input to the correct native heading had p95 8.83 ms in compact mode and 84.26 ms
with simulated Kitty graphics. The calibrated complete office image arrived
with that same native-output read in the graphical condition; the probe cannot
resolve their ordering within one read. Source commit to selected brief had p95
975.63 ms and 1,043.55 ms respectively. All samples completed and both processes
restored terminal flags. These are separate PTY receipt measurements, not a
physical display pass or a 50-task rendering measurement.

Actual Windows, WSL, Linux and macOS terminal-window timings remain **not tested**.

## Reproduction and fixture corrections

Use a Rust 1.90 toolchain and Python 3, from the repository root:

```sh
sh docs/design-audit/native-cargo.sh build --offline --release --manifest-path docs/design-audit/iteration-7/performance/Cargo.toml
python3 docs/design-audit/iteration-7/performance/large_history.py
python3 docs/design-audit/iteration-7/performance/soak.py --binary target/native-macos/release/they-work --scratch docs/design-audit/tmp/iteration-7-performance/repeat --output docs/design-audit/tmp/iteration-7-performance/repeat-evidence --seconds 7200
python3 docs/design-audit/iteration-7/performance/summarize.py
```

The build command assumes the existing native-audit toolchain/cache; the
standalone manifest can also use ordinary Cargo with a configured
`CARGO_TARGET_DIR=target/native-macos`. The production release binary must already
exist. The `--offline` build requires cached dependencies. The first probe build
attempt lacked network resolution; the subsequent offline build succeeded.
Those logs are retained, not counted as product failures.

`smoke/` records an invalid first fixture whose encoded project directories
collapsed into one project. `validated-smoke/` verified the corrected project
count but its resource harness was interrupted when the sandbox blocked `ps`;
there is no completion/resource claim. `final-smoke/` is the corrected five-second
preflight with permission to sample its own child process. It is not the
two-hour result. The benchmark and soak source, standalone lockfile, binary
hashes, raw measurements and failed attempts remain available for review.
