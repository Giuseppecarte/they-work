#!/usr/bin/env python3
"""Check the audit's evidence links, declarations and completion boundaries.

This validates an audit package. It does not rerun the application experiments
or convert an expert review into user or physical-terminal evidence.
"""
import argparse
import ast
import csv
import hashlib
import json
from pathlib import Path
import re
import subprocess
from urllib.parse import unquote, urlparse


ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[2]
BASELINE = "748d371f9769e83798f29a52fffeb90410afc3bd"
REQUIRED = {
    "id", "rank", "area", "classification", "title", "workflow", "expected",
    "observed", "reproduction", "evidence", "severity", "frequency",
    "confidence", "effort", "smallest_change", "acceptance", "limitations",
}
CLASSES = {"confirmed-defect", "code-confirmed-risk", "product-hypothesis", "not-tested"}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--final", action="store_true", help="Require a completed real two-hour run")
    parser.add_argument("--manifest", action="store_true", help="Write an evidence hash manifest after checks")
    args = parser.parse_args()
    errors = []
    counts = {"json_files": 0, "python_files": 0, "markdown_links": 0, "verified_hashes": 0}

    def require(condition, reason):
        if not condition:
            errors.append(reason)

    # Parsing is read-only: avoid py_compile's local bytecode artifacts.
    for path in ROOT.rglob("*"):
        if not path.is_file() or "__pycache__" in path.parts:
            continue
        try:
            if path.suffix == ".json":
                json.loads(path.read_text())
                counts["json_files"] += 1
            elif path.suffix == ".jsonl":
                for line in path.read_text().splitlines():
                    if line.strip():
                        json.loads(line)
            elif path.suffix == ".py":
                ast.parse(path.read_text(), filename=str(path))
                counts["python_files"] += 1
            elif path.suffix == ".md":
                for raw in re.findall(r"\[[^\]]+\]\(([^)]+)\)", path.read_text()):
                    link = raw.strip().strip("<>")
                    parsed = urlparse(link)
                    if parsed.scheme or not parsed.path:
                        continue
                    target = Path(unquote(parsed.path))
                    if not target.is_absolute():
                        target = path.parent / target
                    require(target.exists(), f"Broken local link: {path.relative_to(ROOT)} -> {raw}")
                    counts["markdown_links"] += 1
        except (ValueError, SyntaxError, UnicodeError) as exc:
            errors.append(f"Invalid artifact {path.relative_to(ROOT)}: {exc}")

    register = json.loads((ROOT / "findings.json").read_text())
    findings = register["findings"]
    require(register["schema_version"] == 1, "Unknown findings schema")
    require(register["baseline_commit"] == BASELINE, "Findings baseline mismatch")
    require(0 < len(findings) <= 10, "Roadmap must have one to ten ranked findings")
    require([item["rank"] for item in findings] == list(range(1, len(findings) + 1)), "Ranks must be ordered and unique")
    require(len({item["id"] for item in findings}) == len(findings), "Finding IDs must be unique")
    for item in findings:
        require(REQUIRED <= item.keys(), f"Incomplete finding: {item.get('id')}")
        require(item["classification"] in CLASSES, f"Invalid classification: {item['id']}")
        require(bool(item["acceptance"]) and bool(item["evidence"]), f"No executable criterion/evidence: {item['id']}")
        for link in item["evidence"]:
            require(bool(urlparse(link).scheme) or (ROOT / link).is_file(), f"Missing evidence: {item['id']} -> {link}")

    # Preserve the hashes declared by the independent data and visual reviews.
    for rel in ["data/evidence/metadata.json", "data/evidence/dual-feed/metadata.json"]:
        path = ROOT / rel
        meta = json.loads(path.read_text())
        for name, expected in meta.get("source_sha256", {}).items():
            require(digest(REPO / name) == expected, f"Source hash changed: {name}")
            counts["verified_hashes"] += 1
        for name, expected in meta.get("evidence_sha256", {}).items():
            require(digest(path.parent / name) == expected, f"Data evidence hash changed: {name}")
            counts["verified_hashes"] += 1
    inspection = json.loads((ROOT / "workflows/evidence/visual-inspection.json").read_text())
    for name, result in inspection.items():
        require(digest(ROOT / "workflows/evidence" / f"{name}.png") == result["sha256"], f"Inspected image changed: {name}")
        counts["verified_hashes"] += 1

    measured = json.loads((ROOT / "performance/large-history/results.json").read_text())
    require(digest(ROOT / "performance/source-used-for-measurement.txt") == measured["probe_source_sha256_at_measurement"], "Measured source snapshot mismatch")

    study = json.loads((ROOT / "study/status.json").read_text())
    if study["completed_participant_sessions"] == 0:
        scorecards = list(csv.DictReader((ROOT / "study/scorecard.csv").open()))
        require(len(scorecards) == 5, "Expected five empty recruitment-slot scorecards")
        for row in scorecards:
            require(row["status"] == "not-tested", "An unperformed participant outcome was filled")
            require(all(not value for key, value in row.items() if key not in {"participant_slot", "status"}), "Participant scorecard contains outcomes despite zero sessions")

    soak = json.loads((ROOT / "performance/two-hour/result.json").read_text())
    if args.final:
        require(soak["status"] == "pass", "Two-hour experiment is incomplete or failed; inspect before finalizing")
        require(soak.get("actual_process_seconds", 0) >= 7200, "Two-hour duration was not completed")
        require(soak.get("exit_code") == 0, "Two-hour process exit was not zero")
        require(soak.get("counters", {}).get("poll_errors") == "0", "Two-hour polling errors require review")
        require(register["status"] == "complete-with-documented-limitations", "Findings status is still provisional")

    changed = subprocess.run(["git", "diff", "--name-only", BASELINE, "--", ".", ":(exclude)docs/design-audit/iteration-7/**"], cwd=REPO, capture_output=True, text=True)
    require(changed.returncode == 0 and not changed.stdout.strip(), "Production/baseline files changed outside iteration 7")

    result = {"status": "pass" if not errors else "failed", "scope": "Audit artifact consistency; not product or human validation", "final_completion_checked": args.final, "source_baseline": BASELINE, "soak_status": soak["status"], "participant_sessions": study["completed_participant_sessions"], "checks": counts, "errors": errors}
    evidence = ROOT / "evidence"
    evidence.mkdir(exist_ok=True)
    (evidence / "package-checks.json").write_text(json.dumps(result, indent=2) + "\n")
    if args.manifest and not errors:
        files = {}
        for path in sorted(ROOT.rglob("*")):
            if path.is_file() and "__pycache__" not in path.parts and path.name not in {"manifest.json", "package-checks.json"}:
                files[str(path.relative_to(ROOT))] = {"bytes": path.stat().st_size, "sha256": digest(path)}
        manifest = {"source_baseline": BASELINE, "final": args.final, "file_count": len(files), "total_bytes": sum(item["bytes"] for item in files.values()), "excludes": ["manifest.json (self)", "package-checks.json (checker result)", "__pycache__", "ignored generated fixtures/toolchains/builds outside this iteration"], "files": files}
        (evidence / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps(result, indent=2))
    raise SystemExit(1 if errors else 0)


if __name__ == "__main__":
    main()
