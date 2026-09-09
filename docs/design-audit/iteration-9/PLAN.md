# Iteration 9: all four M findings

Implement DATA-01, DATA-02, WIN-01 and PERF-01 from iteration 7, against
the completed S baseline `77a9b78`. Preserve local operation, original art,
existing provider authority, temporary mode and additive compatibility.
REL-02's operation-ledger redesign and publication/user-study gates remain separate.

## Shared observation foundation

Use typed, independently merged coverage for numbered stream continuity, local
retention and tool correlation. Never add their counters together: missing
provider events, retired local records and uncorrelatable tool starts are
different units, and none is a count of unique missing deliveries or requests.
Ordinary source freshness updates must not erase another producer's loss facts.
Keep a persistent compact notice, with detailed coverage available in the work
panel and every notebook channel, including empty views.

## DATA-01 — stream gaps and missing actors

- Add optional stream-window metadata with serde defaults. A trusted stream has
  source identity, lineage, retained bounds and a high-water mark.
- Count exact missing sequence ranges only when the source metadata establishes
  them. Keep at most 16 range details with cumulative counts; unknown initial
  prefixes or changed lineages remain unknown.
- Defer observations with missing actor metadata within 256 events and 1 MiB of
  serialized event payload. Retire deterministically and disclose that separate
  loss. Do not create fictional workers or capabilities.
- Replay recovered/deferred evidence as historical activity, relationships and
  collaboration. Apply supported current roster state and pending requests last.
- Keep lineage through reconnects to the same host. Rotate and durably save it
  on host restart: event exposure can precede batched persistence, so reusing the
  old lineage could falsely claim continuity after a stale-disk recovery.
- Validate 300/3,000-event disconnects, legacy metadata, native/source mismatch,
  duplicates, absent actors, late material updates and collector reconciliation.

## DATA-02 — fair history in the existing 512-record budget

- Retain the project recorded with each collaboration event independently of
  the current worker roster. Preserve chronological iteration and identity
  upserts. Exact duplicates and older versions consume no new retention action.
- On overflow, retire the oldest record in the largest project partition. Tie
  by oldest source timestamp, project ID, then actor/event ID. A quiet project's
  only delivery survives a busy project's burst while capacity allows.
- Expose per-project retained count, oldest retained source timestamp, cumulative
  local evictions, last eviction observation ordinal and record timestamp.
- Bound project metadata to 512 windows too. Retire only empty windows, in
  observer-order; disclose unknown older counts if a retired project returns.
- Validate 1/20/>512 projects, missing/retired/moved workers, backfills,
  duplicate/upsert behavior, and empty-window wording. Reading or marking seen
  never prevents retention or approves provider work.

## WIN-01 — state replacement and fail-closed recovery

- Publish a synced, private, same-directory candidate through Windows
  `MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)`, without pre-delete or copy.
  Preserve Unix rename/directory-sync behavior.
- Require ordinary private owner files, valid durable snapshot structure and
  checked errors. Distinguish genuinely fresh storage from missing established
  state using the existing endpoint provenance.
- Serialize foreground/background writes. If replacement leaves canonical state
  missing or invalid, retain the synced candidate and latch recovery failure.
  Never promote a temporary file, restore older receipts or resend automatically.
- Show recovery errors in Connections and disable unavailable controls while
  preserving inspectable cached observations and drafts.
- Test sharing failures, concurrent readers, ACL/reparse boundaries, interrupted
  processes, uncertain acknowledgement and unchanged receipt identity.
- **Native Windows x64 and ARM64 CI are completion gates**, as selected by the
  owner. Cross-compilation and macOS fault injection do not satisfy them.

## PERF-01 — bounded Claude tool correlation

- Per cursor: 256 entries / 256 KiB owned strings. Per Claude source: 8,192
  entries / 8 MiB owned strings. Count owned ID copies; distinguish allocation
  capacity and process RSS from logical string bytes.
- Keep source-owned lookup plus synchronous observation-ordinal retirement.
  Identical duplicates do not refresh age; conflicting IDs lose attribution.
  Matched completions release entries and empty/sparse tables are compacted.
- Oversized or evicted starts retain current observed activity where available,
  but late results never acquire guessed activity, success or child identity.
  Preserve the launch-acknowledgement exclusion.
- Publish bounded epoch-scoped loss facts; rewrite/removal cleanup must not
  transfer one worker's losses to a different worker reusing its file path.
- Compare paired and unpaired synthetic replays and modest 1/8/32/128-overlap
  envelopes. Existing limits are engineering defaults, not user-study findings.

## Integration and evidence

Develop provider stream, Windows storage and Claude correlation independently;
integrate through shared coverage and fair retention. Run affected tests, then
formatting, all workspace tests and strict Clippy on the integrated candidate.
Retain bounded complete-screen captures at 32×14, 80×24, 120×36, 192×58, 110×80
and 240×70; verify a visible request is not displaced by coverage.

Keep generated caches under `target/audit/iteration-9/`, selected review evidence
here, and historical iteration 7/8 evidence unchanged. Distinguish model,
compositor, fake-provider process, synthetic RSS and native CI evidence.
Physical power loss, authenticated integrations, actual terminal paint latency,
cross-platform visual certification and participant studies remain not tested
unless separately executed and documented.
