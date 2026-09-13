# Evidence contract

The audited production baseline is `748d371` (UI implementation `5ea7f77`, art
`aecd41e`). The native macOS arm64 executable SHA-256 is
`b2c529653f5b9d6a2ee93cd84cb87772ee8e1dee96b431ac9b645f3bfd33de84`.
This iteration adds audit harnesses and reports; it does not change production
APIs, persistence schemas, provider permissions, or artwork.

## Classification

- **Confirmed defect:** independently expected behavior differs from an executed
  scenario. Record inputs, commands, actual output, and the specific violated
  product promise. A deliberately bounded history is not itself a defect.
- **Code-confirmed risk:** a branch, limit, or ordering exists in source, but its
  consequence has not been exercised in the required environment.
- **Product hypothesis:** a plausible workflow or comprehension problem awaiting
  user evidence. Expert review is not a participant result.
- **Not tested:** the named layer was not run. State why and give a concrete
  follow-up procedure; do not transfer a pass from another layer.
- **Pass:** only the exact tested contract, environment, and inputs passed.

Separate a failed application experiment from an invalid fixture, missing build
prerequisite, or unavailable host permission. Preserve the failure and correction
when a harness needed repair. Never count an intentionally simulated provider
as an authenticated-provider session.

## Findings register interface

`findings.json` is the shared audit record, with schema version 1 and at most ten
prioritized items. Additional hypotheses may remain in the register with
`rank: null`; they do not become extra roadmap commitments. Each item has `id`, `rank`, `area`, `classification`, `title`,
`workflow`, `expected`, `observed`, `reproduction`, `evidence`, `severity`,
`frequency`, `confidence`, `effort`, `smallest_change`, `acceptance`, and
`limitations`. Evidence paths are relative to this directory or explicit public
source URLs. Participant frequency is unknown until actual sessions supply it.

Severity orders proven loss, wrong-target control, silently missing requests,
and unsafe release behavior ahead of convenience work. Within that boundary,
rank user consequence, measured recurrence, confidence, then effort. Keep
low-confidence feature ideas separate; do not imply a numeric score is a user
study or a precise economic forecast.

## Measurement boundaries

| Layer | What it can establish | What it cannot establish |
|---|---|---|
| Source inspection | Actual limits, ordering, and data paths | That a hypothetical failure happened |
| Fixture/replay | Expected records, state transitions, injected failure handling | Current authenticated provider compatibility |
| Actual executable in PTY | Input/output protocol, process exit, restored terminal modes | A physical emulator's pixel appearance or display latency |
| Compositor export | Exact authored pixels, native-cell geometry and masks | Font fallback, OS paint, transport behavior |
| Headless two-hour run | Collector/world stability and sampled resource use | Graphical rendering cost or visible response time |
| Owner-run terminal recording | The recorded terminal/version's behavior | Every supported platform or terminal |
| Participant session | Observed task comprehension and friction for that person | General success rates or accessibility certification |

All retained fixtures are synthetic. Raw growing transcript stores and build
artifacts live in ignored `docs/design-audit/tmp/`; reproducible generators,
compact logs, results, and selected evidence remain under iteration 7.
