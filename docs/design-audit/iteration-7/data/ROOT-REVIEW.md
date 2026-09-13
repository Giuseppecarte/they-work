# Independent review of the discovery synthesis

Reviewed the root findings register, roadmap, top-three briefs, maintenance and performance reports against the final data evidence. No experiments or production changes were made for this review.

## Two corrections before calling the briefs implementation-ready

1. **Name the counted stream unit precisely.** DATA-01's44/2,744 sequence gaps are correct, but the original 300/3,000-observation fixtures include a server approval request with an `id`, as well as notifications. The supervisor sequences both. Consequently `ProviderNotification` is narrower than the thing counted. Prefer `ProviderStreamEvent` or `LiveObservation`, defined to include notifications and incoming provider requests. Keep semantic result/message/database-row counts separate. This is a naming/accounting correction, not a change to measured ranges or severity. It affects `findings.json` DATA-01 wording and the referenced data D1 interface/copy.

2. **Complete D2's metadata retirement clock and tie-break.** Record-allocation acceptance is executable: B1/A511, the 25/26 distribution for 20 sufficiently populated projects, and the three 171-record tie fixture all follow the stated algorithm. However, `least-recently-observed` metadata retirement and `last eviction time` do not yet identify their clock or equal-time tie-break. Source freshness, a user's visit, native event time and local arrival order differ, especially for late records. Choose a pure-core rule, for example an ingestion ordinal updated on distinct collaboration insert/upsert, with OfficeId as the metadata-retirement tie-break. Define whether the reported timestamp is the evicted record's time or an explicitly supplied observation time; do not add an implicit wall-clock read to World. Add an exact 513-project-window retirement/return oracle proving the selected entry, the unknown-history flag and the unchanged 512 bound. This completes the proposed metadata policy; it does not require another discovery experiment.

## Claims that match the evidence

- The isolated paths lose 44 and 2,744 stream observations after cursor 1; current roster 2 and pending 1 remain. Existing partial-history warnings are explicitly acknowledged.
- The integrated tests use a cold collector and cursor 0. Native parent-edge metadata recovers in all 3 cases. The299/2,999-item stores do not recover old result/delegation items outside the per-thread 64 tail; the 5-item aggregated-delta store recovers both results and the delegation record. The synthesis does not claim universal family loss or failure of continuously running collectors.
- DATA-02 correctly distinguishes lost delivery classification from the surviving ordinary text beat. Proposed fair allocation remains future work, not an implemented fix or a permanent unread archive.
- The maintenance report scopes 418 passed tests/3 ignored to one native clean-checkout run with an existing dependency cache. Its storage percentage and local-clone wording do not imply public network performance.
- The performance table matches the raw startup/RSS/poll measurements and separates their boundaries. It does not reinterpret the 1,403.2ms large-history startup as failure of the 150ms interactive feedback target.
- The two-hour result still says `running`; README, findings status and PERFORMANCE do not claim completion or a final pass. That gate remains open.

No ranking change is required by this review. Keep the conditional integrated recovery language and apply the two precision corrections above before implementation handoff.

## Resolution

Both precision changes were incorporated into the root synthesis and D1/D2 briefs: `ProviderStreamEvent` includes incoming requests, and metadata retirement now uses explicit ingestion ordinals with a deterministic 513-project return fixture. No measured result or priority changed.
