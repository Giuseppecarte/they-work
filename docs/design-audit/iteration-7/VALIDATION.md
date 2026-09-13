# Validation record and explicit limits

Baseline: `748d371f9769e83798f29a52fffeb90410afc3bd`; host macOS 26.6.2 arm64;
Rust 1.90.0. Native executable SHA-256:
`b2c529653f5b9d6a2ee93cd84cb87772ee8e1dee96b431ac9b645f3bfd33de84`.
The previous [iteration 6 validation](../iteration-6/VALIDATION.md) remains the
baseline for its own compositor, layout, keyboard and PTY tests. Those checks
were not all repeated or upgraded to human/physical-terminal passes here.

## Executed checks

| Evidence | Result | Exact scope |
| --- | --- | --- |
| [Clean-checkout `make check`](maintenance/network-native-make-check.json) | **Pass:** 418 passed, zero failed, three ignored; formatting and strict Clippy passed | Independent local checkout/build target, native macOS, existing dependency cache plus retrieval; 128.56 seconds |
| [Reliability suite](reliability/evidence/results.json) | **26 pass / 3 defect checks** across nine scenarios | Actual release host/private IPC and fake provider; control count boundary, pre/post-send write faults, crash, concurrent IDs, uncertain acknowledgements, and local archive replacement |
| [Independent control reproduction](reliability/evidence/reproduction/results.json) | **Two defects recur** | EACCES leaves never-sent Sending; ledger cap rejects Stop and Decline |
| [Eight data cases](data/evidence/results.json) | **Four safeguards preserved / four loss scenarios** | Independent input log versus actual supervisor/bridge/World; two burst sizes, deduplication, reordering, identity, unresolved actor and global retention |
| [Three combined-feed cases](data/evidence/dual-feed/results.json) | **Recovered edge 3/3; recent results recover; buried results remain absent** | Actual Codex SQLite collector then bridge in host order; modeled retained snapshot; cold/reopen cursor; no authenticated provider |
| [Five workflow sessions](workflows/evidence/results-all.json) | **Pass for bounded routes and terminal restoration** | Actual executable in PTYs, synthetic history and one fake-provider request; local Seen/Review send no approval, explicit Allow sends once |
| [19 full captures](workflows/evidence/visual-inspection.json) | **Expert reviewed** | Reconstructed native cells and actual Kitty payloads; no novice timing or physical-terminal verdict |
| [PTY response series](performance/pty/results.json) | **25 input + 25 source samples per mode; no failed samples** | 120×36, 8×16 cells, compact and simulated Kitty, reduced motion, three projects/six tasks |
| [Large-history series](performance/large-history/results.json) | **Pass for counts/exits; timings measured** | 20 projects/50 active plus 1,000 older tasks; 10/100/1,000 MiB transcript cases; source probe and CLI have separate hashes/profiles |
| [Two-hour workload](performance/two-hour/result.json) | **Lifecycle pass:** 7,200.109 seconds, exit zero, zero poll errors, worker/project counts unchanged | 174,200 appended unmatched-tool observations; 50 workers/20 projects throughout; headless collection/World only, no graphical rendering or memory-plateau claim |
| [Late memory review](performance/MEMORY-REVIEW.md) | **Code-confirmed unbounded pending map; correlated RSS steps** | The workload supplies unmatched tool starts; no heap profile or paired-tool run. PERF-01 remains open even if lifecycle counts pass. |
| [Audit package checker](evidence/package-checks.json) | See recorded status | JSON/JSONL and Python syntax, local links, declared source/evidence/image hashes, blank participant outcomes, production scope; final mode also requires the completed two-hour result |

The reliability and data defect checks are deliberate independent counterexamples.
They are not normal workspace tests accidentally described as passing. A seeded
boundary and injected fault demonstrate behavior, not real user frequency.

## Performance boundaries

Input to native PTY feedback p95 was 8.83 ms in compact mode and 84.26 ms with
simulated Kitty; complete calibrated image receipt shared the same read as native
feedback. Source commit to selected brief p95 was 975.63/1,043.55 ms. Each endpoint
has 25 raw samples per mode, with method and uncertainty in the
[PTY report](performance/pty/README.md). These numbers do not measure the display
painting pixels. The **150 ms p95 physical input-to-visible target remains not
tested**.

The 1,000 MiB transcript's CLI `--once` took 1.403 seconds, with 109.0 MiB peak RSS;
quiet/append source polls stayed under 9 ms p95 in the three measured cases.
Startup, source ingestion, terminal feedback and graphical completion are
different boundaries. The large-history startup is not mislabeled an interactive
target failure. The [performance report](performance/PERFORMANCE.md) preserves
resource units, warm/uncontrolled OS cache and concurrent-work limitations.

The completed soak recorded 240 usable external `ps` samples: peak sampled RSS
31.14 MiB, first/final ten-minute medians 14.63/30.95 MiB, and mean cumulative
CPU utilization of 1.71% of one core. The pending-tool map has no declared bound
for the fixture's unmatched calls; PERF-01 remains a risk despite the lifecycle
pass. Monotonic duration and UTC wrapper timestamps have different measured
spans, preserved with their uncalibrated boundary in the performance report.

## Failures of setup or method, not application defects

- Default-path contributor run lacked native Rust or a usable Docker daemon;
  the script returned actionable guidance. The offline repeat lacked one cached
  dependency. The configured native run then passed.
- Sandbox-local TCP blocked initial fake-provider runs. The successful bounded
  repeats used permission for local fixture communication; no account access.
- The first soak fixture encoded project folders incorrectly. The next resource
  harness was interrupted by the sandbox's `ps` restriction. Corrected evidence
  is separate, with no pass assigned to the invalid or interrupted attempts.
- The first PTY source-timing pilot was phase-biased. It is preserved separately
  from the final matched schedule. No failed endpoint was omitted.
- The standalone performance probe needed Rust formatting after measurement.
  Its byte-exact original source is retained and hashed; the maintained source
  differs only in formatting. The measured executable and numeric evidence
  remain unchanged. This does not affect the clean production formatting pass.

## Not tested

| Requirement | Reason / next evidence |
| --- | --- |
| Five new developers and three-workday diaries | No participants were supplied or recruited. [Kit prepared](study/README.md), [status: zero sessions](study/status.json); owner schedules with consent. |
| Authenticated Codex and Claude workflows | No owner-provided disposable accounts/projects. Fake peers exercise local protocol logic, not provider compatibility. |
| Physical macOS, Linux, Windows and WSL terminal appearance/latency | No authorized physical-terminal recording was performed. [Owner-run recipe](study/CAPTURE.md) records terminal/version/geometry and timing uncertainty. |
| Actual Windows state replacement, DACLs, console and running installer | No Windows runtime. Source confirms the remove-before-rename ordering; its interruption consequence remains a risk. |
| Public image promotion failure and native release coordination | No registry publication. Source confirms tag advancement before smoke; rehearse a disposable candidate failure before publishing. |
| Real HTTP/TLS release retrieval, signing/OS prompts and cross-version migration | Local distribution archive fixture only, same-build replacement. |
| Actual ENOSPC, physical power loss and every sync failure boundary | Injected EACCES/EFBIG and process termination do not reproduce all storage failures. |
| Indefinite resource use, 50-task graphical soak and all provider history shapes | The real two-hour workload is headless and synthetic; the large-history and six-task PTY probes have separate bounds. |

## Package reproduction

```sh
python3 docs/design-audit/iteration-7/validate.py
sh docs/design-audit/native-cargo.sh fmt --manifest-path docs/design-audit/iteration-7/data/Cargo.toml --check
sh docs/design-audit/native-cargo.sh fmt --manifest-path docs/design-audit/iteration-7/performance/Cargo.toml --check
```

With the completed workload and finalized reports, run
`python3 docs/design-audit/iteration-7/validate.py --final --manifest` to check
completion and regenerate the evidence manifest. This is an audit consistency
check, not a replacement for the lane-specific executable recipes. The final
critique is in [CRITIQUE.md](CRITIQUE.md).
