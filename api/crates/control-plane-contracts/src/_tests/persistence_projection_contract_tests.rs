use serde_json::json;
use uuid::Uuid;

use crate::persistence_projection::{
    audit_row_hash, build_flow_run_title, derive_flow_run_title_from_input_payload,
    display_flow_run_title, trace_node_id_for_locator,
    trace_projection_source_watermark_from_counts, FLOW_RUN_TITLE_MAX_CHARS,
};

#[test]
fn flow_run_title_projection_keeps_normalization_fallback_and_query_traversal() {
    let explicit = format!("  {}tail  ", "界".repeat(FLOW_RUN_TITLE_MAX_CHARS + 5));
    assert_eq!(
        build_flow_run_title(Some(&explicit), "fallback"),
        "界".repeat(FLOW_RUN_TITLE_MAX_CHARS)
    );
    assert_eq!(build_flow_run_title(Some("  "), "  "), "Untitled run");

    let input = json!({
        "ignored": { "query": "deep fallback" },
        "node-start": { "inputs": [{ "query": "  preferred query  " }] }
    });
    assert_eq!(
        derive_flow_run_title_from_input_payload(&input).as_deref(),
        Some("preferred query")
    );
    assert_eq!(display_flow_run_title("", &input), "preferred query");
    assert_eq!(display_flow_run_title("", &json!({})), "Untitled run");
}

#[test]
fn audit_row_hash_keeps_deterministic_chaining_receipts() {
    let flow_run_id = Uuid::parse_str("018f8f0e-7bcd-7a51-9abc-1234567890ab").unwrap();
    let first = audit_row_hash(None, "flow_runs", flow_run_id, &json!(["succeeded", 7]));
    assert_eq!(
        first,
        "sha256:05f4e2462c6bb36b734c38d64b8545b8abec6cdfef73dc6334fcfda8f844e70d"
    );
    assert_eq!(
        audit_row_hash(None, "flow_runs", flow_run_id, &json!(["succeeded", 7])),
        first
    );

    let event_id = Uuid::parse_str("018f8f0e-7bcd-7a51-9abc-1234567890ac").unwrap();
    assert_eq!(
        audit_row_hash(
            Some(&first),
            "run_events",
            event_id,
            &json!(["completed", 8]),
        ),
        "sha256:a04065b42981562c8492557a24768abb59ab700476b31fcfcad1bfd2182dc45c"
    );
}

#[test]
fn trace_projection_helpers_keep_stable_persistence_vectors() {
    let flow_run_id = Uuid::from_u128(1);
    assert_eq!(
        trace_node_id_for_locator(flow_run_id, "root/flow:start"),
        Uuid::parse_str("268ddc12-e2c2-89ac-9a24-8803d4180e54").unwrap()
    );

    let updated_at = time::OffsetDateTime::from_unix_timestamp_nanos(1_234_567_890).unwrap();
    assert_eq!(
        trace_projection_source_watermark_from_counts(updated_at, 1, 2, 3, 4, 5),
        "flow_run_updated_at:1234567890/node_runs:1/callback_tasks:2/events:3/stitched:4/subagents:5"
    );
}
