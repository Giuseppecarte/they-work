# Local composition timing

The release compositor completed 27 cases with 60 recorded samples each after
10 warm-up frames: 1,620 measured frames. The largest per-case p95 was **8.899 ms**;
the largest individual sample was **10.361 ms**. The full distribution summary is
in [composition.csv](evidence/composition.csv), with the command, executable and
source hashes in [generation metadata](evidence/composition-generation.json).

The table shows the largest case p95 across 1, 6 and 20 projects at each size.
Every fixture contains 50 synthetic tasks in total.

| View | 80×24 | 120×36 | 192×58 |
|---|---:|---:|---:|
| Tower | 1.999 ms | 6.582 ms | 8.899 ms |
| Office | 2.001 ms | 4.127 ms | 7.398 ms |
| Inspector | 0.067 ms | 2.759 ms | 4.942 ms |

This is input handling through local UI drawing and native-background masking on
macOS arm64. It uses the actual `Ui` and release `TestBackend`; the clock advances
before each timed interval. Motion is enabled. It excludes source collection,
provider work, IPC, terminal encoding, transport, output writes, OS painting and
display refresh. It cannot certify the end-to-end 150 ms terminal-response goal.

The inspector at 80×24 is a native panel without a scene image, reported as 0×0
physical pixels. At 120×36 and 192×58 its image regions are 632×528 and 1016×880.
Tower and office regions are 640×336, 960×528 and 1536×880. These dimensions are
part of the measured result, not interchangeable workloads.

The [iteration 5 measurements](../iteration-5/input-composition.csv) are retained
as context: their largest per-case p95 values were 9.180 ms, 8.100 ms and 6.526 ms
for tower, office and inspector. The current chrome reserves one fewer terminal
row, the roster and inspector contents changed, and this is a separate run.
Those values therefore do not establish a controlled speedup or regression.
The current 10.361 ms outlier is retained rather than removed by rerunning.

```sh
env -u NO_COLOR -u THEYWORK_COLOR -u THEYWORK_PIXELS COLORTERM=truecolor \
  target/native-macos/release/examples/measure_tower \
  > docs/design-audit/iteration-6/evidence/composition.csv
```

Workspace checks, builds and geometry capture had finished before measurement;
no concurrent CPU-heavy task ran during the benchmark. A subsequent correction
changes only the context label from “1 projects” to “1 project”. The benchmark
was not rerun for that wording change; its recorded hash is the binary actually
measured.
