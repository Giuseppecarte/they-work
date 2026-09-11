#!/usr/bin/env python3
"""Verify retained evidence and source hashes; optionally require local archives."""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--require-previews", action="store_true")
    args = parser.parse_args()
    manifest = json.loads(Path(__file__).with_name("EVIDENCE.json").read_text())
    failures = []
    checked = 0
    missing_previews = []
    for group in ("sources", "evidence", "previews"):
        for name, expected in manifest[group].items():
            path = (ROOT / name).resolve()
            if not path.is_relative_to(ROOT):
                failures.append(f"Outside checkout: {name}")
                continue
            if group == "previews" and not path.exists() and not args.require_previews:
                missing_previews.append(name)
                continue
            if not path.is_file():
                failures.append(f"Missing: {name}")
                continue
            actual = hashlib.sha256(path.read_bytes()).hexdigest()
            if actual != expected:
                failures.append(f"Hash mismatch: {name}")
            checked += 1
    if failures:
        raise SystemExit("\n".join(failures))
    print(f"Verified {checked} source/evidence/archive hashes")
    if missing_previews:
        print(f"{len(missing_previews)} generated preview files absent; archive validation not performed")


if __name__ == "__main__":
    main()
