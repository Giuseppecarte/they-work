#!/usr/bin/env python3
"""Exercise the real headless executable against a growing synthetic Claude store.

This measures collection/world lifecycle, not graphical terminal presentation.
All homes, projects and transcript writes are confined to the supplied scratch root.
"""
import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import time
from datetime import datetime, timezone


def stamp():
    return datetime.now(timezone.utc).isoformat()


def record(worker, cwd, seq, at, kind="Read"):
    inputs = {"file_path": f"src/fixture_{worker:02}.rs"}
    if kind == "Bash":
        inputs = {"command": "echo synthetic-observation-only"}
    return {
        "type": "assistant", "timestamp": at, "sessionId": f"audit-{worker:04}",
        "cwd": str(cwd), "gitBranch": "audit-fixture", "uuid": f"w{worker}-e{seq}",
        "message": {"content": [{"type": "tool_use", "id": f"call-{worker}-{seq}",
                                  "name": kind, "input": inputs}],
                    "stop_reason": "tool_use"},
    }


def prepare(root, workers=50, historical=1000):
    home = root / "claude"
    now = int(time.time() * 1000)
    paths = []
    for worker in range(workers + historical):
        project = root / "projects" / f"project-{worker % 20:02}"
        project.mkdir(parents=True, exist_ok=True)
        project_key = str(project).replace("/", "-")
        path = home / "projects" / project_key / f"audit-{worker:04}.jsonl"
        path.parent.mkdir(parents=True, exist_ok=True)
        active = worker < workers
        at = now if active else now - 3 * 24 * 3600 * 1000
        title = {"type": "system", "timestamp": at, "sessionId": f"audit-{worker:04}",
                 "cwd": str(project), "customTitle": f"Synthetic task {worker:04}"}
        with path.open("w") as stream:
            stream.write(json.dumps(title) + "\n")
            stream.write(json.dumps({"type": "user", "timestamp": at - 201,
                                     "sessionId": f"audit-{worker:04}", "cwd": str(project),
                                     "message": {"content": [{"type": "text", "text": "Synthetic work request"}]}}) + "\n")
            for seq in range(200 if active else 30):
                stream.write(json.dumps(record(worker, project, seq, at - 200 + seq)) + "\n")
        if active:
            paths.append((worker, project, path))
        else:
            os.utime(path, (at / 1000, at / 1000))
    return home, paths


def ps_sample(pid):
    result = subprocess.run(["ps", "-p", str(pid), "-o", "rss=,time=,%cpu="],
                            capture_output=True, text=True)
    parts = result.stdout.split()
    return parts if len(parts) == 3 else ["", "", ""]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--scratch", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--seconds", type=int, default=7200)
    args = parser.parse_args()
    args.binary = args.binary.resolve()
    args.scratch = args.scratch.resolve()
    args.output.mkdir(parents=True, exist_ok=True)
    args.scratch.mkdir(parents=True, exist_ok=True)
    home, workers = prepare(args.scratch)
    command = [str(args.binary), "--sources", "claude", "--claude-home", str(home),
               "--config-dir", str(args.scratch / "config"), "--no-save"]
    env = dict(os.environ)
    for key in ["THEYWORK_CLAUDE_HOME", "THEYWORK_CODEX_HOME", "THEYWORK_CONFIG_DIR"]:
        env.pop(key, None)
    metadata = {"started_at": stamp(), "platform": platform.platform(),
                "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
                "duration_requested_seconds": args.seconds, "expected_active_workers": 50,
                "expected_projects": 20, "historical_conversations": 1000,
                "command": command + ["--headless", "--exit-after", f"{args.seconds}s"],
                "scope": "Real executable headless collection/world soak with synthetic Claude files; no renderer, real terminal or provider.",
                "status": "running"}
    start_probe = time.perf_counter()
    initial = subprocess.run(command + ["--once"], capture_output=True, text=True, env=env)
    metadata["initial_once_seconds"] = time.perf_counter() - start_probe
    (args.output / "initial-once.txt").write_text(initial.stdout + initial.stderr)
    if initial.returncode or "projects=20 workers=50" not in initial.stdout:
        metadata.update(status="fixture-failed", initial_exit=initial.returncode)
        (args.output / "result.json").write_text(json.dumps(metadata, indent=2) + "\n")
        raise SystemExit("Fixture must expose exactly 20 projects and 50 active workers")
    (args.output / "result.json").write_text(json.dumps(metadata, indent=2) + "\n")
    ticks, samples, peak_rss = 0, 0, 0
    started = time.monotonic()
    with (args.output / "headless.txt").open("w") as stdout, \
            (args.output / "resources.csv").open("w", newline="") as resource_file:
        process = subprocess.Popen(metadata["command"], stdout=stdout, stderr=subprocess.STDOUT, env=env)
        metadata["pid"] = process.pid
        (args.output / "result.json").write_text(json.dumps(metadata, indent=2) + "\n")
        resource_writer = csv.writer(resource_file)
        resource_writer.writerow(["elapsed_seconds", "rss_kib", "cpu_time", "cpu_percent", "writer_ticks"])
        next_write, next_sample, next_progress = started, started, started
        print(json.dumps({"started": stamp(), "pid": process.pid, "duration": args.seconds}), flush=True)
        while process.poll() is None:
            current = time.monotonic()
            if current >= next_write:
                ticks += 1
                at = int(time.time() * 1000)
                for worker, project, path in workers:
                    with path.open("a") as stream:
                        stream.write(json.dumps(record(worker, project, ticks + 1000, at,
                                                       ["Read", "Edit", "Bash"][ticks % 3])) + "\n")
                next_write = current + 2
            if current >= next_sample:
                rss, cpu_time, cpu_percent = ps_sample(process.pid)
                resource_writer.writerow([round(current - started, 3), rss, cpu_time, cpu_percent, ticks])
                resource_file.flush()
                peak_rss = max(peak_rss, int(rss or 0))
                samples += 1
                next_sample = current + 30
            if current >= next_progress:
                progress = {"elapsed_seconds": round(current - started, 2), "ticks": ticks,
                            "appended_events": ticks * 50, "peak_rss_kib": peak_rss,
                            "sample_count": samples, "pid": process.pid, "status": "running"}
                (args.output / "progress.json").write_text(json.dumps(progress, indent=2) + "\n")
                print(json.dumps(progress), flush=True)
                next_progress = current + 300
            if current - started > args.seconds + 120:
                process.terminate()
                try:
                    process.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
                metadata["termination_reason"] = "Exceeded requested duration plus 120-second shutdown allowance"
                break
            time.sleep(0.2)
        metadata["exit_code"] = process.wait()
    elapsed = time.monotonic() - started
    final = subprocess.run(command + ["--once"], capture_output=True, text=True, env=env)
    (args.output / "final-once.txt").write_text(final.stdout + final.stderr)
    headless = (args.output / "headless.txt").read_text()
    counters = dict(re.findall(r"([a-z_]+)=([0-9.]+)", headless))
    metadata.update(finished_at=stamp(), actual_process_seconds=elapsed, writer_ticks=ticks,
                    appended_events=ticks * 50, peak_rss_kib=peak_rss, sample_count=samples,
                    final_once_exit=final.returncode, counters=counters,
                    fixture_bytes=sum(p.stat().st_size for p in home.rglob("*.jsonl")))
    valid = (metadata["exit_code"] == 0 and elapsed >= args.seconds and final.returncode == 0
             and "projects=20 workers=50" in final.stdout and counters.get("poll_errors") == "0"
             and counters.get("initial_workers") == "50" and counters.get("final_workers") == "50")
    metadata["status"] = "pass" if valid else "defect-or-unexpected-result"
    (args.output / "result.json").write_text(json.dumps(metadata, indent=2) + "\n")
    print(json.dumps(metadata, indent=2), flush=True)
    raise SystemExit(0 if valid else 1)


if __name__ == "__main__":
    main()
