# Task cards and moderator ground truth

Only read the quoted task text to participants. The expected outcomes are for
the moderator. Use `prepare_session.py` to make an isolated six-conversation,
three-project fixture. Do not open a personal provider store to fill missing
facts.

## 1. Orientation — ten seconds, then continue

“You have come back from a short break. What needs your attention now, and which
project would you enter first? Explain what on screen supports your choice.”

Expected: Alpha has a human approval observation; Gamma has an error, which is
not an approval; Beta has ready tasks. There are three projects and six tasks.
A correct answer may prioritize the error if the participant explains the
tradeoff; confusing it with a required approval is incorrect. Record whether
the participant uses the character, text, count, room or a global tray.

Follow-up after the unaided answer: “Does Ready mean a result was delivered?”
Expected: no. Beta has one explicit final-result record and another completed
turn without a result event. This is a vocabulary probe, not a rule to teach in
advance.

## 2. Triage and action consequence

“Find the checkout release request. You want to remember that you have looked
at it, but you are not authorizing the migration yet. Do that, then explain
whether the request is still waiting and what would send a decision.”

Expected: Mark seen adds a local marker and leaves the request pending. Opening
Review request alone sends no answer. Observed local history may not offer a
live decision; returning to the original conversation can be the valid next
step. Asking for the source or capability is a good response, not a failure.

Optional live-action continuation uses only the local fake-provider scenario
already exercised by [walkthrough.py](../workflows/walkthrough.py): create a
synthetic task with instruction `approval`, review `test-only-command`, predict
the recipient and scope, then allow only that request. Expected JSON reply:
`id=approve-1`, `result.decision=accept`, exactly once. No command actually runs.
Keep this separate from the observed-history request. Log this optional part as
not tested if the moderator has not prepared the fake-provider session.

## 3. Return to an interrupted project

“Inspect the API guide in Beta. Switch to the checkout issue in Alpha. While
you are there, a new API result arrives. Return to Beta and work out what
changed and what you should do next.”

The moderator uses `prepare_session.py --add-result` while the participant is
away. This writes only the fixture's synthetic SQLite record. Expected result:
`NEW WHILE AWAY: API examples now include pagination.` The result should be
available in Deliveries and in the available record history.

After returning, before marking anything seen: “Go back to Alpha, then return
again to Beta. Where would you look for an item you have not deliberately
marked as read?”

Expected behavior from expert inspection: Since your visit can now be empty
because entering Beta advanced the visit baseline; Deliveries still contains
the result without a local seen marker. Do not label the participant wrong for
expecting an unread queue. Record that expectation as evidence for or against
the proposed return-context improvement. A view visit cannot prove actual
reading.

## 4. Presentation and identity comparison

In each assigned condition ask: “Locate the checkout request again, identify
the other task in that project, and tell me which information is missing before
you can act.” Use identical fixtures, source path, profile map and task IDs.

Then ask: “Without relying on the last screen, find the API guide you inspected
earlier.” Record use of the character, alias, title, project, search, or memory
of a position. Ask which cues helped only after the attempt. Do not assume the
character is helpful just because the participant likes it.

For normal/reduced movement: “What do the characters' actions tell you about
what the underlying task is doing?” Correct interpretation distinguishes
recorded status from decorative movement. An asserted real-world inference
from a walk or gag is a comprehension issue even if the animation is attractive.

## 5. Keyboard and recurrence

“Using only the keyboard, open the task you just identified, find its latest
record, then return to your previous project.” Let the participant use Help.
Record whether Tab changes focus or is mistaken for changing floors, whether
focus is visible without color, and how they recover from a nested panel.

“Think of the last time you had three coding tasks in flight. Walk me through
the actual steps you took to find a blocker and resume work. Which one, if any,
would this application replace?” Ask frequency, current workaround and cost in
their words. Do not convert a hypothetical wish into an observed requirement.
