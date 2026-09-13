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
| macOS Codex fixtures used `/var`, a symlink alias | Canonicalize newly created fixture directories in both collector test helpers | SQLite `NOFOLLOW`, source symlink rejection and read-only store behavior |
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
