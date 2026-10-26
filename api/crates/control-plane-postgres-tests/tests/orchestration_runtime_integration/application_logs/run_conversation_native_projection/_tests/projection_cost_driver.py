"""Run saved production-entry test binaries sequentially against an owned PG.

No credentials or services are discovered from development .env files. The
container record must identify the dedicated, fixture-only PostgreSQL instance.
"""
import argparse
import hashlib
import itertools
import json
import math
import os
from pathlib import Path
import statistics
import subprocess
import time


TEST = "application_logs::run_conversation_native_projection::incremental::projection_cost::native_projection_cost_benchmark"


def percentile(values, q):
    ordered = sorted(values)
    position = (len(ordered) - 1) * q
    low, high = math.floor(position), math.ceil(position)
    return ordered[low] + (ordered[high] - ordered[low]) * (position - low)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifact-root", type=Path, required=True)
    parser.add_argument("--histories", default="32,256,1024")
    parser.add_argument("--deltas", default="1,8")
    parser.add_argument("--concurrencies", default="1,2")
    parser.add_argument("--rounds", type=int, default=20)
    parser.add_argument("--item-bytes", type=int, default=16384)
    parser.add_argument("--run-label", default="resource-matrix")
    args = parser.parse_args()
    root = args.artifact_root.resolve()
    meta = json.loads((root / "postgres-container.json").read_text())
    inspection = json.loads(subprocess.check_output(["docker", "inspect", meta["id"]]))[0]
    assert inspection["Id"] == meta["id"] and inspection["State"]["Running"]
    assert inspection["Config"]["Labels"]["oneflowbase.owner"] == "gateway-projection-cost-20261026"
    assert inspection["Image"] == meta["image_id"]
    # This publicly declared fixture password belongs only to the owned test DB.
    database_url = f"postgres://postgres:fixture-only@127.0.0.1:{meta['port']}/postgres"
    matrix = list(itertools.product(
        map(int, args.histories.split(",")), map(int, args.deltas.split(",")),
        map(int, args.concurrencies.split(","))))
    binaries = {name: root / "bin" / name for name in ["baseline", "candidate"]}
    manifest = {"matrix": matrix, "rounds": args.rounds, "item_bytes": args.item_bytes,
                "order": ["baseline", "candidate", "candidate", "baseline"],
                "binaries_sha256": {name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in binaries.items()},
                "postgres_image_id": meta["image_id"], "postgres_container_id": meta["id"],
                "started_at_epoch": time.time(), "processes_planned": len(matrix) * 4,
                "scope": "actual repository transaction; isolated deterministic facts, no upstream/network model"}
    (root / f"{args.run_label}-manifest.json").write_text(json.dumps(manifest, indent=2))
    raw = root / args.run_label
    raw.mkdir(exist_ok=True)
    summaries = []
    for history, delta, concurrency in matrix:
        samples = {name: [] for name in binaries}
        for ordinal, variant in enumerate(manifest["order"]):
            stem = f"n{history}-d{delta}-c{concurrency}-{ordinal}-{variant}"
            result_path = raw / f"{stem}.json"
            assert not result_path.exists(), f"preserve existing evidence: {result_path}"
            environment = dict(os.environ, DATABASE_URL=database_url,
                NATIVE_PROJECTION_BENCH_OUT=str(result_path),
                NATIVE_PROJECTION_BENCH_HISTORY=str(history),
                NATIVE_PROJECTION_BENCH_DELTA=str(delta),
                NATIVE_PROJECTION_BENCH_CONCURRENCY=str(concurrency),
                NATIVE_PROJECTION_BENCH_ROUNDS=str(args.rounds),
                NATIVE_PROJECTION_BENCH_ITEM_BYTES=str(args.item_bytes),
                NATIVE_PROJECTION_BENCH_STATEMENTS="1",
                NATIVE_PROJECTION_BENCH_PG_CPU_STAT=meta["cpu_stat_path"])
            command = ["/usr/bin/time", "-v", "-o", str(raw / f"{stem}.resources"),
                       str(binaries[variant]), "--ignored", "--exact", TEST, "--nocapture"]
            with (raw / f"{stem}.log").open("w") as log:
                completed = subprocess.run(command, env=environment, stdout=log, stderr=subprocess.STDOUT, timeout=600)
            assert completed.returncode == 0, f"{stem} failed; see raw log"
            result = json.loads(result_path.read_text())
            assert (result["history_N_per_run"], result["delta_per_update"], result["concurrency"]) == (history, delta, concurrency)
            result["raw_file"] = str(result_path.relative_to(root))
            samples[variant].append(result)
            print(f"{stem}: {result['updates']} updates; {result['wall_ms']:.1f} ms; cpu_ticks={result['cpu_ticks']}", flush=True)
        for variant, runs in samples.items():
            updates = sum(run["updates"] for run in runs)
            walls = [sample for run in runs for sample in run["samples_ms"]]
            app_cpu = sum(run["cpu_ticks"] / int(run["clock_ticks_per_second"]) for run in runs)
            pg_cpu = sum((run["postgres_cluster_cpu_usec_after"] - run["postgres_cluster_cpu_usec_before"]) / 1e6 for run in runs)
            counts = {}
            for run in runs:
                for counter in run["dml_counters"]:
                    key = f"{counter['relation']}.{counter['operation']}"
                    counts[key] = counts.get(key, 0) + counter["tuples"]
            summaries.append({"variant": variant, "history_N_per_run": history,
                "delta_per_update": delta, "concurrency": concurrency, "process_samples": len(runs),
                "update_samples_N": len(walls), "app_cpu_core_seconds_per_update": app_cpu / updates,
                "postgres_cpu_core_seconds_per_update": pg_cpu / updates,
                "combined_cpu_core_seconds_per_update": (app_cpu + pg_cpu) / updates,
                "wall_p50_ms": statistics.median(walls), "wall_p95_ms": percentile(walls, .95),
                "process_peak_RSS_KiB": max(run["memory_after"]["VmHWM_KiB"] for run in runs),
                "top_level_queries_per_update": sum(run["query_statistics"]["top_level_calls"] for run in runs) / updates,
                "top_level_select_queries_per_update": sum(run["query_statistics"]["top_level_select_calls"] for run in runs) / updates,
                "top_level_wal_bytes_per_update": sum(run["query_statistics"]["top_level_wal_bytes"] for run in runs) / updates,
                "tuple_writes": counts,
                "storage_deltas": [{key: run["storage_after"][key] - run["storage_before"][key] for key in run["storage_before"]} for run in runs],
                "raw_files": [run["raw_file"] for run in runs]})
        (root / f"{args.run_label}-summary.json").write_text(json.dumps(summaries, indent=2))
    manifest["finished_at_epoch"] = time.time()
    (root / f"{args.run_label}-manifest.json").write_text(json.dumps(manifest, indent=2))


if __name__ == "__main__":
    main()
