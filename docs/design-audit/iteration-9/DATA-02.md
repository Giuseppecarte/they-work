# DATA-02: fair retention and truthful project windows

The observer still retains at most **512 collaboration records**. On overflow it
retires an oldest record from the largest project partition, then breaks ties
by oldest source timestamp, project ID, actor and record ID. It retains a
chronological public iterator for existing consumers.

The [core acceptance cases](../../../crates/theywork-core/tests/coverage_retention.rs)
establish the following deterministic outcomes:

| Input | Retained evidence |
| --- | --- |
| One quiet B delivery, then 513 A records | B retains one; A retains 511 and reports two local evictions. |
| 50 records for each of 20 projects | Twelve projects retain 26 records and eight retain 25, with deterministic ties. |
| 513 projects, then the first returns | Both records and project metadata remain bounded to 512; earlier local counts for the retired project are explicitly unknown. |
| Exact duplicate or older version | No additional retention action or observation ordinal. |
| Enriched existing result after its actor changes project | The first-recorded project remains attached to the result; its content can still update. |
| Missing or removed participants | The result remains in its recorded project's tray; missing participants acquire no invented identity or action. |

The strongest useful guarantee is conditional: a busy project cannot evict a
quiet project's only result while a larger partition exists. More than 512
projects cannot each have a record in a 512-record budget. Reading or marking
seen does not pin a record, and this remains an observation window rather than
a durable delivery archive.

## Coverage and controls

Each retained project window reports its count, oldest retained source timestamp,
local eviction count, last eviction observation ordinal and evicted record's
source timestamp. The ordinal advances with accepted collaboration updates; it
is separate from provider sequence numbers and wall-clock time. A replay of an
already evicted record can cause another retention action, so evictions are not
counts of unique missing results.

The work brief has a persistent compact warning with full source/project details.
All notebook channels expose **h coverage**, including empty trays. The report
deduplicates copied stream counters by source and lineage; one 300-event interval
copied to two workers is still 300 missing events. Stream counts are source-wide
and cannot identify which floor an absent event belonged to. Local record counts
are scoped to recorded projects. Tool-correlation categories remain separate.

The report preserves reading position, and its Back button and Esc return to the
same record. Enter and marking keys cannot act on hidden records while coverage
is open. Source unavailability or staleness remains ahead of detailed counts in
narrow footers. At emergency sizes, the work brief says history is limited and
does not advertise an unavailable Details tab. A current request retains its
own explicit Review action.

## Critical corrections during implementation

Review found that an enriched upsert could still move a delivery to its actor's
new floor. Preserving the first-recorded project across upserts corrected that
case. Review also found a counter mislabeled as “stream gaps,” a loss notice
that hid availability, a Back button that closed the whole notebook, and coverage
scrolling that reset the original record position. Directed tests cover these
corrections, with redraw after input and mouse activation through the real Ui.

The full coverage report deliberately favors inspectable facts over compactness.
It can be long with many projects, and it does not make earlier history durable.
This pass does not establish that people understand the distinctions without
instruction; the five-person study remains a publication requirement.
