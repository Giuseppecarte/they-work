# Installing they-work

The container is the recommended route. It keeps the Rust toolchain in the
build image and gives the running program the read-only, no-network boundary
described below.

## Without a checkout

After a version tag is published, Docker is the only host requirement:

~~~bash
curl -fsSL https://raw.githubusercontent.com/Giuseppecarte/they-work/main/docs/install.sh | sh
~~~

The script pulls `ghcr.io/giuseppecarte/they-work:latest`, mounts existing
`~/.claude` and `~/.codex` directories read-only, and skips either home that is
missing. It runs the image as the invoking UID/GID, so private `0600`
transcripts remain readable without widening access. Pin a release when the
image must not change underneath you:

~~~bash
curl -fsSL https://raw.githubusercontent.com/Giuseppecarte/they-work/main/docs/install.sh \
  | THEYWORK_IMAGE=ghcr.io/giuseppecarte/they-work:v1.2.3 sh
~~~

Use `THEYWORK_CLAUDE_HOST` and `THEYWORK_CODEX_HOST` to point at host paths
outside the usual locations. The installer uses the same `--network none`,
`--read-only`, `--cap-drop ALL`, `--security-opt no-new-privileges`, and `:ro`
mount policy as the local command below. The release process is documented in
[`docs/release.md`](docs/release.md).

## From a checkout

Requires Docker and GNU Make for the convenience targets. Rust is not required
on the host.

~~~bash
git clone https://github.com/Giuseppecarte/they-work
cd they-work
make demo
~~~

`make demo` builds a local release image and shows a deterministic office. It
mounts no agent directories. To watch local data:

~~~bash
make run
~~~

`make run` mounts `~/.claude` at `/data/claude` and `~/.codex` at `/data/codex`,
both with `:ro`.

### Running the local image by hand

This is the real-agent runtime command used by the Makefile:

~~~bash
docker build -f docker/Dockerfile -t they-work:local .
docker run --rm -it \
  --user "$(id -u):$(id -g)" \
  --network none \
  --read-only \
  --cap-drop ALL \
  --security-opt no-new-privileges \
  -e TERM -e COLORTERM \
  -e THEYWORK_CLAUDE_HOME=/data/claude \
  -e THEYWORK_CODEX_HOME=/data/codex \
  -e THEYWORK_COLOR -e NO_COLOR \
  -v "$HOME/.claude:/data/claude:ro" \
  -v "$HOME/.codex:/data/codex:ro" \
  they-work:local
~~~

| Setting | Effect |
| --- | --- |
| `--network none` | The application has no network interface. |
| `--read-only` | The container root filesystem is read-only. |
| `--cap-drop ALL` | Linux capabilities are dropped. |
| `--security-opt no-new-privileges` | The process cannot gain additional privileges. |
| `:ro` on both mounts | The kernel refuses writes to agent data. |
| `--user` your own UID/GID | Not root, and no wider than your own account: it reads exactly the files you can read. |
| `--rm` | The stopped runtime container is removed. |

The `make demo` target uses the same security flags but omits both mounts. The
image build itself may use Docker's normal access to download base images; the
running program has no network interface.

## Selecting a project

The new interface has one selected office floor and a camera grid containing
all discovered projects. Use:

~~~text
they-work --project <path>
~~~

The path can be relative to the process working directory or absolute. Resolve
it to the nearest enclosing Git root when applicable, normalize Windows/WSL
spellings, and match it to the collector's normalized project identity.

Without the flag, the current directory wins when it is a discovered project;
otherwise a picker lists discovered projects. Dismissing or being unable to
show the picker falls back to the full camera grid. With no discovered projects,
show an empty grid and the setup hint. `Tab` switches between the selected floor
and grid; movement keys select; `Enter` opens the selected project or desk; and
`Esc`/`Backspace` returns to the parent view.

The complete startup, switching, path, and persistence contract is in
[`docs/project-selection.md`](docs/project-selection.md).

### Optional remembered selection

No preference is read or written by default. This keeps the normal container
truly zero-write; the trade-off is repeating `--project` or a picker choice.

The opt-in form is `--config-dir <path>`. It reads and writes only
`<path>/project`, a single normalized path. In a read-only container, the user
must explicitly add a read-write bind mount:

~~~bash
mkdir -p "$HOME/.config/they-work"
docker run --rm -it \
  --network none --read-only --cap-drop ALL \
  --security-opt no-new-privileges \
  -v "$HOME/.claude:/data/claude:ro" \
  -v "$HOME/.codex:/data/codex:ro" \
  -v "$HOME/.config/they-work:/config:rw" \
  they-work:local --config-dir /config
~~~

An explicit `--project` wins for the current run. An unwritable explicit config
directory is an error, not a reason to use a hidden fallback. This is the only
additional write permission and is the cost of remembering a selection.

## What is read

Claude Code data comes from regular `.jsonl` session files below
`~/.claude/projects/`; symlinks and non-JSONL files are skipped. Codex data comes
from `~/.codex/sqlite/state_5.sqlite` and
`~/.codex/sqlite/thread_history_1.sqlite`, opened read-only.

The records may contain prompts, commands, file paths, agent messages, thread
titles, branches, token counts, and status metadata. That activity and message
text is displayed on screen. The collectors inspect filesystem metadata and
`.git` directory markers to group project roots, but do not read project source
files. Missing homes are skipped and the other collector continues.

For homes outside the usual locations, mount the host path read-only and set the
matching in-container variable. For example, if Codex data is on the Windows
side while Docker runs in WSL:

~~~bash
docker run --rm -it \
  --network none --read-only --cap-drop ALL \
  --security-opt no-new-privileges \
  -v /mnt/c/Users/PC/.codex:/data/codex:ro \
  -e THEYWORK_CODEX_HOME=/data/codex \
  they-work:local
~~~

The path on the right of the mount is the value the program must receive. With
short `-v` syntax, Docker can create a missing host path as an empty directory;
use an existing path or long `--mount` syntax when you want a missing path to
fail instead.

## Setup check before the interactive view

The planned `--check` mode performs one read-only collector scan without raw
mode, an alternate screen, or any rendering. It prints both homes and counts:

~~~text
claude_home=found path=/data/claude
codex_home=missing path=/data/codex
projects=2
workers=6
~~~

Exit `0` means at least one home was available, `1` means neither home was
available or a collector failed, and `2` means invalid arguments. The local
image command will be:

~~~bash
docker run --rm \
  --network none --read-only --cap-drop ALL \
  --security-opt no-new-privileges \
  -v "$HOME/.claude:/data/claude:ro" \
  -v "$HOME/.codex:/data/codex:ro" \
  they-work:local --check
~~~

`--check` is a documented CLI contract for the crate owner and is not
implemented by this packaging change. See
[`docs/project-selection.md`](docs/project-selection.md#non-rendering-setup-check)
for acceptance details.

## Build, test, and configure

The containerized toolchain is pinned to Rust 1.90 and requires no local Rust:

~~~bash
./scripts/cargo fmt --all -- --check
./scripts/cargo clippy --workspace --all-targets -- -D warnings
./scripts/cargo test --workspace
make build
~~~

The release Dockerfile copies workspace manifests and stub sources before the
real source tree, so source-only edits reuse the dependency layer. If Rust is
already installed, `cargo run --release --bin they-work -- --demo` is possible,
but it does not provide Docker's isolation boundary.

Configuration variables:

| Variable | Values |
| --- | --- |
| `THEYWORK_CLAUDE_HOME` | Claude root; `/data/claude` in the container. |
| `THEYWORK_CODEX_HOME` | Codex root; `/data/codex` in the container. |
| `THEYWORK_COLOR` | `none`, `true`, or `256`; unknown/unset values use terminal detection. |
| `NO_COLOR` | Any presence forces monochrome and overrides `THEYWORK_COLOR`. |

Without a forced setting, `COLORTERM=truecolor` or `24bit` selects truecolor;
otherwise the renderer falls back to the 256-color palette.

~~~bash
THEYWORK_COLOR=none make run
NO_COLOR=1 make run
~~~

See [CONTRIBUTING.md](CONTRIBUTING.md) for crate boundaries, read-only source
rules, and the containerized development workflow.

## Troubleshooting

**The grid is empty.** Confirm the configured homes are visible inside the
container. A missing home is skipped; `make demo` is independent of both homes.

**The image cannot see a Windows-side home.** Use the host path syntax accepted
by the Docker daemon, mount it at `/data/claude` or `/data/codex`, and set the
corresponding `THEYWORK_*_HOME` variable to that right-hand path.

**The colors look wrong.** Try `THEYWORK_COLOR=true`, `THEYWORK_COLOR=256`, or
the reliable monochrome fallback `THEYWORK_COLOR=none`/`NO_COLOR=1`.
