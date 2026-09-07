# Task families and collaboration data

The data layer now preserves provider identity, observed relationships, final outputs, and source limitations. Collectors remain read-only. The demonstration includes a parent, two delegated workers, a message, a delivery with a concrete result, and a human question. These events are explicitly demo evidence.

## Contract for views and control adapters

- `ThreadIdentity` preserves provider, canonical source home, native ID, and optional session scope. `worker_id()` length-frames each component, avoiding collisions across providers, homes, and Claude sessions. Codex identities have no session scope; Claude roots have none and children carry their root session. A known identity is not permission to control a task. `WorkerId::native_id()` is only a display/legacy fallback.
- `Relationship` distinguishes delegation, session membership, and forks, with an evidence category and optional native correlation. Session membership never supplies an unobserved immediate parent. Structural cycles and duplicate links are rejected.
- `World::children` and `ancestors` expose structural relationships; `family` includes membership; `tree` produces stable unique rows with depth and relationship type, retaining missing endpoints as placeholders. Relationships do not create workers.
- `CollaborationEvent` has stable source-scoped actor identity, event ID, kind, optional recipient/body, turn/item IDs, correlation, and evidence. A missing recipient is unknown, not implicitly the user. `Result` means an observed output, not quiet activity or a completed status. `Completed`, `Failed`, and `Cancelled` are separate lifecycle observations.
- `World::collaboration_for` includes sent and received events. Removed workers stay available through `worker()` and `retired_workers()`, while `is_present()` distinguishes the live roster. Their deliveries and relationships survive removal. Bounds are 512 events, 512 retired workers, 2,048 relationships, and 64 activity beats per worker.
- `SourceCoverage` reports availability, incompleteness, relationship/message/lifecycle support, observation time, and an explanation. Fresh source observations, identities, relationships, and collaboration backfill never refresh a worker's activity clock or resurrect a retired seat. Unknown and stale coverage do not prove completion or an approval request.
- `WaitReason` separates human approval/input, automatic review, another agent, a process, and unknown waits. Automatic waits continue running until genuinely silent. Human requests block immediately. Older worker JSON defaults safely to unknown identity/coverage/lifecycle.

## Provider observations

Codex reads work subagents in the roster and explicit spawn edges, source spawn metadata, and optional fork metadata. Known approval assessors and internal guardians are excluded from both seats and links. Archived child endpoints can remain graph placeholders without reopening their seats. Supported typed collaboration records normalize spawning, messages, waits, and terminal statuses. Only `agentMessage` with `phase: final_answer` and native item identity yields a delivery; commentary and user messages do not. Current `collabToolCall` and the prior `collabAgentToolCall` shape are supported. The current protocol documents final-message phase and collaboration fields in the [official app-server reference](https://learn.chatgpt.com/docs/app-server).

Codex history polling combines the incremental stream with a bounded 64-item tail per visible worker. Native item content fingerprints detect finalization in place and additional items sharing the same timestamp. Missing optional columns degrade coverage rather than inventing IDs. A lost source reports unavailable coverage without manufacturing a completed turn.

Claude children keep root-session membership from transcript layout. Native `parent_tool_use_id`, explicit parent agent metadata, and correlated `Task`/`Agent` calls can establish immediate delegation. Late child discovery updates the original delegation event rather than duplicating it. A transcript without a native child ID gets a scoped filename fallback, never the parent's identity. Explicit `SendMessage` target IDs are preserved; names or broadcast text do not manufacture an endpoint. A tool result is tied to its native call; a background launch acknowledgement is not a delivery. Explicit completed child results keep child actor and parent recipient. An assistant end-turn/stop-sequence with a durable message ID and no tool use yields a final output. Missing final markers do not.

## Honest limits and compatibility risks

Local transcript/SQLite formats are not guaranteed stable APIs. Optional columns and known typed variants are probed; unknown records are ignored. Message coverage remains partial: history is bounded, archived transcripts may be absent, asynchronous metadata can arrive late, and local stores do not expose every live control request. No complete conversation or delivery is inferred from prose, file proximity, tool-name display labels, or silence. IDs from transcript filename fallback can migrate when stronger native metadata arrives; the old live seat is retired to avoid duplicate workers. Project path normalization is separate from source identity, including WSL paths. Reading local stores never grants control capabilities.

## Verification

On macOS aarch64, 65 active tests passed and the two opt-in personal-store smoke tests remained ignored. Synthetic fixtures cover nested/absent parents, duplicate edges, forks, same native IDs across sources/sessions/providers, legacy serialization, stale/unavailable sources, mutable same-timestamp items, retained deliveries, late joins, background acknowledgements, explicit result recipients, and internal-task exclusion. The read-only test compares database bytes before and after collection. Strict Clippy across core/collect targets passed; package-scoped rustfmt completed.

Evidence: [tests](evidence/data-tests.log), [Clippy](evidence/data-clippy.log). No personal stores or real provider processes were used for this verification. Other operating systems are not claimed by this lane.

Reproduce with the repository-local toolchain (or replace its path with an installed `cargo`):

```sh
env CARGO_HOME="$PWD/docs/design-audit/tmp/native-rust/cargo-home" \
    RUSTUP_HOME="$PWD/docs/design-audit/tmp/native-rust/rustup-home" \
    CARGO_TARGET_DIR="$PWD/target/native-macos" TMPDIR="$PWD/docs/design-audit/tmp" \
    docs/design-audit/tmp/native-rust/cargo-home/bin/cargo test \
    -p theywork-core -p theywork-collect --offline
```
