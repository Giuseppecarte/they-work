# Independent priority review

**Keep REL-02 → D1 → D2 as the impact ranking, provisionally.** This is a
risk-based ordering of reproduced behavior, not measured frequency, novice
failure rates, or an instruction to implement the largest change first. REL-01
is a smaller, credible fallback if REL-02's storage contract is not ready.

| Rank | Evidence that justifies it | Limit the claim and implementation |
|---|---|---|
| 1. REL-02: history blocks current controls | Twice, the actual host at 10,000 receipts rejected both Stop and Decline while advertising both capabilities. A below-limit positive control executes each once. | This loses controls **through this office**, not every possible provider control. The boundary was seeded, its organic frequency is unknown, and no unauthorized approval or duplicate execution occurred. Reserve both bytes and records; do not claim that a reserve alone lasts forever or that controls work with unavailable storage. |
| 2. D1: stream continuity and bounded recovery | Actual supervisor bursts lose 44/2,744 observations on the isolated managed path. The current approval and roster survive; the UI reports generic partial coverage but not the known gap. | Lead with explicit continuity. Unknown-actor deferral is a separate adapter-ordering regression, not a demonstrated common provider behavior. A count of missing observations is not a count of missing results. Do not promise transcript recovery in the first change. |
| 3. D2: unrelated activity displaces a delivery | One unread B result disappears from Deliveries after 513 A messages; no review marker was written and B remains present. | Its raw text survives as an ordinary beat. Describe loss of result classification/discoverability, not deletion of all source text. No human missed a deadline in this audit. Fair retention is not permanent unread retention. |

## Dual-feed evidence changes the D1 scope

The final [three combined-feed cases](../data/evidence/dual-feed/results.json)
(metadata generated 2026-09-08 21:31:01 UTC; reopen cursor zero) use the
actual collector and bridge in the same World with a synthetic local store and a
modeled retained snapshot. In both buried-item cases, the persisted relationship
edge is recovered; the two results and delegation record outside the 64-item
per-thread tail are not recovered, including on the next warm poll. In the
3,000-delta case, aggregation leaves the important items recent: both results and
the delegation record recover without duplicated identities or deliveries.

Thus “a stream gap permanently loses the team and results even with local
sources” overstates the evidence. Cold start/reopen beyond the retained tail is
the demonstrated combined-feed limitation. Continuously running collection,
live-provider schemas, and recovery after additional source updates were not
tested by these cases. The two original process-gap cases remain useful for an
explicitly configured managed-only connection.

## Acceptance criteria after independent challenge

- **D1:** the revised brief now keeps the 300/3,000-event process fixtures and
  includes the combined-feed positive/negative controls above. Keep its explicit
  `ProviderStreamEvent` unit (renamed after the root review to include both notifications and incoming requests): assert exact stream ranges independently of
  recovered semantic facts: recovering two results cannot prove all 2,744
  observations were recovered. Legacy/new-stream coverage must be unknown, not
  “zero missing.” Keep source and stream boundaries on deferred observations.
- **D2:** the revised brief now defines largest-project eviction mechanically,
  with exact 1/511 and 25/26 allocations and deterministic tie-breaking. That
  closes the earlier ambiguous fairness criterion. Also exercise the new record
  being evicted immediately,
  equal timestamps, and more projects than slots. A fairness policy may evict an
  unread result from its own noisy project; do not advertise “unread protected.”
- **Both:** maintain current approval controls and independent source/local-loss
  warnings. Neither proposal should add instruction replay, read new source
  stores implicitly, or grow an unlimited transcript journal. The proposed
  bounded diagnostics are appropriate; expanding full-history recovery would
  need separate scope and resource evidence.
- **REL-02:** the revised [brief](IMPLEMENTATION-BRIEF.md) chooses durable
  generations: active duplicates return their receipt; retired IDs return a
  terminal Not resent result, never a fresh execution. That closes the impossible
  “bounded storage plus complete receipt lookup forever” requirement. The
  current 16 MiB limit covers the whole state, not only
  operations. Seeding a receipt-count limit alone does not validate byte
  admission, repeated approvals, ownership growth or a full archive. Tests must
  distinguish writable storage at an internal bound from actual disk failure.
  Protocol migration, uncertain-outcome retirement and atomic replacement make
  this materially larger than changing one guard. Prefer REL-01 as an initial
  small patch if that work does not fit, while keeping REL-02 first in impact.

## REL-01 and publication compared with retention

REL-01 has a clearer small repair than D1/D2: persistence fails before any
provider call, then the same live host falsely reports an outstanding
acknowledgement. Restart recovers the tested pre-rename permission failure.
It is below D2 in demonstrated product impact because the tested case affects one
failed attempt, returns an initial error and sends nothing; D2 silently removes
an already-observed delivery from the primary result view. Neither frequency is
known. **REL-01 may still be the first implementation by effort and certainty**;
the [fallback brief](REL-01-BRIEF.md) deliberately avoids a persistence redesign.

The [publication-order finding](evidence/static-boundaries.json) is a different
gate: workflow source advances version/`latest` container tags before its smoke
check, while native release publication waits. That order is certain; an actual
bad public image, affected download, or partial release was not observed. Keep
it below confirmed runtime defects in this discovery ranking, but close it
**before the next public container release**. Verify a candidate digest before
promoting user-facing tags and rehearse failed verification in a disposable
registry. This need not displace a product priority with a speculative incident.

Sources: [data findings](../data/DATA.md), [data briefs](../data/BRIEFS.md),
[primary reliability results](evidence/results.json), and
[independent reproduction](evidence/reproduction/results.json). This review
adds no experiment, production change, performance claim, or usability pass.
