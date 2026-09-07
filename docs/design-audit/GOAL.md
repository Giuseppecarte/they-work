# Audit goal — design fidelity, appearance, and ease of use

You are auditing `they-work`, a read-only terminal program that draws the AI
coding agents running on a machine as pixel-art workers in an office. You have
full code access and no history with the project, which is the point: everyone
who has worked on it has stopped seeing it.

This is the second audit. The first (`docs/audit-2026-09-04.md`) attacked
correctness and found ten defects by running the published artefact rather than
reading it. This one attacks **appearance, design fidelity and usability** — and
unlike the first, you are asked to *fix* what you find, not only report it.

---

## Working arrangement

- **Work on a branch.** Create `audit/design-and-usability` off `main`. Do not
  push to `main` and do not merge.
- **Use sub-agents.** Split the work — one per area below is a reasonable
  shape — and have them investigate in parallel. Then have your own agents
  implement the improvements you decide on, on the same branch.
- **Everything you produce lives in `docs/design-audit/`.** Findings,
  measurements, screenshots, decisions, and a record of what you changed and
  why. One reader with only the repository should be able to follow it.
- **Commit on the branch as you go**, so the work is reviewable in pieces rather
  than one wall of diff. Commit messages describe the change and its reason; no
  AI assistant is named anywhere in code, comments, documentation or commit
  messages.
- **Do not create files outside the repository.**

---

## The three questions

### 1. Why does it still look the way it looks?

Eight rounds of work have gone into the appearance. Characters were redrawn from
coloured rectangles into 24×34 figures with faces. The sign was fixed. The room
was composed. And it still does not look like the design it is copying.

Find out why, structurally rather than cosmetically. Some threads worth pulling,
none of them authoritative:

- `worker_size` returns `(14, 20)` at sextants while characters are authored at
  24×34 — a fractional squeeze of about 0.58, which the project's own rule
  forbids. What else is scaled that way?
- The manager and the workers occupy the same box but contain different amounts
  of body, so heads come out at different sizes.
- Sextants are gated on a `TERM_PROGRAM` whitelist that Windows Terminal is not
  on and does not set, so the primary author's terminal silently renders worse
  than it can.

Compare what is drawn against `docs/design/v2/` — the boards there are generated
from real pixel maps in `gen.py` and `cast.py`, so you can diff intent against
implementation rather than argue about a picture.

### 2. How faithful is the implementation to the proposed design?

Go surface by surface: the floor, the guard office, the desk, the phone, the
settings screen, the first-run screen, the help overlay. For each, say how close
it is, what is missing, and whether the gap is a defect, an unfinished piece, or
a limit of the medium.

Be specific about which gaps are worth closing. A design that cannot survive a
terminal is a design problem, not an implementation one, and saying so is a
finding.

### 3. Is it actually usable?

Nobody except the author has ever run this. Approach it as someone who has not:

- Install and start it by following the documentation exactly, from a clean
  environment, as a user who is not the one who built it.
- Can you tell, within ten seconds of it opening, who is working and who is
  waiting on you? That is the product's entire claim.
- Are the keys discoverable? Does the help overlay answer what you actually ask?
- What happens when there are no agents, one agent, twenty offices, a very long
  project name, a tiny terminal, a very large one?
- Do `--doctor` and `--once` tell a confused person what to do next?

---

## My blind spots — check these specifically

I have been the only reviewer of this project. These are the places I am most
likely to be wrong, and I would rather you attacked them than confirmed them.

- **I have never seen this program on a real terminal.** Every judgement I have
  made about appearance came from rasterising captured output. The sessions I
  work in report `xterm-256color`. If the thing looks different in a real
  window, I would not know.
- **I chose 24×34 as the character size and "author once, scale by integers" as
  the rule.** Both were reasoned from pixel budgets, not from looking at
  results. They may be wrong for terminals.
- **I decided the graphics protocol rung is the design target**, with character
  rungs degrading below it. That may be backwards — most people will run this in
  a terminal without image support.
- **I invented the colour law** (shirt hue is the agent, amber means blocked and
  appears nowhere else) and the ranked-slot wardrobe idea. Nobody has tested
  whether either actually helps a person read the screen.
- **I assumed the isometric office is the right metaphor.** The camera grid, the
  phone and the desk view all inherit that assumption. A plain list might serve
  the stated purpose better, and `--once` already prints one.
- **I have mis-assigned work twice** — telling one developer to build things
  already built, and putting a renderer defect in a packaging lane. The lane
  boundaries may be wrong.
- **I have optimised hard for "execute, do not read"** after that caught several
  real defects. That may have blinded me to problems visible only by reading —
  architecture, naming, whether the code is pleasant to work in.
- **No second person has ever used this.** Every usability claim in the project
  is untested.

Add blind spots I have not listed. Something obvious to a newcomer and invisible
to me is the most valuable thing this audit can produce.

---

## Method

**Run things.** Every serious defect this project has shipped was invisible to
code review and obvious within one execution: a container that could list agent
directories and open no files, an installer that 404'd while reporting success,
a published image that rendered in its worst mode for every user because the
container set no locale. Reading the code would not have found any of them.

Useful entry points, none prescriptive:

    make demo                      the imaginary company, reads nothing
    make run                       real agents, read-only mounts
    make shot                      renders every surface beside its design reference
    docker run ... --doctor        what it found and what to do about it
    docker run ... --once          every office and worker as plain text
    THEYWORK_ENCODING=sextants     forces the best character rung

`docs/audit-2026-09-04.md` has a reproduction harness worth reusing.

---

## Deliverables

In `docs/design-audit/`:

1. **`FINDINGS.md`** — what is wrong, ordered by consequence to a user. Each one
   reproducible from the document alone: what you did, what happened, what you
   expected, why it matters. Open with the three things you would fix first.
2. **`CHANGES.md`** — what you actually changed on the branch and why, including
   anything you decided *not* to change and the reason.
3. **Evidence** — screenshots, measurements, captured frames. Say how each was
   produced so someone else can reproduce it.
4. **The branch itself**, with the improvements implemented and the tests
   passing: `./scripts/cargo test --workspace`, and
   `./scripts/cargo clippy --workspace --all-targets -- -D warnings` clean.

Say plainly what you could not test and why. No macOS machine, no Kitty
terminal, no Windows Terminal graphics session has ever been available to this
project — untested is a finding, and untested reported as passing is how the
existing defects survived.

## What I do not want

No summary of the architecture. No list of what is done well. No roadmap. If an
area is fine, say "checked, found nothing" in one line and move on.
