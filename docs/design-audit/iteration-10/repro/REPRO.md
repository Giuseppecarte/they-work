# REPRO-01 closure verification

The bounded reproduction is executed in two independent clean checkouts of the
candidate. Both use the repository's Rust 1.90 pin and lockfile, Python 3.12,
Pillow 12.3.0 with locked wheel hashes, and the licensed bundled DejaVu Mono fonts.
Bootstrap prepares dependencies; smoke renders exactly one complete 80×24 tower
at 8×16 cell geometry and executes exactly one named collector fixture. The 13
runner contract tests remain a separate test suite.

The initial clean baseline is `4097e4f`. A release-cache defect discovered by the
separate image build required candidate `7cb26e45e13de5fa9bfa3a3be7c2eeb59a1011c1`.
It changes the Docker cache stub and its regression test, with no Rust, collector
fixture, font or reproduction-runner change. Final smoke is nevertheless repeated
in both clean checkouts to bind the retained capture and source metadata to the
new commit. Earlier baseline metadata and command logs are retained separately.

## Preparation and commands

The checkouts are local `git clone --no-hardlinks --no-checkout` copies followed
by detached checkout of the exact commit; they contain no copied build output or
provider stores. macOS receives an explicitly selected existing Rust toolchain
and Cargo cache. Linux uses the previously prepared Rust/Python environment
identified in `summary.json` and a new checkout-local Cargo cache. Existing
historical audit checkouts and evidence are not modified.

Run the following inside each checkout, choosing the local Python 3.12 and Cargo
executables when they are not on PATH:

```sh
python3 scripts/audit.py bootstrap --python /path/to/python3.12 --cargo /path/to/cargo
python3 scripts/audit.py smoke --output target/audit/verified-smoke
target/audit/venv/bin/python -B scripts/test-audit.py
```

macOS bootstrap initially encountered restricted DNS; its unsuccessful log is
retained. The authorized networked retry installed the hash-verified dependency
and ran the locked Cargo fetch. Smoke uses offline Cargo and disables rustup
automatic installation. Linux smoke and contracts additionally use Docker
`--network none`; no provider store is mounted in either environment.

## Evidence and limits

`summary.json` records actual counts, clean status before/after, decoded image
and geometry comparison, final source hashes, and the full-frame inspection.
The automatic smoke metadata keeps its original `visual_review: not-tested`;
subsequent visual inspection is recorded separately, tied to the exact PNG hash.

A passing replay verifies the complete compositor/native-cell composition. It is
not a physical terminal transport, input latency, provider-login or Windows test.
Linux is an ARM64 Docker environment on the macOS host. Public GitHub-hosted
macOS/Linux jobs have not been dispatched. Compression bytes may differ between
platforms; comparisons use decoded RGB, native cells and hit geometry.
