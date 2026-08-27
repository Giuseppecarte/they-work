# they-work

A virtual office for your AI coding agents.

You start three agents on a project and they scatter into their own threads.
`they-work` puts them back in one building: every thread is an employee at a
desk, typing, reading, editing, or drinking coffee, drawn as pixel art in your
terminal. Every project you have open is another office, and the top-level view
is a bank of security-camera feeds so you can watch all of them at once.

It reads. That is all it does. See [What it can and cannot do](#what-it-can-and-cannot-do).

## Quick look, no setup

```bash
make demo
```

That builds the container and shows an imaginary company. It reads nothing from
your machine, so it is a safe way to see what the thing is before pointing it at
your real work.

## Watch your actual agents

```bash
make run
```

## What it shows

| In the office | Is really |
| --- | --- |
| A floor | A project directory an agent is working in |
| An employee at a desk | One Claude Code session or one Codex thread |
| Typing at the keyboard | The agent is running a shell command |
| Reading a folder | The agent is reading a file |
| Hammering | The agent is editing a file |
| Staring into space | The model is reasoning |
| Coffee break | The thread has gone quiet |
| The camera grid | All of your projects at once |

## Supported agents

- **Claude Code** — reads the session transcripts under `~/.claude/projects/`.
- **Codex** — reads the thread database under `~/.codex/`.

Both are things the tools already write to your own disk. `they-work` does not
ask either agent for anything, and neither agent knows it is being watched.

## What it can and cannot do

This is a toy that watches your development environment, so it is worth being
precise about what you are installing.

**It can:** open agent transcript files for reading, and draw them.

**It cannot:** write to those files, write anywhere else, reach the network, or
start or stop your agents. The container runs with `--network none`, drops all
capabilities, mounts your agent directories `:ro`, and runs as a non-root user.
The read-only guarantee is enforced by the container, not by good intentions.

If you would rather not take that on faith, the collectors are the only code
that touches your disk and they live in one small crate:
[`crates/theywork-collect`](crates/theywork-collect).

## Installing

See [INSTALL.md](INSTALL.md). The short version is that Docker is the only
requirement and nothing lands on your system outside the image.

## Layout

```
crates/
  theywork-core/     domain model: offices, workers, activities, events
  theywork-collect/  read-only readers for Claude Code and Codex
  theywork-render/   half-block pixel-art canvas, sprites, and views
  theywork-tui/      the binary, which is only wiring
```

`theywork-core` is the contract. `collect` and `render` both depend on it and
neither depends on the other.

## License

MIT
