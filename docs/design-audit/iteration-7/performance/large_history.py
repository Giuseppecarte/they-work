#!/usr/bin/env python3
"""Bounded synthetic history scaling, using the real CLI and direct source probe."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import subprocess
import time

from soak import prepare, record


def percentile(values, p):
    values = sorted(values)
    return values[max(0, math.ceil(len(values) * p) - 1)]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--sizes", default="10,100,1000")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[4]
    out = Path(__file__).resolve().parent / "large-history"
    scratch = root / "docs/design-audit/tmp/iteration-7-performance/large-history"
    out.mkdir(parents=True, exist_ok=True)
    scratch.mkdir(parents=True, exist_ok=True)
    cli = root / "target/native-macos/release/they-work"
    probe = root / "target/native-macos/release/theywork-source-performance-audit"
    metadata = {"platform": platform.platform(), "cli_sha256": hashlib.sha256(cli.read_bytes()).hexdigest(),
                "probe_sha256": hashlib.sha256(probe.read_bytes()).hexdigest(), "cases": [],
                "scope": "Fresh collector state, OS page cache uncontrolled/warm; headless two-hour process concurrent. CLI startup includes initial scan and text output, not a first displayed frame."}
    for size in map(int, args.sizes.split(',')):
        case = scratch / f"{size}MiB"
        home, workers = prepare(case, workers=50, historical=1000)
        worker, project, transcript = workers[0]
        # One realistically shaped large tool-result body. Content is synthetic;
        # no command is run. Many small records are a separate workload from this.
        value = {"type": "user", "timestamp": int(time.time()*1000), "sessionId": "audit-0000",
                 "cwd": str(project), "message": {"content": [{"type":"tool_result",
                 "tool_use_id":"fixture-call", "content":"synthetic output " * 2048}]}}
        line = (json.dumps(value) + '\n').encode()
        target = size * 1024 * 1024
        with transcript.open('ab') as stream:
            while stream.tell() < target:
                stream.write(line)
            stream.write((json.dumps(record(worker, project, 999999, int(time.time()*1000)))+'\n').encode())
        fixture_bytes = sum(p.stat().st_size for p in home.rglob('*.jsonl'))
        command = [str(cli),'--sources','claude','--claude-home',str(home),
                   '--config-dir',str(case/'config'),'--no-save','--once']
        started = time.perf_counter()
        result = subprocess.run(['/usr/bin/time','-l']+command,capture_output=True,text=True,timeout=300)
        cli_seconds = time.perf_counter()-started
        (out/f'{size}MiB-cli.txt').write_text(result.stdout)
        (out/f'{size}MiB-time.txt').write_text(result.stderr)
        started = time.perf_counter()
        measured = subprocess.run([str(probe),str(home),str(transcript),str(project)],capture_output=True,text=True,timeout=300)
        if measured.returncode:
            (out/f'{size}MiB-probe-error.txt').write_text(measured.stdout+measured.stderr)
            raise SystemExit(measured.returncode)
        stats = json.loads(measured.stdout)
        (out/f'{size}MiB-polls.json').write_text(json.dumps(stats,indent=2)+'\n')
        rss = re.search(r'(\d+)\s+maximum resident set size',result.stderr)
        item = {'transcript_target_mib':size,'fixture_bytes':fixture_bytes,'cli_seconds':cli_seconds,
                'cli_exit':result.returncode,'cli_peak_rss_bytes':int(rss.group(1)) if rss else None,
                'collector_cold_ms':stats['cold_ms'],'world_fold_ms':stats['fold_ms'],
                'quiet_poll_p95_ms':percentile(stats['quiet_poll_ms'],.95),
                'append_poll_p95_ms':percentile(stats['append_poll_ms'],.95),
                'final_workers':stats['final_workers'],'final_projects':stats['final_projects'],
                'probe_total_seconds':time.perf_counter()-started}
        metadata['cases'].append(item)
        (out/'results.json').write_text(json.dumps(metadata,indent=2)+'\n')
        print(json.dumps(item),flush=True)
        assert result.returncode==0 and stats['final_workers']==50 and stats['final_projects']==20


if __name__ == '__main__':
    main()
