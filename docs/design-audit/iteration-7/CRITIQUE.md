# Critical review of the discovery work

The audit is useful because its first priorities are reproducible and bounded.
It is not evidence that the product is finished. The user-facing value of the
office metaphor, the quality of real terminal presentation and authenticated
provider control still require evidence from the people and environments that
were unavailable here.

## What changed after challenge

1. **The initial data result overstated the whole-app consequence.** An isolated
   supervisor stream can lose the relationship and result events, but the
   collector may recover equivalent facts. Three additional combined-source
   cases established recovered parent metadata in all cases and recovered recent
   results in one. The roadmap now targets truthful stream coverage and bounded
   reconciliation, with older-result loss qualified by cold/reopen tail limits.
   See [the counterevidence](data/DUAL-FEED.md).
2. **A missing-event count is not a missing-result count.** The stream includes
   notifications and incoming requests, and many events can describe one durable
   item. The proposed `ProviderStreamEvent` unit makes that distinction explicit.
   The [root review](data/ROOT-REVIEW.md) also required deterministic metadata
   retirement and a 513-project return oracle. Both corrections are incorporated
   into the briefs.
3. **A bigger operation limit is not a complete control fix.** Count and byte
   caps interact, receipts must prevent replay after restart, and finite storage
   cannot return every old payload forever. The selected control brief defines
   generations and terminal Retired responses, preserves exact active request
   identity during live rotation, and separates that from upgrade/reconnect.
   It does not promise successful control when physical storage is unavailable.
4. **The first source-latency series had phase bias.** Appending immediately
   after the previous observation appeared favored a full polling interval.
   The final bounded series uses a recorded schedule across phases, with the
   same source path/identities between modes. The pilot is retained with its
   limitation. Neither series becomes a physical display measurement.
5. **Visiting a floor is not evidence of a lost delivery.** The Since your visit
   baseline advances on a return, but the unread result remains in Deliveries.
   This is a comprehension question until a participant demonstrates a missed
   next step. It stays outside the reproduced data-loss findings.
6. **Early memory stability did not describe the full run.** A late RSS increase
   prompted a bounded source review. The fixture supplies tool starts without
   results, and the collector retains unmatched tool IDs in an unbounded map.
   Its predicted capacity steps correlate with the measured RSS changes. The
   new PERF-01 risk enters the roadmap at rank 7; the top three stay unchanged.
   The report does not attribute every resident page to that map or generalize
   this unpaired fixture to normal completed tool traffic.

## Utility and visual fidelity

The [complete tower capture](workflows/evidence/compare-tower.png) supports
project and character recognition, but task titles and full state meanings need
inspection. The [compact capture](workflows/evidence/compare-compact.png)
displays those facts directly. The [reduced-motion capture](workflows/evidence/compare-reduced.png)
keeps figures at their labeled workstations, while moving figures can leave
their stationary aliases. These are observed visual differences; this review
cannot rank human task speed or aesthetic preference from images alone.

The graphics are actual transmitted artwork, reconstructed together with native
cells. The replay uses Menlo/Pillow and approximates the blank-cell mask above
the background image. Therefore it is appropriate evidence for the captured
composition and route, but cannot establish how Kitty, iTerm2, Sixel or a
different font paints it in a real window. The primary review independently
inspected the tower, compact roster and resolved-request screen in addition to
the lane review's 19 recorded images. No new artwork was generated for this
discovery phase.

Seen and Review correctly send no provider answer in the synthetic request
route. After the explicit answer, the screen combines a no-longer-pending
heading with a receipt saying the reply was written and awaits provider
resolution. Those statements may describe different moments, but the wording
could still confuse a new reader. Test the prediction of recipient/effect and
post-action state before adding another confirmation or status label.

## Confidence and maintenance limits

- Synthetic fixtures establish mechanisms and counterexamples, not their
  prevalence. The 10,000-receipt boundary was seeded, not reached through a
  participant's normal usage. The measured two-hour collector workload does
  not grow that ledger.
- Five expert PTY workflows and 19 reviewed captures are not five user sessions.
  All recruitment slots and scorecards remain empty. No user success rate,
  ten-second triage result or preference winner is claimed.
- The normal clean-checkout checks passed, but setup used an existing dependency
  cache and local Rust installation. The archive exercise used an actual
  production binary in the release format with local download substitution.
  Neither proves a fresh public download works on every platform.
- New audit artifacts are checked for links, hashes, Python syntax and findings
  structure. That helps another reviewer inspect the evidence; it does not
  replace the runtime experiments. Full raw generated stores and build products
  stay ignored. A byte-exact source snapshot preserves the timing probe as
  measured; its maintained Rust source was subsequently formatted without
  changing behavior or recorded numbers.
- Long files alone did not produce a refactor recommendation. The suggested
  responsibility changes are tied to demonstrated control/coverage boundaries.
  No production API or persistence migration was slipped into this audit.

## Stop rule

The real two-hour process has exited and its counters/resources have been
reviewed. Every investigation lane has evidence or a concrete documented
limitation. The register has ten ranked actions and one unranked hypothesis; the top three have
chosen behavior, failure handling and executable fixture oracles. That is the
discovery completion condition; it is not the application's publication gate.

The unresolved work that could change priorities is specific: participant
misunderstandings, authenticated provider persistence/control behavior, actual
terminal paint latency, and Windows/release failure rehearsals. Repeating
screen matrices or adding speculative features would not answer those questions.
