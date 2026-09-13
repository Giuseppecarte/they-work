#!/usr/bin/env python3
"""Verify repaired admission against an actual binary and isolated fake provider.

Historical fixture helpers are reused unchanged. These assertions use the new
Rejected receipt contract. No personal stores or authenticated providers are used.
"""
import argparse
import importlib.util
import json
from pathlib import Path
import platform
import sys
import time

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent
LEGACY = ROOT / "docs/design-audit/iteration-7/reliability/run_reliability.py"
spec = importlib.util.spec_from_file_location("reliability_fixture", LEGACY)
fixture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixture)
MESSAGE = "Not sent: local state could not be saved. Restore storage access and submit again."


def run_case(name, binary, folder, output):
    checks = []
    host = fixture.Host(binary, folder, output, quota=4096 if name == "quota" else None)

    def check(label, passed, actual):
        checks.append(fixture.check(label, passed, actual))

    def rejected(response):
        value = response.get("value") or {}
        return not response.get("error") and value.get("status") == "Rejected" and value.get("detail") == MESSAGE

    try:
        host.start()
        request = host.start_request("failed-intent", "x" * 5000 if name == "quota" else "working")
        if name == "permission":
            host.state_dir.chmod(0o500)
        response = host.rpc(request)
        check("initial response is explicitly not sent", rejected(response), response)
        check("provider was never launched", not host.records(), host.counts())
        snapshot = host.snapshot()
        check("snapshot retains the rejection and original storage error",
              snapshot["operations"]["failed-intent"]["status"] == "Rejected" and bool(snapshot["last_error"]),
              {"status": snapshot["operations"]["failed-intent"]["status"], "last_error": snapshot["last_error"]})
        host.state_dir.chmod(0o700)
        retry = host.rpc(request)
        check("same ID remains rejected without executing", rejected(retry) and not host.records(), retry)
        changed = host.rpc(host.start_request("failed-intent", "different payload"))
        check("different payload cannot replace the operation", bool(changed["error"]) and not host.records(), changed)
        if name == "quota":
            # RLIMIT_FSIZE is a hard process limit. Only this isolated host is
            # restarted to remove it; this is not a disk-exhaustion experiment.
            host.stop()
            host.quota = None
            host.start()
            check("restart does not execute any instruction", not host.records(), host.counts())
        new_request = host.start_request("explicit-new-intent")
        submitted = host.rpc(new_request)
        check("explicit new ID executes once", (submitted.get("value") or {}).get("status") == "Confirmed"
              and host.counts()["turn/start"] == 1, submitted)
        duplicate = host.rpc(new_request)
        check("duplicate successful submission is not replayed",
              (duplicate.get("value") or {}).get("status") == "Confirmed" and host.counts()["turn/start"] == 1,
              host.counts())
        if name == "permission":
            host.stop()
            host.start()
            old = host.rpc(request)
            check("persisted rejection survives restart", rejected(old) and host.counts()["turn/start"] == 1, old)
        host.export()
        return checks
    finally:
        host.stop()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=HERE / "admission")
    args = parser.parse_args()
    binary = args.binary.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    scratch = ROOT / "target/audit/control-admission" / str(time.time_ns())
    scratch.mkdir(parents=True)
    results = {
        "binary_sha256": fixture.sha(binary), "binary": str(binary), "platform": platform.platform(),
        "kind": "executable supervisor with isolated fake provider", "authenticated_provider": False,
        "physical_disk_exhaustion": "not tested; quota uses RLIMIT_FSIZE", "cases": {},
    }
    for name in ("permission", "quota"):
        folder = scratch / name
        folder.mkdir()
        checks = run_case(name, binary, folder, output)
        results["cases"][name] = checks
        print(name, "pass" if all(check["status"] == "pass" for check in checks) else "FAIL", flush=True)
    fixture.save(output / "results.json", results)
    passed = all(check["status"] == "pass" for checks in results["cases"].values() for check in checks)
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
