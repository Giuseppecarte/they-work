# Release verification and recovery

The [release workflow](../.github/workflows/release.yml) runs for tags matching
`v*.*.*`; that glob is not semantic-version validation. This guide describes the
current workflow, not evidence that a new public release has run. Normal native
installation is documented in [INSTALL.md](../INSTALL.md).

## Candidate gate

1. The existing native matrix builds, tests and archives all six native targets.
2. Buildx builds one `linux/amd64` + `linux/arm64` index and pushes only a unique
   `candidate-<run-id>-<attempt>` tag. Version and `latest` are not passed to the
   build action. Candidate tags in a public package may be visible; they are
   unpromoted artifacts, not private storage or supported release pointers.
3. `test-published-image.py` resolves the registry index, pulls that immutable
   digest for each explicit platform, and checks locale plus Kitty, iTerm2 and
   no-reply fallback in a 160×48 PTY. Every process must exit normally after `q`.
   A timeout, crash or forced cleanup fails verification. Reports distinguish
   daemon-native architecture from emulation; these are simulated terminal
   responses, not physical-terminal or provider-account validation.
4. The final job downloads the native archives and requires exactly the six
   expected files with matching SHA-256 sidecars. It rechecks the verified index
   and writes an immutable intent naming commit, digest, destinations, native
   checksums and the previous `latest` digest.
5. Uploading that intent and verification as `release-intent-<run-id>` must succeed
   **before** any public tag changes. An existing intent is never overwritten.
6. Promote version, read back its exact index digest, then promote `latest` and
   read it back. The source is a single `repository@digest`; there is no rebuild,
   platform filter, manifest merge or annotation edit. Both platform manifests
   and attestation descriptors remain part of the same index. Only then create
   the GitHub Release with the verified native files.

Docker documents the single-index copy behavior in
[imagetools create](https://docs.docker.com/reference/cli/docker/buildx/imagetools/create/).
The full destination digest, not merely the presence of two architecture names,
is the promotion invariant. Do not substitute `docker pull/tag/push`, which can
operate on only the locally selected platform.

The final job uses a shared concurrency group across release tags and does not
cancel an in-progress publication. This serializes this workflow's publication
phase; it does not establish semantic version order or prevent writes by other
publishers. Registry authentication/transport errors stop promotion rather than
being treated as an absent tag. The helper uses Python 3.11+ and Docker Buildx.

## Local checks and disposable-registry rehearsal

```sh
PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-release-image.py
```

The deterministic suite uses an in-memory registry boundary and tests failed
verification/checksums, conflicting tags, partial writes, lost acknowledgements,
normal PTY exit requirements and idempotent retries. It performs no registry
writes. CI runs it on pull requests and before candidate builds.

For an actual local registry rehearsal, use an owned disposable registry bound
only to loopback, build the current image to it with both platforms, and retain
Buildx's metadata. Supply the exact resulting digest and actual source commit:

```sh
python3 scripts/test-published-image.py \
  --image localhost:5058/they-work@sha256:<candidate-digest> \
  --commit <full-source-commit> --report target/release-check/verification.json
python3 scripts/rehearse-release-image.py \
  --verification target/release-check/verification.json \
  --output target/release-check/rehearsal
```

The rehearsal refuses non-loopback registries. It creates baseline and test tags
only in that local repository, then injects interruption before a write or after
an accepted write. It checks real registry digests. Its six native files are
**checksum fixtures**, not native executables; the real native release matrix
remains required. Its injected GitHub Release failure tests reporting only; it
does not contact GitHub. Stop and remove only the registry/container created for
that rehearsal afterwards. Do not prune unrelated Docker data.

## Recover a partial publication

There is no atomic transaction across two registry tags and GitHub Releases.
Keep the candidate and the run's artifacts; do not delete a verified version or
roll back `latest` automatically.

| Observed boundary | Recovery |
|---|---|
| Candidate smoke or native checksum failed | Version and latest were not touched. Repair and create a new verified candidate. |
| Version write succeeded; latest failed | Keep the verified version. Resume using the original intent and digest. |
| Registry accepted a write but its response was lost | Read back the destination. The desired digest means success/no-op, including latest. |
| Version points to another digest | Stop. Never overwrite it as part of retry. |
| Latest differs from both the recorded previous digest and candidate | Stop automatic recovery; another publication may have advanced it. |
| Images promoted; GitHub Release attempt failed | Report images as published and native publication as incomplete/unknown. Inspect the remote release and its assets before completing it. Do not roll back images. |

Download the original `release-intent-<run-id>` and all `native-*` artifacts from
that run into an isolated directory. Retain the verification and intent bytes
unchanged. The workflow retains artifacts for 90 days; if they are unavailable,
do not infer a verified intent from mutable tags or fabricate a replacement.
Ensure no other publisher is active when recovering outside the serialized job.

```sh
python3 scripts/release_image.py promote \
  --intent <downloaded>/intent.json \
  --verification <downloaded>/verification.json \
  --native-dir <downloaded-native-files> \
  --record <recovery-output>/promotion.json
```

This is a real publication command: use it only for an authorized release. It
rechecks verification and native hashes before writes. Existing matching tags
are no-ops; a conflicting version or a later latest stops it. A failed read is
unknown, not permission to overwrite. The completed-step log can be incomplete
after runner death, so registry state is always read during recovery.

Do not rerun the whole build expecting the digest to remain identical. Attestations
and build metadata may change it. A fresh intent cannot replace an already
published version, and the workflow refuses to overwrite an earlier run intent.
For native publication, inspect `gh release view` and downloaded asset hashes:
complete missing verified assets explicitly, or create the release if absent.
Never clobber differing assets automatically. The outcome record reports the
workflow result and asks for remote inspection after failure; it cannot prove
that a failed API call left no release or assets behind.

## Historical publication records

The following checkpoints are retained as historical evidence. Their dated
access results, installer examples and descriptions of then-current behavior
were not revalidated by the candidate-promotion change above.

<details>
<summary>Earlier publication and installer probes</summary>

## v0.1.0 publication

[Release run 33824108031](https://github.com/Giuseppecarte/they-work/actions/runs/33824108031)
succeeded on its first attempt. Tag `v0.1.0` points to
`f100965df74f48a5a852fcf79e83f90fbbd409b7`. Both
`ghcr.io/giuseppecarte/they-work:v0.1.0` and `latest` are anonymously readable.

Published index digest:

~~~text
sha256:b8bf5a70b41ceafcc3331fd790c411e3fd808d3541074dff75ec36858f8ba214
~~~

**Download size: 30,057,713 bytes (28.67 MiB)** of compressed Linux/amd64
image layers, before cache reuse. This is the sum of registry layer sizes
(28,232,655 + 3,316 + 1,821,742 bytes), not the unpacked disk footprint.
Config/manifest metadata and the provenance attestation add a small transfer
overhead. The number can be checked without pulling layers:

~~~sh
docker buildx imagetools inspect ghcr.io/giuseppecarte/they-work@sha256:c0e82c59fb8169fda150e6fb34bc7d4f64fcded9d415e6441a419b07e3390334 --raw
~~~

The runnable platform is Linux/amd64. The second manifest is a provenance
attestation, not another CPU architecture. See the
[M11 transcript](release-v0.1.0-transcript.md) for commands, results, and the
successful fresh-container installer test as UID 10002.

Use the [Docker-only README command](../README.md#start-here) to see a demo.
For local data, use the single [checksum-verified installer procedure](../INSTALL.md#without-a-checkout).
It needs a POSIX shell, curl, sha256sum, and Docker. Do not use an unverified
`curl | sh` pipeline or skip checksum verification.

To select the release or nonstandard data directories, export these variables
before running that verified block:

~~~sh
export THEYWORK_IMAGE=ghcr.io/giuseppecarte/they-work:v0.1.0
export THEYWORK_CLAUDE_HOST=/mnt/c/Users/Example/.claude
export THEYWORK_CODEX_HOST=/mnt/c/Users/Example/.codex
~~~

Use only paths that exist on the Docker daemon's host. Demo mode needs no data
mounts. Live mode with no stores shows setup guidance rather than an office.

## Clean-host probe

On 2026-08-30, the documented no-clone command was run from `/tmp` with an
empty temporary `HOME` and no repository in the working directory. The public
script URL failed before any installer code could run:

~~~text
$ env HOME=/tmp/they-work-m9-installer.ypz6AY/home /usr/bin/time -p sh -c 'curl -fsSL https://raw.githubusercontent.com/Giuseppecarte/they-work/main/docs/install.sh | sh'
curl: (22) The requested URL returned error: 404
real 0.15
user 0.01
sys 0.00
~~~

The pipeline returned status 0 because the final `sh` received no input and
the POSIX shell reports the final pipeline stage. No installer prompt appeared
and no Docker command ran. The exact current installer body was then staged in
the same clean temporary area to separate the script-fetch failure from the
image-fetch failure:

~~~text
$ env HOME=/tmp/they-work-m9-installer.ypz6AY/home THEYWORK_IMAGE=ghcr.io/giuseppecarte/they-work:latest /usr/bin/time -p sh /tmp/they-work-m9-installer.ypz6AY/install.sh
Pulling ghcr.io/giuseppecarte/they-work:latest ...
Error response from daemon: Head "https://ghcr.io/v2/giuseppecarte/they-work/manifests/latest": denied
real 0.60
user 0.00
sys 0.01
~~~

The staged installer exited with status 1 after 0.60 seconds. This is the
current front-door finding: the public raw script is not reachable, and the
`latest` GHCR package is denied.

### Detailed stranger transcript

The complete fresh-container transcript, including the public fetch failure,
the UID correction, the interactive first screen, the direct permission check,
and the judgement about what a new user would do next, is in
[docs/installer-transcript.md](installer-transcript.md). After the script is
published on the default branch and the GHCR package is public, rerun that
transcript's public step; the local replay now passes the invoking UID/GID to
the image.

## Historical clean-host status — 2026-09-02

The earlier pipeline examples above have been replaced with temporary-file
commands so a failed download keeps its curl exit status. A fresh UID 10001
container with no checkout ran the current README block and received:

~~~text
curl: (22) The requested URL returned error: 404
installer_status=22
~~~

The same fresh identity received a denied response and status 1 from the
documented GHCR image. The no-clone path is therefore not published end to end
yet. The current full transcript, including the successful local private-data
and interactive replays, is in
[docs/installer-transcript.md](installer-transcript.md).

</details>
