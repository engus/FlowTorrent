"""Run bounded release probes and preserve client-only resource reports."""
import argparse
import csv
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", default="native")
    parser.add_argument("--seconds", type=int, default=15)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--scenarios", nargs="+", default=["empty", "active5", "paused20", "hidden5", "cycles5"],
                        choices=["empty", "active5", "paused20", "hidden5", "cycles5"])
    args = parser.parse_args()
    if not 10 <= args.seconds <= 86400:
        parser.error("seconds must be 10..86400")
    root = (args.output or Path(os.environ["RUNNER_TEMP"]) / "flowtorrent-resources").resolve()
    root.mkdir(exist_ok=False)
    suffix = ".exe" if sys.platform == "win32" else ""
    target_dir = Path("target") if args.target == "native" else Path("target") / args.target
    binary = (target_dir / "release" / "examples" / ("measure_resources" + suffix)).resolve()
    summary = []
    for scenario in args.scenarios:
        folder = root / scenario
        subprocess.run([str(binary), str(folder), scenario, str(args.seconds)], check=True,
                       timeout=max(360, args.seconds + 240))
        with (folder / "samples.csv").open(newline="") as source:
            rows = list(csv.DictReader(source))
        if len(rows) != args.seconds:
            raise RuntimeError(f"{scenario}: missing samples")
        visible = {row["visible"] for row in rows}
        if scenario == "hidden5" and visible != {"false"}:
            raise RuntimeError("hidden workload did not stay hidden")
        if scenario == "cycles5" and visible != {"false", "true"}:
            raise RuntimeError("show/hide cycle not observed")
        progress = int(rows[-1]["checked_bytes"]) - int(rows[0]["checked_bytes"])
        if scenario.endswith("5") and progress <= 0:
            raise RuntimeError(f"{scenario}: no positive transfer control")
        elapsed = float(rows[-1]["elapsed_seconds"]) - float(rows[0]["elapsed_seconds"])
        cpu = (int(rows[-1]["cpu_time_ns"]) - int(rows[0]["cpu_time_ns"])) / 1e9
        report = {"scenario": scenario, "samples": len(rows), "progress_delta_bytes": progress,
                  "cpu_percent_one_core": round(100 * cpu / elapsed, 3)}
        for metric in ("resident_bytes", "private_bytes", "physical_footprint_bytes"):
            values = [int(row[metric]) for row in rows if row[metric]]
            report[metric] = ({"median": statistics.median(values), "peak": max(values)}
                              if values else None)
        summary.append(report)
        (root / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(report), flush=True)


if __name__ == "__main__":
    main()
