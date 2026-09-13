# Critical review

The implementation improves what can be trusted: loss is disclosed in its own
units, quiet projects retain a fair share, late recovery cannot clear a current
request, and unsafe control storage stops execution. It does not restore history
that was never observed or turn the office into a transcript archive.

## Defects found and corrected before handoff

1. **Exposed sequences could outlive the saved tail.** Keeping the same lineage
   across a host restart would reuse sequence numbers after a stale disk recovery.
   Restart now rotates the durably initialized lineage and discloses prior
   uncertainty; reconnecting to the same running host retains it.
2. **Enriched results could move floors.** Retaining the event's project only on
   initial insert was insufficient: a later upsert used the actor's new envelope.
   Existing record identity now keeps its first-recorded project during enrichment.
3. **Coverage could mislead.** A 300-event missing interval was labeled as 300
   gaps, a limit notice replaced source health, and floor scope could imply that
   source-wide losses all belonged to that floor. Units, health priority and
   source-wide scope are now explicit.
4. **The coverage reader could lose context.** Mouse Back closed the notebook
   while Esc only closed the report, and report scrolling reused the record's
   offset. Both now preserve the selected record and its reading position; hidden
   mark/request actions stay inactive.
5. **Emergency wording advertised an absent tab.** The 32×14 brief now says
   history is limited and asks for enlargement without referring to an unavailable
   Details tab. The Review button remains a separate action.
6. **Recovery could hide its original failure.** Live fault projection now
   preserves the intent-persistence error alongside its recovery explanation.
   A type/structure-invalid canonical snapshot cannot silently reset receipts.
7. **An old test represented a stronger failure than its name implied.** The S
   fixture moved canonical state aside and placed a directory at its name. This
   now explicitly tests checked restart after canonical disruption. Valid-file
   pre-write failures still support a new explicit submission in the running host.
   A rejection that never reached disk is not invented on restart.
8. **The capture environment did not match its labels.** Inherited `NO_COLOR`
   overrode requested palettes. The exporter now rejects incorrect effective
   modes and zero matching cases. The final 96-case matrix was regenerated with
   explicit environment settings; early monochrome exports are not color evidence.

## Utility and visual review

Eight complete replays were inspected across all six requested sizes, both cell
geometries, and normal/light/monochrome modes. At 80×24, the brief exposes the task,
request, Editing observation and latest result. At 120×36, the office remains
beside the Details panel with readable figures and a separate request action.
At 32×14, the emergency view prioritizes the request and warns about history;
the task title can be clipped and the full tabbed brief requires enlargement.
The coverage reader uses the available area for scrollable source/project facts.
The retained [visual evidence](visual/manifest.json) states which frames were
reviewed; the other exported compositions have automatic route/bounds checks only.

Tall and very wide notebooks still leave spare space, and the detailed coverage
report is deliberately technical. These are existing layout characteristics or
tradeoffs of an explicit diagnostic report, not evidence that another redesign
is required. Actual first-user understanding has not been tested in this batch.

## Resource tradeoff

In the synthetic unpaired replay, pending correlations fell from an unbounded
128,000 entries to the chosen 8,192-entry cap. Sampled peak RSS decreased from
32,960 to 16,656 KiB. In the paired replay all 128,000 outcomes remained correctly
attributed; after completions, the new tables and order index returned to zero
capacity. Paired peak RSS increased from 10,944 to 12,400 KiB.

There is a processing cost. Debug-build polling p95 rose from 73.8 to 92.2 ms
for paired traffic and from 57.1 to 175.3 ms for the overload case. These are
synthetic poll durations, not input-to-visible latency. They justify the memory
bound but do not prove the 150 ms terminal-response target. The exact workload,
sampling uncertainty and binaries are recorded in the [PERF report](perf/PERF-01.md).

## What remains open

Native Windows x64 and ARM64 execution is a selected completion gate for WIN-01.
Wiring the jobs and cross-compiling their source does not satisfy it. The push to
the configured GitHub destination was rejected by automatic approval review and
explicit destination approval was requested. No alternative upload path was used.

Physical power loss, non-NTFS filesystems, authenticated Codex/Claude trials,
physical terminal paint latency, the five-person study and REL-02's larger
operation-ledger reliability work remain separate requirements. No result here
closes those gates.
