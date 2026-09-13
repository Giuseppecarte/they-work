# Candidate: retire operation history without replaying old actions

## Problem and chosen first release

REL-02 is confirmed twice on release `b2c529…`: at 10,000 receipts, an owned
active task cannot be stopped or its supported current request declined through
the office, although both actions remain advertised. The below-limit control
executes both. Frequency in ordinary use is unknown; the boundary was seeded.
See [results](evidence/results.json) and
[independent reproduction](evidence/reproduction/results.json), case `ledger`.

**Choose bounded receipt generations, not an unlimited archive.** Duplicates in
the active generation return their original receipt; any retired-generation ID
returns terminal `Retired` and never executes. This deliberately changes the
old-result lookup contract: bounded storage cannot return every historical
receipt forever. `Retired` means **this retry was not sent**, not that the original
operation never ran. No new ID or retry is created automatically.

Production is unchanged. The limits below are proposed acceptance constants,
not measurements of a finished implementation.

## Identity, storage and rotation

1. Add a versioned ledger header with a random, durable `ledger_namespace` and
   monotonically increasing `ledger_epoch`. Both are separate from provider
   source identity and host/connection generation. Restart preserves them. New
   IDs are `v2:<namespace>:<epoch>:<nonce>` and stay within the existing 128-byte
   ID limit. The current header comes from an authenticated host snapshot. An
   unknown namespace, future epoch or malformed ID is rejected, never accepted
   as a fresh operation. Epoch exhaustion fails closed; it does not wrap.
2. An active-generation duplicate checks its request fingerprint before any
   capacity check and returns the stored receipt. A changed payload is rejected.
   Store a versioned SHA-256 fingerprint of the canonical request instead of
   duplicating its prompt in every new receipt. Choose recursive lexicographic
   object-key order, preserved array order and exact decoded string values,
   encoded as UTF-8 JSON; freeze that encoding under the protocol version and
   test it across restart. Do not hash a truncated request or normalize prompt
   whitespace. Limit display-only receipt detail to 4 KiB; preserve exact
   IDs. Reject an operation before sending if its complete receipt representation
   cannot fit the chosen 64 KiB per-receipt bound.
3. Retain at most 8,192 receipts and 4 MiB of encoded receipt state in one active
   generation. Before an insertion crosses either bound, prepare the next epoch
   with an empty receipt map. Preserve source binding, owned threads, active
   turns, exact pending requests and their reply-sent markers. Rotation changes
   operation history only; it does not reconnect, resend, cancel or resolve work.
4. Rotation holds the existing mutation mutex and the state lock while preparing
   and atomically persisting the new state. No handler may be executing a
   provider operation during that transition; provider notifications cannot be
   overwritten by a stale cloned snapshot. Publish the new epoch in memory only
   after successful persistence. A command that arrived with the just-retired
   epoch returns `Retired`; **do not silently retag and execute it**. The user
   refreshes and explicitly submits a new action. This small extra interaction
   at rollover is preferable to an invisible replay.
5. Keep no growing retired-ID set. The namespace plus monotonic current epoch
   rejects every earlier-generation ID, including a nonce that was never sent.
   No provider operation is inferred from that rejection. Terminal receipt
   payloads are retired, not archived to an ever-growing directory. Optional
   user export is outside this first release.
6. Retire unsettled outcomes conservatively. Under the mutation lock there is no
   currently executing command; saved Sending on restart already becomes
   Uncertain. Before retirement, record a bounded diagnostic: total retired
   uncertain count plus at most 32 recent summaries, each at most 1 KiB,
  identifying its exact operation/thread IDs when they fit. Summaries omit
   oversized text rather than truncate identities, and disclose omitted
   details/counts. They grant no replay or control authority. Return `Retired;
   previous outcome may be uncertain—inspect the original conversation`, even
   when a particular old receipt is no longer available. Do not turn retirement
  into confirmation.

## Admission and the 16 MiB boundary

The current limit covers the **whole** state, including notifications, roster and
pending requests. The count cap is not its only failure path.

- Normal admission (Start, Send/Steer, Reconnect) requires the exact projected
  encoded state to fit 12 MiB, leaving 4 MiB of headroom below the existing 16 MiB
  storage/transport bound. Retire operation history first if that would restore
  admission. If the remaining live state alone prevents it, reject before any
  provider call and explain the capacity condition.
- Stop and exact-request Reply may use that headroom, subject to the hard byte
  bound and the per-receipt bound. They are not exempt from durable intent. Before
  rejecting because of receipt bytes/count, retire the old generation; the next
  explicit operation gets a fresh, empty receipt window. Repeated generations
  replenish capacity without accumulating receipt files or tombstone lists.
- Check the actual serialized candidate before sending. Project capabilities
  from the current capacity state; receipt retirement is a recoverable condition,
  while an unrepresentable live state is an explicit control limitation. For a
  reply too large to durably represent, keep the supported smaller choices
  accurately available rather than implying every response fits.
- **Guarantee only the reproduced boundary:** old operation history must not
  prevent controls when the live non-ledger state plus the bounded receipt fits
  and storage works. A roster/pending-state payload that alone exceeds 16 MiB is
  a separate capacity failure, not solved by receipt rotation. Asynchronous
  provider metadata can consume headroom; do not present the reserve as a
  permanent guarantee. Actual disk-full, permission and sync failures also
  cannot guarantee control delivery. Report them, send nothing before durable
  intent, and keep post-send uncertainty/no-replay semantics.

## Compatibility and migration

Version the private control protocol and state so an old host cannot ignore the
new ledger header and execute retired IDs. Updated clients detect a legacy host
and require an explicit host upgrade, without automatically killing its provider
connection. Old clients cannot issue v2 mutations. Do not launch two owning hosts
for the same state directory.

On migration under the directory lock, preserve the existing v1 state once as a
read-only private `legacy-state.json` (at most the existing 16 MiB bound), then
atomically write v2 state with ownership preserved and a new ledger namespace.
Never overwrite this backup on retry. A legacy bare ID found there returns its
recorded receipt, converting saved Sending to conservative Uncertain; changed
payloads are rejected. An absent bare ID returns `Legacy ID not accepted; nothing
resent`, never a fresh execution. All new work needs v2 IDs. Migration interruption
must leave either valid v1 or committed v2 plus its legacy lookup file; conflicting
or corrupt files stop mutation instead of guessing. Ordinary reconnect remains
explicit after host restart.

Migration is not a live provider takeover: saved pending IDs do not become
actionable on a new connection. Preserve durable ownership, invalidate stale
connection-scoped requests as today, then accept only freshly observed requests
after explicit reconnect. The no-change guarantee for current turn/request IDs
applies to in-process ledger rotation, not a host upgrade/restart.

Use atomic replacement of the entire header+receipt transition. The present
Windows remove-before-rename branch is insufficient for this requirement: the
implementation must supply and fault-test a Windows replacement/recovery method
before enabling migration/rotation there. Do not silently initialize a fresh
namespace when expected state is missing or corrupt. Preserve a durable directory
identity/initialization marker so missing state is distinguishable from first use.
If replacement returns an ambiguous durability error, stop mutations until the
committed state is reloaded or recovery fails closed; do not keep accepting IDs
against a possibly stale in-memory epoch.

## User behavior, tests and rollout

The snapshot exposes ledger protocol/epoch and an optional retirement notice.
`Retired` is terminal for that operation ID, not success. Keep the instruction
draft, show the reason, and require a fresh explicit action after refreshing the
snapshot. Revalidate the original worker, active turn and exact request on that
action. Never use a newly observed request as a substitute for the retired one.

Acceptance:

1. Migrate the existing 10,000-receipt fixture, preserve ownership, explicitly
   reconnect and obtain a fresh fixture turn/request; Stop and Decline execute
   once each. Separately, rotate a full v2 ledger with an already active task
   and request, preserving those exact IDs. Repeated count/byte rollovers keep
   those controls usable without growing storage.
2. Active duplicate returns its receipt; changed payload is rejected. Retired,
   foreign, future and malformed IDs cause zero provider calls, before/after
   restart. A Retired response never clears the draft or automatically obtains
   and submits a fresh ID. Test two clients straddling rotation.
3. Both Confirmed and Uncertain retired receipts remain protected from replay;
   bounded uncertainty diagnostics disclose lost detail. Current pending request
   IDs and reply-sent markers survive rotation unchanged. Receiving a new request
   does not authorize replay of a former reply.
4. Inject failure before/after each migration and replacement commit boundary,
   including rename/directory sync and Windows replacement. Recovery never resets
   the namespace/epoch, discards ownership or executes a retired operation.
5. Exercise actual encoded sizes: prompt escaping, max receipt/detail, receipt
   window full, live state at 12 MiB, controls approaching 16 MiB, and live state
   alone too large. Assert bounded files and honest capabilities/reasons.
   Physical storage failures remain explicit failures with no unsupported
   success claim.
6. Re-run existing stale-request, uncertain-send, concurrent-ID, no-auto-resume
   and control tests. Reuse the independent audit peer; add a narrow fixture
   for phase-specific persistence failure. No authenticated provider is needed
   for these invariants; provider compatibility remains a separate release gate.

Roll out protocol/state negotiation and tested migration together, with the v1
backup retained. Once v2 IDs have been used, do not downgrade a v2 directory into
the old writer. Validate native platform replacement paths before enabling this
feature there. This is larger than a guard removal; if that migration work cannot
fit the implementation window, take [REL-01](REL-01-BRIEF.md) as the small first
fix and leave REL-02 explicitly open rather than ship an unbounded exemption.
