# Workflow comparisons and outside signals

Research date: 2026-09-08. These are comparisons of first-party documentation,
not hands-on certifications of the other products. No third-party application
was installed, signed into, or used to start work. The Codex introduction is a
dated product description; the Claude and Agent Deck pages are changing documents.
Their published behavior is a research input, not a compatibility guarantee for
an installed provider version.

## The same jobs, compared

| Job | Codex app | Claude agent view | Agent Deck | They-work question |
|---|---|---|---|---|
| Keep parallel work organized | Project-organized tasks and worktree isolation | Cross-project background-session table, with project filtering | Session groups, search, and worktree support | Does a floor make project context easier to recover than a grouped list? |
| Understand current work | Task conversations with changes available for review | State rows and a peek panel for current output, questions, or results | Running/waiting/done overview and attachment to a session | Does the work brief expose enough evidence before opening the original conversation? |
| Intervene deliberately | Feedback and change review in the task | Reply in peek, or attach to the full console | Explicit session attachment and separate creation controls | Can a person predict whether an input starts, continues, acknowledges, or approves work? |
| Resume context | Existing CLI/IDE history and project/task context | Background sessions persist while away; attached sessions provide a recap | Search, groups, archive, and native session forking | Are Since your visit, unread deliveries, and selected identity sufficient across interruptions? |

Sources for the three product columns: [Codex introduction, published 2026-02-02](https://openai.com/index/introducing-the-codex-app/),
[Claude agent-view documentation](https://code.claude.com/docs/en/agent-view),
and [Agent Deck's maintained repository README](https://github.com/asheshgoplani/agent-deck).

Agent Deck additionally documents a useful form convention: advancing a field
does not create a session; creation has a separate explicit shortcut. Its README
describes this as a change in v1.9.55. This is a comparison input for the existing
they-work F5 send convention, not evidence that they-work should adopt its keys.
[Source](https://github.com/asheshgoplani/agent-deck)

Claude's documented row age and waiting duration describe different things.
They-work's observation age is another distinct measure. Copying a label or
timer from another product without preserving its meaning could make the
information less trustworthy. Test participants' interpretation of each timer
before adding more of them. [Source](https://code.claude.com/docs/en/agent-view)

## Current issue reports: signals, not prevalence estimates

Each row is one outside report. A report does not reproduce a they-work defect,
establish current product behavior, or estimate how many users are affected.
No counts of comments, reactions, or duplicates are treated as independent users.

| Source and reported date | Signal and retrieved status | Experiment it motivates here |
|---|---|---|
| [Codex #29008](https://github.com/openai/codex/issues/29008), 2026-06-19 | Open feature request for approval notifications when the app is out of view, including deduplication | Diary: record requests missed while the office is backgrounded. Consider notifications only if participants report a recurring need. |
| [Codex #27875](https://github.com/openai/codex/issues/27875), 2026-06-12 | Open report of notifications failing on older tasks while desktop state remains available | Test repeated task lifecycles and reconnection. A refreshed screen must not substitute for continuity of evidence. |
| [Codex #38986](https://github.com/openai/codex/issues/38986), 2026-08-17 | Open report of internal approval-review tasks appearing as duplicate user tasks | Keep fixtures separating user workers, delegation children, and internal processes. Match identities before counting workers. |
| [Claude #58259](https://github.com/anthropics/claude-code/issues/58259), 2026-05-12 | Closed report that backgrounding an interactive session injected a continuation interpreted as consent | Verify that inspecting, attaching, marking seen, and restoring the terminal send no affirmative answer or new instruction. |
| [Claude #75456](https://github.com/anthropics/claude-code/issues/75456), 2026-07-07 | Open documentation report about composer preservation after an unavailable command | Check drafts through unsupported actions, resize, and return; document the tested behavior rather than relying on old instructions. |
| [Claude #75465](https://github.com/anthropics/claude-code/issues/75465), 2026-07-07 | Open report of focus escape sequences appearing after attachment | Include focus events, mouse release, and terminal restoration in owner-run console validation. |

## Product opportunities to test, not preselected features

1. **Reorientation:** preserve enough context to answer “what changed while I was
   elsewhere?” without reopening every task. This could mean better coverage
   information or navigation rather than another inbox.
2. **Action vocabulary:** establish whether users distinguish seeing an alert,
   reviewing a result, and granting approval before changing labels or placement.
3. **Quiet supervision:** determine whether the tower earns its screen space
   through recognition and timely attention. Compare identical facts with the
   compact roster and reduced motion; record mistakes as well as speed.
4. **Recoverable control:** make transport/state failures legible and reversible.
   Product comparisons do not justify copying another tool's automatic retry or
   resume behavior into they-work's explicit-send contract.

Remote orchestration, another provider, a cost dashboard, new avatar accessories,
and automated approvals are not selected roadmap items merely because another
tool offers them. They need observed user demand and their own implementation
and authority decisions.
