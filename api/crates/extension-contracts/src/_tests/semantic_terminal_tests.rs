use serde_json::json;

use crate::{
    count_tokens_input_tokens_from_output_payload, CompactOperationReceipt, CountTokensReceipt,
    NativeOperationTerminal, ProviderCompactProfile, ProviderCompactResult,
    ProviderCountTokensResult, ProviderWireOperation,
};

#[test]
fn count_tokens_terminal_keeps_exact_payload_and_authenticity_checks() {
    let receipt = CountTokensReceipt::new(ProviderCountTokensResult {
        operation: ProviderWireOperation::CountTokens,
        input_tokens: 37,
        ..ProviderCountTokensResult::default()
    })
    .unwrap();
    let terminal = NativeOperationTerminal::CountTokens(receipt.clone());
    let payload = terminal.as_payload().unwrap();
    assert_eq!(
        payload,
        json!({
            "semantic_terminal": "count_tokens",
            "result": {
                "operation": "count_tokens",
                "input_tokens": 37,
                "method": "upstream_api",
                "coverage": "complete",
                "unknown_block_count": 0
            }
        })
    );
    assert_eq!(
        NativeOperationTerminal::from_payload(&payload).unwrap(),
        Some(terminal)
    );
    assert_eq!(
        count_tokens_input_tokens_from_output_payload(&payload).unwrap(),
        Some(37)
    );

    let error = CountTokensReceipt::new(ProviderCountTokensResult {
        operation: ProviderWireOperation::Generate,
        ..ProviderCountTokensResult::default()
    })
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "CountTokens receipt requires a typed CountTokens provider result"
    );

    let overflow = CountTokensReceipt::new(ProviderCountTokensResult {
        input_tokens: u64::MAX,
        ..ProviderCountTokensResult::default()
    })
    .unwrap()
    .as_payload()
    .unwrap();
    assert_eq!(
        count_tokens_input_tokens_from_output_payload(&overflow)
            .unwrap_err()
            .to_string(),
        "CountTokens result exceeds the application log numeric range"
    );
}

#[test]
fn compact_terminal_keeps_exact_payload_and_profile_authenticity() {
    let result = ProviderCompactResult::ResponseItems {
        operation: ProviderWireOperation::Compact,
        profile: ProviderCompactProfile::ResponsesCompact,
        response_items: vec![json!({ "type": "message", "content": "compacted" })],
    };
    let receipt = CompactOperationReceipt::from_provider_result(result).unwrap();
    let payload = receipt.as_payload().unwrap();
    assert_eq!(
        payload,
        json!({
            "semantic_terminal": "compact",
            "result": {
                "result_type": "response_items",
                "operation": "compact",
                "profile": "responses_compact",
                "response_items": [{ "type": "message", "content": "compacted" }]
            }
        })
    );
    assert_eq!(
        CompactOperationReceipt::from_payload(&payload).unwrap(),
        receipt
    );

    let forged = ProviderCompactResult::ResponseItems {
        operation: ProviderWireOperation::Generate,
        profile: ProviderCompactProfile::ResponsesCompact,
        response_items: vec![json!({ "type": "message" })],
    };
    assert_eq!(
        CompactOperationReceipt::from_provider_result(forged)
            .unwrap_err()
            .to_string(),
        "Compact receipt requires a typed provider Compact result"
    );
}
