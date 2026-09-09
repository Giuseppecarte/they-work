# Study-launcher preparation: passed on one host

**Five executable PTY launches passed; zero participant sessions.**
[Results](evidence/preflight.json) record the exact candidate SHA-256
`ca446efcfc928d3f3cfdb0ab6a98b97e7122765093d38e1315a5eda1781abe48`,
macOS 26.6.2 ARM64 and Python 3.12.14. This was before the later receipt wrapping
fix; the study routes exercised here do not submit an instruction or render that
receipt. The actual participant baseline remains **unfrozen** in
[baseline.json](baseline.json).

This preflight executes `prepare_session.py` and `launch_fixture.py`, then sends
real input bytes to the native executable. It is independent of
[REPRO-01's one-screen smoke](../../../AUDIT.md); running that smoke neither
validates these launchers nor substitutes for a moderator's dry run.

## Exercised paths

- Tower, compact and reduced-motion launchers open the same six synthetic tasks
  and three projects from the same source path. The two SQLite file hashes stay
  unchanged throughout all three conditions. Preference state changes locally.
- Each actual launcher opens Attention at 120×36, resizes to 80×24, searches for
  the API guide and opens its work brief using only keyboard input.
- Tower/reduced use a **simulated Kitty capability response** with 8×16 cells.
  The stream contains an image only in those conditions, and the saved motion
  flag matches each requested condition. This checks selection of the modes;
  it does not measure motion recognition or certify a physical terminal.
- Fresh semantic variants A and B each retain six tasks. Their separate injected
  synthetic result appears in the selected 80×24 brief through the actual
  `--add-result` helper. A follow-up check confirms the B facts manifest preserves
  the real worker IDs and exactly matches its SQLite titles.
- An unlocked baseline is rejected. A temporary test manifest with the exact
  binary hash is accepted; a mismatched hash is rejected. The real baseline
  manifest is untouched and remains unlocked.
- All five application launches exit 0 and restore the PTY terminal attributes.
  No account, real provider command, outreach or participant action occurred.

Representative replays inspected: [tower](evidence/launcher-tower.png),
[compact](evidence/launcher-compact.png),
[reduced](evidence/launcher-reduced.png), and
[the new B result](evidence/return-variant-B.png). They show complete project
context, visible native labels and the injected result. These are ANSI plus
transmitted-image replays, **not operating-system screenshots**. Normal motion
can place an actor away from its station label; whether this confuses people is
still the UX-02 hypothesis, not a resolved usability result.

## Reproduce this preparation on the tested host

Use Python with Pillow 12.3.0 and pyte 0.8.2, the macOS Menlo font, and an already
built candidate executable. The historical PTY helper also searches the ignored
`docs/design-audit/tmp/python` dependency directory; the evidence used pyte 0.8.2
there. This is a declared legacy dependency, not part of the portable REPRO smoke.

```sh
python3 docs/design-audit/iteration-8/study/preflight.py \
  --binary target/native-macos/release/they-work
```

The test deliberately resets only generated `study-P01` fixtures and copies the
candidate into ignored scratch so a concurrent rebuild cannot change a run.
Do not run it over a slot containing participant notes or during the study.
Raw PTY streams are losslessly gzip-compressed; small JSON traces, PNG replays
and native text are retained beside [the result](evidence/preflight.json).

The launchers use Unix paths and executable stubs, and the capture chain imports
`fcntl`, `termios` and Menlo. **Native Windows, Linux and WSL study preparation are
not tested.** The product's broader platform support and the new REPRO runner do
not certify this kit on those systems. The owner must separately dry-run the
actual launcher/fixture in the chosen physical terminal before recruitment;
record a setup failure separately from participant performance.

The optional live-approval continuation was not rerun through this study launcher
in this preflight. Its historical expert evidence is retained, while the current
[REL-01 PTY](../docs/evidence/not-sent/results.json) tests a different, managed
instruction scenario. Prepare and rehearse the optional approval task separately
before including it in a participant session. Physical rendering, assistive
technology, comprehension, diary use and prototype follow-ups remain **not tested**.
