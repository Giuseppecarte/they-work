#!/usr/bin/env python3
"""Build and run the bounded synthetic audit. No installed provider is invoked."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
import platform
from pathlib import Path
import shutil
import subprocess

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--skip-build", action="store_true")
    options = parser.parse_args()
    env = os.environ.copy()
    bundled = REPO / "docs/design-audit/tmp/native-rust"
    if not shutil.which("cargo") and (bundled / "cargo-home/bin/cargo").exists():
        env["CARGO_HOME"] = str(bundled / "cargo-home")
        env["RUSTUP_HOME"] = str(bundled / "rustup-home")
        env["PATH"] = str(bundled / "cargo-home/bin") + os.pathsep + env.get("PATH", "")
    env.setdefault("CARGO_TARGET_DIR", str(REPO / "target/native-macos"))
    env["TERM"] = "xterm-256color"
    for key in ("NO_COLOR", "THEYWORK_COLOR", "THEYWORK_PIXELS"):
        env.pop(key, None)
    out = HERE / "evidence"
    out.mkdir(exist_ok=True)
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S")
    scratch = REPO / "docs/design-audit/tmp/iteration-7-data" / stamp
    scratch.mkdir(parents=True, exist_ok=False)
    binary = Path(env["CARGO_TARGET_DIR"]) / "debug/theywork-data-audit"
    if not options.skip_build:
        with (out / "build.log").open("w") as log:
            subprocess.run(["cargo", "build", "--offline", "--locked", "--manifest-path", str(HERE / "Cargo.toml")],
                           cwd=REPO, env=env, check=True, stdout=log, stderr=subprocess.STDOUT)
    with (out / "run.log").open("w") as log:
        subprocess.run([str(binary), str(scratch), str(out)], cwd=REPO, env=env,
                       check=True, stdout=log, stderr=subprocess.STDOUT)
    sources = ["crates/theywork-control/src/supervisor.rs", "crates/theywork-control/src/bridge.rs",
               "crates/theywork-core/src/world.rs", "crates/theywork-core/src/lib.rs",
               "crates/theywork-render/src/work_brief.rs", "crates/theywork-render/src/views/workboard.rs"]
    metadata = {
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO, text=True).strip(),
        "binary_sha256": digest(binary),
        "source_sha256": {name: digest(REPO / name) for name in sources},
        "runner_sha256": digest(HERE / "src/main.rs"),
        "fake_provider_sha256": digest(HERE / "fake_provider.py"),
        "raw_synthetic_trace_directory": str(scratch.relative_to(REPO)),
        "evidence_sha256": {p.name: digest(p) for p in sorted(out.iterdir()) if p.is_file() and p.name != "metadata.json"},
        "platform": platform.system() + " " + platform.machine(),
        "retained_raw_data": "Synthetic traces/config only, in ignored scratch. Endpoint tokens are not published.",
    }
    (out / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    print((out / "run.log").read_text(), end="")
    print("Evidence:", out.relative_to(REPO))


if __name__ == "__main__":
    main()
