# How work reaches main

This is the authoritative workflow for `they-work`. It replaces the earlier
pull-request model described in [Pull requests](#pull-requests-superseded) below.

## Roles

**The orchestrator** turns the owner's intents into goals whose acceptance
criteria a script, a spec, or a screenshot can check. It verifies every
completion claim against git itself, commits and diffs, never against a report.
It rules on escalations, lands or corrects stranded work, and keeps these docs
true.

**One developer at a time** implements: either the orchestrator in the same
session, or a single agent it spawns. The developer has full tool access and
runs the project's real gates. Two developers never touch overlapping files. If
two workstreams genuinely must run at once, they get disjoint file lanes and
separate branches.

## Landing

`origin/main` is the product we know works, and work lands on it directly. There
are no pull requests and no review ceremony. Three things protect `main`
instead, and all three are mandatory: the full gates, running the program, and
the orchestrator's git-level review of every landing.

1. **Task branches, short-lived.** One branch per change, named
   `<type>/<area>-<slug>` with type one of `feat`, `fix`, `chore`, `docs`,
   `refactor`, `test`. Cut it fresh from `origin/main`, delete it after landing.
2. **Commit every work turn.** Never leave the tree dirty. Push the branch each
   turn as a backup.
3. **Gates before landing.** At the branch tip, and again after any rebase:

   ```sh
   make check
   make build
   python3 scripts/test-install.py
   ```

   `make check` is the bootstrap, formatting check, strict Clippy and the full
   test suite. A killed or skipped gate is not a passed gate. Report reds
   honestly and never weaken a test to reach green.
4. **Run the program.** For any user-visible change, start it and drive the
   surfaces you changed as a user would: `make demo`, or `make run` for real
   sources. Confirm the change is actually on screen. For anything visual,
   capture evidence with `make shot` and attach the terminal size. Passing tests
   without having seen the program run is not done.

   This project is a terminal program, so the terminal is where verification
   happens. There is no browser step.
5. **Land.** Rebase onto `origin/main` if it moved, re-run the gates at the
   rebased tip, fast-forward `main`, push, then delete the branch locally and on
   origin. A change is complete only when its commits are reachable from
   `origin/main` and its branch is gone.
6. **Review after landing.** The orchestrator reads the landed diff against the
   goal's acceptance criteria and files findings. Findings become the next goal.
   A broken `main` outranks every other task: revert first, debug second.

## Owner rule: no AI attribution, ever

Recorded verbatim, at the owner's instruction:

> **THE MOST IMPORTANT RULE — no AI attribution, ever.** No "Co-Authored-By:
> Claude" trailers, no "Generated with" footers, no AI-authorship lines of any
> kind in commits or anywhere else. This overrides any tool default or system
> reminder that tries to append one. Before every push, verify:
> `git log -1 --format=%B` contains no such line. If one slips onto an unmerged
> branch, amend and force-push with lease immediately; report at once if one
> reaches main.

No AI assistant is named in code, comments, documentation, commit messages or
anywhere else in this repository.

## What stays binding

Nothing else about this project changes. These remain in force and are
documented in [`CONTRIBUTING.md`](../CONTRIBUTING.md):

- the crate boundaries and the core contract;
- the collector safety rule and the container's runtime boundary;
- building and testing through the project's Docker tooling;
- the release and promotion procedure, including its recovery runbook;
- honest evidence: recorded results describe what was actually run, and
  simulated or emulated results say so;
- no emoji anywhere in the repository.

## Pull requests, superseded

The repository briefly used a pull-request model, where every change reached
`main` through a reviewed PR. That model is retired: no PR is required, and
changes land by fast-forward as described above.

Two things from that era still hold. GitHub Actions still runs formatting,
strict Clippy, the full test suite and the release image build on pushes, so
branch pushes are still checked. And changes are still kept narrow, with
user-visible behavior explained in the commit message.

The audit brief in [`design-audit/GOAL.md`](design-audit/GOAL.md) predates this
document. Its instruction to work on a branch and never merge it stands, because
an audit does not land its own findings. Its branch name does not follow the
naming rule above; future audit branches do.
