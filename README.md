# they-work

> A virtual office for the AI coding agents already running on your machine.

You start three agents on a project and they scatter into separate threads.
`they-work` puts them back in one room: every thread is an employee at a desk,
typing, reading, editing, or waiting on you — drawn as pixel art in your
terminal. Every project you have open is another office, and one view is a wall
of camera feeds so you can watch all of them at once.

It reads agent transcripts and main database contents. It cannot start or stop
your agents, alter those records, or reach the network. On a writable native
SQLite store, SQLite may update an already-existing `-shm` coordination sidecar;
the container mounts stores read-only, and a cold WAL store without its existing
sidecars is refused rather than created.

![The office floor](docs/office.png)

## Start here

Requires Docker and an interactive terminal. No checkout, Git, Make, or Rust.
The first release contains a Linux/amd64 image; other architectures need Docker
emulation and have not been verified.

~~~bash
docker run --rm -it --network none --read-only --cap-drop ALL \
  --security-opt no-new-privileges -e TERM -e COLORTERM -e TERM_PROGRAM -e THEYWORK_ENCODING \
  ghcr.io/giuseppecarte/they-work:v0.1.0 --demo
~~~

This shows an imaginary company and **mounts nothing from your disk**. Press
`q` to quit. Docker downloads the image first; the running office has no external
network access. See the [verified installer procedure](INSTALL.md#without-a-checkout)
to use your own agent data, including checksum verification before running any
downloaded shell code.

If you prefer building from source, install Git and GNU Make as well:

~~~bash
git clone https://github.com/Giuseppecarte/they-work
cd they-work
make demo
make run
~~~

That mounts your agent directories read-only, with no network, and shows your
real projects.

### Look before you leap

Two commands print and exit, without drawing anything:

~~~bash
make build
docker run --rm --user "$(id -u):$(id -g)" --network none \
  -v "$HOME/.claude:/data/claude:ro" -v "$HOME/.codex:/data/codex:ro" \
  they-work:local --doctor
~~~

`--doctor` says which agents it found, where, how much they hold, and — when
something is wrong — what to do about it. Swap `--doctor` for `--once` to get
every office and worker as plain text, blocked ones first. Both are useful over
SSH, in a pipe, or when the office looks emptier than you expected.

## What you are looking at

| View | Key | What it is |
| --- | --- | --- |
| **The floor** | default | one project as an isometric office, a desk per thread |
| **The guard office** | `Tab` or `0` | every project at once, each behind its own camera pane |
| **A desk** | `Enter` | one worker up close, with their timeline |
| **The phone** | `p` | a messaging app: standup, blocked, shipping, watercooler |
| **Settings** | `s` | camera, light, theme, colour depth, motion |
| **Help** | `?` | every key |

Tabs across the top switch offices; `1`–`9` jump straight to one. A tab's dot
turns amber the moment anyone in that project is blocked, even while you are
looking somewhere else.

![A desk](docs/desk.png)

Colour means the same thing everywhere. **Shirt** is which agent — orange for
Claude Code, blue for Codex. The **bar under a name** is status: green running,
grey idle, amber blocked, red failed. Amber appears nowhere else in the
interface, so if you see it, someone is waiting on you.

## What it reads, and what you are agreeing to

The collectors have a narrow, read-only input boundary:

- **Claude Code** — regular `.jsonl` session files below `~/.claude/projects/`.
  Symlinks and non-JSONL files are skipped.
- **Codex** — the SQLite databases `~/.codex/sqlite/state_5.sqlite` and
  `~/.codex/sqlite/thread_history_1.sqlite`, opened in SQLite's read-only mode.

The main databases and transcripts are not changed. SQLite can coordinate
through an existing `-shm` sidecar when a native store is writable; the Docker
commands mount stores read-only, and the program refuses a cold WAL-mode store
whose required `-wal`/`-shm` sidecars are absent rather than creating them.

Those records contain your prompts, the commands your agents ran, file paths,
their messages back, thread titles, branches and token counts. All of it is
displayed on screen, so treat the terminal as having the same sensitivity as
those files. The collectors read filesystem metadata and `.git` markers to group
activity under a project root; they never read your project's source.

A missing agent is normal — the other one carries on. If a store lives somewhere
unusual, point `THEYWORK_CLAUDE_HOME` or `THEYWORK_CODEX_HOME` at it. Codex on
the Windows side while you run in WSL is common enough that discovery looks for
it under `/mnt/*/Users/*` on its own.

The flags in `make run` are deliberately visible:

- `--network none` — no external network connectivity; container loopback remains
- `--read-only` — the container filesystem cannot be written
- `--cap-drop ALL` and `--security-opt no-new-privileges`
- `:ro` on both agent mounts
- `--user` your own uid, so it reads exactly what you can read and no more
- `--rm` — nothing persists when you quit

Building the image uses Docker's normal network access to pull base images. That
is separate from the running program, which has none.

## What is inside

Five crates. `theywork-core` is the contract; the collectors and the renderer
both depend on it and neither depends on the other.

| Crate | Does |
| --- | --- |
| `theywork-core` | the domain model — offices, workers, activities, events, status |
| `theywork-collect` | read-only readers for Claude Code and Codex |
| `theywork-render` | the pixel canvas, sprites and views |
| `theywork-terminal-image` | Kitty and Sixel transport for terminals that can show images |
| `theywork-tui` | the binary: arguments, discovery, the frame loop |

Data flows one way. A collector tails a transcript or reads a database and emits
normalised events; `World` folds those into offices and workers; the renderer
draws whatever `World` currently says. Nothing downstream parses an agent's
format, and nothing upstream knows how anything is drawn.

Without a graphics protocol, the picture is built from **half-block, quadrant
or sextant characters**, whichever your terminal and font supports. At the
captured office region, that remains a 160×86 character canvas; it is not a
claim about the physical pixels displayed by every terminal.

When a graphics protocol is negotiated and the terminal reports its cell
geometry, the renderer makes one source pixel per physical terminal pixel in
the covered rectangle. Sixel, Kitty graphics, and iTerm2 inline PNG all receive
that renderer frame. For example, a 160×86 covered cell rectangle in a
terminal reporting 10×10-pixel cells produces a 1600×860 image. If the cell
geometry is unavailable, the program keeps the character renderer; the final
dimensions always come from the terminal report, not this example.

| Platform path | Status tested in this worktree |
| --- | --- |
| Windows Terminal under WSL, Sixel | Sixel encoding and the true-density renderer frame are covered by tests, but visual output was **not tested** here: this WSL session is `xterm-256color`, not Windows Terminal. |
| macOS, Kitty protocol | Kitty encoding and the true-density renderer frame are covered by tests, but visual output was **not tested**: no macOS/Kitty machine was available. |
| macOS, iTerm2 inline images | iTerm2 encoding and the true-density renderer frame are covered by tests, but visual output was **not tested**: no macOS/iTerm2 machine was available. |

To verify a real terminal, run the demo in that terminal and look for a clean
pixel image rather than the character fallback. The diagnostics and supported
protocol selection are automatic; no protocol flag is required. Please report
the terminal application, version, cell size, image dimensions, and whether
the image appears or falls back to characters.

The intended design for every surface lives in [`docs/design/`](docs/design) as
plain HTML you can open in a browser. Where the code and a board disagree, the
board is what was meant.

## Configuration

| Flag | |
| --- | --- |
| `--project <path>` | open one project |
| `--all` | start on the guard office |
| `--demo` | the imaginary company; reads nothing |
| `--doctor` | report what was found, then exit |
| `--once` | report every office and worker, then exit |
| `--view iso\|top\|side` | starting camera |
| `--light` / `--dark` | starting appearance |
| `--color auto\|true\|256\|none` | colour depth |
| `--config-dir <path>` | opt in to remembering settings |
| `--headless --exit-after <dur>` | run the loop without a terminal |

| Variable | |
| --- | --- |
| `THEYWORK_CLAUDE_HOME` | where Claude Code's data is |
| `THEYWORK_CODEX_HOME` | where Codex's data is |
| `TERM_PROGRAM` | forwarded to retain iTerm2's older capability fallback |
| `THEYWORK_ENCODING` | override character art: `sextants`, `quadrants`, or `half-blocks` |
| `THEYWORK_COLOR` | force a colour depth |
| `NO_COLOR` | honoured above everything else |

Nothing is written to disk unless you pass `--config-dir`.

## Working on it

~~~bash
make check   # format, clippy, and the full test suite
make shot    # render every surface beside its intended design
~~~

`make shot` writes `docs/shots/index.html`, which puts each rendered view next to
its design reference. That comparison is how visual changes get reviewed — the
frames are generated rather than stored, so what you open is always the code you
have checked out.

You do not need Rust installed; `scripts/cargo` runs the toolchain in a
container as you. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT
