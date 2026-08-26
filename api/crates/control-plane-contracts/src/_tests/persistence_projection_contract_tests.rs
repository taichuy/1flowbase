use serde_json::json;
use uuid::Uuid;

use crate::persistence_projection::{
    audit_row_hash, build_flow_run_title, derive_flow_run_title_from_input_payload,
    display_flow_run_title, FLOW_RUN_TITLE_MAX_CHARS,
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
