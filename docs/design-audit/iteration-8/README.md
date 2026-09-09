# Iteration 8: complete the small improvements

**Four definite changes are implemented and locally verified. UX-01 and UX-02
have a prepared, preflighted study but no participant evidence; neither is closed.**
Work remains on `audit/design-and-usability`; this batch does not publish a release.
The [approved plan](PLAN.md), [acceptance record](VALIDATION.md) and
[critical review](CRITIQUE.md) separate implemented behavior from pending validation.

| Item | Delivered | Status and evidence |
| --- | --- | --- |
| REL-01 | Pre-dispatch storage failures return Rejected/Not sent, preserve the operation and draft, and never automatically resend. The composer displays the complete recovery instruction. | Local acceptance passed. [Control evidence](control/REL-01.md), [visible flow](docs/REPORT.md#receipt-comparison-and-correction). |
| RELEASE-01 | Build one candidate index, verify both platforms and native checksums, retain intent, then promote that exact digest with conflict-aware recovery. | Local registry and PTY acceptance passed; public publication not tested. [Report](release/RELEASE.md). |
| REPRO-01 | `scripts/audit.py bootstrap` and `smoke`, locked dependencies, bundled licensed fonts, complete screen and exact collector fixture, macOS/Linux CI jobs. | Isolated prepared macOS/Linux candidate checkouts passed; Linux smoke ran with networking disabled. [Report](repro/REPRO.md). |
| DOC-01 | Current source, navigation, temporary-mode, polling, control, reproduction and release contracts; historical proposal marked accordingly. | Final executable keyboard routes passed at 80×24. [Report](docs/REPORT.md). |
| UX-01 | Fixed-baseline study, return-context probes, diary and conditional prototype rules. | Prepared; zero participants/diaries/follow-ups. [Study status](study/status.json). |
| UX-02 | Identity and decision probes, one-prototype limit, equivalent follow-up fixtures and default-activation gate. | Prepared; no qualifying evidence, no prototype enabled. [Gate](study/GATES.md). |

## Review and try

The implementation preserves production APIs and persisted schemas, provider
authority boundaries, local operation and temporary mode. Changes were committed
in separate control, release, reproduction, receipt-layout and documentation
pieces. No merge, push, public tags or GitHub Release was performed.

From a normal prepared checkout:

```sh
python3 scripts/audit.py bootstrap
python3 scripts/audit.py smoke
```

Use Python 3.12 for the replay venv and the pinned Rust 1.90 toolchain; explicit
selectors and prerequisite errors are described in [AUDIT](../../AUDIT.md).
For an interactive synthetic preview, use the standard demo instructions in
[INSTALL](../../../INSTALL.md). On this audit host the verified native executable
is `target/native-macos/release/they-work`; its final SHA-256 is recorded in the
[keyboard evidence](docs/evidence/results.json). The path is an audit-host build
location, not a new installation convention.

## Next external step

The owner needs to arrange the five new participants and perform the intended
physical-terminal dry run. Then freeze one binary in [baseline.json](study/baseline.json),
run the five sessions and optional diaries, and apply the already authorized
qualification rules. Zero sessions cannot justify either a default prototype or
“no change needed.” Missing environments remain not tested.

Larger iteration 7 findings remain open: ledger capacity, incomplete observed
history, delivery retention across projects, Windows state replacement and
long-running memory growth. Authenticated integrations, platform certification,
actual public distribution and user comprehension remain publication gates.
