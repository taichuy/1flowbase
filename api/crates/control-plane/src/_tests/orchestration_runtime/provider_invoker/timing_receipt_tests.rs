use super::*;

#[test]
fn c2_clocked_timeline_keeps_ingress_and_flush_calculable() {
    let mut timing = ProviderStreamTiming::new(false);
    timing.observe(1, "text_delta", 12, 37, 40).unwrap();
    timing.observe(2, "finish", 8, 52, 53).unwrap();

    assert_eq!(timing.first_ingress_ms(), Some(37));
    assert_eq!(timing.max_append_delay_ms(), Some(3));

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
