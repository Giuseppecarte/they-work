# REL-01 — truthful admission failure

The supervisor now returns `Rejected / Not sent` when its initial intent save
fails before provider execution. The operation ID, fingerprint and draft remain
bound to the original action. A same-ID retry in that running host remains
rejected; a fresh explicit submission can execute once after storage recovers.

The initial storage error remains in the snapshot's existing `last_error`
field. Saving the corrected rejection is attempted separately. A failure of that
second write reports that the rejection exists only in the running host. No
public API or persistence schema changed. The UI already preserves drafts for
non-confirmed receipts, so this correction adds no new approval or resend flow.

## Executed checks

- Three private, per-host injection tests cover EACCES, EFBIG, successful saving
  of the corrected rejection, and failure after rename but before directory
  sync. If the failed correction leaves Sending on disk, restart recovers
  Uncertain and never launches a provider.
- A process test submits the same operation from eight clients while the state
  destination cannot be replaced. All receive Rejected; the provider log is
  empty. Restoring storage preserves that rejection, rejects a changed payload,
  and permits one explicit new ID to execute exactly once across duplicate
  submissions and restart. This uses a destination-directory obstruction and
  works without depending on the test user's root/non-root permissions.
- The complete control crate suite passed: 20 tests, zero failures. Strict
  Clippy for the control crate and all its targets passed.
- [Executable admission evidence](admission/results.json) additionally uses
  actual Unix directory permissions and a process file-size limit with the
  compiled release binary. Both scenarios passed. The file-size fixture needs
  a host restart to remove its hard RLIMIT_FSIZE; this is not evidence of
  physical disk exhaustion or same-process quota recovery.
- [Existing control regression evidence](regression/results.json) reuses the
  unchanged iteration 7 harness for concurrency, current controls, uncertain
  acknowledgement, provider-gated crash and post-send persistence failure.

Reproduce admission with:

```sh
python3 docs/design-audit/iteration-8/control/verify_admission.py --binary /path/to/they-work
```

Run the crate's normal tests through the supported Cargo entry point. These
tests use isolated directories and offline fake providers; process tests need
local loopback sockets. The injection writer is compiled only for unit tests.
The security writer's real rename/sync sequence remains the production path.

## Critical review and limits

The correction is intentionally limited to a proven pre-dispatch failure.
After the provider receives work, a missing durable acknowledgement remains
uncertain. A failed write does not establish that no intent reached disk, so
removing an ID or replaying it after restart would be unsafe. The tests cover
that distinction instead of only asserting a changed label.

This does not repair operation-history capacity (REL-02), Windows state
replacement (WIN-01), or guarantee durable error reporting on inaccessible
storage. Authenticated providers, physical disk exhaustion and real terminal
paint are not tested by these fixtures. The companion documentation/PTY review
records the visible receipt and retained draft separately.
