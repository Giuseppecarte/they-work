# Five-person discovery study

**Status: prepared; 0 of 5 sessions conducted. All participant outcomes are
not tested.** The adjacent workflow evidence is an expert inspection of a native
executable using synthetic records. It contains no participant observations.

Recruit five people who have not used this application: two who regularly use
Codex, two who regularly use Claude Code, and one who uses both. These are
recruitment slots, not enrolled or invented people. Aim for their normal mix of
terminals, keyboard habits and visual needs; do not promise accessibility
coverage from five people. The fixture uses Codex-shaped synthetic history for
all slots. Provider familiarity is a recruiting characteristic, not a claim
that both collectors are tested by these tasks.

One moderator runs a 45-minute session with each participant. Use the
[protocol](PROTOCOL.md), [task cards](TASKS.md), [blank scorecard](scorecard.csv),
[three-workday diary](DIARY.md), and [owner capture instructions](CAPTURE.md).
Consent to notes and any recording is obtained separately; screen recording is
optional. Do not contact, schedule or record anyone as part of preparing this kit.

## What this study can decide

1. Whether people distinguish local reading markers, reviewing a request, and
   sending a provider decision, including who receives that decision.
2. Whether returning to a project restores enough context to choose a next step,
   and what people expect “Since your visit” to include.
3. Whether the tower, compact roster, characters and movement help a specific
   task: locating a request, recognizing a coworker, and explaining current work.
4. Whether keyboard focus, labels and scope controls are discoverable on the
   participant's actual terminal, at a size they can read comfortably.

No statistical claim about a population should come from five sessions. Record
counts as “3 of 5 participants” and keep severity, conditions and contrary
examples. Do not turn subjective preference into proof of task performance.

## Counterbalancing

Assign slots after recruitment; do not assign order based on perceived skill.
T = graphics tower with motion enabled, C = native compact roster, R = graphics
tower with reduced motion. Actual capability is checked before the session. If
graphics are unavailable, record that condition as unavailable; do not pretend
native output is the graphics condition.

| Slot | Recruiting experience | Condition order | Triage-first or return-first |
| --- | --- | --- | --- |
| P01 | Codex | T → C → R | Triage |
| P02 | Claude Code | C → R → T | Return |
| P03 | Both | R → T → C | Triage |
| P04 | Claude Code | T → R → C | Return |
| P05 | Codex | R → C → T | Triage |

This uses five of six possible orders; C → T → R is absent and initial exposure
is not perfectly balanced. The same facts and identities appear in each
condition, intentionally. Learning/carryover is therefore expected. Report
first-exposure performance separately and use the later comparisons as
directional evidence and preference probes, not a clean causal ranking.

The two workflow blocks are also counterbalanced 3:2. A participant who has
already learned the request vocabulary will not supply another independent
first-impression response in the second block.

## Prepared versus outstanding

- Prepared: task scripts, expected fixture facts, ordering, blank scorecard,
  owner-controlled capture recipe, diary prompts and decision rules.
- Verified by expert PTY walkthrough: state/reading/action separation, retained
  result versus visit baseline, current shortcut destinations and three
  presentation conditions. See [workflow report](../workflows/REPORT.md).
- Not tested: participant comprehension, time-to-task baselines, preference,
  real-terminal rendering/input, assistive technology, daily adoption, and
  authenticated provider workflows.
- Outstanding before recruitment: moderator dry run on the intended physical
  terminal; confirm graphics capability; choose note/recording arrangements;
  prepare the local synthetic approval scenario if that optional live-action
  task is used. The kit never requires personal conversation data.

## Decision rules

Log each issue as an observed breakdown, an interpretation, and a candidate
change. A wrong-recipient instruction or a mistaken approval is high severity
even once: reproduce it before proposing a fix. A recurring comprehension or
navigation issue is a roadmap candidate when at least two participants encounter
the same underlying problem independently, or one severe failure has a clear
mechanism. An isolated preference goes into the evidence backlog.

For each candidate, write the smallest implementation brief: affected workflow,
evidence links and counterevidence, proposed behavior, explicit non-goals,
acceptance task, and what would falsify the hypothesis. The study should yield a
ranked, testable roadmap, not a count of decorative changes.
