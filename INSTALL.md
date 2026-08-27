# Installing they-work

There are two ways in. The container is the recommended one: it needs nothing on
your machine but Docker, and it enforces the read-only guarantee for you.

## With Docker (recommended)

Requires Docker. Nothing else is installed on your system — the Rust toolchain
exists only inside the build layer and is thrown away.

```bash
git clone https://github.com/OWNER/they-work
cd they-work
make demo
```

`make demo` shows an imaginary company and reads nothing. When you are happy:

```bash
make run
```

`make run` mounts your agent directories read-only and runs with no network.

### Running it by hand

If you would rather not use the Makefile, this is exactly what `make run` does:

```bash
docker build -f docker/Dockerfile -t they-work:local .
docker run --rm -it --network none -e TERM -e COLORTERM \
  -v "$HOME/.claude:/data/claude:ro" \
  -v "$HOME/.codex:/data/codex:ro" \
  they-work:local
```

Every flag is there for a reason:

| Flag | Why |
| --- | --- |
| `--network none` | The container has no network interface at all. |
| `:ro` on both mounts | The kernel refuses writes to your agent directories. |
| `--rm` | Nothing persists after you quit. |

## From source, without Docker

Requires a Rust toolchain (1.83 or newer).

```bash
git clone https://github.com/OWNER/they-work
cd they-work
cargo run --release --bin they-work -- --demo
```

Run without `--demo` to watch your real agents. Outside the container, the
read-only behaviour is a property of the code rather than of the sandbox, so
prefer the container if that distinction matters to you.

## Configuration

`they-work` finds your agent directories on its own. Override them if they live
somewhere unusual:

| Variable | Default |
| --- | --- |
| `THEYWORK_CLAUDE_HOME` | `/data/claude` in the container, otherwise `~/.claude` |
| `THEYWORK_CODEX_HOME` | `/data/codex` in the container, otherwise `~/.codex` |

## Troubleshooting

**The office is empty.** `they-work` only shows agents that have been active
recently. Start a Claude Code or Codex session and it will appear within a
second or two. `make demo` confirms the renderer works.

**The pixel art looks like mush.** Your terminal needs truecolor. Check that
`COLORTERM=truecolor` is set and use a modern terminal.

**Docker cannot see `$HOME/.codex`.** If you run Codex on Windows and
`they-work` in WSL, your agent directories may be on the Windows side. Point the
environment variables at the real location.
