use super::*;

#[test]
fn c2_clocked_timeline_keeps_ingress_and_flush_calculable() {
    let timeline = vec![
        json!({
            "sequence": 1,
            "event_kind": "text_delta",
            "size_bytes": 12,
            "ingress_ms": 37,
            "runtime_append_ms": 40
        }),
        json!({
            "sequence": 2,
            "event_kind": "finish",
            "size_bytes": 8,
            "ingress_ms": 52,
            "runtime_append_ms": 53
        }),
    ];

    assert_eq!(first_runtime_ingress_ms(&timeline), Some(37));
    assert_eq!(max_runtime_flush_ms(&timeline), Some(3));

    let mut metadata = json!({});
    attach_gateway_stage_timing(&mut metadata, 11, Some(37), Some(3)).unwrap();
    let receipt = &metadata[GATEWAY_PROVIDER_STAGE_TIMING_METADATA_KEY];
    assert_eq!(receipt["flow_ms"], 11);
    assert_eq!(receipt["ingress_ms"], 37);
    assert_eq!(receipt["flush_ms"], 3);
    let serialized = serde_json::to_string(receipt).unwrap();
    for forbidden in [
        "credential",
        "prompt",
        "tool_output",
        "encrypted_content",
        "cursor",
        "response_id",
    ] {
        assert!(!serialized.contains(forbidden));
    }
}

#[test]
fn c2_gateway_timing_wraps_non_object_metadata_without_discarding_it() {
    let mut metadata = Value::Null;

    attach_gateway_stage_timing(&mut metadata, 11, Some(37), Some(3)).unwrap();

    assert_eq!(
        metadata["_1flowbase_upstream_provider_metadata"],
        Value::Null
    );
    assert_eq!(
        metadata[GATEWAY_PROVIDER_STAGE_TIMING_METADATA_KEY]["flow_ms"],
        11
    );
}
