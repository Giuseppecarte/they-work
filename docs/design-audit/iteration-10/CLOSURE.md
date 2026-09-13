# S/M closure decision

**Six of eight engineering items are verified. The gate to REL-02 remains closed.**
WIN-01 needs native Windows x64 and ARM64 execution. RELEASE-01 needs its final
rehearsal with actual native CI archives. Neither requirement is substituted by
cross-compilation, checksum fixtures or historical results.

The original clean baseline is `4097e4f`. A clean Docker build reproduced an
omitted explicit Cargo target; commit `7cb26e4` fixes its dependency-cache layer
and adds a regression which fails on the old layer. That commit is the integrated
application candidate. A later contributor-documentation clarification states
the Python 3.11 minimum for the new TOML-reading helper test; it does not change
the tested application or image. Audit-only files record this verification.

| Finding | Size | Decision | Evidence and remaining limit |
| --- | --- | --- | --- |
| REL-01 | S | Verified | [Recovery report](control/REL-01.md): actual write failures, stable receipt identity, visible ordinary-failure draft, and checked canonical recovery with no resend. Native Windows remains WIN-01's gate. |
| RELEASE-01 | S | Blocked / not tested in full | [Release report](release/RELEASE.md): regression fixed; 21 helper tests, six architecture/mode PTYs and nine registry scenarios pass. Actual native CI archives remain unavailable. |
| REPRO-01 | S | Verified | [Reproduction](repro/REPRO.md): clean macOS and Linux ARM64 container checkouts each pass 13 runner tests, exactly one collector test and one complete frame; no tracked/store changes. Decoded images and geometry match. |
| DOC-01 | S | Verified | [Routes](docs/DOC-01.md): five existing groups plus coverage/recovery; temporary mode, reopening and polling contracts agree with implementation. English UI copy is retained. |
| DATA-01 | M | Verified | [Data report](data/REPORT.md): three lineage, nine reconciliation and two native fake-provider cases pass; precise known gaps and historical/current authority remain separate. |
| DATA-02 | M | Verified | [Retention report](data/REPORT.md): seven core retention cases and affected UI checks pass; quiet-project records, original project attribution and bounded coverage are preserved. |
| WIN-01 | M | Blocked / not tested | [Native gate](windows/README.md): both Windows architectures must execute and retain the specified replacement/recovery/process evidence. No native Windows job has run. |
| PERF-01 | M | Verified | [Resource report](perf/PERF-01.md): eight 1/8/32/128-overlap replays, 1,024,000 starts, all 512,000 paired outcomes and bounded unmatched state. Large-batch poll cost is disclosed. |

The final native macOS workspace run passes **466 ordinary tests**, zero failures
and four ignored tests, with **three additional harness-free process cases**.
Formatting, strict all-target Clippy, optimized build and normal demo exit pass.
The ignored resource replay is executed separately for PERF-01. Counts from
directed checks overlap the workspace run; they are not independent trials.
See [integration results](validation/integration-results.json) and the separate
[native process report](validation/native-process.json).

## Fresh, reused and failed evidence

Fresh checks identify their actual source commit. Checks which started before
the Docker-only fix are explicitly reused on `7cb26e4` through unchanged relevant
Rust and binary hashes. Clean image and reproduction runs use the new commit.
The [candidate manifest](validation/candidate.json) and lane manifests record
these distinctions rather than relabelling old executions.

The first workspace attempt, under concurrent build/replay load, failed the
existing 100-frame timing assertion at 5,002 ms against its 5,000 ms limit.
That log is retained in `validation/shared-load-attempt/`. After compiler-heavy
jobs ended, the full unchanged suite passed. No timing threshold was weakened;
neither run proves physical-terminal response latency. Sandbox/socket and audit
dependency/probe failures are also retained with their causes and corrected
execution procedures.

## Gate to the next item

1. Obtain exact-candidate CI on Windows x64 and ARM64, including raw logs,
   architecture/filesystem metadata and the required nonzero test results.
2. Retrieve all six actual native archive/checksum pairs from the same candidate's
   native jobs. Rehearse checksum and promotion recovery in a disposable registry
   with those files; preserve exact verified index identity and attestations.
3. Inspect the new evidence, resolve any failure, then mark both rows verified.
   Only then begin REL-02 using the existing iteration-7 implementation brief.

The remote audit branch was absent in a read-only query. Upload to
`https://github.com/Giuseppecarte/they-work.git` remains pending explicit destination
authorization after the earlier automatic approval rejection. No push, public
tag change or GitHub Release operation was performed in this batch. Owner-run
CI with matching candidate evidence can also satisfy these requirements.

## Work intentionally still open

UX-01 and UX-02 have zero participants, sessions, diaries and follow-ups.
[Provisional study preflight](study/README.md) passes, but the participant baseline
is unfrozen until the engineering gate passes. No speculative prototype or
evidence-free no-change conclusion was introduced.

REL-02 remains unimplemented: no ledger-generation, migration, receipt-status or
provider-control change is included here. Authenticated providers, public
publication, physical terminal recordings/latency, power loss and broader
filesystem coverage remain separate publication requirements.
