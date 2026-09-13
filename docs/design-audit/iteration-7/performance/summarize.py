#!/usr/bin/env python3
"""Summarize measured soak samples without upgrading a running job to a pass."""
import csv
from datetime import datetime
import json
import math
from pathlib import Path
import statistics


def percentile(values, fraction):
    return sorted(values)[max(0, math.ceil(len(values) * fraction) - 1)]


def cpu_seconds(value):
    parts = value.split(":")
    return sum(float(part) * 60 ** index for index, part in enumerate(reversed(parts)))


def main():
    root = Path(__file__).resolve().parent
    run = root / "two-hour"
    result = json.loads((run / "result.json").read_text())
    rows = list(csv.DictReader((run / "resources.csv").open()))
    samples = [row for row in rows if row["rss_kib"] and row["cpu_time"]]
    elapsed = [float(row["elapsed_seconds"]) for row in samples]
    rss = [int(row["rss_kib"]) for row in samples]
    summary = {
        "scope": "Sampled resources of the actual headless collector/world process; no graphics or authenticated provider.",
        "status": result["status"],
        "binary_sha256": result["binary_sha256"],
        "requested_seconds": result["duration_requested_seconds"],
        "completed_seconds": result.get("actual_process_seconds"),
        "usable_resource_samples": len(samples),
        "missing_resource_samples": len(rows) - len(samples),
        "measurement_uncertainty": "ps samples approximately every 30 seconds can miss peaks; cumulative CPU is rounded by ps; other audit processes share the host.",
    }
    if samples:
        steady = [(t, memory) for t, memory in zip(elapsed, rss) if t >= 60]
        first = [memory for t, memory in steady if t <= 660]
        last = [memory for t, memory in steady if t >= max(60, elapsed[-1] - 600)]
        summary.update(
            sampled_seconds=elapsed[-1] - elapsed[0],
            sampled_max_rss_kib=max(rss),
            steady_rss_median_kib=statistics.median(memory for _, memory in steady) if steady else None,
            first_ten_steady_minutes_median_rss_kib=statistics.median(first) if first else None,
            last_ten_minutes_median_rss_kib=statistics.median(last) if last else None,
            sampled_cpu_percent_p95=percentile([float(row["cpu_percent"]) for row in samples], .95),
        )
        if len(samples) > 1 and elapsed[-1] > elapsed[0]:
            consumed = cpu_seconds(samples[-1]["cpu_time"]) - cpu_seconds(samples[0]["cpu_time"])
            summary["sampled_cumulative_cpu_seconds"] = consumed
            summary["mean_one_core_percent_from_cpu_delta"] = 100 * consumed / (elapsed[-1] - elapsed[0])
    for key in ["appended_events", "fixture_bytes", "writer_ticks", "exit_code", "counters"]:
        if key in result:
            summary[key] = result[key]
    if "finished_at" in result:
        summary["utc_wrapper_span_seconds"] = (datetime.fromisoformat(result["finished_at"]) - datetime.fromisoformat(result["started_at"])).total_seconds()
        summary["clock_boundary_note"] = "Process duration and resource intervals use monotonic time. UTC metadata spans the wrapper including initial/final probes; the observed difference between these clocks/boundaries was not calibrated or attributed."
    (run / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
