# Project selection and setup contract

Each project is one office floor. The camera grid is the only multi-project
view. A project is identified by the normalized repository root used by the
collectors, not by a display name or by an individual agent session.

This document is an implementation contract for the CLI and collector owners.
It does not change a crate by itself.

## Startup selection

Add one optional argument:

~~~text
they-work [--project <path>] [--config-dir <path>] [--demo]
~~~

`--project` accepts one path. It may be relative to the process working
directory or absolute. Resolve relative paths against the process working
directory, normalize `/`, `\\`, `.`, and `..`, and apply the same Windows/WSL
normalization already used for collector office IDs. If the path is inside a
Git worktree, resolve it to the nearest enclosing directory containing `.git`.
Compare that normalized root with the normalized `OfficeId` produced by the
collectors.

An explicit project has these rules:

1. Apply it to both collectors before their first poll through the existing
   path-filtering contract. A child working directory must match its enclosing
   selected root.
2. Open that project as the single office floor. Do not populate unrelated
   projects in the floor view.
3. If no current data matches it after the first scan, keep the requested path
   visible as an empty floor and show a clear `no recent activity` notice. A
   malformed argument is a CLI error; an empty project is not.
4. If more than one normalized office would match, choose the exact root and
   report the normalized path in the header so the choice is auditable.

## No-flag behavior

After the first collector scan, use this deterministic precedence:

1. Resolve the process current directory to a normalized repository root. If
   it is one of the discovered projects, open that office floor.
2. If the current directory is not a discovered project and at least one
   project exists, show a picker. List each project with a stable short name
   and its full normalized path. Sort by normalized path and keep that order
   between frames. `Enter` chooses the highlighted project.
3. If the picker cannot be shown or is dismissed, show all discovered projects
   in the camera grid. With zero projects, show an empty camera grid and the
   setup hint rather than inventing a selection.

The picker is a selection step, not a second data source. It must not write a
preference unless `--config-dir` is explicitly present.

## Switching projects

The selected office floor is the normal single-project view. `Tab` opens the
camera grid; arrows and `h`/`j`/`k`/`l` move its selection; `Enter` opens the
selected project floor. `Esc` or `Backspace` returns to the parent view.

Inside a floor, the same movement keys select desks and `Enter` opens desk
detail. The help overlay must list these bindings, along with `p` for the
phone overlay and `?` for help.

The camera grid must retain every discovered project even when one project is
selected at startup. Switching therefore never requires restarting the
collectors or rescanning a different agent home.

## Remembering a choice

The default is option (a): do not persist. Without `--config-dir`, startup
selection is determined only by `--project`, the current directory, the picker,
and the camera grid. The default container remains read-only and writes no
preference. The cost is repeating `--project` or making the same picker choice
on the next run.

The recommended opt-in extension is option (b): `--config-dir <path>`. It has
to be explicit because the runtime root is read-only:

- With `--config-dir /config`, read one UTF-8 file at `/config/project` if it
  exists. Its contents are one normalized project path followed by an optional
  newline.
- Apply the remembered path only after an explicit `--project` has been ruled
  out. A missing or stale remembered path falls through to current-directory,
  picker, and camera-grid behavior.
- When the user confirms a project in the picker or camera grid, replace the
  file atomically. An explicit `--project` wins for the current run and may
  update the file when `--config-dir` is also supplied.
- If the directory is not writable, report the error and do not silently use a
  hidden fallback. Without `--config-dir`, do not read a config file at all.

The user must opt into the only extra write permission with a read-write bind
mount, for example:

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

This trades the default zero-write guarantee for a narrowly scoped config
directory that contains a project path, not agent transcripts. That trade is
visible in the command and reversible by removing the mount and flag.

## Non-rendering setup diagnosis

The implemented `--doctor` diagnostic mode:

- resolves and reports both configured agent homes;
- reports `found` or `missing` for each home without treating one missing
  source as fatal;
- inspects each store read-only and reports project, thread, and active counts;
- reports owner and permission details for unreadable paths; and
- exits without entering raw mode, creating an alternate screen, or drawing.

Use stable key/value lines so the command is useful in a shell and readable by
a person:

~~~text
claude_home=found path=/data/claude
codex_home=missing path=/data/codex
claude_store=readable projects=2 threads=2 active=2
codex_store=unavailable reason="home is not a directory"
~~~

Exit status is `0` when at least one home exists and every existing home is
readable, `1` when neither exists or an existing home is unreadable, and `2`
for invalid arguments.

The local-image command is:

~~~bash
docker run --rm \
  --network none --read-only --cap-drop ALL \
  --security-opt no-new-privileges \
  -v "$HOME/.claude:/data/claude:ro" \
  -v "$HOME/.codex:/data/codex:ro" \
  they-work:local --doctor
~~~
