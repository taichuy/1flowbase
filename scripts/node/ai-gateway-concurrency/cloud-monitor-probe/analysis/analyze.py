"""Offline, whitelisted-field analysis; no network, processes, DB or product imports."""
import argparse
import json
import math
import re
from statistics import mean


def number(v):
    return isinstance(v, (int, float)) and not isinstance(v, bool) and math.isfinite(v)


def stats(values):
    values = sorted(v for v in values if number(v))
    if not values:
        return {"n": 0}
    return {"n": len(values), "mean": mean(values), "max": values[-1],
            "p95_sample": values[math.ceil(.95 * len(values)) - 1]}


def matched_trace(trace, count):
    missing = []
    for k in ("stream_id", "correlation_evidence", "clock_id", "source_clock_id", "client_clock_id", "source_boundary", "client_boundary"):
        if not isinstance(trace.get(k), str) or not trace[k]:
            missing.append(k)
    if trace.get("correlation_evidence") != "explicit_same_stream_and_delta_sequence":
        missing.append("explicit_same_stream_and_delta_sequence")
    if trace.get("source_clock_id") != trace.get("clock_id") or trace.get("client_clock_id") != trace.get("clock_id"):
        missing.append("same_verified_clock_domain")
    arrays = [trace.get(k) for k in ("delta_sequence", "source_send_ns", "client_receive_ns")]
    if not all(isinstance(a, list) for a in arrays):
        return {"status": "unattributable", "missing": sorted(set(missing + ["matched_per_delta_arrays"]))}
    sequence, source, client = arrays
    if not isinstance(count, int) or isinstance(count, bool) or not len(sequence) == len(source) == len(client) == count:
        missing.append("equal_complete_delta_counts")
    if not all(isinstance(x, int) and not isinstance(x, bool) for x in sequence) or sequence != sorted(set(sequence)):
        missing.append("unique_ordered_delta_sequence")
    def ns(a):
        # Strings preserve ns beyond JS float precision. No floating-point ns accepted.
        if not all(isinstance(v, str) and v.isdecimal() for v in a):
            raise ValueError("exact_nonnegative_ns_strings")
        a = list(map(int, a))
        if a != sorted(a):
            raise ValueError("monotonic_delta_timestamps")
        return a
    try:
        source, client = ns(source), ns(client)
    except ValueError as e:
        missing.append(str(e))
    uncertainty = trace.get("max_clock_error_ns")
    if not isinstance(uncertainty, int) or isinstance(uncertainty, bool) or uncertainty < 0:
        missing.append("bounded_clock_error")
    if missing:
        return {"status": "unattributable", "missing": sorted(set(missing))}
    if any(c + uncertainty < s for s, c in zip(source, client)):
        return {"status": "invalid", "errors": ["receive_precedes_source_beyond_clock_bound"]}
    send_gaps = [(b-a)/1e6 for a,b in zip(source,source[1:])]
    receive_gaps = [(b-a)/1e6 for a,b in zip(client,client[1:])]
    return {"status": "matched_boundary_observation", "source_boundary": trace["source_boundary"],
            "client_boundary": trace["client_boundary"], "source_gap_ms": stats(send_gaps),
            "receive_gap_ms": stats(receive_gaps),
            "additional_gap_ms": stats([c-s for s,c in zip(send_gaps,receive_gaps)]),
            "observed_path_lag_ms": stats([(c-s)/1e6 for s,c in zip(source,client)]),
            "max_clock_error_ms": uncertainty/1e6,
            "causal_attribution": "path_between_declared_boundaries_only; no PG/network/host cause assignment"}


def request(row, index):
    out = {"request_index_within_phase": index, "success": row.get("ok") is True,
           "delta_count": row.get("delta_count"), "validation_errors": []}
    ttft, total, gaps = row.get("ttft_ms"), row.get("elapsed_ms"), row.get("interdelta_gap_ms")
    if not number(ttft) or not number(total) or not 0 <= ttft <= total:
        out["validation_errors"].append("0<=ttft<=elapsed required")
    if not isinstance(gaps, list) or not all(number(v) and v >= 0 for v in gaps):
        out["validation_errors"].append("nonnegative_finite_client_gap_array required")
        gaps = []
    count = row.get("delta_count")
    if not isinstance(count, int) or isinstance(count, bool) or count < 1:
        out["validation_errors"].append("positive_integer_delta_count required")
    elif len(gaps) != count-1:
        out["validation_errors"].append("delta_count_minus_one_gaps required")
    if not out["validation_errors"]:
        span = sum(gaps)
        tail = total - ttft - span
        if tail < -1e-6:
            out["validation_errors"].append("last_delta_after_request_end")
        else:
            out["client_partition_ms"] = {"before_first_delta": ttft, "first_to_last_delta": span,
                                          "last_delta_to_end": max(0,tail), "total": total}
    out["stream_contract"] = {k:row[k] for k in ("output_complete","completed","errors") if isinstance(row.get(k),(bool,int))}
    out["client_gap_ms"] = stats(gaps)
    out["gaps_over_1000ms"] = sum(v > 1000 for v in gaps)
    if gaps and not out["validation_errors"]:
        largest = max(range(len(gaps)), key=lambda i:gaps[i])
        start = ttft + sum(gaps[:largest])
        out["largest_client_gap"] = {
            "after_delta_ordinal_1based":largest+1,"to_delta_ordinal_1based":largest+2,
            "gap_ms":gaps[largest],"request_relative_start_ms":start,
            "request_relative_end_ms":start+gaps[largest],
            "gap_end_to_last_delta_ms":sum(gaps[largest+1:]),
            "scope":"client parsed delta boundaries only"}
    trace = row.get("observation_trace")
    out["trace"] = matched_trace(trace,count) if isinstance(trace,dict) else {
        "status": "unattributable", "missing": ["explicit_request_mapping", "source_per_delta_timestamps",
                                                   "shared_clock_identity_and_bound", "host_and_DB_stage_boundaries"]}
    return out


def phase(raw):
    rows = [r for r in raw.get("requests",[]) if isinstance(r,dict) and "output_complete" in r]
    analyzed = [request(r,i) for i,r in enumerate(rows)]
    out = {"name": raw.get("name"), "stream_requests": len(rows),
           "successes": sum(r["success"] for r in analyzed),
           "streams_with_gap_over_1000ms": sum(r["gaps_over_1000ms"] > 0 for r in analyzed),
           "invalid_client_partitions": sum(bool(r["validation_errors"]) for r in analyzed), "requests": analyzed,
           "database_aggregate": {k:v for k,v in raw.get("database_delta",{}).items() if number(v)},
           "database_attribution": "aggregate only; TopSQL/table/write-purpose/commit/drain causality unavailable"}
    out["database_validation_errors"] = ["negative aggregate delta: "+k for k,v in out["database_aggregate"].items() if v < 0]
    trace_ids = [r.get("observation_trace",{}).get("stream_id") for r in rows if isinstance(r.get("observation_trace"),dict)]
    if len(trace_ids) != len(set(trace_ids)):
        out["trace_correlation_error"] = "duplicate stream IDs in phase; request mapping ambiguous"
        for r in analyzed:
            r["trace"] = {"status":"unattributable","missing":["unique_same_stream_request_mapping"]}
    out["client_partition_mean_ms"] = {
        k: stats([r.get("client_partition_ms",{}).get(k) for r in analyzed])
        for k in ("before_first_delta","first_to_last_delta","last_delta_to_end","total")}
    resources = raw.get("resources",{})
    out["resource_window"] = {"wall_seconds":resources.get("wall_seconds"),
                              "scope":"phase including pacing/background; not per-stream demand"}
    out["resource_window"]["groups"] = {}
    for k,v in resources.get("groups",{}).items():
        if k in ("api","postgres","plugins","mock","driver","sampler"):
            out["resource_window"]["groups"][k] = {f:v[f] for f in (
                "cpu_seconds","average_cpu_percent_single_core","rss_sample_peak_bytes","pss_sample_peak_bytes")
                if number(v.get(f))}
    return out


def fixture(raw):
    phases = [phase(p) for p in raw.get("phases",[]) if isinstance(p,dict)]
    comparisons = []
    for p in phases:
        m = re.fullmatch(r"fixedstream_h(\d+)_c(\d+)",str(p["name"]))
        if not m:
            continue
        suffix = "h"+m[1]+"_c"+m[2]
        direct = [q for q in phases if q["name"] in ("direct_before_"+suffix,"direct_after_"+suffix)]
        valid_gw = [q["client_partition_ms"] for q in p["requests"] if q["success"] and "client_partition_ms" in q]
        valid_direct = [r["client_partition_ms"] for q in direct for r in q["requests"]
                        if r["success"] and "client_partition_ms" in r]
        if valid_gw and valid_direct:
            delta = {k:mean(r[k] for r in valid_gw)-mean(r[k] for r in valid_direct) for k in valid_gw[0]}
            comparisons.append({"gateway_phase":p["name"],"gateway_n":len(valid_gw),"direct_n":len(valid_direct),
                                "difference_of_means_ms":delta,
                                "scope":"pooled direct before/after; unpaired client-boundary differences, no causal stages"})
    return {"label":raw.get("label"),"kind":raw.get("kind"),"phases":phases,"direct_comparisons":comparisons,
            "legacy_source_evidence": {"request_timeline_count":len(raw.get("mock_request_timeline",[])),
                "global_source_gap_ms":stats(raw.get("actual_mock_interdelta_gap_ms",[])),
                "scope":"global source gaps lack request/phase mapping; cannot align to C8 client gaps"},
            "missing_for_PG_attribution":["per-queryid/toplevel stats snapshots","per-table deltas",
                "writer final integrity and drain proof","same-stream host ingress/append/commit/client timing"]}


def analyze(raw):
    fixtures = raw.get("results") if isinstance(raw.get("results"),list) else [raw]
    return {"schema":"gap-offline-v1","mode":"offline exported fixture JSON; no collectors",
            "fixtures":[fixture(f) for f in fixtures if isinstance(f,dict)],
            "warnings":["Small-N P95 is descriptive; failures and invalid rows retained",
                        "res.write boundary is not network flush; client callback can batch deltas",
                        "SQL aggregates and total CPU cannot causally assign interdelta gaps"]}


if __name__ == "__main__":
    parser=argparse.ArgumentParser()
    parser.add_argument("input");parser.add_argument("output")
    args=parser.parse_args()
    with open(args.input) as f: raw=json.load(f)
    with open(args.output,"w") as f: json.dump(analyze(raw),f,ensure_ascii=False,indent=2);f.write("\n")
