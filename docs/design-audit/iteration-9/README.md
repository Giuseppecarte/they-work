# Iteration 9 — the four M improvements

The [implementation plan](PLAN.md) covers all four M findings from iteration 7.
The engineering changes are implemented on `audit/design-and-usability`.
**WIN-01 remains awaiting native Windows x64 and ARM64 execution.**

- [DATA-01](data-stream/DATA-01.md): known stream gaps, bounded actor reconciliation
  and historical replay that cannot replace current requests.
- [DATA-02](DATA-02.md): fair project retention within 512 records, immutable
  recorded project attribution, bounded truthful eviction metadata and coverage
  inspection.
- [WIN-01](windows/README.md): safe native replacement, checked recovery and
  explicit unavailable controls after a storage fault.
- [PERF-01](perf/PERF-01.md): bounded Claude correlation and truthful late results,
  with paired/unpaired resource evidence and the measured processing tradeoff.

[Validation](VALIDATION.md) separates executed checks from pending environments.
[Critique](CRITIQUE.md) records the defects found during review and the remaining
practical limits. These changes do not complete REL-02, authenticated-provider
trials, terminal certification or participant studies.

## Reproduce the bounded UI evidence

Use the pinned toolchain and prepared dependencies from `python3 scripts/audit.py
bootstrap`. Then, from the repository root:

```sh
env -u NO_COLOR TERM=xterm-256color COLORTERM=truecolor THEYWORK_ENCODING=quadrants \
  cargo run --locked -p theywork-render --example coverage_inventory
target/audit/venv/bin/python docs/design-audit/iteration-9/replay.py \
  inspector-now-image-80x24-8x16 inspector-now-image-32x14-8x16 \
  inspector-details-image-120x36-8x16 coverage-image-192x58-8x16 \
  deliveries-image-110x80-10x20 changes-image-240x70-10x20 \
  attention-light-80x24-8x16 coverage-mono-80x24-10x20
```

The exporter writes 96 complete Ui compositions, text and click geometry beneath
`target/audit/iteration-9/captures/`: six surfaces, six terminal sizes, both cell
geometries, and additional light/monochrome cases at 80×24. It rejects zero matching
cases and unexpected effective color modes. The optional first argument is a
case-name filter, not an output directory. Replay verifies the bundled font
checksums and uses those fonts only. No provider is invoked.

The native audit helper `sh docs/design-audit/native-cargo.sh` can substitute for
Cargo on the prepared local macOS audit checkout. Cache/output and full raw
captures remain ignored; selected full-screen replays and manifests are retained
with this iteration. Rasterizing Ui cells is compositor evidence, not a photograph
or recording of a physical terminal.
