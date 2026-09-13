# Critical review

## What the implementation review changed

The initial REL-01 engineering fix was correct at the provider boundary but
incomplete on screen: the composer dropped “submit again.” A full-frame review
caught it after the fake-provider state assertions passed. Receipt rows now
wrap, and acceptance asserts the full sentence in rendered cells. This is why
the before/after frame is retained alongside the provider-call evidence.

The first release-image verification used an obsolete fallback expectation.
It waited for quadrants while the current no-image default is a native roster,
so it never sent quit. The fix changes the oracle to require meaningful roster
content and still requires no images, explicit quit and normal exit. The same
immutable candidate then passed; the failure was not hidden by rebuilding.

Reproduction initially had two harness assumptions: an image diagnostic name
was treated as proof of mode, and a shared Git clone depended on an inaccessible
external object store in Docker. The final test uses actual RGBA/native geometry
and independent object stores. A final cross-review also required consistent
runner-test counts and a before/after tracked-diff comparison; both are retained.

## Remaining risks and useful limits

“Not sent” is supported only before dispatch. If the durable state retained
Sending and the correction could not be saved, restart must still show Uncertain.
This can be frustrating but is safer than claiming the failed write established
that automatic replay is harmless. Ledger-capacity and Windows replacement
problems remain separate work.

Release tags and native assets cannot be published atomically. Retained intent,
exact digest checks and explicit partial-state recovery reduce ambiguity; they
do not provide registry compare-and-swap against unrelated publishers or prove
remote GitHub permissions. Public operation still needs its own rehearsal.

The full-frame smoke is deliberately small and reproducible. It proves one
bounded route and one meaningful collector case, not everyday usefulness or
terminal response time. The study kit still has declared Unix/macOS capture
dependencies and requires a moderator dry run in the participant's actual
terminal. It should not be mistaken for a universally portable study runner.

The technical receipt ID still consumes visible space where it fits; the outcome
gets priority when it does not. Whether simpler vocabulary or moving-worker
markers help people must be decided by UX-01/UX-02's participant evidence. No
preference-based redesign was smuggled into those conditional items.

## Completion judgment

The four definite small changes meet their locally executable acceptance.
The human study and the remote/physical publication gates do not. The batch is
ready for code review and owner-arranged validation, not a claim that the entire
project is finished or ready to publish. A next investigation should target a
named unresolved question from the retained findings, not repeat passed checks
without a new cause.
