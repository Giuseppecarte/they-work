#!/usr/bin/env python3
"""Run bounded current-candidate checks; retain counts, commands and raw evidence."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
CARGO = ["sh", "docs/design-audit/native-cargo.sh"]
GROUPS = {
    "data": [
        ("lineage", ["test", "--locked", "-p", "theywork-control", "--lib", "stream::tests", "--", "--test-threads=1"], 3),
        ("reconciliation", ["test", "--locked", "-p", "theywork-control", "--test", "data_stream", "--", "--test-threads=1"], 9),
        ("stream-process", ["test", "--locked", "-p", "theywork-control", "--test", "control", "data_stream", "--", "--test-threads=1"], 2),
        ("retention", ["test", "--locked", "-p", "theywork-core", "--test", "coverage_retention", "--", "--test-threads=1"], 7),
        ("coverage-ui", ["test", "--locked", "-p", "theywork-render", "coverage_", "--", "--test-threads=1"], 3),
        ("short-request", ["test", "--locked", "-p", "theywork-render", "--lib", "source_warning_and_current_request_action_survive_a_short_panel"], 1),
    ],
    "integration": [
        ("format", ["fmt", "--all", "--", "--check"], None),
        ("workspace", ["test", "--locked", "--workspace", "--", "--test-threads=1"], "nonzero"),
        ("clippy", ["clippy", "--locked", "--workspace", "--all-targets", "--", "-D", "warnings"], None),
        ("release", ["build", "--locked", "--release", "--bin", "they-work"], None),
    ],
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("group", choices=GROUPS)
    parser.add_argument("--output", type=Path, help="New evidence directory beneath target/audit for a repeat run.")
    args = parser.parse_args()
    output = HERE / ("data" if args.group == "data" else "validation")
    if args.output is not None:
        output = (ROOT / args.output).resolve()
        if not output.is_relative_to(ROOT / "target/audit"):
            parser.error("Repeat output must stay beneath the ignored target/audit directory.")
    output.mkdir(parents=True, exist_ok=True)
    if any((output / f"{name}.log").exists() for name, _, _ in GROUPS[args.group]):
        parser.error("Existing evidence would be overwritten; preserve the previous attempt first.")
    results = {
        "candidate": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "started_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "platform_evidence": "native macOS; no authenticated providers or terminal paint",
        "cases": [],
    }
    for name, arguments, expected in GROUPS[args.group]:
        command = CARGO + arguments
        path = output / f"{name}.log"
        environment = os.environ.copy()
        if name == "workspace":
            environment["THEYWORK_STORAGE_REPORT"] = str(output / "native-process.json")
        start = time.monotonic()
        with path.open("w") as log:
            process = subprocess.run(command, cwd=ROOT, env=environment, stdout=log, stderr=subprocess.STDOUT)
        raw = path.read_text()
        counts = [(int(a), int(b), int(c)) for a, b, c in re.findall(
            r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;", raw)]
        passed = sum(c[0] for c in counts)
        count_ok = expected is None or (passed > 0 if expected == "nonzero" else passed == expected)
        status = "pass" if process.returncode == 0 and count_ok else "fail"
        native_report = None
        if name == "workspace" and status == "pass":
            native_path = output / "native-process.json"
            if native_path.exists():
                native_report = json.loads(native_path.read_text())
            if (not native_report or native_report.get("platform") != "macos"
                    or native_report.get("architecture") != "aarch64"
                    or len(native_report.get("cases", [])) != 3
                    or any(case.get("status") != "pass" for case in native_report.get("cases", []))):
                status = "fail"
        case = {"name": name, "command": command, "status": status,
                "exit_code": process.returncode, "elapsed_seconds": round(time.monotonic() - start, 3),
                "expected_passed": expected, "passed": passed,
                "failed": sum(c[1] for c in counts), "ignored": sum(c[2] for c in counts),
                "log": path.name, "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                "process_cases": "Inspect harness-free results separately; not included in ordinary test count."}
        if native_report is not None:
            case["native_process_cases"] = len(native_report["cases"])
            case["native_process_report_sha256"] = hashlib.sha256(native_path.read_bytes()).hexdigest()
        results["cases"].append(case)
        (output / f"{args.group}-results.json").write_text(json.dumps(results, indent=2) + "\n")
        print(f"{name}: {status}; {passed} ordinary tests; {case['elapsed_seconds']}s", flush=True)
        if status != "pass":
            print(raw[-3000:], flush=True)
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
