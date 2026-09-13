#!/usr/bin/env python3
"""Run documented checks in the independent audit checkout, with exact setup recorded."""
import argparse
import json
import os
from pathlib import Path
import platform
import subprocess
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--offline", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[4]
    out = Path(__file__).resolve().parent
    checkout = root / "docs/design-audit/tmp/iteration-7-maintenance/checkout"
    env = dict(os.environ)
    env.update(CARGO_HOME=str(root / "docs/design-audit/tmp/native-rust/cargo-home"),
               RUSTUP_HOME=str(root / "docs/design-audit/tmp/native-rust/rustup-home"),
               CARGO_TARGET_DIR=str(checkout / "target"),
               CARGO_NET_OFFLINE=str(args.offline).lower(), THEYWORK_TOOLCHAIN="native")
    env["PATH"] = env["CARGO_HOME"] + "/bin:" + env["PATH"]
    stem = "offline-native" if args.offline else "network-native"
    started = time.perf_counter()
    with (out / f"{stem}-make-check.log").open("w") as log:
        result = subprocess.run(["make", "check"], cwd=checkout, env=env, stdout=log,
                                stderr=subprocess.STDOUT, timeout=1800)
    metadata = {
        "command": "make check", "seconds": time.perf_counter() - started,
        "exit": result.returncode, "platform": platform.platform(),
        "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=checkout, text=True).strip(),
        "setup": "Pinned native Rust 1.90; independent clean checkout/build target; preexisting dependency cache, not an empty download cache.",
        "offline": args.offline,
        "checkout_tracked_changes": subprocess.check_output(["git", "status", "--short"], cwd=checkout, text=True),
    }
    (out / f"{stem}-make-check.json").write_text(json.dumps(metadata, indent=2) + "\n")
    print(json.dumps(metadata), flush=True)
    raise SystemExit(result.returncode)


if __name__ == "__main__":
    main()
