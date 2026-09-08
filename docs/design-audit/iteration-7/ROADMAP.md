# Ranked improvement roadmap

Fix current-control capacity, explain observation gaps, and protect a quiet
project's retained delivery evidence first. These priorities come from executed
counterexamples. The later design work is conditional on new-user evidence;
the audit does not establish that another art pass would be more useful.

The authoritative [findings register](findings.json) includes reproduction,
expected/actual behavior, counterevidence, severity, frequency, confidence,
effort and acceptance criteria. Detailed implementation proposals for the top
three are linked from [BRIEFS.md](BRIEFS.md). No production change in this roadmap
has been implemented by iteration 7.

## Ten items, ordered by consequence and evidence

| Rank | Item | Evidence class | Smallest next delivery | Scope estimate |
| ---: | --- | --- | --- | --- |
| 1 | **REL-02: retain Stop and Reply when receipt history fills** | Reproduced twice | Bounded receipt lifecycle with explicit retirement, durable current control, and truthful capabilities | L |
| 2 | **DATA-01: disclose known stream gaps and defer unresolved actors** | Executed isolated and combined-source cases | Structured provider event coverage and bounded reconciliation, preserving facts recovered by the collector | M |
| 3 | **DATA-02: share retained evidence fairly across projects** | Executed independent event log and rendered delivery tray | Deterministic per-project retention plus truthful local-eviction notices, within the existing 512-record budget | M |
| 4 | **REL-01: repair the never-sent but Sending receipt** | Reproduced twice | Transactional pre-send admission and explicit recovery, preserving uncertain-send handling | S |
| 5 | **RELEASE-01: verify before promoting public image tags** | Source-confirmed ordering; consequence untested | Candidate digest verification and an isolated failed-release rehearsal | S |
| 6 | **WIN-01: preserve recoverable Windows state during replacement** | Source-confirmed gap; runtime untested | Windows replacement/recovery contract with injected interruption tests | M |
| 7 | **PERF-01: bound unmatched tool correlation** | Source-confirmed unbounded map; RSS correlation in an unpaired fixture | Entry/byte budgets and truthful late-result/coverage behavior for incomplete histories | M |
| 8 | **DOC-01: make current instructions match current behavior** | Actual PTY routes and source contracts | Correct modal shortcuts, source persistence, and polling documentation | S |
| 9 | **REPRO-01: make evidence reproducible from a clean checkout** | Checked dependency and storage boundaries | Portable audit bootstrap, dependency/font manifest, and bounded capture policy | S |
| 10 | **UX-01: validate return-context needs** | Product hypothesis | Test interruption/return tasks and diary; prototype only the demonstrated missing context | S prototype + scheduling |

S = roughly 1–3 engineering days, M = 4–8, L = 9–15. These are scope estimates
for an engineer familiar with the repository, not promised dates. They exclude
participant recruitment, authenticated-provider availability and platform
certification. The receipt contract makes item 1 larger than a limit increase;
raising the limit would merely postpone the same failure.

## Why this order

At 10,000 receipts, two advertised controls fail even though the provider still
owns a live task and request. That is a stronger present-tense failure than a
layout preference. DATA-01 and DATA-02 affect the evidence on which the office's
daily supervision depends, but their descriptions stay narrow: generic partial
history warnings already exist, the independent collector recovers recent
results and persisted parent edges, and the quiet delivery's plain text can
survive after its delivery classification is evicted.

REL-01 has a smaller fix and can be included alongside the first control change,
but its proven pre-send failure has an existing restart workaround. Its
priority does not justify changing post-send uncertainty into automatic retry.
Release and Windows risks require blocking validation before publication even
though their unexecuted consequences rank below reproduced failures here.

Documentation and reproducibility work are useful small parallel deliveries.
The successful 418-test clean-checkout run argues against a broad build-system
rewrite. Measured storage concentration argues for a policy for new evidence,
not an unapproved rewrite of Git history.

The late [memory review](performance/MEMORY-REVIEW.md) adds PERF-01 without changing
the top three: missing tool results leave an unbounded correlation map. The
synthetic workload contains no matching results, and RSS steps correlate with
that map's expected allocation thresholds. Normal paired traffic and heap-level
attribution remain untested. This finding warrants a bounded-state correction,
not a claim that ordinary completed conversations have the same growth curve.

UX-02, character attribution and decision vocabulary, remains an **unranked
hypothesis** in the shared register. Its probes stay in the already planned
five-person study; no extra feature implementation is committed beyond the ten
roadmap actions.

## Design experiments before implementation

Use the existing [study kit](study/README.md). It currently has **zero enrolled
participants and zero completed sessions**. The first-exposure results matter
more than preferences after learning the same facts in three modes.

- **Return context:** compare the current visit/delivery distinction with one
  lightweight alternative only after an observed breakdown. Measure original
  task recovery, missed next steps and repeated navigation. Do not combine
  search history, unread filters and a new dashboard without choosing a cause.
- **Moving-worker identity:** ask a person to identify the moving worker and its
  real task before showing the inspector. If attribution fails, test one
  persistent marker/tether treatment against the current scene.
- **Request vocabulary:** ask what Seen, Review, Allow and the resulting receipt
  will do, including recipient and effect. Keep the exact provider decision
  explicit. Any accidental approval is a critical failure, not an acceptable
  average.

Advance a design candidate on two independent similar breakdowns or one severe
mechanism-backed failure. Preserve counterexamples. Reject a new control if the
existing one is understood unaided. Five sessions provide qualitative direction;
they cannot establish population success rates or accessibility certification.

## Publication gates and unresolved questions

These are validation requirements, not extra roadmap features:

- Real authenticated Codex and Claude trials with owner-supplied disposable
  projects, including console return, uncertain sends and stale requests.
- Actual macOS, Linux, Windows and WSL terminal recordings, with versions and
  measured input-to-visible, encoded/painted-image completion and source delay
  reported separately. PTY receipt and compositor timing do not certify paint.
- Native distribution retrieval/update/recovery on supported platforms,
  including Windows running-executable/state replacement and release-promotion
  failure. The completed archive rehearsal used local fixture downloads.
- Five new-user sessions and optional diaries; do not claim usability outcomes
  until those people actually take part.

Further investigation must answer a question that could change a decision. The
remaining useful questions are: how often real task histories reach the
reproduced retention/admission conditions; whether another source is delayed by
a large initial history scan; and which recognition/return/action confusions
new users actually experience. The two-hour headless workload addresses
collector stability, not those missing human or display observations.
