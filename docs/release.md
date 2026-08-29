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
