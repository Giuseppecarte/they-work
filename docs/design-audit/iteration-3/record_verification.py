#!/usr/bin/env python3
"""Summarize completed audit checks and bind the PTY smoke to the native binary."""
import datetime
import hashlib
import json
import pathlib
import platform
import re
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[3]
OUT = ROOT / "docs/design-audit/iteration-3/evidence"
BINARY = ROOT / "target/native-macos/release/they-work"


def suite(name):
    log = (OUT / name).read_text()
    results = re.findall(r"test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored", log)
    assert len(results) == 13, (name, "incomplete workspace log", len(results))
    assert all(status == "ok" and failed == "0" for status, _, failed, _ in results), name
    return {"log": name, "passed": sum(int(row[1]) for row in results),
            "failed": 0, "ignored": sum(int(row[3]) for row in results), "groups": len(results)}


def main():
    native = suite("native-workspace-tests.log")
    linux = suite("linux-workspace-tests.log")
    assert native["passed"] == linux["passed"] == 258
    assert native["ignored"] == linux["ignored"] == 2
    for name in ["native-clippy.log", "linux-clippy.log", "native-build.log"]:
        log = (OUT / name).read_text()
        assert "Finished" in log and "error:" not in log and "error[" not in log, name
    assert not (OUT / "fmt.log").read_text().strip(), "formatting check reported changes"
    digest = hashlib.sha256(BINARY.read_bytes()).hexdigest()
    graphics = json.loads((OUT / "graphics-pty/results.json").read_text())
    assert graphics["binary_sha256"] == digest, "graphics smoke used an older binary"
    assert graphics["exit"] == {"exit": 0, "terminal_restored": True}
    for overlay in ["help", "finder"]:
        assert graphics[overlay]["native_heading_after_every_image"]
    smoke = (OUT / "demo-once.txt").read_text()
    assert "projects=3 workers=6" in smoke and "fictional demo request" in smoke
    assert "status=failed" in smoke
    assert all(0 < int(tokens) < 100_000 for tokens in re.findall(r"tokens=(\d+)", smoke))
    source_changes = subprocess.check_output(["git", "diff", "HEAD", "--name-only", "--", "crates"], cwd=ROOT, text=True)
    assert not source_changes.strip(), "commit the verified source before recording its revision"
    record = {
        "recorded_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "branch": subprocess.check_output(["git", "branch", "--show-current"], cwd=ROOT, text=True).strip(),
        "native_platform": {"system": platform.system(), "machine": platform.machine()},
        "native_workspace": native, "linux_workspace": linux,
        "strict_clippy": {"native": "passed", "linux": "passed"}, "format": "passed",
        "native_release": {"path": str(BINARY.relative_to(ROOT)), "sha256": digest, "bytes": BINARY.stat().st_size},
        "final_smokes": ["cli-help.txt", "demo-once.txt", "graphics-pty/results.json"],
        "source_setup_pty": {"sessions": 13, "exit_and_terminal_restoration": "passed"},
        "finder_pty": "all cases passed", "published": False,
        "limits": ["Two opt-in live-store tests ignored; synthetic fixtures were used.",
                   "PTY/font replays and controlled protocol responses are not terminal-window screenshots.",
                   "Windows/WSL, Linux graphical terminals, Kitty and iTerm2 visual sessions remain unmeasured."]
    }
    (OUT / "verification.json").write_text(json.dumps(record, indent=2) + "\n")
    print(json.dumps(record, indent=2))


if __name__ == "__main__":
    main()
