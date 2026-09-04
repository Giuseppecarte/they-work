# v0.1.0 release and install verification

Status: release published, Docker-only README demo verified, and the approved
fresh-container installer trial completed successfully as UID 10002.
Documentation and diagnostic changes remain uncommitted as requested.

## Release

Commands and output:

~~~text
$ git ls-remote origin refs/heads/main refs/tags/v0.1.0
f100965df74f48a5a852fcf79e83f90fbbd409b7 refs/heads/main
$ git tag v0.1.0 f100965df74f48a5a852fcf79e83f90fbbd409b7
$ git push origin refs/tags/v0.1.0
To ssh://github.com/Giuseppecarte/they-work.git
 * [new tag]         v0.1.0 -> v0.1.0
~~~

The GitHub CLI was not logged in. Public API polling followed this exact run,
without authenticating or changing repository/package settings:

~~~sh
curl -fsSL 'https://api.github.com/repos/Giuseppecarte/they-work/actions/workflows/release.yml/runs?per_page=1'
curl -fsSL https://api.github.com/repos/Giuseppecarte/they-work/actions/runs/33824108031
curl -fsSL https://api.github.com/repos/Giuseppecarte/they-work/actions/runs/33824108031/jobs
~~~

[Run 33824108031](https://github.com/Giuseppecarte/they-work/actions/runs/33824108031),
attempt 1, succeeded. It started at `2026-09-04T01:01:10Z` and completed at
`2026-09-04T01:03:13Z`. The build/publish step ran from `01:01:27Z` through
`01:03:08Z`. No release-workflow repair or rerun was performed.
One initial formatting command failed because `jq` was unavailable; subsequent
queries used the raw API response and `rg`. That was not a workflow failure.

~~~text
$ docker buildx imagetools inspect ghcr.io/giuseppecarte/they-work:v0.1.0
Name:      ghcr.io/giuseppecarte/they-work:v0.1.0
MediaType: application/vnd.oci.image.index.v1+json
Digest:    sha256:b8bf5a70b41ceafcc3331fd790c411e3fd808d3541074dff75ec36858f8ba214
~~~

Both `docker manifest inspect ghcr.io/giuseppecarte/they-work:v0.1.0` and the
same command for `latest` succeeded anonymously. Their runnable Linux/amd64
manifest is `sha256:c0e82c59fb8169fda150e6fb34bc7d4f64fcded9d415e6441a419b07e3390334`.
The second manifest is a provenance attestation. There is no native ARM image.

Before publication, the version manifest returned `denied`. After publication,
both real tags are readable, and a deliberately missing tag returns `not found`.
The package is now published and public; it is not a restricted package.
An arbitrary `denied` response by itself still cannot establish whether a
different package is missing or private.

## Docker-only README trial

This was run against the published image, not a local build. It is independent
of the then-blocked nested-installer trial; that separate proof is recorded
in the completed approved trial below.

Verbatim command:

~~~sh
script -q -e --log-out /tmp/they-work-m11-readme.typescript --log-timing /tmp/they-work-m11-readme.timing -c 'docker run --rm -it --network none --read-only --cap-drop ALL --security-opt no-new-privileges -e TERM -e COLORTERM ghcr.io/giuseppecarte/they-work:v0.1.0 --demo'
~~~

The complete raw output, including ANSI screen updates and every wait interval,
is in `/tmp/they-work-m11-readme.typescript` (12,045 bytes) and
`/tmp/they-work-m11-readme.timing` (2,483 bytes) on the audit host.
Byte-exact portable copies are retained as
[terminal output](evidence/v0.1.0-readme.typescript.b64) and
[timing](evidence/v0.1.0-readme.timing.b64). Base64 preserves carriage returns
and terminal control bytes without embedding them in a readable document.
Decoded copies were compared with the originals using `cmp`; both matched.
These are demo evidence, not a substitute for the fresh-installer transcript.

Decoded SHA-256 values:

~~~text
ba671645466fc942a318f5566c34b7fcb26435d1cb6549426352cd77da2fa4c7  terminal output
ae00e60199902c2fe7ebbff2ff1dbd5867bde93a677f02c1648391486c00dade  timing
~~~

The recording began at `2026-09-03 19:06:55-06:00` in an 80×24 terminal and
ended at `19:07:17-06:00`, with `COMMAND_EXIT_CODE="0"`.

The image was not initially present. Docker downloaded it, printed the index
digest above, and opened `FLOOR / checkout`, `/home/dev/checkout`, three workers,
with `checkout`, `infra`, and `website` tabs. At this terminal size it used the
compact view. Input was `q`; the terminal restored and the command exited zero.
No mounts, installer script, or host Docker socket were passed into this runtime.

## Stranger installer: initial approval boundary

The proposed test used a fresh `they-work-m9-stranger:local` container as
UID/GID `10002:10002`, an empty `/tmp/stranger-home`, no checkout, and no personal
data. It needed the host Docker socket so the installer could launch its runtime.
Auto-review rejected that host-control exposure before execution.

A read-only checksum check proved that the pinned public installer and the
fully reviewed tagged source were identical:

~~~text
$ curl -fsSL https://raw.githubusercontent.com/Giuseppecarte/they-work/v0.1.0/docs/install.sh | sha256sum
2a665a28b75d9fa22f07b7ae8aa686a3bfd6e263309eb45b522cd8a9221fa2d4  -
$ git show v0.1.0:docs/install.sh | sha256sum
2a665a28b75d9fa22f07b7ae8aa686a3bfd6e263309eb45b522cd8a9221fa2d4  -
~~~

The retry enforced this hash before script execution but was also rejected
because the socket still grants host-level Docker control. Neither rejected
command ran. No alternative route around that restriction was attempted.
Explicit approval for this exposure was subsequently received. The completed
trial below used the approved socket access, not a workaround.

## Deliberate failure tests

First, the original weakness was reproduced:

~~~text
$ head -n 3 docs/install.sh | sh; printf 'truncated_script_status=%s\n' "$?"
truncated_script_status=0
~~~

The documented bootstrap now checks the complete pinned installer hash before
execution. A valid-shell prefix fails that check rather than silently succeeding.
The release fixture is frozen at `scripts/fixtures/install-v0.1.0.sh`; tests do
not need network access, repository tags, or a full Git history.

~~~text
$ python3 scripts/test-install.py
Ran 6 tests
OK
~~~

All six named cases passed: failed download, valid-shell truncation, complete
release reaching Docker and preserving pull failure, missing image, ambiguous
denial, and other Docker errors. Download/truncation tests assert Docker never
runs. The three pull classifications preserve the mocked Docker exit 17.
These are deterministic boundary tests, not substitutes for real registry probes.

The actual documented download block was also run with only its release URL
changed to the deliberately nonexistent `m11-intentionally-missing` ref:

~~~text
curl: (22) The requested URL returned error: 404
Installer download failed; nothing was executed.
exit=1
~~~

A real image failure through the revised local installer:

~~~text
$ THEYWORK_IMAGE=ghcr.io/giuseppecarte/they-work:m11-intentionally-missing sh docs/install.sh
Pulling ghcr.io/giuseppecarte/they-work:m11-intentionally-missing ...
Error response from daemon: failed to resolve reference "ghcr.io/giuseppecarte/they-work:m11-intentionally-missing": ghcr.io/giuseppecarte/they-work:m11-intentionally-missing: not found
Image pull failed: image or tag not found: ghcr.io/giuseppecarte/they-work:m11-intentionally-missing
exit=1
~~~

The diagnostic improvements are uncommitted local changes; they are not in the
already-published v0.1.0 installer. The documented checksum intentionally pins
that existing release, not uncommitted code. Its Docker errors already propagate
nonzero; the newer script adds clearer classification.

## Documentation corrections and remaining scope

- README now starts with Docker-only demo, not Git/Make; source-build requirements
  explicitly include GNU Make. The command was executed as shown.
- Installer downloads are pinned and checksum-verified. Duplicate unchecked
  current-install blocks were removed from release documentation; old transcript
  commands remain labeled as historical evidence.
- Network wording now says no external connectivity, not no network interface.
- The no-home live path does not open an empty office. The published runtime
  shows a first-run screen with setup guidance and waits for input; `q` exits 0.
  `--once` with no homes instead reports zero projects/workers and exits 0.
- The release trigger is a `v*.*.*` glob, not a semantic-version validator.
- The first image is Linux/amd64 only; other architecture support is not claimed.
- CI now runs the six installer regression tests. This CI change is uncommitted
  and has not run on GitHub; the original release run above was unchanged.
- Concurrent renderer edits were left untouched. Compact rendering and first-run
  UI behavior belong to the crate owner; no crate changes were made here.

No files were staged or committed by this work. The tag push was explicitly
requested. The approved fresh-container proof and portable transcript are now
recorded below. Pending source/documentation changes are ready for the owner
to review and commit; the published tag was not moved or rewritten.

## Approved stranger trial — complete

A new container used UID/GID 10002:10002, with supplemental group 1001 only
for the approved host Docker socket. It had no checkout or personal data mounts.
The test image supplies Docker CLI, curl, sha256sum, and a shell; its writable
home was created empty at /tmp/stranger-home. The existing Docker daemon/cache
was shared, so this is not a clean-daemon or Docker-installation test.

The command follows the checksum-verified INSTALL.md block, with exactly its
documented no-store variation, `sh "$installer" --demo`. The outer setup and
`set -x` are the recording harness, not extra application installation steps.
No credentials were mounted or supplied.

Exact launch command (wrapped by `script -q -e --log-out
/tmp/they-work-m11-approved.typescript --log-timing
/tmp/they-work-m11-approved.timing -c`):

~~~sh
docker run --rm -it --user 10002:10002 --group-add 1001 -e HOME=/tmp/stranger-home -w /tmp -v /var/run/docker.sock:/var/run/docker.sock they-work-m9-stranger:local sh -lc 'set -x
id
pwd
mkdir -p "$HOME"
(
  set -e
  installer=$(mktemp)
  trap '\''rm -f "$installer"'\'' EXIT
  if ! curl -fsSL https://raw.githubusercontent.com/Giuseppecarte/they-work/v0.1.0/docs/install.sh -o "$installer"; then
    echo "Installer download failed; nothing was executed." >&2
    exit 1
  fi
  if ! printf '\''%s  %s\n'\'' '\''2a665a28b75d9fa22f07b7ae8aa686a3bfd6e263309eb45b522cd8a9221fa2d4'\'' "$installer" | sha256sum -c -; then
    echo "Installer verification failed: truncated or modified download; nothing was executed." >&2
    exit 1
  fi
  sh "$installer" --demo
)

status=$?
echo installer_status=$status
exit "$status"'
~~~

The complete byte-exact recording is saved as
[terminal output](evidence/v0.1.0-stranger.typescript.b64) and
[timing](evidence/v0.1.0-stranger.timing.b64). Both decode with `base64 -d`;
comparison against the original files passed. The timing file preserves waits,
including the download, registry request, startup, and interactive interval.
The only application input was `q`, after observing the running office.
No confirmation prompt or error occurred.

Decoded SHA-256 values:

~~~text
8963bd75fa9537d734d6c3e80aa8b4c75654663c432b9db7d5c88ba38a8d330a  terminal output
4cdbb84dc11854d2cbb0143100910a8531567632185ff1a7fe8be55342296810  timing
~~~

The recording spans 2026-09-03 19:26:22–19:26:39 -06:00. It records:

~~~text
uid=10002 gid=10002 groups=1001,10002
/tmp
/tmp/tmp.nMbhEA: OK
Pulling ghcr.io/giuseppecarte/they-work:latest ...
latest: Pulling from giuseppecarte/they-work
Digest: sha256:b8bf5a70b41ceafcc3331fd790c411e3fd808d3541074dff75ec36858f8ba214
Status: Downloaded newer image for ghcr.io/giuseppecarte/they-work:latest
ghcr.io/giuseppecarte/they-work:latest
No agent home found; starting the empty office.
~~~

The last line is the original v0.1.0 installer wording, preserved verbatim;
it is inaccurate for demo mode and is corrected in the local script change.
The application then opened `FLOOR / checkout`, three workers, with checkout,
infra, and website tabs and changing worker states. At 80×24, the display was
compact rather than the large pixel-art layout. The full ANSI screen is retained,
not replaced with a reconstructed screenshot. After `q`, the terminal restored,
the downloaded temporary script was removed by its trap, and the harness printed:

~~~text
+ status=0
+ echo 'installer_status=0'
installer_status=0
+ exit 0
Script done on 2026-09-03 19:26:39-06:00 [COMMAND_EXIT_CODE="0"]
~~~

The runtime and test container used `--rm` and exited. No host data was removed.
The pulled public image remains in the Docker cache. This closes the separate
stranger-install proof using the published installer and image; uncommitted
local code was not substituted for either.
