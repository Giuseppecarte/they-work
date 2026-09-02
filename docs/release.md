# Release image and no-clone install

Version tags publish the runtime image at:

~~~text
ghcr.io/giuseppecarte/they-work:<version>
ghcr.io/giuseppecarte/they-work:latest
~~~

The [`release.yml`](../.github/workflows/release.yml) workflow runs only for a
semantic version tag such as `v1.2.3`. It builds `docker/Dockerfile`, publishes
the version tag and updates `latest`. Pull requests and ordinary branch pushes
never receive package-publishing permissions.

Before the first release, set the GHCR package visibility to **Public** in the
repository's package settings. The workflow only needs `packages:write` to
push the image; it does not store a personal token or change account settings.
The installer below is intentionally anonymous after that one-time package
setting is in place.

After a release exists, anyone with Docker can run the current image without a
checkout:

~~~bash
curl -fsSL https://raw.githubusercontent.com/Giuseppecarte/they-work/main/docs/install.sh | sh
~~~

Pin a release instead of `latest` when reproducibility matters:

~~~bash
curl -fsSL https://raw.githubusercontent.com/Giuseppecarte/they-work/main/docs/install.sh \
  | THEYWORK_IMAGE=ghcr.io/giuseppecarte/they-work:v1.2.3 sh
~~~

The installer requires Docker, pulls only the selected image, mounts existing
agent homes read-only, and skips a home that is absent. Override the host-side
locations when they are not in the usual places:

~~~bash
curl -fsSL https://raw.githubusercontent.com/Giuseppecarte/they-work/main/docs/install.sh \
  | THEYWORK_CLAUDE_HOST=/mnt/c/Users/PC/.claude \
    THEYWORK_CODEX_HOST=/mnt/c/Users/PC/.codex sh
~~~

The running process still has the invoking `--user UID:GID`, `--network none`,
a read-only root, dropped capabilities, no-new-privileges, and no writable
agent mount. The exact policy and the optional project/configuration arguments
are in [INSTALL.md](../INSTALL.md).

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
