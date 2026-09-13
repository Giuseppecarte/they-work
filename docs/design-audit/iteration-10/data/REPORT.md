# DATA-01 and DATA-02 closure

Both findings pass their bounded engineering acceptance on the current Rust
sources. The 25 directed checks ran against `4097e4f`; the integrated candidate
`7cb26e4` changes only Docker cache preparation and its Python regression. The
[candidate manifest](../validation/candidate.json) proves the relevant source hashes are unchanged.

| Check | Executed result | What it establishes |
| --- | --- | --- |
| Lineage | 3 passed | Restart/overflow changes lineage without reusing exposed sequence authority. |
| Reconciliation | 9 passed | Duplicate/reordered observations, bounded missing actors, oversized events, exact known gaps, late history, and independent SQLite collector recovery. |
| Native fake-provider process | 2 passed | 300/3,000-event bursts produce exactly 44/2,744 unavailable events; current requests survive, provider calls are not repeated, and a stale saved tail cannot claim continuity. |
| Retention | 7 passed | Quiet-project delivery survival, 20/>512 projects, deterministic retirement, original project attribution, duplicate/enriched records, missing workers and bounded coverage metadata. |
| Coverage interaction | 3 passed | Six viewport sizes, deduplicated source counts, hidden-action safety and mouse Back/Esc equivalence with reading context preserved. |
| Short request panel | 1 passed | A current request remains reachable alongside a coverage warning. |

[data-results.json](data-results.json) records the exact commands, counts and raw
log hashes. Zero matches fail the runner. Counts overlap the complete workspace
run and must not be presented as independent trials.

The first process attempt could not bind a local socket inside the default
sandbox. Its failed log remains in [sandbox-attempt](sandbox-attempt/data-results.json).
The approved loopback-enabled rerun passed both process cases; this is an
environment distinction, not a repaired application defect.

The complete-screen matrix is recorded in [visual evidence](../visual/manifest.json):
96 route/bounds/effective-color checks pass; eight full replays have a separate
visual verdict. Historical source gaps, local record retirements and tool-start
correlation losses remain different units. None establishes a count of unique
missing deliveries, a complete transcript or permission to answer a request.

Native provider accounts, physical terminal transport and human comprehension
remain not tested. DATA-02 retains an in-memory shared window; it does not pin
reviewed/unread records forever or guarantee one record per project beyond the
512-record capacity.
