# Top-three implementation briefs

These briefs describe future changes selected by the discovery audit. They are
ready for separate implementation reviews; iteration 7 itself changes no
production APIs, schemas or permissions.

## 1. Preserve current control when operation history fills

Use the chosen contract, interfaces, migration, failure behavior and boundary
fixtures in [the control brief](reliability/IMPLEMENTATION-BRIEF.md). The key
constraint is durable idempotency: an old operation ID must never dispatch again
merely because its full receipt aged out. Retirement must be explicit to both
client and UI, with no automatic conversion into a new instruction.

The smaller [pre-send persistence correction](reliability/REL-01-BRIEF.md) may
share the control implementation review, but it has a distinct oracle. A proven
never-sent failure is not an uncertain send. Preserve the existing successful
crash/concurrency/uncertainty cases as regression tests.

**Rollout:** add storage/protocol compatibility fixtures first; exercise
admission and recovery at count and byte boundaries with the fake provider;
then expose matching capabilities and receipt copy in the office. Keep legacy
state as recoverable input during migration. Enable the behavior in the local
native build only after stop/reply/restart tests pass. Authenticated sessions
and Windows replacement remain publication gates. Do not use an actual owner's
active task to test forced crashes or capacity exhaustion.

## 2. Report actual observation coverage and reconcile unresolved actors

Use **D1** in [the data briefs](data/BRIEFS.md), together with the
[dual-feed correction](data/DUAL-FEED.md). The first release reports known gaps
and retries still-available observations; it does not promise complete
backfill. A provider event is not necessarily a result, a message, or a durable
history row. Recovered relationships and results remain valid even when a
different feed has a gap.

**Rollout:** introduce additive lineage/coverage metadata and compatibility
defaults in control/core; migrate the host cursor to the reconciler; then
render that same coverage in WorkBrief, Deliveries and Changes. Run both the
eight-case independent event-log suite and the three combined-source cases.
Unknown legacy coverage must stay unknown. Keep the compatibility wrapper only
while callers migrate, with no separate UI interpretation of sequence math.
Release notes must describe the new warning accurately, without claiming a new
transcript archive. A mixed old/new state must remain observable and never
trigger a provider mutation.

## 3. Preserve fair local evidence windows across projects

Use **D2** in [the data briefs](data/BRIEFS.md). The allocation/tie-break policy
is explicit and remains within 512 records. The implementation protects a
quiet project's result from a different project's flood while disclosing
overflow within each project's own retained window. It does not make an
unbounded promise about unread history.

**Rollout:** implement core retention and local-loss metadata first, preserving
native-ID deduplication; then update tray/brief empty states from the shared
projection. Reuse the coverage presentation from item 2 while keeping
provider event gaps and local eviction as separate facts. Run the B1/A513
counterexample, deterministic 20-project allocation, metadata overflow, late
events, restart and duplicate cases. Recheck the existing 20-project/50-task
resource fixture after the change. Existing visit/review preference keys keep
their meaning; no retained text is silently written to disk.

## Shared definition of done

For each implementation, preserve the independent failing fixture, capture the
new expected result and any counterevidence, and run the appropriate existing
workspace tests, formatting and strict lint checks. Match every new visible
warning/action with its keyboard and mouse route. A local reading marker must
never answer a request. Record which real provider/platform checks remain
unperformed, and review only affected visual surfaces after the core behavior
is correct. An all-green compositor test does not close a control or usability
gate.
