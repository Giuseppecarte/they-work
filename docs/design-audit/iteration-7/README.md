# Iteration 7 — finding the next valuable improvements

The audit identifies three first changes: preserve task controls when operation
history fills, make known observation gaps explicit, and stop one busy project
from displacing another project's retained delivery evidence. The
[ranked roadmap](ROADMAP.md), [shared findings register](findings.json), and
[top-three implementation briefs](BRIEFS.md) explain the evidence and chosen
behavior.

**Status: discovery complete with documented limitations.** The real two-hour
headless run finished with 50 workers/20 projects throughout, zero polling errors,
and exit zero. Its memory growth exposed an additional unmatched-tool
correlation risk, now ranked seventh. Five-person validation is prepared but
**not tested**: zero participants were supplied or recruited in this environment.
No production feature, API, persistence schema, provider permission or artwork
was changed.

The production baseline is commit `748d371`, following iteration 6's UI/art
changes. Start with [iteration 6 validation](../iteration-6/VALIDATION.md) for
previously established compositor and PTY behavior. This iteration does not
reinterpret those results as proof of human understanding, authenticated
integration or physical-terminal performance.

## Investigation coverage

| Area | Evidence and experiment | Remaining boundary |
| --- | --- | --- |
| Daily workflows | [Executed routes](workflows/REPORT.md), request decisions, source controls, delivery review, project return, [documentation mismatches](workflows/DOC_DRIFT.md) | New-user comprehension and three-workday use |
| Design and comprehension | Same-source tower/compact/reduced-motion comparisons, complete request and return screens | Physical terminal rendering, attribution speed, preference and accessibility |
| Data fidelity and continuity | [Eight independent event-log cases](data/DATA.md), [three integrated source/snapshot cases](data/DUAL-FEED.md), rendered delivery loss | Current authenticated provider schemas and real failure frequency |
| Long-running behavior | [Large-history timing and real two-hour workload](performance/PERFORMANCE.md), separate bounded PTY response experiment | Physical paint latency, other operating systems and indefinite resource behavior |
| Controls, installation and releases | [Nine reliability scenarios / 29 checks](reliability/RELIABILITY.md), real local archive install/replacement, crash and idempotency probes | Windows runtime, real distribution network, actual disk exhaustion, authenticated accounts |
| Maintainability and contribution | [Clean-checkout `make check`, documentation and artifact audit](maintenance/MAINTENANCE.md) | A new contributor, empty-cache/network and non-macOS rehearsal |

The two reproduced control issues account for three failing checks in the
reliability suite; 26 other checks passed. They are audit findings, not failures
of the normal workspace verification, which passed 418 tests with three ignored
tests plus formatting and strict Clippy in a clean checkout.

## Read and reproduce

- [Evidence contract](EVIDENCE.md): classification, measurement boundaries and
  the shared record format.
- [Comparisons](COMPARISONS.md): current first-party Codex, Claude agent view and
  Agent Deck workflows; issue reports treated as signals rather than prevalence.
- [Study kit](study/README.md): five recruitment slots, 45-minute sessions,
  counterbalancing, blank scorecards, voluntary diary and owner-run capture.
- [Independent priority critique](reliability/PRIORITY-REVIEW.md): challenges to
  ranking, indefinite retention and overclaiming data loss.
- [Validation record](VALIDATION.md) and [critical review](CRITIQUE.md): what
  passed, what failed, what changed after challenge, and what remains untested.

Raw growing stores, local toolchains and build products stay under ignored
`docs/design-audit/tmp/`. The retained evidence includes generators, source and
binary hashes, compact logs, expected event facts and representative complete
screens. Screens reconstructed from PTY output are labeled as such; no physical
terminal screenshot or participant result has been invented.

The current user-approved discovery scope supersedes the older
`docs/design-audit/GOAL.md` instruction to implement production fixes and avoid
a roadmap. This iteration supplies a roadmap and future implementation briefs
while preserving the production baseline. It remains on
`audit/design-and-usability`, with no merge or push.
