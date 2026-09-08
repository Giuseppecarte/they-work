# History continuity and retention: bounded discovery

This audit reproduced four loss scenarios and confirmed four existing safeguards using eight synthetic cases. It did not change production code. These are behavioral observations, not a completed production fix or a long-running performance result.

**Read the [integrated dual-feed follow-up](DUAL-FEED.md) before ranking these findings.** It confirms that a real SQLite collector recovers durable relationship metadata and recent results; delivery loss persists on cold load when the result is outside the per-thread tail. The original eight-case evidence below is unchanged.

## Results

| Case | Independent expected log | Observed production behavior | Verdict |
| --- | --- | --- | --- |
| Consumer paused for 300 observations | Two results, one parent→child delegation, one pending approval | Snapshot cursor 1 resumes at 46; 44 observations are unavailable. Both people remain and approval is still pending, but both results and the relationship are absent from World. | Reproduced gap |
| Consumer paused for 3,000 observations | Same semantic facts, larger burst | Cursor 1 resumes at 2,746; 2,744 observations are unavailable. Same semantic loss and preserved pending approval. | Reproduced gap |
| Duplicate item and repeated snapshot | One result | One result remains. | Preserved safeguard |
| Newer then older version of one native item | Newer text remains | The older collaboration record does not replace newer evidence. | Preserved safeguard |
| Repeated child recipients and a self recipient | One parent→child edge | One edge, no self edge or duplicate delegation. | Preserved safeguard |
| Mismatched sender in tool item | No accepted relationship | No relationship accepted. | Preserved safeguard |
| Result arrives before its actor is in the snapshot roster | No fabricated actor; result can be recovered once actor is known | The bridge skips the item; advancing the cursor means a later roster alone does not recover it. Explicit replay of the still-retained event does recover it. | Reproduced at adapter boundary |
| Unread result in B, followed by 513 messages in A | B's result was never reviewed | The global 512-record collaboration ring expels B's result. B remains present, with its text beat still available, but no result classification or delivery row. | Reproduced retention loss |

The process cases use the actual `run_supervisor`, `ControlClient`, `snapshot_events`, `World`, `WorkBrief`, and `Workboard`. A small Python process substitutes only the provider. It reads a supplied JSONL scenario, acknowledges one synthetic start, and emits that scenario verbatim. The harness compares the complete emitted log to its input and checks the sequence count independently; expected result IDs and edge are declared before reading the output. There was exactly one `turn/start` and no approval reply per process case.

The absent-actor case is a public adapter input scenario, not evidence that a particular installed provider emits that ordering. The ordering and retention cases execute production in-process, without a provider. No production parser is copied into the oracle.

## What the user currently sees

The gap does **not** silently remove every warning. WorkBrief correctly says `Partial local history`, and the notebook says `Local history · 2 partial`. The missing information is the **specific, known sequence loss**: neither states that 44 or 2,744 observations were discarded. WorkBrief has zero team members and zero results after the gap, despite the supervisor retaining two people. The pending approval correctly remains `Approval needed`.

After B's result is evicted, the Deliveries panel says `No recorded deliveries yet. A quiet or finished turn alone is not a delivery.` The Changes panel says `No recorded changes since the previous visit. History may be incomplete.` B's raw text survives as a non-result record, so this is a loss of the delivery/evidence classification, not proof that every copy of the text was erased.

Actual native-cell output from `Workboard::draw`, at 120×32:

- [Paused consumer: Deliveries](evidence/gap-300-deliveries.txt)
- [Paused consumer: Changes](evidence/gap-300-changes.txt)
- [Quiet B before the flood](evidence/retention-b-before-deliveries.txt)
- [Quiet B after the flood](evidence/retention-b-after-deliveries.txt)
- [Quiet B: Changes after the flood](evidence/retention-b-after-changes.txt)

These are text-buffer exports, inspected for the exact copy above. They are not screenshots of a physical terminal and do not add a visual usability pass. WorkBrief model coverage, records, and team counts are captured in [results.json](evidence/results.json).

## Boundaries and reproducibility

Run from the repository root after coordinating the shared Cargo target:

```sh
python3 docs/design-audit/iteration-7/data/run.py
```

The standalone [Cargo manifest](Cargo.toml) uses an empty `[workspace]` and path dependencies. It does not alter membership or workspace dependencies. `Cargo.lock` fixes its dependency set. The script builds offline; it requires dependencies already available in the Cargo cache, Rust, and Python 3. It uses the available native toolchain or the repository's ignored audit toolchain. Set `CARGO_TARGET_DIR` to use another cache. `--skip-build` reruns the existing audit executable.

All provider homes, projects, emitted logs, private state and endpoint tokens are generated under ignored `docs/design-audit/tmp/iteration-7-data`. The runner launches the exact Python fixture path, never an installed Codex/Claude executable. The host process is killed and waited on by its owner after each case; the fixture exits when its stdin closes. Generated credentials are not copied into the published evidence. No personal store is read and no provider instruction or login is issued.

The first run was blocked by the workspace sandbox's local TCP restriction, before provider startup. [environment-block.log](evidence/environment-block.log) records that attempt. The successful repeat received permission for the loopback listener. This environment prerequisite is not a product bug.

Evidence provenance is in [metadata.json](evidence/metadata.json): source commit, hashes of exercised production files, runner and fake provider, executable hash, platform, and artifact hashes. [run.log](evidence/run.log) records completion; [build.log](evidence/build.log) records the build. A concurrent lightweight root-owned soak was running; this audit makes no latency, throughput or RSS claim.

Not tested here: recovery supplied by a collector when the same conversation is also observed from its source store; reconnect against live providers; long-running memory/disk growth; Windows runtime; real terminal rendering; restart durability beyond the existing supervisor API. In particular, the reproduced missed events describe the managed snapshot→World path in isolation, not guaranteed permanent loss when an independent collector later replays equivalent evidence.

## Proposed follow-up

The highest priority is explicit stream continuity and bounded retry for unresolved participants. The second is a fair, bounded local retention policy with per-project loss metadata. [Implementation briefs](BRIEFS.md) define the chosen behavior, interfaces, migration, failure handling and acceptance tests. Neither proposes a transcript archive or automatic replay of user instructions.
