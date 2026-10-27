use super::*;
use crate::provider_contract::{ProviderMessage, ProviderMessageRole};
use serde_json::json;

fn historical_reply(block: Value) -> ProviderInvocationInput {
    ProviderInvocationInput {
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::Assistant,
            content: "JADE2685 confirmed".into(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: Some(json!([block, {"type":"text","text":"JADE2685 confirmed"}])),
        }],
        ..Default::default()
    }
}

#[test]
fn canonical_plain_reasoning_history_has_complete_nonzero_count_and_grows_with_text() {
    let short = estimate_provider_count_tokens(&historical_reply(
        json!({"type":"reasoning","text":"confirmed goal"}),
    ))
    .unwrap();
    let long = estimate_provider_count_tokens(&historical_reply(
        json!({"type":"reasoning","text":"confirmed goal with preserved constraints ".repeat(100)}),
    ))
    .unwrap();
    assert_eq!(short.coverage, ProviderCountTokensCoverage::Complete);
    assert_eq!(short.unknown_block_count, 0);
    assert!(short.input_tokens > 0);
    assert_eq!(long.coverage, ProviderCountTokensCoverage::Complete);
    assert!(long.input_tokens > short.input_tokens + 500);
}

#[test]
fn opaque_reasoning_and_unrecognized_blocks_stay_partial() {
    for block in [
        json!({"type":"reasoning","encrypted_content":"opaque"}),
        json!({"type":"unrecognized","text":"not a recognized representation"}),
    ] {
        let count = estimate_provider_count_tokens(&historical_reply(block)).unwrap();
        assert_eq!(count.coverage, ProviderCountTokensCoverage::Partial);
        assert_eq!(count.unknown_block_count, 1);
        assert!(count.input_tokens > 0);
    }
}

#[test]
fn visible_canonical_tool_blocks_are_estimable_without_treating_opaque_as_text() {
    let visible = estimate_provider_count_tokens(&historical_reply(json!({
        "type":"tool_use","id":"call-1","name":"lookup","input":{"key":"value"}
    })))
    .unwrap();
    assert_eq!(visible.coverage, ProviderCountTokensCoverage::Complete);
    assert!(visible.input_tokens > 0);
    let opaque = estimate_provider_count_tokens(&historical_reply(json!({
        "type":"tool_result","tool_use_id":"call-1","content":[{"type":"reasoning_redacted","data":"opaque"}]
    }))).unwrap();
    assert_eq!(opaque.coverage, ProviderCountTokensCoverage::Partial);
}
