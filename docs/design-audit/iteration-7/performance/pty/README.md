# Bounded executable PTY receipt timings

These measurements concern **bytes received by an observing Python process**.
They are not physical visible-response times, terminal certification, provider
service latency or a claim about all project sizes. No terminal GUI, personal
account or real provider was used. The existing native release executable was
run without rebuilding or changing production code.

The final [results.json](results.json) retains every sample, the binary SHA-256,
calibration frame hashes, pixel geometry, conditions, failures and process exit
checks. The binary is
`b2c529653f5b9d6a2ee93cd84cb87772ee8e1dee96b431ac9b645f3bfd33de84`.

## Final observations

Each measured endpoint below has 25 samples. Both processes exited with code
zero and restored terminal flags; no failed samples were omitted. The matched
office image was 960×528 pixels within the 120×36 terminal.

| PTY endpoint | Compact p50 / p95 / max (ms) | Simulated Kitty p50 / p95 / max (ms) |
| --- | --- | --- |
| Input → correct native floor heading | 6.34 / 8.83 / 9.13 | 79.23 / 84.26 / 96.54 |
| Input → complete calibrated office image | Not tested: image-free mode | 79.23 / 84.26 / 96.54 |
| Committed observation → selected brief | 502.80 / 975.63 / 1056.87 | 604.30 / 1043.55 / 1098.21 |

All 25 graphical input samples matched the calibrated office image. Native
feedback and graphical completion shared their receipt timestamp in this run;
the observer cannot resolve their ordering within that read. These observations
support separate investigation of graphical turn-around and source polling;
they do not identify an encoder bottleneck or establish a physical display SLA.

## Three separate endpoints

1. **Input → correct native semantic feedback.** Immediately before writing
   key `1` to the PTY, record `time.monotonic_ns()`. The baseline is the Tower.
   Stop at the read whose parsed ANSI cells first contain
   `alpha-shop / Office`. The baseline assertion requires that heading to be
   absent before the key; a stale matching screen cannot satisfy the sample.
2. **Input → complete matching graphical payload.** In the graphical condition,
   decode the Kitty payload and compare its exact RGBA SHA-256 to the office
   frame calibrated in two earlier, unmeasured Tower → Office cycles. The two
   office hashes must match each other and differ from the stable tower hash.
   Stop at the read containing the final chunk's terminating escape sequence
   for that matching image. Record the timestamp before decoding/hashing. If
   these preconditions fail, this endpoint is not tested rather than replaced
   with “some output arrived.” This measures completion of a different image
   transmission; it does not isolate encoder CPU time or prove a cache miss.
3. **Committed synthetic observation → selected brief.** The API-guide brief
   remains selected. Commit an `agentMessage` with a unique literal token to
   the fixture's SQLite history and update its thread timestamp. Start after
   both commits finish; record commit duration separately. Stop when that exact
   token, the expected task title and `OBSERVED` are present in parsed native
   cells. An old message cannot satisfy the sample.

Every final mode requests 25 input samples and 25 source samples. The native
condition explicitly selects half-block fallback, which uses the compact
roster; graphical completion is **not applicable/not tested** there. The second
condition simulates a successful Kitty direct-image capability reply and
8×16-pixel cells at 120×36. The frame dimensions and hash calibration are in the
result. Reduced motion is enabled in both conditions to remove decorative
frame changes from graphical correlation.

Both final conditions use the same generated source path and stable task IDs,
with the same three-project/six-task fixture structure. The fixture is recreated
between conditions so the first mode's appended messages do not change the
second mode's initial workload. Timestamps are fresh, not frozen. This is not
the separate large-roster rendering benchmark.

## Timing uncertainty and sampling

The observer timestamps `os.read` receipts, then parses the received bytes.
It does not have terminal compositor timestamps or high-speed camera evidence.
Several outputs can share one read: native semantic feedback and image completion
can therefore have the same timestamp. That is coalesced receipt, not proof of
their relative order below that granularity. Earlier decoding/parsing, observer
scheduling, PTY buffering, application polling and concurrent machine activity
can all affect these values. No dedicated idle-host or CPU-isolation claim is
made. Human key travel, display scanout and authenticated provider response are
outside the measurement.

The final source series inserts deterministic pre-append delays sampled without
replacement from 0–1100 ms using seed 731; the same schedule is used in each
mode. This spreads arrivals over collection phases. It is still a small,
sequential local fixture sample, not a statistical estimate of production
freshness. Percentiles use the nearest-rank rule; raw samples and maxima remain
available.

The [earlier sequential pilot](pilot-sequential-results.json) is preserved.
It had no execution failures, but appended almost immediately after the previous
observation was displayed. That phase bias made source timings cluster near a
full collection interval. It was not silently discarded or reported as a
general source-freshness percentile. The final run corrects that method and also
reuses one source path across modes to preserve derived identity.

## Reproduce

Use the same Python environment as the [workflow harness](../../workflows/REPORT.md):

```sh
python3 docs/design-audit/iteration-7/performance/pty/probe.py --samples 25
```

The probe imports the existing working PTY/capability helper, makes its own
fixture under `docs/design-audit/tmp/iteration-7-workflows/latency-*`, and never
touches another audit's fixture or scripts. It writes a compact JSON result;
large ANSI streams and image buffers are not saved. All failures, including
partial sample series, are retained in the result. Nonzero exit denotes a
failed endpoint or incomplete flow, not a slow but valid sample.

No participant sessions were conducted as part of this measurement. The study's
physical-terminal and comprehension checks remain not tested.
