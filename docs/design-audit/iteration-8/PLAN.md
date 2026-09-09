# Small-item implementation plan

Date: 2026-09-09. Status: **four definite changes implemented and locally verified;
UX study prepared, participant validation not tested**. See the
[delivery report](README.md) and [acceptance results](VALIDATION.md).
Planning baseline: `03ee7b2` on `audit/design-and-usability`.

This batch covers every S-sized entry in iteration 7's
[roadmap](../iteration-7/ROADMAP.md) and [findings register](../iteration-7/findings.json):
four definite engineering/documentation changes, one ranked research item, and
one unranked research hypothesis. S means roughly 1–3 engineering days per
bounded change, excluding recruitment and external validation.

The goal is to make failures truthful, releases safer, evidence reproducible,
and instructions accurate before deciding whether another UI feature is useful.
Selecting this small batch does not lower the priority or close the larger
control-capacity, observation-gap, delivery-retention, Windows-state or memory
findings. Completing it will not make the project ready for publication.

## Sequence and scope

| Order | Item | Concrete delivery | Estimate | Dependency |
| --- | --- | --- | --- | --- |
| 1 | **REL-01** | A pre-send storage failure reports “Not sent,” preserves the draft and never silently retries | 1–2 days | None; independent of REL-02 |
| 1, parallel | **RELEASE-01** | Verify the exact multi-architecture candidate before promoting version/latest; rehearse failed and partial publication | 2–3 days | Disposable registry and Docker/Buildx/QEMU for runtime evidence |
| 1, parallel | **REPRO-01** | A clean-checkout command produces one complete screen and runs one existing data fixture | 1–3 days | Declared Rust/Python/font prerequisites |
| 2 | **DOC-01** | Installation, source persistence, polling and contributor instructions match the application | 1–2 days | Start now; finalize against REL-01 and REPRO-01, plus any changed release instructions |
| 3 | **UX-01** | Test returning to work; decide “no change” or evaluate one narrow prototype | Study effort separate; 1–3 days if a prototype is justified | Stable candidate and actual participants |
| 3, same study | **UX-02** | Test moving-worker attribution and decision vocabulary; record a separate decision | Shared study; 1–3 days if a prototype qualifies | Originally unranked; included in this approved batch |

Use a separate reviewable change for each of the four definite items. Keep
candidate images and audit outputs isolated from public releases and personal
conversation stores. With one engineer, the definite work is approximately
**5–10 engineering days**. Parallel execution may shorten elapsed time; review
and integration still need time. Do not present this as a promised calendar date.

Prepare study fixtures and the recruitment brief early. Contacting participants
requires the owner's arrangement or explicit authorization; this plan does not
send invitations. Run the baseline study on one fixed candidate after the
relevant changes pass. Do not change the UI between baseline participants.

## REL-01 — accurately reject work that was never sent

**Chosen behavior.** If saving the intent fails before provider execution, return
the existing `Rejected` status with a specific message:

> Not sent: local state could not be saved. Restore storage access and submit again.

Keep the operation ID and fingerprint. Retrying that ID in the live host returns
the same terminal rejection, including after storage access returns. Across a
restart, that guarantee requires the rejection to have reached disk; otherwise
preserve the conservative recovery behavior described below. A fresh, explicit
submission uses a new ID. Keep the recipient-bound draft available. Never
introduce an automatic resend.

**Implementation boundary.** Change the initial persistence branch in
`crates/theywork-control/src/supervisor.rs`. Correct the in-memory receipt and
attempt to persist that correction without concealing the original error or
claiming it became durable. Use private, per-instance failure injection for
tests. Inspect `crates/theywork-tui/src/control_host.rs`; it already retains drafts
unless confirmed, so change presentation only if the executed failure flow
cannot explain the rejection. Follow the existing
[implementation brief](../iteration-7/reliability/REL-01-BRIEF.md).

**Acceptance.**

- Inject a failure before writing: zero provider calls; initial response,
  snapshot and same-ID retry all report Rejected/Not sent.
- Restore storage access: the old ID remains rejected; an explicit new ID sends
  exactly once. A different payload with the old ID and concurrent same-ID
  clients cannot bypass the fingerprint or dispatch work.
- Exercise a file-size failure and an injected failure after rename but before
  directory sync. No provider call occurs. If a Sending record reached disk,
  restart still treats it conservatively as Uncertain and never replays it.
- Preserve the existing post-send uncertainty, lost-acknowledgement, restart,
  stale-request, stale-turn and concurrency tests.
- In a fake-provider PTY, show the recipient, rejection reason and preserved
  draft; explicit resubmission succeeds once.

No ledger generations, persistence schema migration, Windows replacement fix,
or reinterpretation of errors after provider execution belongs in this item.
The separate post-response “awaiting resolution” wording is not this defect.

## RELEASE-01 — promote only a verified image

**Chosen behavior.** Build and push once to a unique candidate reference. Verify
its immutable multi-architecture index digest. Only after success and native
archive checksum verification may the workflow advance version and latest.
Promotion must reuse that exact index without rebuilding it.

**Implementation sequence.**

1. Update `scripts/test-published-image.py` to use the supplied candidate digest
   and an explicit platform for pull and every run. Test both `linux/amd64` and
   `linux/arm64`, retain the existing locale, Kitty, iTerm2 and fallback checks,
   and require normal exit after `q`. Record the index, platform manifests and
   exit results. Label QEMU execution as emulated, not native ARM validation.
2. Change `.github/workflows/release.yml` so candidate publication and smoke
   verification precede public-tag promotion. Put promotion after downloading
   and checking native archives. Grant registry permissions only to the jobs
   that need them. Serialize the final promotion phase without cancelling an
   in-progress publication.
3. Add one small promotion helper and a recovery runbook. Copy the full index
   from a single `repository@digest` source with `imagetools create`; verify
   each destination resolves to that same digest, preserving both platforms
   and any attestation descriptors. Record commit, candidate digest, intended
   tags and prior latest digest. Retain that intent and verification evidence
   as a job artifact **before** the first public-tag mutation. Record subsequent
   completed steps when possible; recovery must inspect actual registry tags
   rather than trust a potentially incomplete step log.

The copy behavior is documented by
[Docker imagetools create](https://docs.docker.com/reference/cli/docker/buildx/imagetools/create/).
Use [index inspection](https://docs.docker.com/reference/cli/docker/buildx/imagetools/inspect/)
to check the result. A shared
[GitHub concurrency group](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#concurrency)
prevents these jobs from overlapping; it does not impose semantic version order
or exclude publishers outside this workflow.

**Failure contract and acceptance.** Rehearse against a disposable registry
with a real multi-architecture digest, never public version/latest tags:

| Scenario | Required result |
| --- | --- |
| Smoke or native checksum fails | Public version and latest are unchanged; candidate may remain |
| Successful smoke and promotion | Both tags resolve to exactly the verified index; both platform checks passed |
| Failure between version and latest | Report partial publication; retain the verified version; no automatic deletion/rollback |
| Retry with the recorded digest | Existing identical version is a no-op; absent version may be created; a conflicting version stops promotion |
| Latest already equals candidate, including a successful write with lost acknowledgement | Treat promotion as success/no-op after inspecting the registry |
| Latest differs from both the prior reference and candidate | Stop automatic recovery rather than overwrite a later publication |
| GitHub Release fails after promotion | Report verified images as published and native release as incomplete; document explicit recovery |

Keep failure injection deterministic and save the previous/next digests and
exit codes. Do not claim atomicity across GHCR and GitHub Releases. Registry
migration, signing, distributed rollback, and redesigning native distribution
would exceed S and must be scoped separately. If no disposable registry can be
run, source review alone does not close this item; record runtime validation as
not tested.

## REPRO-01 — a small portable evidence entry point

**Chosen behavior.** A contributor can bootstrap declared audit dependencies and
run one smoke command from a clean checkout without an ignored local toolchain,
an author's home directory, Menlo, browser automation or provider credentials.

**Proposed interface, to be implemented:**

```sh
python3 scripts/audit.py bootstrap
python3 scripts/audit.py smoke
```

- Use Rust 1.90.0 from the existing toolchain file and `Cargo.lock`, with normal
  supported build entry points. Preflight native Cargo and explain any missing
  dependency; do not silently install a different toolchain or replace the
  existing contributor build system.
- Use a local virtual environment and a pinned Python dependency manifest.
  Use a small redistributable monospace regular/bold font set, retaining its
  license and hashes. Make font selection explicit and record the actual font;
  never silently substitute a host font in a comparison.
- Reuse the existing `ui_inventory` capture path with an exact
  `tower-image-80x24` case and 8×16 cell geometry, fixed time and disabled motion.
  Compose image plus native text into a complete 640×384 screen, not an isolated
  sprite sheet. Label the result as a reconstructed compositor screen, not a
  real-terminal capture. Resolve the executable from Cargo's JSON output rather
  than a platform-specific target directory.
- Run the existing Codex collector fixture
  `codex_updates_same_timestamp_items_and_retains_removed_deliveries_without_writing_stores`.
  It supplies a bounded data check without authentication or live providers.
- Put generated stores, virtual environments, logs and screens under ignored
  `target/audit/`. Emit a manifest with source commit/dirty state, commands,
  tool versions, dependency/font hashes, geometry, output hashes and outcomes.
  A cache-miss/offline failure must explain the missing prerequisite.

The small implementation consists of `scripts/audit.py`, a font-configurable
`scripts/audit_replay.py`, the Python dependency lock and licensed font files,
and a current `docs/AUDIT.md` linked from CONTRIBUTING. Add only the bounded
smoke to native CI and retain its output as a job artifact. Establish Python
3.12 as the tested baseline and record Pillow/FreeType versions. Bootstrap may
fetch declared dependencies; smoke should run offline after preparation.

**Acceptance.** Execute the documented bootstrap and smoke path from a clean
checkout on macOS and Linux or WSL; list each actual environment. Verify the
fixture passes, exactly one test ran, exactly one expected full-screen image and
native-text capture exist, content/hit geometry is consistent, and no tracked
files change. A zero-match filter is a failure. Missing fonts or glyphs must not
activate an invisible fallback. Repeating on the same environment
must preserve expected facts and geometry; do not promise byte-identical font
rasterization across operating systems. Review the complete screen visually.

The S boundary is one screen, one existing fixture and a documented dependency
path. Porting the entire historical audit or native Windows PTY tooling is out
of scope. Keep iteration 7 evidence and its baseline validator unchanged; new
implementation results belong in this iteration. Retain only representative
screens, small logs and manifests. Do not rewrite Git history or delete earlier
evidence to reduce repository size.

## DOC-01 — make the first-use path truthful

**Chosen behavior.** Readers follow the current interface without guessing which
shortcut opens a modal, changes focus, changes floors or saves preferences.

Update `INSTALL.md` for `c/C → Connections → Local sources/provider login`,
`v → Advanced → Camera`, Tab/Shift-Tab semantic focus, and the contextual meaning
of `d` in source selection versus the office. Describe floor navigation in its
actual focus context. Verify `w/W` and `o/O` before keeping or removing promises.
Explain Remember and `--no-save` separately from source access and provider login.

Update `CONTRIBUTING.md` to remove the obsolete claim that `--config-dir` is the
only consent to save source selection. Mark `docs/project-selection.md` as a
historical specification and point to current instructions. Correct the Source
trait comment in `crates/theywork-core/src/source.rs`: the host polls sources
sequentially in the background and waits one second after each traversal; it
does not poll per rendered frame or guarantee one-second freshness. Check links
to `docs/CONTROLS.md` and update changed release/audit instructions as needed.

**Acceptance.** Follow each changed route in an isolated 80×24 keyboard-only
PTY, resetting modal state between cases. Verify Remember, temporary mode and
reopening without affecting personal preferences. Check documented commands
and links, and verify the REL-01 rejection text against the integrated build.
Preserve evidence of actual behavior; documentation checks do not prove a new
user will understand it. No shortcut redesign or redundant unit tests are
needed for text-only corrections.

## UX-01 and UX-02 — use the same study, make separate decisions

**Current evidence:** zero enrolled participants, zero completed sessions, zero
diaries. The existing [study kit](../iteration-7/study/README.md) is ready; do not
substitute an expert walkthrough for human results.

Recruit two primarily Codex users, two primarily Claude users and one who uses
both, all unfamiliar with they-work. Preserve the five 45-minute sessions,
counterbalanced Tower/compact/reduced-motion order, keyboard-only and resize
tasks, simulated requests and voluntary three-workday diary. Record unaided
performance before hints and distinguish first exposure from learned behavior.
Budget preparation and analysis separately; moderation plus ten minutes of
notes per participant alone takes about 4 hours 35 minutes. Recruitment and diary
elapsed time cannot be compressed into a 1–3 day prototype estimate.

| Item | Probe and evidence | Decision |
| --- | --- | --- |
| UX-01 | Use TASKS §3 for project interruption/return, §5 and diary for meaning/continuity. Record original task recovery, missed next steps, repeated navigation and confidence. A second visit clearing “Since your visit” is not itself proof of missed work. | In priority order: clarify wording when users confuse visits with reading; test a Not marked seen Deliveries filter when they understand the distinction but cannot locate those records; retain search context only for demonstrated search-recovery failure. If users understand the current model, close with no UI change. |
| UX-02 | Use tasks 1, 2 and 4 to identify a moving character and task, locate attention, and predict Seen/Review/Allow and the receipt after responding. | A demonstrated attribution failure may justify one following-marker treatment; a demonstrated decision-language failure may justify one wording treatment. Keep findings separate from REL-01. No new art suite or combined redesign. |

The iteration 7 register's return-study reference points to task 5; use task 3
as the primary return task here without rewriting the completed evidence record.

Advance a prototype only after two distinct participants encounter the same
underlying problem or one severe reproducible failure involving an unintended
decision, incorrect recipient or lost next step. Choose one cause per item;
do not bundle unread filters, query history and dashboards. For UX-02, prioritize
decision-language confusion over character attribution. A qualifying prototype
within S is already authorized; record its priority and scope. Preflight the
study launchers independently of the one-screen reproduction smoke.

Keep the baseline UI fixed through the five sessions and voluntary diaries.
Offer 20-minute follow-ups with equivalent fresh scenarios and alternating
old/new presentation order. Record learning effects. Require the qualifying
breakdown to disappear for affected participants without new recipient/action
mistakes before enabling a prototype by default; missing follow-up evidence
leaves it unvalidated. Preserve drafts, exact requests, record identities and
marker semantics. Inspection must never approve a request; any accidental
approval is a critical failure. Five participants provide qualitative evidence,
not population rates or certification.

Completion for each research item is a documented decision: no change,
supported prototype with results, or not tested because participants are missing.
The last state is a limitation, not a validated resolution. Do not label all S
items complete while the human validation remains unperformed.

## Review and completion record

For each change, retain its implementation commit, acceptance results, one
concrete before/after example, unresolved limits and a short critique under
`docs/design-audit/iteration-8/`. Keep a status table with planned, in progress,
passed, failed or not tested. Preserve original observations in iteration 7.

Run affected meaningful tests after each implementation and the normal
format/test/Clippy check on the integrated code candidate. Documentation-only
changes need route/command/link verification. Do not rerun the two-hour soak or
the full historical screenshot matrix without a changed path or new failure
that justifies it. Separate unit/compositor checks, executable PTYs, disposable
registry rehearsals and physical-terminal evidence in every report.

Review the scope after the first working fix/prototype. If an item needs a new
persistence schema, a new history service, a broader release architecture or a
complete UI redesign, stop expanding that item and create an explicit follow-up
estimate. Do not quietly turn an S task into an M/L task.

The definite engineering batch is complete when REL-01, RELEASE-01, REPRO-01 and
DOC-01 satisfy their acceptance checks. The complete S investigation additionally
needs evidence-based UX-01/UX-02 decisions. Authenticated-provider trials,
physical-terminal certification, and the outstanding M/L defects remain visible
publication requirements after this batch.
