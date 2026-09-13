# WIN-01: preserve Windows control state during replacement

The implementation removes the application-created missing-file interval and
shares one fault gate between foreground admission and background event saves.
Native Windows completion remains **not tested** until both CI architectures
execute the new replacement and fake-provider cases successfully.

## Behavior

- A private, current-user-owned sibling file receives the complete bytes and is
  synced and closed before publication. Windows uses
  `MoveFileExW(MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)`, without
  deleting the existing destination, permitting cross-volume copying, or
  scheduling a delayed move. Unix keeps rename and parent-directory sync.
- Existing destinations must be ordinary files owned by the current user.
  Metadata failures, foreign ownership and reparse points are errors. The
  replacement retains the staged file's protected owner-only DACL.
- If publication fails, a valid canonical snapshot permits a later explicit
  save. If the canonical state is absent, unreadable or invalid, the synced
  candidate is preserved if it remains available, and a shared in-memory fault
  blocks subsequent foreground/background saves before another temporary file
  is allocated. The original failure remains in the error chain.
- A checked restart reads only `state.json`. It never promotes a candidate or
  rolls back to an older backup. A missing state plus an existing endpoint
  refuses startup: the endpoint can only have been published after the initial
  state was saved. Truly new directories containing neither can initialize.
- Storage parsing requires the existing durable `generation`, `codex_home`,
  `threads` and `operations` keys. JSON such as `{}` cannot silently become an
  empty receipt ledger through model defaults. Additive legacy field defaults
  remain compatible; no on-disk schema or journal is introduced.
- `ControlClient::saved_snapshot_optional` distinguishes a fresh directory from
  damaged established storage for the office. The existing `saved_snapshot`
  interface remains available. `connect_or_spawn` performs the same check before
  launching a detached host, so a useful recovery error reaches its caller.
- Existing admission and restart policy remains: never automatically resend;
  saved `Sending` becomes `Uncertain`; IDs and fingerprints are preserved.
- A latched fault clears advertised authority and current requests in live
  snapshot responses while preserving the host's internal provider state.
  The additive `storage_recovery_required` response flag lets the office
  distinguish recovery from ordinary provider errors without parsing prose.
  Healthy internal snapshots omit the false flag from serialized storage.
  Explicit provider operations check canonical storage before RPC launch or
  reuse; routine snapshots read the latched error without reparsing disk state.

The fault gate requires an explicit host restart after canonical storage is
repaired. Restoring an older snapshot can lose receipt deduplication and is not
an automatic recovery procedure. Preserve affected files for inspection. Manual
deletion of both state and endpoint cannot be distinguished from a new directory
using the existing format.

The older S admission process fixture deliberately moved canonical state aside
and placed a directory at its name. That now exercises the established-state
loss rule: the fixture restores the exact original file, verifies the recovery
flag remains set, and performs a checked restart before its new explicit
submission. Its rejected receipt was explicitly reported as held only in the
running host; the restored canonical predates it, so the fixture verifies that
no durable receipt is invented and nothing is replayed after restart. It then
verifies the newly durable explicit operation deduplicates across restart.
A pre-write failure with canonical state still valid retains
same-host recovery, covered by admission unit tests and the native Windows
sharing-violation process case. Original intent-persistence errors remain visible
alongside the recovery reason.

## Evidence and native gate

[Local results](evidence/results.json) records commands, environment, source
hashes and evidence hashes. The local evidence covers four shared storage tests,
one live-host fault projection test, three executable macOS fake-provider
scenarios, strict Clippy, and a Windows
cross-check. The cross-check uses the same production source and Windows tests
through an ignored isolated manifest that excludes collector-only development
dependencies; it is compilation evidence, not Windows runtime evidence.

The process fixture is a Rust test executable which also acts as its own offline
provider. It records receipt deduplication across restart, interruption after
provider receipt but before acknowledgement, and failure to initialize missing
established state. Its Windows-only fourth case holds a destination handle
without delete sharing: the send is rejected, original bytes remain unchanged,
the same ID stays rejected in the live host, and a new explicit ID runs once
after the handle is released. No provider account, model or Python runtime is
used.

The `windows-2022` x64 and `windows-11-arm` ARM64 native CI jobs must retain:

1. Six Windows unit-test results: ownership/DACL preservation for state, config
   and endpoint; real sharing violation; concurrent readers; unsafe destination
   rejection; interruption before staging, before publication and after
   publication; and its child-process entry point.
2. Exactly four shared recovery tests, including 300 rejected follow-up writes
   after an injected ambiguous publication with only one candidate retained.
   One additional live-host test confirms disabled advertised authority,
   preserved internal requests and a fault that lasts until a checked restart.
3. Exactly four passing native process cases with matching platform and compiled
   architecture; zero matching tests or a missing result file fails the gate.
4. Runner, OS, filesystem, source revision, raw logs and structured results.

The symlink test requires permission to create disposable file symlinks on the
Windows runner. It fails rather than claiming a security pass if that facility
is unavailable. The CI job itself has not been executed from this local checkout.
Physical power loss, non-NTFS local filesystems, network shares and authenticated
provider trials remain **not tested**.

## Review and limits

The initial source comment incorrectly asserted Windows rename could not replace
a destination. Rust already supports this behavior. The explicit native call
chooses replacement and write-through flags while removing the unsafe manual
deletion. Microsoft documents these flags in
[MoveFileExW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw).
This does not certify every filesystem or physical-power-loss outcome.

`ReplaceFileW` was not selected because its documented partial failures add
displaced-original recovery states and its write-through flag is unsupported.
The chosen implementation avoids introducing an old-backup rollback that could
discard receipt identities. See
[ReplaceFileW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew).

The remaining limitation is operational recovery: automatic replay or backup
selection would require stronger provenance than this bounded change provides.
The office exposes the failure and disables unsafe controls while retaining
observed facts. Native CI evidence is a required completion gate, not a claim
inferred from local compilation.
