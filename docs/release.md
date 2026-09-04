# Release image and no-clone install

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

## Future releases

The [release workflow](../.github/workflows/release.yml) triggers on tags matching
`v*.*.*`; this glob is not semantic-version validation. It builds the tagged
source, pushes the version and `latest`, and runs with package-write permission.
No workflow changes were made to obtain the v0.1.0 success.

After each release, verify both publication and anonymous access separately.
A `denied` response alone cannot distinguish an unpublished package from a
restricted one. Once a successful publish is established, denied anonymous
access requires checking package visibility/permissions. A nonexistent tag
in the now-public package was tested and returned `not found`, exit 1.
The v0.1.0 package is neither missing nor restricted to authenticated clients.

The running process uses the invoking UID/GID, no external network, a read-only
root, dropped capabilities, no-new-privileges, and read-only agent mounts.
The direct demo image defaults to non-root UID 10001.

The following older probes are historical evidence, not current install commands.

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

## Current clean-host status — 2026-09-02

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
