# Decisions after evidence, not after preparation

This is the UX-01/UX-02 implementation gate from [the approved plan](../PLAN.md).
Both items are included in scope; UX-02 remains a separately evaluated hypothesis.
No participant has been enrolled, observed, contacted or represented by an expert
walkthrough. Both gates are **not met** and both outcomes are **not tested**.

## Fixed baseline

Use the same five participants throughout: two regular Codex users, two regular
Claude Code users and one who uses both, all unfamiliar with They Work. Each
baseline session is 45 minutes. Freeze one binary and source commit in
[baseline.json](baseline.json) before the first participant, after the integrated
candidate checks and an owner-operated physical-terminal dry run. Set `locked`
to true and record the actual SHA-256 and artifact path. The launcher rejects an
unlocked or mismatched binary unless explicitly invoked for engineering
`--preflight`. Do not change that baseline through all five sessions **and any
optional three-workday diaries**. Missing or declined diaries remain missing;
three diary entries by one person are still one participant.

The fixture, normal/reduced motion and compact presentation remain controlled
conditions, not UI revisions. Keep source directory and worker identities the
same across each participant's presentation comparison. Record first exposure
separately from learned performance; five slots cannot balance six orders fully.

## Qualification and tie breakers

A candidate qualifies when **two distinct participants** independently encounter
the same underlying failure, or when one severe failure has a demonstrated,
reproducible mechanism. Repeating the same participant's mistake, an expert
inspection alone, or a stated preference does not satisfy the two-person gate.
Retain contradictory observations. Severe means an unintended decision, an
incorrect recipient or a reproducible lost next step, not a cosmetic dislike.

| Item | Primary probe | Smallest qualifying prototype; choose at most one |
| --- | --- | --- |
| UX-01 | Task 3's repeated return to Beta; Task 5 and diaries provide supporting context | Clarify visit-versus-read copy if meaning is the failure. A `Not marked seen` filter only if people understand the distinction but cannot recover the item. Retain a query only for a demonstrated lost-search-context failure. Do not combine these changes. |
| UX-02 | Task 2's seen/review/decision prediction, then Task 4's moving-worker identification | Change decision vocabulary if consequence is misunderstood, or preserve visible worker attribution during movement if identity is lost. If both qualify, prioritize the consequential decision risk. |

The approved all-S scope authorizes **one qualifying S-sized prototype per item**;
there is no second permission gate. It does not authorize a feature when evidence
fails to qualify, a combined redesign, or another prototype after the first one.
If an issue exceeds S, record and rank it separately. “No change” needs the actual
observations and counterevidence; zero sessions is not a validated no-change result.

Use [decision-record.md](decision-record.md) to separate observation, mechanism,
proposal, falsifier and outcome. Preserve provider facts, recipient-bound drafts,
record IDs, approval scope and local reading markers. REL-01 concerns rejection
before sending; a later provider request awaiting resolution is a different state.

## Optional 20-minute follow-ups

After a qualifying prototype, invite the **same five** participants through the
owner's arrangements. No invitations are sent by this kit. Use equivalent fresh
fixtures A and B, after baseline sessions and diaries have finished. Choose old
then new for P01/P03/P05 and new then old for P02/P04; swap dataset assignment for
P02/P04 to avoid giving the new UI only one dataset. Use separate locked manifests for the recorded old and prototype binaries with
`launch_fixture.py --candidate-manifest <manifest>`; preserve baseline.json.
The order is deliberately 3:2; report that imbalance and learning effects, including memory of the baseline.

Allocate 2 minutes to context, 6 minutes to each version, 4 minutes to consequence
prediction and comparison, and 2 minutes to debrief. Use the task corresponding
to the qualified item, not the entire 45-minute battery. Keep success criteria
identical and record errors and help before preference. Use new titles/content
and reset reading/visit state before each condition. For variant B, read “billing/schema change/SDK/authentication” in place of
“checkout/migration/API/pagination”; the same tasks and consequences apply.
Match project count, task states, delivery count and request scope; record the fixture hashes. The
`prepare_session.py --variant A|B --reset` helper supplies two equivalent semantic
variants in the same isolated slot; never reset it during a baseline session.

Keep the prototype off by default until the qualifying breakdown **no longer
occurs for the affected participants** on the corresponding follow-up task, with
no new wrong-recipient or mistaken-decision errors and no new navigation failure.
Report numerator/denominator and contrary cases, not population claims. Missing
follow-ups or unavailable graphics remain **not tested**; do not substitute a
compositor replay or engineering check for those observations.
