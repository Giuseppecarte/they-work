# RELEASE-01 implementation and verification

The workflow now publishes only a uniquely named candidate before verification.
It checks the exact registry index on both runnable architectures, validates the
six native archive checksums, retains intent and verification before any public
release-tag write, and then promotes version followed by latest. The helper
reads back every destination; lost acknowledgements are recoverable no-ops and
conflicting tags stop publication. No rollback or rebuild occurs during recovery.

Implementation: [.github/workflows/release.yml](../../../../.github/workflows/release.yml),
[helper](../../../../scripts/release_image.py),
[verifier](../../../../scripts/test-published-image.py),
[tests](../../../../scripts/test-release-image.py),
[rehearsal](../../../../scripts/rehearse-release-image.py), and
[operator guide](../../../release.md). The deterministic suite also runs in
the ordinary CI verify job.

## Executed results

- **20 deterministic tests passed.** They cover failed verification/checksums,
  missing native/platform/mode inputs, changed commit/index evidence, version and
  latest conflicts, partial writes, lost acknowledgements, immutable intent,
  transport/auth failures, explicit platform selection and normal PTY exit.
  [Log](evidence/unit-tests.log).
- **Actual multi-architecture image built and pushed only to localhost.** The
  index includes linux/amd64, linux/arm64 and two provenance-attestation
  descriptors. [Build log](evidence/build.log), [metadata](evidence/build-metadata.json).
- **Six product PTYs passed:** Kitty, iTerm2 and no-reply native roster on both
  architectures; all received q and exited with code 0 without forced cleanup.
  ARM64 ran natively on the Docker Linux daemon; AMD64 ran through emulation.
  Locale checks passed for both. [Report](evidence/verification.json).
- **Nine local-registry scenarios passed.** These use the actual multi-arch
  index and real registry writes: failed smoke, failed checksum, success,
  interruption between tags, lost version acknowledgement, lost latest
  acknowledgement, conflicting version, later latest, and an injected native
  publication failure outcome. [Results](evidence/rehearsal/results.json),
  [log](evidence/rehearsal.log). The final index digest matches the tested index,
  including its attestation descriptors; no platform manifests were rebuilt.
- Python syntax, YAML syntax and local documentation links were checked. These
  checks do not mean GitHub Actions ran remotely.

Tested index:

```text
sha256:ddc6a0fb6dccece7eab549e6701e16b9b02cfe3405dbb82ff440cebb825ed092
```

This local build used the shared working tree. The verification's commit
`5fa7d55d89af9c407ea95327d8080322e2bef13e` is its reference baseline, recorded after
building, not a claim that all build-context files came from a clean committed
checkout. The report explicitly records `source_state: dirty`. The immutable
image digest identifies the exact binary/container actually exercised. CI uses
its checked-out event commit and retains that verification separately.

## Critique and corrected verifier expectation

The first run passed all four graphics cases but timed out in both fallback
cases. It had **not sent q**: the old release verifier waited for quadrant glyphs,
while the current default image-free office intentionally draws a native roster.
The [initial report](evidence/verification-initial.json),
[initial log](evidence/verification-initial.log) and
[diagnostic ANSI](evidence/fallback-before.ansi) retain this failure.

The corrected oracle waits for actual demo project, three task titles, human
question state, person count and Back navigation, then sends q. It still rejects
unexpected image transmission, crash, timeout and forced cleanup. It does not
change production rendering or weaken the normal-exit requirement. The same
image digest then passed all six cases. Explicit legacy-camera quadrant rendering
is outside this default-fallback check.

The workflow cannot atomically update two tags and GitHub Releases. Its retained
intent precedes promotion; recovery consults actual registry state, including a
latest that already equals the candidate. A version at another digest or a latest
outside the recorded previous/candidate pair blocks writes. A shared concurrency
group serializes this workflow, but external publishers can still race; manual
recovery must ensure publication is quiescent. No automatic deletion is offered.

## Limits and cleanup

The rehearsal's native files are **six checksum fixtures**, not native packages.
It tests the checksum gate and preserves tags after failure; the existing native
matrix remains responsible for actual native builds/archives. The GitHub Release
failure was an injected workflow outcome, not a remote API call. No public tag,
GHCR permission, GitHub artifact upload, signing, anonymous download or actual
GitHub Release publication was tested or performed. PTYs used simulated protocol
responses, not physical terminals or provider accounts.

The registry container was bound to 127.0.0.1:5058 and removed with its owned
volume after the run; [cleanup log](evidence/cleanup.log). Other Docker resources
were not pruned. No personal source store was mounted. The release changes do
not alter production state schemas or the native workflow.

Reproduction commands and partial-publication recovery are in
[docs/release.md](../../../release.md). A new local build may have a different
index because provenance metadata changes; always test and promote its exact
recorded digest rather than substitute this audit's digest.

The retained build log expands carriage-return progress records and trims trailing
whitespace for readable diffs. `RESULTS.json` records both its original and retained
hashes; the build outcome and command text are unchanged.
