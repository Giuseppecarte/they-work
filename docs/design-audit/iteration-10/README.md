# Iteration 10: six S/M engineering items verified

**REL-02 has not started.** Six of eight engineering findings pass; native Windows x64/ARM64 and release rehearsal with actual native CI archives remain blocked. UX-01 and UX-02 remain open with zero participants.

See the [closure decision](CLOSURE.md), [machine-readable register](closure.json) and [critical review](CRITIQUE.md). They separate engineering results from publication, physical-terminal and human evidence.

Baseline: `4097e4f`. Integrated application candidate: `7cb26e4`, which fixes a reproduced Docker cache-build failure and adds a regression. A later CONTRIBUTING clarification documents Python 3.11+ for that helper. The [candidate manifest](validation/candidate.json) identifies source/binary hashes and reuse of unchanged Rust checks; the final [integration run](validation/integration-results.json) passes 466 ordinary tests plus 3 separate native process cases, formatting, strict Clippy and the release build.

Fresh evidence includes 21 release helper tests, 6 image PTYs, 9 disposable-registry scenarios, two clean reproduction checkouts, 12 documentation/control PTYs, the complete 1/8/32/128 correlation envelope, and 96 complete composition exports (8 visually reviewed). Counts overlap existing suites and do not add up to independent trials. Details and exact limits are in each lane report.

## Reproduction

`python3 docs/design-audit/iteration-10/verify_evidence.py` checks retained file/source hashes, actual integration counts and the open closure gate without running the application.

`python3 docs/design-audit/iteration-10/run_checks.py data --output target/audit/iteration-10/repeat-data` executes bounded data, retention and affected UI checks on the prepared native macOS toolchain.

`python3 docs/design-audit/iteration-10/run_checks.py integration --output target/audit/iteration-10/repeat-integration` runs formatting, the locked workspace suite, strict all-target Clippy and release build. Supervisor fixtures need local loopback access. The separate native process report must contain all 3 actual macOS cases.

The runner refuses to overwrite existing logs. Preserve prior attempts before rerunning. The first sandbox/socket failure and the first shared-load timing failure remain recorded separately. Bulk generated output lives under ignored `target/audit/iteration-10/`; previous tracked audit evidence is preserved.

## Remaining action

Run the native CI jobs and obtain all six actual distribution archive/checksum pairs for candidate `7cb26e4`. The remote audit branch is absent. Upload to `https://github.com/Giuseppecarte/they-work.git` remains pending explicit destination authorization after automatic approval review rejected the earlier push; no upload workaround was attempted. Matching owner-run artifacts are also acceptable.

Only after both open engineering rows pass can REL-02 begin. The provisional launcher preflight does not freeze a participant baseline or close either UX investigation.
