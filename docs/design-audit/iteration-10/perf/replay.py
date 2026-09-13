#!/usr/bin/env python3
"""Repeat PERF-01 against a frozen revision, changing only the envelope test."""

import argparse
import hashlib
import io
import json
import math
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tarfile
from datetime import datetime, timezone


ROOT = Path(__file__).resolve().parents[4]
WORK = ROOT / "target/audit/iteration-10/perf"
REVISION = "4097e4f706faff924fbe71fb075ec469909746bf"
TEST = "claude::correlation_tests::correlation_resource_replay"
PROBE = "crates/theywork-collect/src/claude_correlation_tests.rs"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def dump(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def rust_environment():
    env = os.environ.copy()
    toolchain = ROOT / "docs/design-audit/tmp/native-rust"
    env.update(
        CARGO_HOME=str(toolchain / "cargo-home"),
        RUSTUP_HOME=str(toolchain / "rustup-home"),
        TMPDIR=str(ROOT / "docs/design-audit/tmp"),
    )
    env["PATH"] = str(toolchain / "cargo-home/bin") + os.pathsep + env["PATH"]
    return env


def prepare():
    WORK.mkdir(parents=True, exist_ok=True)
    checkout = WORK / "frozen"
    checkout.mkdir()  # Refuse to replace any earlier checkout or measurement.
    archive = subprocess.check_output(
        ["git", "archive", REVISION, "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "crates"],
        cwd=ROOT,
    )
    with tarfile.open(fileobj=io.BytesIO(archive)) as bundle:
        bundle.extractall(checkout, filter="data")
    source_hashes = {
        str(path.relative_to(checkout)): sha(path)
        for path in sorted(checkout.rglob("*"))
        if path.is_file()
    }
    original_inputs = [name for name in source_hashes if name.startswith(("crates/theywork-core/", "crates/theywork-collect/")) or name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml")]
    assert all(sha(ROOT / name) == source_hashes[name] for name in original_inputs), "run the original exact probe from the frozen source inputs"
    original = (checkout / PROBE).read_text()
    augmented = original
    replacements = [
        (
            '    assert!(mode == "paired" || mode == "unpaired");',
            '    assert!(mode == "paired" || mode == "unpaired");\n'
            '    let overlap: usize = std::env::var("THEYWORK_PERF_OVERLAP")\n'
            '        .expect("THEYWORK_PERF_OVERLAP").parse().unwrap();\n'
            '    assert!([1, 8, 32, 128].contains(&overlap));\n'
            '    let batches = 2560 / overlap;',
        ),
        ("for batch in 0..80 {", "for batch in 0..batches {"),
        ("for call in 0..32 {", "for call in 0..overlap {"),
        ("let seq = batch * 32 + call;", "let seq = batch * overlap + call;"),
        ("if phase == 0 { 1600 } else { 0 }", "if phase == 0 { 50 * overlap } else { 0 }"),
        ('"overlap_per_worker":32', '"overlap_per_worker":overlap'),
    ]
    for before, after in replacements:
        assert augmented.count(before) == 1, before
        augmented = augmented.replace(before, after)
    (checkout / PROBE).write_text(augmented)
    (WORK / "envelope-probe.rs").write_text(augmented)
    env = rust_environment()
    cargo = str(Path(env["CARGO_HOME"]) / "bin/cargo")
    command = [
        cargo, "test", "--locked", "--offline", "--manifest-path", str(checkout / "Cargo.toml"),
        "--target-dir", str(WORK / "build"), "-p", "theywork-collect", "--lib",
        "--no-run", "--message-format=json",
    ]
    with (WORK / "build.stdout.jsonl").open("w") as out, (WORK / "build.stderr.log").open("w") as err:
        subprocess.run(command, cwd=ROOT, env=env, stdout=out, stderr=err, check=True)
    artifacts = [
        json.loads(line) for line in (WORK / "build.stdout.jsonl").read_text().splitlines()
        if line.startswith("{")
    ]
    binaries = [Path(a["executable"]) for a in artifacts if a.get("executable") and a["target"]["name"] == "theywork_collect"]
    assert len(binaries) == 1
    shutil.copy2(binaries[0], WORK / "envelope-test")
    original_log = (WORK / "focused-tests.log").read_text()
    match = re.search(r"Running unittests src/lib.rs \(([^)]+)\)", original_log)
    assert match, "run the current frozen collector unit tests first"
    shutil.copy2(ROOT / match[1], WORK / "original-test")
    dump(WORK / "preparation.json", {
        "revision": REVISION,
        "prepared_at": datetime.now(timezone.utc).isoformat(),
        "platform": platform.platform(),
        "python": platform.python_version(),
        "rustc": subprocess.check_output([str(Path(env["CARGO_HOME"]) / "bin/rustc"), "-Vv"], env=env, text=True),
        "build_command": command,
        "build_environment": {key: env[key] for key in ("CARGO_HOME", "RUSTUP_HOME", "TMPDIR")},
        "source_sha256_before_test_only_augmentation": source_hashes,
        "test_only_replacements": replacements,
        "envelope_probe_sha256": sha(WORK / "envelope-probe.rs"),
        "original_probe_sha256": hashlib.sha256(original.encode()).hexdigest(),
        "binary_sha256": {name: sha(WORK / name) for name in ("original-test", "envelope-test")},
    })
    match_sources(REVISION)
    print(WORK / "preparation.json", flush=True)


def match_sources(candidate):
    prepared = json.loads((WORK / "preparation.json").read_text())
    candidate = subprocess.check_output(["git", "rev-parse", candidate], cwd=ROOT, text=True).strip()
    inputs = prepared["source_sha256_before_test_only_augmentation"]
    names = [name for name in inputs if name.startswith(("crates/theywork-core/", "crates/theywork-collect/")) or name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml")]
    rows = []
    for name in names:
        content = subprocess.check_output(["git", "show", candidate + ":" + name], cwd=ROOT)
        digest = hashlib.sha256(content).hexdigest()
        rows.append({"path": name, "sha256": digest, "matches_frozen_measurement": digest == inputs[name]})
    assert all(row["matches_frozen_measurement"] for row in rows), "candidate source inputs changed; do not reuse these measurements"
    dump(WORK / "candidate-source-match.json", {
        "measured_revision": REVISION, "integrated_candidate": candidate,
        "reused_on_basis": "All relevant core/collector and build-input source bytes match; unrelated changes do not alter this test.",
        "matched_files": len(rows), "files": rows,
    })
    print(f"PASS source match: {len(rows)} files at {candidate}", flush=True)


def run(mode, overlap):
    key = f"{mode}-{overlap}"
    run_root = WORK / "frozen/target/audit" if overlap != 32 else WORK
    output = run_root / key
    assert not output.exists(), f"refusing to overwrite {output}"
    binary = WORK / ("original-test" if overlap == 32 else "envelope-test")
    assert binary.is_file()
    env = rust_environment()
    env.update(THEYWORK_PERF_MODE=mode, THEYWORK_PERF_OUTPUT=str(output), THEYWORK_PERF_OVERLAP=str(overlap))
    command = ["/usr/bin/time", "-l", str(binary), TEST, "--exact", "--ignored", "--nocapture"]
    record = {
        "revision": REVISION, "mode": mode, "overlap_per_worker": overlap,
        "started_at": datetime.now(timezone.utc).isoformat(), "command": command,
        "binary_sha256": sha(binary), "cwd": str(ROOT),
        "environment": {key: env[key] for key in ("THEYWORK_PERF_MODE", "THEYWORK_PERF_OUTPUT", "THEYWORK_PERF_OVERLAP", "TMPDIR")},
    }
    with (WORK / f"{key}.stdout.log").open("w") as out, (WORK / f"{key}.time.log").open("w") as err:
        result = subprocess.run(command, cwd=ROOT, env=env, stdout=out, stderr=err)
    record["exit_code"] = result.returncode
    record["finished_at"] = datetime.now(timezone.utc).isoformat()
    record["raw_result"] = str(output / "result.json")
    log = (WORK / f"{key}.stdout.log").read_text()
    record["executed_tests"] = 1 if "test result: ok. 1 passed; 0 failed; 0 ignored;" in log else 0
    dump(WORK / f"{key}.run.json", record)
    assert result.returncode == 0 and record["executed_tests"] == 1, record
    raw = json.loads((output / "result.json").read_text())
    assert raw["calls"] == raw["activities"] == 128000
    assert raw["outcomes"] == (128000 if mode == "paired" else 0)
    assert raw["overlap_per_worker"] == overlap
    assert all(sample["rss_kib"] is not None for sample in raw["samples"]), "RSS was unavailable; retain this incomplete run without claiming a resource pass"
    shutil.copy2(output / "result.json", WORK / f"{key}.json")
    print(f"PASS {key}: one executed test, 128000 calls, {len(raw['samples'])} samples", flush=True)


def summarize():
    prepared = json.loads((WORK / "preparation.json").read_text())
    source_match = json.loads((WORK / "candidate-source-match.json").read_text())
    assert all(row["matches_frozen_measurement"] for row in source_match["files"])
    results = []
    for overlap in (1, 8, 32, 128):
        for mode in ("paired", "unpaired"):
            key = f"{mode}-{overlap}"
            raw = json.loads((WORK / f"{key}.json").read_text())
            execution = json.loads((WORK / f"{key}.run.json").read_text())
            timing = (WORK / f"{key}.time.log").read_text()
            times = re.search(r"([\d.]+) real\s+([\d.]+) user\s+([\d.]+) sys", timing)
            peak = re.search(r"(\d+)\s+maximum resident set size", timing)
            assert times and peak
            samples = raw["samples"]
            assert len(samples) == (2560 // overlap) * (2 if mode == "paired" else 1)
            assert execution["executed_tests"] == 1 and execution["exit_code"] == 0
            if mode == "paired":
                assert all(s["evicted"] == s["oversized"] == s["ambiguous"] == 0 for s in samples)
                assert all(s["entries"] == s["lookup_capacity"] == s["order_entries"] == 0 for s in samples if s["phase"] == 1)
            else:
                assert samples[-1]["entries"] == 8192
                assert samples[-1]["evicted"] == 128000 - 8192
            polls = sorted(s["poll_ms"] for s in samples)
            results.append({
                "mode": mode, "overlap_per_worker": overlap,
                "calls": raw["calls"], "activities": raw["activities"], "outcomes": raw["outcomes"],
                "executed_tests": 1, "sample_count": len(samples),
                "peak_entries": max(s["entries"] for s in samples),
                "peak_owned_string_bytes": max(s["owned_string_bytes"] for s in samples),
                "peak_lookup_capacity": max(s["lookup_capacity"] for s in samples),
                "final_entries": samples[-1]["entries"],
                "final_lookup_capacity": samples[-1]["lookup_capacity"],
                "final_retirements": samples[-1]["evicted"],
                "peak_sampled_rss_kib": max(s["rss_kib"] for s in samples),
                "process_peak_rss_bytes": int(peak[1]),
                "poll_p95_ms_nearest_rank": polls[math.ceil(len(polls) * 0.95) - 1],
                "poll_total_seconds": sum(polls) / 1000,
                "replay_elapsed_seconds": raw["elapsed_seconds"],
                "process_wall_seconds": float(times[1]),
                "process_user_cpu_seconds": float(times[2]),
                "process_system_cpu_seconds": float(times[3]),
                "raw_result_sha256": sha(WORK / f"{key}.json"),
            })
    dump(WORK / "RESULTS.json", {
        "schema_version": 1, "status": "verified",
        "revision": REVISION, "platform": prepared["platform"],
        "profile": "Native macOS ARM64 Rust debug test binaries",
        "integrated_candidate": source_match["integrated_candidate"],
        "matching_integrated_source_files": source_match["matched_files"],
        "limits": {"cursor_entries": 256, "cursor_owned_string_bytes": 262144, "source_entries": 8192, "source_owned_string_bytes": 8388608},
        "recorded_at": datetime.now(timezone.utc).isoformat(),
        "resource_cases": results,
        "focused_unit_tests": {"claude": 14, "collector_library_total_including_claude": 27, "ignored_resource_test": 1},
        "measurement_limits": [
            "Synthetic native macOS ARM64 debug test processes; no authenticated provider or physical terminal.",
            "Each case has 50 workers and 128000 tool starts; overlap is fixture concurrency, not a real-user distribution.",
            "RSS samples are taken after each poll. Owned strings exclude allocator metadata and inline data structures.",
            "Process CPU includes fixture generation and sampling overhead. Poll timing excludes those operations.",
            "The probe retains every sample and serializes the complete result after the final RSS sample; process high-water RSS includes that measurement storage.",
            "Nearest-rank p95 values have not been calibrated against physical-terminal input response.",
            "Sequential replay cases run on a shared development machine; filesystem caches and unrelated work are not isolated.",
            "No new two-hour soak, Windows/Linux RSS curves or heap profiling.",
        ],
    })
    for result in results:
        print(f"{result['mode']:8} {result['overlap_per_worker']:3}: peak {result['peak_entries']:4} entries, {result['poll_p95_ms_nearest_rank']:.3f} ms poll p95, {result['process_user_cpu_seconds']:.2f}+{result['process_system_cpu_seconds']:.2f} CPU seconds")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    sub.add_parser("prepare")
    sub.add_parser("summarize")
    matching = sub.add_parser("match-source")
    matching.add_argument("--candidate", required=True)
    command = sub.add_parser("run")
    command.add_argument("--mode", choices=("paired", "unpaired"), required=True)
    command.add_argument("--overlap", type=int, choices=(1, 8, 32, 128), required=True)
    args = parser.parse_args()
    if args.action == "prepare":
        prepare()
    elif args.action == "summarize":
        summarize()
    elif args.action == "match-source":
        match_sources(args.candidate)
    else:
        run(args.mode, args.overlap)


if __name__ == "__main__":
    main()
