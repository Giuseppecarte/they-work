# UX-01 and UX-02 remain open

The actual study launchers pass a fresh provisional preflight on the `7cb26e4` application binary. Five launcher PTYs cover Tower, compact, reduced motion and two fresh result variants; manifest rejection, hash matching, resize, source-store integrity and normal exit are checked. [Structured results](preflight.json) retain the exact binary hash. These are five simulated launcher checks, not five people.

No participants are enrolled; no sessions, diaries, prototypes or follow-ups have occurred. The participant baseline remains unfrozen until all eight engineering gates pass. A later study must retain its fixed binary even while REL-02 changes are developed elsewhere.

The historical preflight writes into its own iteration-8 evidence directory, so it ran in an isolated archive under ignored `target/audit/iteration-10/study-checkout`. The original historical evidence was not modified. The first attempt lacked pyte; adding the entire historical package tree through PYTHONPATH then shadowed the pinned Pillow with an incompatible build. Both failures are retained in `dependency-attempt/`. The successful preparation copies only the existing pyte/wcwidth packages into the isolated historical helper location and uses the pinned audit venv for Pillow. [Dependencies](dependencies.json) record versions and hashes. No global package was installed or changed.

This macOS launcher replay uses Menlo and prepared PTY dependencies. It is separate from the portable bundled-font REPRO smoke; success does not establish physical-terminal paint or usability. Missing follow-up evidence will leave any qualifying prototype unvalidated.
