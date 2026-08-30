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

The running process still has `--network none`, a read-only root, dropped
capabilities, no-new-privileges, and no writable agent mount. The exact policy
and the optional project/configuration arguments are in [INSTALL.md](../INSTALL.md).

## Clean-host probe

On 2026-08-30, the reviewed installer was staged in a temporary directory and
run from `/tmp` with an empty temporary `HOME`, the published image name, and no
local image substitution. No repository was present in the probe's working
directory. The stranger experience was:

~~~text
$ env HOME=/tmp/they-work-m8-installer.0dqlfZ/home THEYWORK_IMAGE=ghcr.io/giuseppecarte/they-work:latest sh /tmp/they-work-m8-installer.0dqlfZ/install.sh
Pulling ghcr.io/giuseppecarte/they-work:latest ...
Error response from daemon: Head "https://ghcr.io/v2/giuseppecarte/they-work/manifests/latest": denied
~~~

It exited with status 1. This is the current release-state finding: the GHCR
package has not been made public and no semver release image has been published
yet. The public no-clone script URL was checked again from `/tmp` without
executing fetched content:

~~~text
$ curl -fsSL https://raw.githubusercontent.com/Giuseppecarte/they-work/main/docs/install.sh
curl: (22) The requested URL returned error: 404
~~~

After the first tag is published, the package is set Public, and the installer
is present on the default branch, rerun both probes. The intended successful
experience is a Docker-only, no-checkout launch with existing homes mounted
read-only or an empty office when neither home exists.
