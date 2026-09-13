# Restoring native CI before the main merge

The owner enabled the required workflow permission and commit `8084397`
restored the active CI, native build and guarded release workflows to PR #1.
Native packaging explicitly installs Python 3.13 on all six runners; the
reproduction smoke retains Python 3.12. Branch and pull-request runs never
publish a release.

The first native runs exposed four issues that the earlier application-only
CI did not cover. Exact failure excerpts and job IDs are retained under
[ci-restoration/](ci-restoration/).

| Case | Correction | Boundary preserved |
| --- | --- | --- |
| Windows Clippy compared unit-valued file identities | Use the same optional device/inode identity type across platforms; unsupported metadata still returns `None` | No invented identity or replacement authority |
| macOS Codex fixtures used `/var`, a symlink alias | Canonicalize newly created fixture directories in collector and CLI test helpers | SQLite `NOFOLLOW`, source symlink rejection and read-only store behavior |
| A Linux process fixture timed out during provider initialization | Use the production RPC budget; exercise a 750 ms cold startup and assert confirmed, single dispatch | Missing-acknowledgement timeout, restart and no-replay checks |
| ARM64 musl debug rendering took 5,408 ms against the 5,000 ms budget | Run this exact budget test alone in the optimized profile, with a mandatory one-test result and retained logs | Same frames, assertions and threshold; debug correctness suite still runs all other tests |

The native timing artifact uses `frame-timing-*`, so it cannot enter release
downloads matching `native-*`. The required-verification job still runs its
full debug suite. No performance assertion is removed or relaxed.

A separate checkout check reproduced conversion of golden files to CRLF under
`core.autocrlf=true`, while the serializer requires exact LF bytes. A narrow
golden-directory attribute now enforces LF; it changes no recorded frames.
This was a locally reproduced Windows-checkout compatibility issue, not an
additional observed failure in the first native jobs.

## Validation

The macOS fixture failure was reproduced with the default aliased temp path;
the same compiled test binary passed when given the canonical path. After the
fixture fix, both native collector suites pass using the default macOS temp
location. The cold-start test asserts one initialize, one thread start, one
turn start and one decision; the uncertainty case still deliberately waits
for an acknowledgement timeout.

Fresh integrated command output is retained in `ci-restoration/checks.json`
and its corresponding logs: 471 workspace tests pass (4 ignored), formatting
and strict Clippy pass, and exactly one optimized timing test passes for all
three encodings. Native Windows execution and Linux ARM64 timing
must be confirmed by the subsequent GitHub run; local macOS tests cannot
replace those results. Earlier failed CI runs remain failure evidence.

## Critical review

The first matrix confirmed that local test success did not establish native
portability. Most corrections concern fixture assumptions, but the Windows
comparison also blocked compilation. The optimized frame check is a separate
measurement from debug correctness, compositor p95 and real-terminal latency.
These checks support a local testing baseline, not authenticated-provider,
physical-terminal or release-publication certification.

## Second native pass

Both Linux architectures, required verification, and both audit-smoke jobs
passed on `b202913` (push run `34777496710`, PR run `34777499056`). The macOS
CLI suite exposed the same aliased temporary-directory issue in its separate
fixture helper: 20 passed and 14 failed. On both Windows architectures, the
discovery timestamp fixture lacked Windows metadata-write access and the flag
required to open a directory. Excerpts are retained in
`ci-restoration/round-2/failures.json` and its linked logs.

The fixture corrections keep provider-store access and path-normalization
behavior unchanged. A bounded Windows review also corrected impossible fixture
inputs: a Linux UNC path built from a native Windows drive path, a Linux
project-key directory containing a Windows drive colon, and two CLI assertions
expecting raw filesystem spelling where the interface prints normalized IDs.
The Unix WSL cases and dedicated normalization tests remain covered. Native workspace tests now use `--no-fail-fast`, so one
failed test executable does not prevent execution of the remaining suites.
A failed suite still fails the job. Native confirmation of these corrections
is pending the next candidate run.

The `b202913` Linux x86_64 and ARM64 CI archives were downloaded and inspected:
GitHub artifact digests and included archive checksums match, executable mode
and ELF architecture are correct, and neither executable has an ELF interpreter
or dynamic-library dependency. The archive inspection is retained in
`ci-restoration/round-2/linux-archive-inspection.json`. This verifies the archive
format and static linkage; physical WSL rendering remains owner validation.

The integrated follow-up passed formatting, strict all-target Clippy, and
471 workspace tests (4 ignored), plus the separate storage-process
report. Commands, durations and source hashes are recorded in
`ci-restoration/round-2/checks.json` and `manifest.json`.

## Windows behavior exposed by the complete suite

Candidate `22d9ab9` passed the Linux builds, macOS ARM64 build, both audit
smokes and required verification. The complete Windows suites on x64 and
ARM64 reproduced two production defects, beyond the earlier fixture issues:

- Filesystem traversal converted the Windows verbatim prefix before removing
  it. The resulting `/?/C:/...` office ID disagreed with the original drive
  path and could split a repository across floors.
- Private Windows state was created with the process's default owner and then
  restricted. An elevated process can default to the Administrators group,
  while subsequent validation correctly requires the current user's SID.

The correction normalizes verbatim prefixes consistently before traversal
and creates private Windows objects with an explicit current-user owner and
protected owner-only permissions. Existing state owned by another identity
must still be rejected; the application does not take ownership of it.

A separate diagnostic fixture now distinguishes a Unix/WSL `/mnt/c` crossover
path from a native missing home. Normalized CLI expectations and the Git
worktree worker count remain assertions, with full stdout on failure.

Raw failure excerpts for both Windows architectures are retained in
`ci-restoration/round-3/`. Native confirmation of this correction requires the
next candidate run. These findings show why the earlier local test pass was
insufficient to claim native Windows readiness.

Microsoft documents that a new object's default owner can differ from the
process token's user ([owner of a new object](https://learn.microsoft.com/en-us/windows/win32/secauthz/owner-of-a-new-object)).
The new creation descriptor explicitly selects the user; the existing owner
comparison remains intact. Independent review also required retaining the
post-create ACL-support check because security descriptors require filesystem
support ([CreateFileW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew)).
The existing six-test Windows replacement contract is preserved and its ACL
case now checks newly created/reopened directories and locks, exact one-ACE
owner-only DACLs, and file ownership before and after replacement.

Local integration of the Windows correction passed 473 workspace tests
(4 ignored) plus three separate storage process cases, formatting and
strict all-target Clippy. The actual Windows-target production control library
also passes strict Clippy. Cross-compilation is recorded separately from native
Windows execution in `ci-restoration/round-3/manifest.json`; the latter remains
pending the next frozen commit.
