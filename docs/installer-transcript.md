# No-clone installer: stranger transcript

Run on 2026-08-30 from the project-building host. The host build process was
UID 1000. The stranger container was a fresh `docker:27-cli`-based image with
`curl`, running as UID/GID 10001, with `/tmp` as its working directory and no
repository mount. The Docker socket was mounted only for the staged local
replay; the public README command had no repository or project-data mount.

## Fresh-container setup

This is harness setup, not a command a new user needs to know:

~~~text
$ docker build --pull=false -t they-work-m9-stranger:local -f- /tmp <<'EOF'
FROM docker:27-cli
RUN apk add --no-cache curl
RUN adduser -D -u 10001 stranger
USER 10001:10001
WORKDIR /tmp
EOF
#8 DONE 0.6s
WARNING: current commit information was not captured by the build: failed to read current commit information
~~~

The temporary image build exited 0. Its first probe confirmed the identity and
location:

~~~text
uid=10001(stranger) gid=10001(stranger) groups=10001(stranger)
/tmp
~~~

The product image was already built locally as `they-work:local`. The fixture
used for the failure replay had two Claude `.jsonl` files owned by UID 1000,
mode `0600`; the success replay used equivalent files owned by UID 10001,
also mode `0600`.

## 1. Exact README command

The first executable path in [README.md](../README.md) is the following
no-checkout command. It was run inside the fresh container exactly as written:

~~~text
$ docker run --rm -it --network bridge --user 10001:10001 --group-add 1001 -e HOME=/tmp/they-work-stranger-home -w /tmp they-work-m9-stranger:local sh -lc 'id; pwd; echo "--- README command ---"; curl -fsSL https://raw.githubusercontent.com/Giuseppecarte/they-work/main/docs/install.sh | sh; status=$?; echo "pipeline_status=$status"; exit "$status"'
uid=10001(stranger) gid=10001(stranger) groups=1001,10001(stranger)
/tmp
--- README command ---
curl: (22) The requested URL returned error: 404
pipeline_status=0
~~~

There was no installer prompt, Docker pull, application screen, or wait. The
outer container exited 0 because the final `sh` in the `curl | sh` pipeline
received no input; `curl` itself reported the 404. A new user would see the
error but could also receive a successful shell status.

The next live boundary was checked separately from the same kind of fresh
container, since the missing script prevented the README pipeline from
reaching it:

~~~text
$ docker run --rm --network bridge --user 10001:10001 --group-add 1001 -v /var/run/docker.sock:/var/run/docker.sock --workdir /tmp they-work-m9-stranger:local sh -c 'id; echo "--- image pull ---"; docker pull ghcr.io/giuseppecarte/they-work:latest'
uid=10001(stranger) gid=10001(stranger) groups=1001,10001(stranger)
--- image pull ---
Error response from daemon: error from registry: denied
denied
~~~

That pull exited 1. The public script is therefore not currently reachable,
and the public `latest` package is not currently pullable.

## 2. Installer body before the fix

To isolate the installer from those two external failures, the current script
was staged in `/tmp` and the pull operation was intercepted by a temporary
shim. The first staged attempt used `sh -lc` for the outer shell. That login
shell replaced the injected `PATH`, so the shim was bypassed:

~~~text
$ docker run ... sh -lc '... sh /tmp/they-work-m9-installer.ypz6AY/install.sh --once ...'
uid=10001(stranger) gid=10001(stranger) groups=1001,10001(stranger)
/tmp
--- staged installer body ---
Pulling they-work:local ...
Error response from daemon: pull access denied for they-work, repository does not exist or may require 'docker login'
installer_status=1
~~~

This was a harness mistake, not an installer result. I changed only the outer
shell to `sh -c`, retained the fresh UID, read-only mounts, and local pull shim,
and reran the body. Before the installer fix, the inner image ran as its
default UID 10001 while the fixture files belonged to UID 1000; it could list
the project directories but not read their contents.

## 3. Corrected bounded run

The installer now passes the invoking `id -u:id -g` to `docker run`. The
original UID-1000-owned fixture remains the safe-denial control recorded in
section 5. With the stranger fixture, the files belong to the invoking UID
10001 and remain mode `0600`. This is the corrected command and its complete
bounded output:

~~~text
$ docker run --rm -it --network none --read-only --user 10001:10001 --group-add 1001 --env HOME=/tmp/they-work-m9-stranger-owned/fixture/home --env PATH=/tmp/they-work-m9-stranger-owned/bin:/usr/local/bin:/usr/bin:/bin --env THEYWORK_IMAGE=they-work:local --env THEYWORK_CLAUDE_HOST=/tmp/they-work-m9-stranger-owned/fixture/claude --env THEYWORK_CODEX_HOST=/tmp/they-work-m9-stranger-owned/fixture/missing-codex --workdir /tmp -v /var/run/docker.sock:/var/run/docker.sock -v /tmp/they-work-m9-stranger-owned/fixture:/tmp/they-work-m9-stranger-owned/fixture:ro -v /tmp/they-work-m9-stranger-owned/install.sh:/tmp/they-work-m9-stranger-owned/install.sh:ro -v /tmp/they-work-m9-stranger-owned/bin/docker:/tmp/they-work-m9-stranger-owned/bin/docker:ro they-work-m9-stranger:local sh -c 'id; pwd; echo "--- corrected installer body ---"; sh /tmp/they-work-m9-stranger-owned/install.sh --once; status=$?; echo "installer_status=$status"; exit "$status"'
uid=10001(stranger) gid=10001(stranger) groups=1001,10001(stranger)
/tmp
--- corrected installer body ---
Pulling they-work:local ...
Using local test image: they-work:local
Codex home not found; continuing with Claude data only.
they-work --once
timestamp_ms=1788137761320
projects=2 workers=2
office=/tmp/alpha workers=1
  worker name="session-" agent=claude status=blocked activity=idle idle_age=4h 56m 1s tokens=0 waiting_on="alpha fixture"
office=/tmp/beta workers=1
  worker name="session-" agent=claude status=blocked activity=idle idle_age=4h 56m tokens=0 waiting_on="beta fixture"
installer_status=0
~~~

The process exited 0. Both private transcript files were read without changing
their ownership or mode.

The missing-home branch was also exercised in a fresh UID 10001 container:

~~~text
$ docker run --rm -it --network none --read-only --user 10001:10001 --group-add 1001 --env HOME=/tmp/they-work-m9-no-homes --env PATH=/tmp/they-work-m9-stranger-owned/bin:/usr/local/bin:/usr/bin:/bin --env THEYWORK_IMAGE=they-work:local --workdir /tmp -v /var/run/docker.sock:/var/run/docker.sock -v /tmp/they-work-m9-stranger-owned/install.sh:/tmp/they-work-m9-stranger-owned/install.sh:ro -v /tmp/they-work-m9-stranger-owned/bin/docker:/tmp/they-work-m9-stranger-owned/bin/docker:ro they-work-m9-stranger:local sh -c 'id; echo "--- no-home installer body ---"; sh /tmp/they-work-m9-stranger-owned/install.sh --once; status=$?; echo "installer_status=$status"; exit "$status"'
uid=10001(stranger) gid=10001(stranger) groups=1001,10001(stranger)
--- no-home installer body ---
Pulling they-work:local ...
Using local test image: they-work:local
No agent home found; starting the empty office.
they-work --once
timestamp_ms=1788138228057
projects=0 workers=0
installer_status=0
~~~

That path also exited 0 and did not create a host-side home.

## 4. Corrected interactive run

The same corrected installer was then run without `--once`. The terminal clear
and cursor-control bytes are omitted below; the visible frame is exact. The
frame was redrawn while the process waited for input, with no new prompt or
error:

~~~text
$ docker run --rm -it --network none --read-only --user 10001:10001 --group-add 1001 --env HOME=/tmp/they-work-m9-stranger-owned/fixture/home --env PATH=/tmp/they-work-m9-stranger-owned/bin:/usr/local/bin:/usr/bin:/bin --env THEYWORK_IMAGE=they-work:local --env THEYWORK_CLAUDE_HOST=/tmp/they-work-m9-stranger-owned/fixture/claude --env THEYWORK_CODEX_HOST=/tmp/they-work-m9-stranger-owned/fixture/missing-codex --workdir /tmp -v /var/run/docker.sock:/var/run/docker.sock -v /tmp/they-work-m9-stranger-owned/fixture:/tmp/they-work-m9-stranger-owned/fixture:ro -v /tmp/they-work-m9-stranger-owned/install.sh:/tmp/they-work-m9-stranger-owned/install.sh:ro -v /tmp/they-work-m9-stranger-owned/bin/docker:/tmp/they-work-m9-stranger-owned/bin/docker:ro they-work-m9-stranger:local sh -c 'id; pwd; echo "--- corrected installer interactive body ---"; sh /tmp/they-work-m9-stranger-owned/install.sh; status=$?; echo "installer_status=$status"; exit "$status"'
uid=10001(stranger) gid=10001(stranger) groups=1001,10001(stranger)
/tmp
--- corrected installer interactive body ---
Pulling they-work:local ...
Using local test image: they-work:local
Codex home not found; continuing with Claude data only.
THEY WORK — first run
A read-only terminal office for the agents already running here.

WHAT WAS FOUND
  Claude Code: found
    claude_home=found path=/data/claude
    claude_store=projects=2 threads=2 active=2
  Codex: missing
    codex_home=missing path=/data/codex
    codex_store=unavailable: home is not a directory

WHAT THIS READS
read=Claude Code data comes from regular .jsonl session files below ~/.claude/projects/; symlinks and non-JSONL files are skipped. Codex data comes from ~/.codex/sqlite/state_5.sqlite and ~/.codex/sqlite/thread_history_1.sqlite, opened read-only. The collectors inspect filesystem metadata and .git directory markers to group activity under a project root; they do not read project source files.
discovery_overrides=THEYWORK_CLAUDE_HOME=/data/claude THEYWORK_CODEX_HOME=/data/codex

PICK AN OFFICE
> office="alpha" path="/tmp/alpha" workers=1 status=blocked=1 failed=0 running=0 idle=0
  office="beta" path="/tmp/beta" workers=1 status=blocked=1 failed=0 running=0 idle=0

↑↓ choose   Enter open office   Tab guard office   q quit
~~~

Input: `q`

~~~text
installer_status=0
~~~

The interactive process waited at the picker until `q`; it did not require a
project selection to prove that the two private sessions had been discovered.

## 5. Direct permission boundary

For an independent read check, a fresh UID 10001 container invoked the product
image with the same read-only Claude mount:

~~~text
$ docker run --rm --network none --read-only --user 10001:10001 --group-add 1001 --env PATH=/tmp/they-work-m9-installer.ypz6AY/bin:/usr/local/bin:/usr/bin:/bin --workdir /tmp -v /var/run/docker.sock:/var/run/docker.sock -v /tmp/they-work-m9-installer.ypz6AY/fixture:/tmp/they-work-m9-installer.ypz6AY/fixture:ro -v /tmp/they-work-m9-installer.ypz6AY/bin/docker:/tmp/they-work-m9-installer.ypz6AY/bin/docker:ro they-work-m9-stranger:local sh -c 'id; echo "--- direct read-boundary check ---"; docker run --rm --network none --read-only --user 10001:10001 -v /tmp/they-work-m9-installer.ypz6AY/fixture/claude:/data/claude:ro --entrypoint /bin/sh they-work:local -c "id; find /data/claude/projects -mindepth 1 -maxdepth 1 -type d -printf \"%f\\n\" | sort; cat /data/claude/projects/alpha/session-alpha.jsonl"'
uid=10001(stranger) gid=10001(stranger) groups=1001,10001(stranger)
--- direct read-boundary check ---
uid=10001(watcher) gid=10001(watcher) groups=10001(watcher)
alpha
beta
cat: /data/claude/projects/alpha/session-alpha.jsonl: Permission denied
~~~

That failing control uses the old UID-1000-owned fixture. It demonstrates why
the identity pass is necessary; the corrected run above demonstrates that it
does not widen access when the invoking user owns the private files.

## Judgement

I hesitated once in the harness: `sh -lc` reset `PATH` and caused a misleading
registry pull. A normal user following the README would not encounter that
harness detail. The real hesitation is earlier and visible: the very first
command returns a raw-script 404, shows no prompt, and reports a zero pipeline
status. I would stop there. If I tried the documented image directly, the fresh
container receives the registry's `denied` response and I would stop there too.

The local release path is now correct for private data: the installer passes
the invoking UID/GID, the runtime retains `--network none`, `--read-only`,
dropped capabilities, no-new-privileges, and `:ro` mounts, and UID-owned `0600`
transcripts are ingested. End-to-end success from the README still depends on
publishing `docs/install.sh` on the default branch and making the GHCR package
public; the fresh stranger cannot reach that success while either external
condition remains false.
