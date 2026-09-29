use super::*;

#[test]
fn empty_stream_has_no_timing_and_details_require_opt_in() {
    let timing = ProviderStreamTiming::new(false);
    assert_eq!(timing.event_count(), 0);
    assert_eq!(timing.first_ingress_ms(), None);
    assert_eq!(timing.max_append_delay_ms(), None);
    assert_eq!(
        timing.summary(),
        json!({
            "schema_version": 1,
            "capture_mode": "summary",
            "event_count": 0,
            "total_size_bytes": 0,
            "first_ingress_ms": null,
            "last_ingress_ms": null,
            "max_append_delay_ms": null,
            "event_kind_counts": {},
        })
    );
    assert_eq!(timing.into_details(), None);
    let detailed = ProviderStreamTiming::new(true);
    assert_eq!(detailed.summary()["capture_mode"], "detailed");
    assert_eq!(detailed.into_details(), Some(json!([])));
}

#[test]
fn non_monotonic_timestamps_preserve_extrema_and_ignore_negative_delays() {
    let mut timing = ProviderStreamTiming::new(false);
    timing.observe(1, "text_delta", 5, 90, 100).unwrap();
    timing.observe(2, "finish", 7, 10, 8).unwrap();
    timing.observe(3, "text_delta", 0, 70, 140).unwrap();
    timing.observe(4, "finish", 2, 50, 51).unwrap();
    assert_eq!(timing.event_count(), 4);
    assert_eq!(timing.first_ingress_ms(), Some(10));
    assert_eq!(timing.max_append_delay_ms(), Some(70));
    assert_eq!(
        timing.summary(),
        json!({
            "schema_version": 1,
            "capture_mode": "summary",
            "event_count": 4,
            "total_size_bytes": 14,
            "first_ingress_ms": 10,
            "last_ingress_ms": 90,
            "max_append_delay_ms": 70,
            "event_kind_counts": { "finish": 2, "text_delta": 2 },
        })
    );
    let mut negative_only = ProviderStreamTiming::new(false);
    negative_only.observe(1, "finish", 0, 20, 19).unwrap();
    assert_eq!(negative_only.max_append_delay_ms(), None);
}

#[test]
fn online_summary_matches_offline_original_array_and_detailed_keeps_every_record() {
    let mut summary = ProviderStreamTiming::new(false);
    let mut detailed = ProviderStreamTiming::new(true);
    let mut originals = Vec::new();
    for sequence in 0_u64..4096 {
        let event_kind = if sequence % 7 == 0 {
            "finish"
        } else {
            "text_delta"
        };
        let size_bytes = (sequence % 113) as usize;
        let ingress_ms = 5000 - sequence;
        let runtime_append_ms = if sequence % 11 == 0 {
            0
        } else {
            ingress_ms + sequence % 19
        };
        originals.push(json!({
            "sequence": sequence,
            "event_kind": event_kind,
            "size_bytes": size_bytes,
            "ingress_ms": ingress_ms,
            "runtime_append_ms": runtime_append_ms,
        }));
        summary
            .observe(
                sequence,
                event_kind,
                size_bytes,
                ingress_ms,
                runtime_append_ms,
            )
            .unwrap();
        detailed
            .observe(
                sequence,
                event_kind,
                size_bytes,
                ingress_ms,
                runtime_append_ms,
            )
            .unwrap();
    }
    let mut counts = BTreeMap::<String, u64>::new();
    for record in &originals {
        *counts
            .entry(record["event_kind"].as_str().unwrap().to_owned())
            .or_default() += 1;
    }
    let expected = json!({
        "schema_version": 1,
        "capture_mode": "summary",
        "event_count": originals.len(),
        "total_size_bytes": originals.iter().map(|record| record["size_bytes"].as_u64().unwrap()).sum::<u64>(),
        "first_ingress_ms": originals.iter().map(|record| record["ingress_ms"].as_u64().unwrap()).min(),
        "last_ingress_ms": originals.iter().map(|record| record["ingress_ms"].as_u64().unwrap()).max(),
        "max_append_delay_ms": originals.iter().filter_map(|record| record["runtime_append_ms"].as_u64().unwrap().checked_sub(record["ingress_ms"].as_u64().unwrap())).max(),
        "event_kind_counts": counts,
    });
    assert_eq!(summary.summary(), expected);
    let mut detailed_summary = detailed.summary();
    detailed_summary["capture_mode"] = json!("summary");
    assert_eq!(detailed_summary, expected);
    assert_eq!(detailed.into_details(), Some(Value::Array(originals)));
    assert_eq!(summary.into_details(), None);
}

#[test]
fn summary_storage_does_not_grow_with_event_count_or_capture_sensitive_body() {
    let sensitive_body = "sensitive answer text and user-token";
    let mut timing = ProviderStreamTiming::new(false);
    timing
        .observe(0, "text_delta", sensitive_body.len(), 0, 0)
        .unwrap();
    timing.observe(1, "finish", 0, 1, 1).unwrap();
    let kind_count = timing.event_kind_counts.len();
    let state_size = std::mem::size_of_val(&timing);
    for sequence in 2_u64..100_000 {
        timing
            .observe(
                sequence,
                "text_delta",
                sensitive_body.len(),
                sequence,
                sequence,
            )
            .unwrap();
    }
    assert!(
        timing.details.is_none(),
        "default mode must not allocate an event vector"
    );
    assert_eq!(timing.event_kind_counts.len(), kind_count);
    assert_eq!(std::mem::size_of_val(&timing), state_size);
    let receipt = timing.summary().to_string();
    assert!(!receipt.contains(sensitive_body));
    assert!(!receipt.contains("user-token"));
    assert!(!receipt.contains("body"));
    assert_eq!(timing.event_count(), 100_000);
    assert_eq!(timing.into_details(), None);
}

#[test]
fn integer_overflow_is_explicit_and_leaves_every_fact_unchanged() {
    for field in ["event_count", "total_size_bytes", "event_kind_counts"] {
        let mut timing = ProviderStreamTiming::new(true);
        timing.observe(1, "text_delta", 1, 2, 3).unwrap();
        match field {
            "event_count" => timing.event_count = u64::MAX,
            "total_size_bytes" => timing.total_size_bytes = u64::MAX,
            "event_kind_counts" => {
                timing.event_kind_counts.insert("text_delta", u64::MAX);
            }
            _ => unreachable!(),
        }
        let before = timing.summary();
        let details = timing.details.clone();
        assert!(timing.observe(2, "text_delta", 1, 0, 100).is_err());
        assert_eq!(timing.summary(), before);
        assert_eq!(timing.details, details);
    }
}

#[cfg(target_pointer_width = "64")]
#[test]
fn integer_receipts_keep_values_above_float_precision_exactly() {
    let size_bytes = (1_usize << 53) + 7;
    let mut timing = ProviderStreamTiming::new(true);
    timing
        .observe(u64::MAX, "finish", size_bytes, u64::MAX, u64::MAX)
        .unwrap();
    let summary = timing.summary();
    assert_eq!(
        summary["total_size_bytes"].as_u64(),
        Some(size_bytes as u64)
    );
    assert_eq!(summary["first_ingress_ms"].as_u64(), Some(u64::MAX));
    assert_eq!(summary["max_append_delay_ms"].as_u64(), Some(0));
    let details = timing.into_details().unwrap();
    assert_eq!(details[0]["sequence"].as_u64(), Some(u64::MAX));
    assert_eq!(details[0]["size_bytes"].as_u64(), Some(size_bytes as u64));
}
