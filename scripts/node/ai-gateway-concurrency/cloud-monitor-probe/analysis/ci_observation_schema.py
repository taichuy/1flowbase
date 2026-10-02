"""Pure adapter/reviewer for CI observation.js export; no collector or IO."""
from bisect import bisect_right
from analyze import matched_trace


def exact_ns(value):
    if not isinstance(value,str) or not value.isdecimal():
        raise ValueError("exact decimal ns string required")
    return int(value)


def review(observation, correlation, loop_rows=(), loop_interval_ms=50):
    errors=[]
    if observation.get("probe_id") != correlation.get("probe_id"):
        errors.append("probe_id mismatch")
    trace=dict(correlation)
    trace["stream_id"]=correlation.get("probe_id")
    try:
        trace["max_clock_error_ns"]=exact_ns(correlation.get("max_clock_error_ns"))
    except ValueError:
        errors.append("clock error must be exact decimal ns")
    deltas=observation.get("delta_trace",[])
    count=len(deltas)
    if [d.get("delta_index") for d in deltas] != list(range(count)):
        errors.append("client exact ordered delta indices required")
    if [d.get("parse_ns") for d in deltas] != correlation.get("client_receive_ns"):
        errors.append("correlation parse timestamp mismatch")
    if correlation.get("delta_sequence") != list(range(count)):
        errors.append("correlation sequence mismatch")
    matched=matched_trace(trace,count)
    if matched["status"] != "matched_boundary_observation":
        errors.append("source/client correlation incomplete or invalid")
    raw=observation.get("raw_data",[])
    try:
        starts=[exact_ns(r.get("receive_ns")) for r in raw]
        parsed=[exact_ns(d.get("parse_ns")) for d in deltas]
        if not starts or starts!=sorted(set(starts)) or parsed!=sorted(parsed):
            raise ValueError("ordered raw callback/parse timestamps required")
        if any(not isinstance(r.get("bytes"),int) or isinstance(r.get("bytes"),bool) or r["bytes"]<=0 for r in raw):
            raise ValueError("positive callback bytes required")
        owner=[bisect_right(starts,t)-1 for t in parsed]
        if any(i<0 for i in owner):
            raise ValueError("parse before first raw callback")
    except ValueError as e:
        errors.append(str(e));starts=[];parsed=[];owner=[]
    ends=[]
    if raw and all("callback_exit_ns" in r for r in raw):
        try:
            ends=[exact_ns(r["callback_exit_ns"]) for r in raw]
            if any(e<s or (i+1<len(starts) and e>starts[i+1]) for i,(s,e) in enumerate(zip(starts,ends))):
                raise ValueError("overlapping or negative raw callback duration")
            if any(t>ends[i] for t,i in zip(parsed,owner)):
                raise ValueError("parse outside associated raw callback")
        except (ValueError,IndexError) as e:
            errors.append(str(e));ends=[]
    if errors:
        return {"status":"invalid_or_incomplete","errors":errors,"matched_trace":matched}
    result={"status":"boundary_review","callback_mapping":"sync parse latest raw callback start; complete-frame bytes may span earlier callbacks",
            "callback_duration_evidence":"complete" if ends else "missing_callback_exit_ns",
            "matched_trace":matched}
    if len(parsed)>1:
        j=max(range(len(parsed)-1),key=lambda i:parsed[i+1]-parsed[i])
        a,b=owner[j],owner[j+1];gap=parsed[j+1]-parsed[j]
        r={"after_delta_index":j,"to_delta_index":j+1,"parse_gap_ms":gap/1e6,
           "raw_callback_gap_ms":(starts[b]-starts[a])/1e6,
           "previous_parse_offset_ms":(parsed[j]-starts[a])/1e6,
           "next_parse_offset_ms":(parsed[j+1]-starts[b])/1e6,
           "same_raw_callback":a==b}
        if ends and a!=b:
            parts=[ends[a]-parsed[j],starts[b]-ends[a],parsed[j+1]-starts[b]]
            assert sum(parts)==gap
            r["partition_ms"]={k:v/1e6 for k,v in zip(("previous_callback_remaining","between_callbacks","next_callback_before_parse"),parts)}
        elif ends:
            r["partition_ms"]={"within_same_callback":gap/1e6}
        lag=[];coverage=False
        try:
            rows=[(exact_ns(x["timestamp_ns"]),exact_ns(x["lag_ns"])) for x in loop_rows]
            if rows:
                if [x[0] for x in rows]!=sorted(set(x[0] for x in rows)):
                    raise ValueError("unordered loop probe")
                coverage=rows[0][0]<=parsed[j] and rows[-1][0]>=parsed[j+1]
                # A delayed timer can fire just AFTER the observed delta gap.
                lag=[delay for timestamp,delay in rows if timestamp>=parsed[j] and timestamp-delay<=parsed[j+1]]
        except (ValueError,KeyError) as e:
            result["loop_validation_error"]=str(e)
        r["loop_probe"]={"brackets_gap":coverage,"interval_ms":loop_interval_ms,
                         "overlapping_timer_lag_max_ms":max(lag)/1e6 if lag else None,
                         "scope":"timer overshoot evidence, not CPU duration or socket-ready time"}
        result["largest_gap_review"]=r
    result["client_false_gap_exclusion"]="not yet proven; require callback exits and bracketed lag, compare raw-vs-parse partitions"
    return result
