# Repository audit — 2026-09-03

## Published state

The final remote check returned `f6eee2430efefb0e1e67949fb083366f90a08a88`.
Local `main` and `origin/main` pointed there, with zero commits ahead or behind.
There were no other local or remote branches. These are snapshot findings:
other developers committed and pushed during this audit.

Commands:

~~~sh
git ls-remote origin refs/heads/main
git rev-parse main origin/main
git rev-list --left-right --count main...origin/main
git branch -a -vv
git for-each-ref --format='%(refname) %(objectname)'
git status --short --branch
git diff --name-only --cached
git stash list
git diff --stat 'stash@{0}^1' 'stash@{0}'
~~~

The divergence result was `0 0`; no files were staged by this audit.
The pre-existing stash has eight changed files, 948 insertions and 135 deletions.
It is unique historical WIP, not a branch; retain it for its owner's review.

At handoff, `.dockerignore`, `INSTALL.md`, `docs/release.md`, and this report
are intentional source/documentation changes to commit, not ignore.
`crates/theywork-render/src/lib.rs` also changed concurrently; its owner must
review and commit it. This audit did not edit any crate or stage/commit files.
Earlier concurrent crate, golden, design-board, and image changes were included
by another process in `f6eee24`; they were not discarded by this audit.

## Fresh-clone proof

The initial published `e429b1c` failed plain `make check`: the Cargo wrapper
defaults to network isolation and its empty cache could not resolve/download
`serde`. An explicit locked fetch followed by `make check` passed.
The fix adds a networked `fetch` prerequisite, uses `fmt-check` instead of
mutating `fmt`, and orders the check prerequisites. A separate empty-cache
clone with only that Makefile patch passed and retained only its Makefile diff.
The fix was subsequently included in published `f6eee24` by another process.

The decisive, unmodified published-clone run was:

~~~sh
audit_clone=$(mktemp -d /tmp/they-work-m10-published.XXXXXX)
git clone ssh://git@github.com/Giuseppecarte/they-work.git "$audit_clone/repo"
cd "$audit_clone/repo"
git rev-parse HEAD
make check
git status --short
~~~

Actual directory: `/tmp/they-work-m10-published.tJ9INN/repo`.
Commit: `f6eee2430efefb0e1e67949fb083366f90a08a88`.
Result: exit 0; format check, strict Clippy, and all tests passed; final status
was empty. Test counts: collector unit 10, collector integration 18 passed/1
ignored, core 9, renderer 60, terminal image 8, TUI unit 10, CLI integration 15.
Doc tests also passed. The collector soak completed in 59.57 seconds.
Make reported a 0.1-second future timestamp/clock-skew warning; all recipes
ran and the clone remained unchanged.

An earlier harness invocation ran from `/tmp` instead of the clone and reported
no Makefile target; it was corrected and is not evidence about the repository.
The fresh-clone test used the existing Docker daemon/toolchain image but an
empty checkout-local Cargo cache. It was not a test of installing Docker itself.

## Tracked artifacts and history

Commands used:

~~~sh
git ls-files -ci --exclude-standard
git ls-files docs/shots
git log origin/main --oneline -- docs/shots
git rev-list --objects origin/main
git count-objects -vH
git ls-files | rg '\.(patch|diff|orig|rej|bak|tmp|log|zip|tar|gz|exe)$'
git log --all --format= --name-only | sort -u |
  rg '(^|/)(AGENTS\.md|CLAUDE\.md|\.agents|\.claude|\.codex)(/|$)'
git log --all --format='%H %B' --regexp-ignore-case --grep='co-authored-by'
~~~

- No currently tracked ignored files or patch, backup, archive, or log artifacts
  were found. `docs/shots/` is ignored and absent from the current index.
- Historical `docs/shots/` content remains: 42 blob versions totaling 21,860,464
  bytes before compression. Removing current paths does not erase old blobs.
  A coordinated history rewrite would be needed to eliminate that history;
  this audit did not rewrite published commits.
- `docs/office.png` and `docs/desk.png` are generated demo stills deliberately
  referenced by the README. Retain these curated illustrations.
- `docs/references/*.png` are also generated: `scripts/render-design.sh`
  renders the tracked HTML design boards. They are curated review references,
  not original drawing inputs. Retaining them supports checkout-time review;
  ignoring them is an optional repository-size trade-off, not a safe blanket
  deletion while concurrent visual work is underway.
- Renderer goldens are generated test fixtures and belong under version control.
- The ignored dependency cache contains a third-party `AGENTS.md`; it is not a
  project file, tracked path, or historical tracked path. No prohibited project
  configuration paths were found across all locally referenced history.
- Two disallowed credit trailers survived only in
  `refs/original/refs/heads/main`. Its tree matched rewritten `e429b1c` exactly.
  After approval, only that obsolete ref was removed with
  `git update-ref -d refs/original/refs/heads/main d0882545b0e5d74e7f7164146ef8dcac2376a156`.
  The final all-ref co-author scan returned no matches. No objects were pruned;
  old objects/reflogs may remain recoverable locally. The stash was preserved.

Docker's build context previously included the ignored Cargo cache and shots:
one build transferred 346.21 MB. `.dockerignore` now excludes both directories.
This does not delete them or alter the runtime image's file layout.

## Documentation command audit

The inventory covered `INSTALL.md`, `CONTRIBUTING.md`, and Markdown under
`docs/`, including fenced commands, inline flags, scripts, and environment
variables. Historical transcripts and example version tags are not current
installation guarantees.

| Surface | Evidence and finding |
| --- | --- |
| `--check` | Not implemented: runtime probe exited 2 with unknown-option error. Stale references in three documents were replaced with the existing `--doctor` and reported here. |
| `--doctor` | Help and CLI tests confirm read-only store diagnostics. Missing-home probe exited 1; tests cover one missing source, empty stores, unreadable stores, and fixture counts. |
| Other runtime flags | `--project`, `--all`, `--demo`, `--once`, `--headless`, `--exit-after`, `--view`, `--light`, `--dark`, `--color`, `--config-dir`, `-h`, and `--help` match runtime help and parser/CLI tests. |
| Build/check commands | `make help` confirms targets; full `make check`, `make build`, and interactive `make demo` passed. `make check` now bootstraps the cache; individual Cargo commands still need the documented initial fetch. |
| Screenshot/design commands | `make shot`, `scripts/shot.py`, and executable `scripts/render-design.sh` exist. They require Python and Chrome/Chromium for export; they are not Docker-only runtime requirements. The design script was syntax-checked, not rerun over concurrent reference edits. |
| Golden-update command | Matches the existing renderer test and wrapper environment forwarding. The matching test passed; update mode was not run because it writes another owner's fixtures. |
| Docker examples | Used flags exist. Network isolation retains loopback, so the former “no network interface” claim was corrected. Read-only roots, read-only data mounts, dropped capabilities, and no-new-privileges are separate controls. |
| Installer/version examples | `v1.2.3` is an example, not a verified published release. Shell syntax checks passed for the installer and both shell wrappers. |

Commands included `rg -n` inventories, `make help`,
`sh -n docs/install.sh scripts/cargo scripts/render-design.sh`, runtime
`docker run ... --help`, `--doctor`, and the intentionally rejected `--check`.
The runtime probes were isolated and did not mount personal data.

## Publication and failure handling

The earlier installer 404 was real, but external visibility changed during the
audit. At the final check, GitHub's unauthenticated repository API returned
`private: false`, and the raw installer download exited 0. The image probe
`docker manifest inspect ghcr.io/giuseppecarte/they-work:latest` still exited 1
with `denied`. This does not distinguish a missing package from restricted
visibility; anonymous installation is not yet proven. No publication settings
were changed by this audit.

The install/release docs now separate current observations from historical
transcripts. Their download form uses `set -e`, a temporary file, then `sh`;
it no longer hides a failing download behind a successful pipeline consumer.
A known-missing raw URL tested in that fail-fast form exited 22 and never ran
the following command. The installer also stops on image-pull failure.

## Final verification

- Live-worktree `make check`: exit 0, including full workspace tests.
- Published fresh-clone `make check`: exit 0 and clean status.
- `make build`: exit 0. A subsequent source-only rebuild reported the dependency
  build layer `CACHED` and compiled the real source without refetching crates.
- Interactive `make demo`: opened the demo office; `q` restored the terminal and
  exited 0. No personal data was mounted.
- `git diff --check`: passed at audit handoff.

Remaining owner decisions: publish/permit anonymous access to the runtime image,
review the preserved stash, and decide whether historical shot blobs warrant a
coordinated rewrite. Current source/documentation edits remain uncommitted by
this audit as required.
