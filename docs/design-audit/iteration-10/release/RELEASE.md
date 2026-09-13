# RELEASE-01 closure verification

**Closure remains blocked on actual native CI archives.** Local verification uses
an owned registry bound only to `127.0.0.1:5060`; it does not change public tags,
contact the GitHub Release API, or substitute fixture files for native packages.
The final machine-readable status is in `RESULTS.json`.

## Reproduced defect and correction

The clean baseline `4097e4f` failed its actual multiarchitecture Docker build.
The dependency-cache layer copied a Cargo manifest with the explicit harness-free
`storage_process` test, but created only library and binary stubs. Cargo validates
explicit target paths even during a normal release build, so it rejected the
manifest before compilation. `build.log` retains the failure.

Candidate `7cb26e45e13de5fa9bfa3a3be7c2eeb59a1011c1` adds the missing temporary test
stub to the cache layer. The subsequent complete source copy replaces that stub
with the real process test. Runtime code and native-provider authority do not
change. The regression test executes the actual Docker cache file-preparation
commands in a temporary directory and checks explicitly declared Cargo targets.
`regression-before.log` proves the old layer fails that test. The final suite
passes 21 tests using pinned Python 3.12; release tooling requires Python 3.11+
as already documented.

## Final verification procedure

The fixed image is built once from an independent clean detached checkout, with
linux/amd64, linux/arm64 and provenance attestations. Buildx metadata identifies
the immutable index. The verifier pulls and tests that exact digest, using
explicit platform arguments. For each architecture it checks locale, Kitty,
iTerm2 and the native roster fallback through executable PTYs, then sends `q`
and requires normal exit. AMD64 is emulated on the ARM64 Linux Docker daemon.

The nine local-registry recovery scenarios inspect real registry state: failed
smoke, failed checksum, success, interruption between tags, lost version/latest
acknowledgements, version conflict, later latest and an injected incomplete native
publication outcome. Their native inputs remain six clearly labelled checksum
fixtures. Passing those scenarios is auxiliary recovery evidence, not completion
of the plan's actual-native-archive criterion.

## Required external evidence

Obtain all six native archive/checksum pairs and native test results from the
same frozen candidate's CI run. Retain the run identity, source commit and raw
artifacts. Rehearse checksum gating and promotion recovery with those actual
archives in an isolated local repository; do not relabel the fixture rehearsal.
Native archive execution and packaging must be evidenced by the corresponding
native jobs, including Windows x64 and ARM64. No such artifacts are currently
available for this candidate.

Public GHCR authentication, publication, GitHub Release recovery and physical
terminal transport remain separate unperformed publication checks. A PTY with
simulated protocol responses does not certify a physical terminal. An index
may be rebuilt for a later candidate, but every promotion must use its own exact
verified immutable digest without rebuilding during promotion.

## Critical review

The fresh clean build found a real release regression that older successful
image evidence could not cover. The small cache-context regression now catches
that class of missing explicit target before an expensive image build. The
full build remains necessary to validate Cargo and the runtime image. Local
registry recovery can establish digest preservation and idempotence; it cannot
establish that native CI packages exist or that public publication succeeds.
