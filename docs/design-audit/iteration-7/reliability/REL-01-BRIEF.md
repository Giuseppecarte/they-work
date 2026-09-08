# Fallback implementation: report an intent that was not sent

**Chosen scope:** repair the live-host receipt after the initial intent write
fails, before `Host::execute` is called. Reuse `OperationStatus::Rejected` with a
clear reason: `Not sent: local state could not be saved. Restore storage access
and submit again.` Keep the stable ID and request fingerprint. Repeating that
ID returns the same terminal rejection, never a false pending acknowledgement;
a deliberate resubmission creates a new ID using the existing UI behavior.
Do not automatically resend under either ID.

`supervisor.rs:256–271` currently inserts Sending then propagates persistence
failure, leaving that in-memory entry behind. Catch only this initial error,
replace the in-memory receipt with the rejection and return it to the caller.
Attempt to persist the rejection without masking the original failure or claiming
that the rejection is durable. Leave post-`execute` error handling unchanged.
The existing receipt/snapshot UI must show the rejection reason and retain the
draft; verify that behavior rather than introducing a new acknowledgement flow.

**Important boundary:** `write_private` can fail after rename, during directory
sync. “Save failed” does not prove no intent reached disk; it does prove this
handler has not yet called the provider. If restart finds a saved Sending
receipt, keep the existing conservative Uncertain/no-replay treatment. Do not
delete a possibly durable receipt, describe it as safely retryable after restart,
or extend this repair to failures after the provider was contacted.

Acceptance, using the existing offline peer and isolated directories:

1. Permission denial before intent: provider calls remain zero; initial response,
   snapshot and same-ID retry are Rejected with Not sent, including after access
   is restored. A new explicit operation ID executes once after restoration.
2. Same ID with a different payload is still rejected; concurrent same-ID callers
   cannot create a provider request or replace the stored fingerprint.
3. File-size-limit failure has the same live-host result. Fault injection after
   rename but before directory sync leaves no provider call; restart conservatively
   retains any durable Sending as Uncertain and does not replay it.
4. Existing uncertain-ack, after-send persistence, restart and concurrent-send
   cases remain unchanged. Explicit controls with stale turn/request IDs remain
   rejected by their existing validation.

Owner: control supervisor plus focused tests; touch renderer only if the existing
Rejected receipt path fails to expose the reason or preserve the draft. No schema
migration, provider/API change, ledger-capacity fix or Windows replacement claim.
The directory-sync fault needs a focused persistence seam in tests; EACCES alone
cannot stand in for that boundary. Roll out after these tests and the existing
control suite, retaining the synthetic production-release reproduction as the
before evidence. No implementation is included in this audit.
