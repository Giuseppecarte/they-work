# Workflow discovery: reading, acting, returning

This is an **expert inspection**, not a novice usability result. Five short
executable sessions exercised three presentation conditions, a project-return
route and one synthetic managed request. **Zero participant sessions have been
conducted.** No production code, provider account or personal conversation store
was changed.

The tested binary is `target/native-macos/release/they-work`, SHA-256
`b2c529653f5b9d6a2ee93cd84cb87772ee8e1dee96b431ac9b645f3bfd33de84`.
The [machine-readable result](evidence/results-all.json), action-by-action
[return trace](evidence/reorientation.trace.json), [request trace](evidence/approval.trace.json)
and actual [fake-provider log](evidence/provider.jsonl) retain what happened.
These are complete bounded workflows, not a new all-screen gallery.

## 1. Reading, reviewing and approving are correctly separate — comprehension is untested

The executable shows a pending `test-only-command` returned by a local fake
provider. Pressing `r` in Attention adds `[seen]` while `APPROVAL NEEDED` remains.
The provider log has no response to `approve-1`. Opening Review request still
sends no response. Clicking Allow this request emits exactly one
`{"id":"approve-1","result":{"decision":"accept"}}`; only then does the
pending request clear. The stub never executes the displayed command.

Evidence: [before](evidence/approval-before-seen.png),
[marked seen](evidence/approval-after-seen.png),
[review without an answer](evidence/approval-review-no-answer.png),
[after explicit answer](evidence/approval-after-explicit-answer.png).

The last capture also exposes a language tension: its heading says the selected
request is no longer pending, while the operation receipt still says “Reply
written; awaiting provider resolution.” The fake-provider state already has no
pending request. This does not imply a second decision or lost response, but
the historical write receipt and current state can suggest different next
steps. Preserve this case in the comprehension test and clarify temporal copy
only after deciding which state the user needs to act on.

The text already explains that seen is a local reading marker. That is useful
counterevidence against immediately redesigning the controls. Unknown: whether
a new user notices this explanation, understands which source can accept an
answer, and predicts the recipient and effect before clicking. Test that
prediction; do not infer understanding from a visible button or correct code.

**Hypothesis to test:** reading vocabulary and action scope still require too
much interpretation. A worthwhile improvement would reduce wrong predictions,
not just add another label. Reject a new confirmation/panel if participants
already distinguish the three states unaided.

## 2. A visit baseline is not a reading baseline

The route was Beta → Alpha → inject a new synthetic Beta result → Beta → Alpha
→ Beta. No record-opening or Mark seen action was performed on the new result
during these returns. On the second return, This floor / Since your visit is
empty; This floor / Deliveries still contains the new result without `[seen]`.
The result was retained. Its arrival text can also be visible in the task
panel, so the walkthrough cannot establish that a human failed to read it.

Evidence: [first return without a record action](evidence/return-beta-no-record-action.png),
[Since your visit after the second return](evidence/beta-since-second-return.png),
[retained delivery](evidence/beta-deliveries-unseen.png).
The mechanism is in `Ui::draw`: entering a different office sets its visit
baseline and draws update `last_visits`; the notebook compares event times with
that baseline. Reading markers are separate maps.

The finder also reopens with an empty query after repeated task switching.
[Reopened finder](evidence/finder-reopened.png) and the input trace demonstrate
this behavior; `Finder::show` explicitly clears the query. Neither behavior is
automatically a defect: a fresh search may be intentional, and “visit” is an
accurate label. The unanswered product question is whether those choices match
how people resume work after interruptions.

**Hypothesis to test:** people need a persistent return context more often than
a fresh overview. Run the interruption task and diary before choosing among
unseen results, recent task history, retained search, or clearer existing scope.
Measure the missed next step and repeated navigation. Do not equate a view visit
with deliberate review, and do not implement all four candidates speculatively.

## 3. The same facts have different reading costs in the tower and compact roster

All three initial conditions reuse byte-identical databases **and the same
source path**. That matters because source path participates in stable identity;
copying a database to another home changes the derived worker identity. The
initial hashes and path are recorded for every condition. The six task IDs,
titles, projects, results and request/error observations remain the same.
Wall-clock age and animation phase are not frozen.

- [Graphics tower](evidence/compare-tower.png): three floors, stable characters,
  workplaces, project totals and attention symbols. Detailed task titles and
  state meanings need further inspection.
- [Compact roster](evidence/compare-compact.png): the same six tasks with titles,
  explicit Approval needed / Error reported / Ready labels and coverage text.
  It gives up the office scene and character recognition.
- [Reduced-motion tower](evidence/compare-reduced.png): the same art/identity
  condition with motion disabled. Twelve art samples over three seconds are
  stored as hashes; the final result records how many were distinct. This is a
  bounded runtime observation, not a full animation or performance test.

The three corresponding Attention captures expose the same approval and error:
[tower](evidence/attention-tower.png), [compact](evidence/attention-compact.png),
[reduced](evidence/attention-reduced.png). The initial fixture includes two
explicit final results and completed turns without such results, so “Ready” is
not interchangeable with “delivered.”

**Hypothesis to test:** the tower may be a stronger recognition/awareness surface
while the compact roster may be faster for triage. There is no measured human
winner. Counterbalance the presentations on identical facts; ask who needs a
decision, what is known and which project is visible before asking preference.
For art, measure recognition after switching and false inferences from motion.
In the final motion capture some characters are at the room edges while their
aliases remain under their empty workstations; the reduced-motion capture puts
those figures back at their labeled stations. This is a concrete attribution
hypothesis, not proof that a person loses track. Ask the participant to identify
the moving character before showing its task. More decorative detail is not a
substitute for either result.

## 4. Documentation teaches an older navigation model

See the [documentation audit](DOC_DRIFT.md). The executable route confirms that
`v` opens Advanced, `c`/`C` open Connections, and main-view Tab advances semantic
focus rather than cycling floors. `d` in an office opens Office Design. These
are discoverability costs that can be addressed without inventing a new feature.
The first-launch demo shortcut has a different context; it should not be called
a broken key based on an office-only test.

## Limits and next evidence

The reviewer already knows the application, wrote the fixtures and consulted
the code. Successful routes therefore overestimate what an unfamiliar person
can discover. This six-task/three-project sample does not establish behavior at
the user's real concurrency or with long-lived provider histories. Exact
counterexamples and scope are retained instead of averaging them into a pass
score. The previous 360 visual passes concern inspected pixels, not understanding
or daily adoption.

The [five-person study kit](../study/README.md) is ready for owner-led scheduling:
two Codex users, two Claude Code users and one who uses both; 45 minutes each;
counterbalanced task/condition order; blank scorecards and a three-workday diary.
Participant results, actual-terminal rendering, assistive-technology use and
authenticated provider operation remain **not tested**.

No new visual concept was produced. The current tower, compact view and motion
setting already provide a useful comparison; another mockup would add less
evidence than observing the return and approval tasks.

## Reproduce

From the repository root, use Python with Pillow plus the existing audit `pyte`
dependency at `docs/design-audit/tmp/python`:

```sh
python3 docs/design-audit/iteration-7/workflows/walkthrough.py --only all
```

The recorded run used
`/Users/gc/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3`.
The existing release binary is required; the harness does not compile or edit
the application. It creates an isolated HOME, config, SQLite sources, provider
stubs and temporary project directories under
`docs/design-audit/tmp/iteration-7-workflows`. The managed request scenario needs
the application's local control socket; the sandboxed attempt could not start
that host, so the completed run used permission for this bounded local fixture.
No real provider executable is on the fixture PATH.

ANSI files are actual executable output. PNGs reconstruct native cells with
Menlo/Pillow and decode the actual transmitted Kitty image at its cursor
position. Visible native glyphs are painted above the negative-z image; the
protocol does not export the compositor's entire blank-cell mask. They are
inspection replays, **not** physical terminal screenshots or an independent
validation of Kitty/iTerm/Sixel implementations. The simulated capability reply
is used only in the expert harness, never in the participant launcher.

All five final PTY sessions must report exit zero and restored terminal flags.
The fake-provider log and invocation log distinguish local actions from real
service traffic. The launcher doctor smoke check and scorecard-format check are
preparation checks; they do not increment the participant session count.
