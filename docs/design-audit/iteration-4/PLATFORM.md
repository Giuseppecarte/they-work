# Platform and packaging audit

The new control crate now builds in the Linux release image and runs its offline
process suite in the development image. This audit used Linux ARM64 containers
on the local macOS host. It did not install a real provider, sign in, submit a
model turn, publish an image, or install an executable outside the checkout.

## Defects found and corrected

The runtime Dockerfile caches dependencies with an explicit list of workspace
manifests. Its list and stub loop omitted `theywork-control`; a real build failed
with a missing workspace-member manifest. Both now include the crate. The
[before log](platform-container-before.log) and
[successful build](platform-container-build.log) preserve the evidence.

The development image had no Python, so eight control fixture tests failed
before starting their fake providers. Python 3 is now a development dependency;
it is not included in the runtime image. The
[failed run](platform-linux-control-before.log),
[image rebuild](platform-dev-image.log), and
[successful control suite](platform-linux-control-tests.log) show the boundary.
An existing `they-work-dev:local` image must be rebuilt after this change;
`scripts/cargo` only builds that tag when absent.

Review also found that a new live provider connection could make older,
disconnected managed threads appear available. The bridge now checks each
thread's disconnected state as well as the connection. The process regression
test explicitly verifies unavailable coverage for the old thread and available
coverage for the newly created thread, without resuming the old task.

## Executed checks

| Check | Result and evidence |
| --- | --- |
| Linux architecture | `linux/arm64`, [image inspection](platform-linux-target.log) |
| Offline control processes | 15 reported tests passed: 13 functional cases plus two process-entry helpers; includes detached supervisor persistence, uncertain sends, scoped approvals, history-only reconnect and native Ctrl-C in a real PTY. [Log](platform-linux-control-tests.log) |
| Strict Linux Clippy | Entire workspace and all targets passed with `-D warnings`. [Log](platform-linux-clippy.log) |
| Native installer harness | 11 passed in Linux and 11 on macOS, with mocked releases and temporary destinations under `target/installer-tests`. [Linux](platform-linux-native-installer.log), [macOS](platform-native-installer-tests.log) |
| Docker installer harness | 10 passed in Linux and 10 on macOS, including download/checksum failures and PTY boundaries. Docker and downloads are mocked. [Linux](platform-linux-docker-installer.log), [macOS](platform-docker-installer-tests.log) |
| Local runtime image | Release build includes `theywork-control`. `--help`, `--demo --once` and bounded headless demo passed with no network, a read-only root filesystem, all capabilities dropped and no-new-privileges. [Build](platform-container-build.log), [help](platform-container-help.log), [demo](platform-container-demo.log) |
| Packaging script | Existing script smoke-tested and archived the locally built `aarch64-unknown-linux-gnu` executable, plus LICENSE and SHA256. Output stayed under `target/platform-linux/dist`. This is a glibc test artifact, not a published musl release. [Log](platform-linux-package.log) |
| Windows control compilation | `x86_64-pc-windows-gnu` strict Clippy, including all compilable test targets, passed from macOS. Unix process fixtures are excluded on Windows. [Log](platform-windows-control-clippy.log) |

The installer logs deliberately contain error messages for rejected fixtures;
their unittest summaries are the success criteria. The before logs deliberately
retain the two reproduced failures.

## CI coverage and remaining limits

`Cargo.toml` includes control as a workspace member and the TUI depends on it.
The required Docker CI runs workspace formatting, strict Clippy and tests;
there is no package whitelist that excludes control. The native workflow does
the same before building `--bin they-work` and invoking the packaging script.
Its declared targets are Linux x86_64/ARM64 musl, macOS x86_64/ARM64, and Windows
x86_64/ARM64 MSVC. The release workflow requires those native jobs and separately
builds both Linux container architectures.

This review inspected those workflow definitions; it did not run GitHub Actions
or the complete six-target matrix. Local execution here proves Linux ARM64
glibc and the recorded macOS checks. Windows runtime ACLs, detached processes,
console restoration, the PowerShell installer, Windows ARM64 and MSVC builds
remain unexecuted locally. The GNU cross-check does not prove those MSVC jobs.
Linux musl and x86_64 release execution also remain for CI.

The default Docker distribution remains an observation path: it has read-only
source mounts and no host provider executable. Native installation is needed
for provider login, managed control and official console handoff. Successful
fixture tests do not certify a user's installed Codex/Claude version or any
real paid-turn outcome. See [control limits](CONTROL.md).

## Reproduction

Run from the checkout with Docker available. The Cargo wrapper uses offline
networking after its dependencies have been cached:

```sh
docker build -f docker/Dockerfile.dev -t they-work-dev:local .
THEYWORK_TOOLCHAIN=docker ./scripts/cargo test -p theywork-control -- --test-threads=2
THEYWORK_TOOLCHAIN=docker ./scripts/cargo clippy --locked --workspace --all-targets -- -D warnings
docker build -f docker/Dockerfile -t they-work:iteration4-platform .
docker run --rm --network none --read-only --cap-drop ALL \
  --security-opt no-new-privileges they-work:iteration4-platform --demo --once
python3 scripts/test-native-install.py
python3 scripts/test-install.py
```

The runtime test uses a local tag deliberately. It is not a substitute for the
release workflow's published-image digest and graphics-protocol verifier.
