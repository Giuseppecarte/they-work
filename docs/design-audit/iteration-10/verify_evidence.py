#!/usr/bin/env python3
"""Read-only integrity and closure checks for the retained iteration-10 evidence."""
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def read(name):
    return json.loads((HERE / name).read_text())


def main():
    manifest = read("EVIDENCE.json")
    for base, entries in [(HERE, manifest["files"]), (ROOT, manifest["source_hashes"])]:
        for name, expected in entries.items():
            path = (base / name).resolve()
            assert path.is_relative_to(base), f"Evidence path leaves its root: {name}"
            assert path.is_file(), f"Missing evidence: {name}"
            assert hashlib.sha256(path.read_bytes()).hexdigest() == expected, f"Changed evidence: {name}"
    closure = read("closure.json")
    rows = closure["engineering_findings"]
    assert {row["id"] for row in rows} == {
        "REL-01", "RELEASE-01", "REPRO-01", "DOC-01", "DATA-01", "DATA-02", "WIN-01", "PERF-01"
    }
    verified = sum(row["status"] == "verified" for row in rows)
    assert verified == closure["verified"] == 6
    assert closure["required"] == len(rows) == 8
    assert closure["gate_open"] is False and closure["REL-02"]["status"].startswith("not started")
    for row in rows:
        assert (HERE / row["evidence"]).is_file(), row["id"]
        if row["status"] != "verified":
            assert row["blocker"], row["id"]
    integration = read("validation/integration-results.json")
    assert all(case["status"] == "pass" and case["exit_code"] == 0 for case in integration["cases"])
    workspace = next(case for case in integration["cases"] if case["name"] == "workspace")
    assert (workspace["passed"], workspace["failed"], workspace["ignored"]) == (466, 0, 4)
    native = read("validation/native-process.json")
    assert native["platform"] == "macos" and native["architecture"] == "aarch64"
    assert len(native["cases"]) == 3 and all(case["status"] == "pass" for case in native["cases"])
    visual = read("visual/manifest.json")
    assert visual["automatic_pass"] and len(visual["cases"]) == 96
    assert sum(case["visual_review"] == "pass" for case in visual["cases"]) == 8
    study = read("study/status.json")
    assert study["completed_sessions"] == study["participants_enrolled"] == 0
    assert study["baseline_frozen"] is False
    print(f"Verified {len(manifest['files'])} retained files and {len(manifest['source_hashes'])} source hashes.")
    print("Closure remains 6/8; native Windows and actual CI archives are blocked; REL-02 has not started.")


if __name__ == "__main__":
    main()
