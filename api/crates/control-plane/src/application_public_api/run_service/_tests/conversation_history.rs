use super::*;

fn native_message(
    role: &str,
    content: &str,
    tool_call_id: Option<&str>,
    tool_calls: Option<Vec<Value>>,
) -> AssistantConversationNativeMessage {
    AssistantConversationNativeMessage {
        role: role.to_string(),
        content: content.to_string(),
        name: None,
        tool_call_id: tool_call_id.map(ToOwned::to_owned),
        is_error: None,
        content_blocks: None,
        tool_calls,
    }
}

#[test]
fn assistant_history_closes_complete_interrupted_tool_calls_and_drops_orphan_outputs() {
    let history = assistant_conversation_native_history_to_values(vec![
        native_message("user", "Find an order", None, None),
        native_message(
            "assistant",
            "",
            None,
            Some(vec![
                json!({ "id": "call_order", "name": "find_order", "arguments": { "id": "42" } }),
                json!({ "id": "call_order", "name": "find_order", "arguments": { "id": "42" } }),
                json!({ "id": "", "name": "incomplete", "arguments": {} }),
            ]),
        ),
        native_message("tool", "stale", Some("unknown_call"), None),
        native_message("user", "Try again", None, None),
    ]);

    assert_eq!(history.len(), 4);
    assert_eq!(
        history[0],
        json!({ "role": "user", "content": "Find an order" })
    );
    assert_eq!(
        history[1]["tool_calls"],
        json!([
            { "id": "call_order", "name": "find_order", "arguments": { "id": "42" } }
        ])
    );
    assert_eq!(history[2]["role"], "tool");
    assert_eq!(history[2]["tool_call_id"], "call_order");
    assert_eq!(history[2]["is_error"], true);
    assert_eq!(history[2]["content"], INTERRUPTED_TOOL_CALL_CONTENT);
    assert_eq!(
        history[3],
        json!({ "role": "user", "content": "Try again" })
    );
}

#[test]
fn assistant_history_keeps_real_tool_results_and_drops_duplicate_or_unmatched_outputs() {
    let mut failed_result = native_message("tool", "Order not found", Some("call_order"), None);
    failed_result.name = Some("find_order".to_string());
    failed_result.is_error = Some(true);
    let history = assistant_conversation_native_history_to_values(vec![
        native_message(
            "assistant",
            "Checking both records",
            None,
            Some(vec![
                json!({ "id": "call_customer", "name": "find_customer", "arguments": "not JSON" }),
                json!({ "id": "call_order", "name": "find_order", "arguments": null }),
            ]),
        ),
        native_message("tool", "Orphan result", Some("unknown_call"), None),
        native_message("tool", "Missing call id", None, None),
        failed_result,
        native_message("tool", "Duplicate failure", Some("call_order"), None),
        native_message("tool", "Customer found", Some("call_customer"), None),
        native_message("tool", "Duplicate success", Some("call_customer"), None),
        native_message("user", "Continue", None, None),
    ]);

    assert_eq!(
        history,
        vec![
            json!({
                "role": "assistant",
                "content": "Checking both records",
                "tool_calls": [
                    { "id": "call_customer", "name": "find_customer", "arguments": "not JSON" },
                    { "id": "call_order", "name": "find_order", "arguments": null },
                ],
            }),
            json!({
                "role": "tool",
                "content": "Order not found",
                "name": "find_order",
                "tool_call_id": "call_order",
                "is_error": true,
            }),
            json!({ "role": "tool", "content": "Customer found", "tool_call_id": "call_customer" }),
            json!({ "role": "user", "content": "Continue" }),
        ]
    );
}

#[test]
fn assistant_history_omits_absent_empty_or_all_invalid_tool_calls() {
    for tool_calls in [
        None,
        Some(vec![]),
        Some(vec![
            json!(null),
            json!("not a call"),
            json!([]),
            json!({}),
            json!({ "name": "inspect_image", "arguments": {} }),
            json!({ "id": "call_image", "arguments": {} }),
            json!({ "id": "call_image", "name": "inspect_image" }),
            json!({ "id": null, "name": "inspect_image", "arguments": {} }),
            json!({ "id": 42, "name": "inspect_image", "arguments": {} }),
            json!({ "id": "call_image", "name": null, "arguments": {} }),
            json!({ "id": "call_image", "name": 42, "arguments": {} }),
            json!({ "id": " \t\n", "name": "inspect_image", "arguments": {} }),
            json!({ "id": "call_image", "name": " \t\n", "arguments": {} }),
        ]),
    ] {
        let mut message = native_message("assistant", "No tool needed", None, tool_calls);
        message.name = Some("assistant_name".to_string());
        message.is_error = Some(false);
        message.content_blocks = Some(vec![json!({ "type": "text", "text": "No tool needed" })]);

        let history = assistant_conversation_native_history_to_values(vec![message]);

        assert_eq!(history.len(), 1);
        assert_eq!(
            serde_json::to_string(&history[0]).expect("history is serializable"),
            r#"{"role":"assistant","content":"No tool needed","name":"assistant_name","is_error":false,"content_blocks":[{"type":"text","text":"No tool needed"}]}"#
        );
    }
}

#[test]
fn assistant_history_keeps_first_complete_call_with_original_fields_and_media_arguments() {
    let mut message = native_message(
        "assistant",
        "Inspect the image",
        None,
        Some(vec![
            json!({ "id": "call_image", "name": "inspect_image" }),
            json!({
                "id": " call_image ",
                "name": " inspect_image ",
                "arguments": {
                    "image": { "type": "image", "source": "data:image/png;base64,AQID" },
                    "caption": "  preserve whitespace  ",
                },
                "provider_metadata": { "signature": "signed-call", "index": 7 },
            }),
            json!({ "id": "call_image", "name": "different_tool", "arguments": "duplicate" }),
        ]),
    );
    message.name = Some("vision_assistant".to_string());
    message.content_blocks = Some(vec![json!({ "type": "text", "text": "Inspect the image" })]);

    let history = assistant_conversation_native_history_to_values(vec![message]);

    assert_eq!(history.len(), 2);
    assert_eq!(
        serde_json::to_string(&history[0]).expect("history is serializable"),
        r#"{"role":"assistant","content":"Inspect the image","name":"vision_assistant","content_blocks":[{"type":"text","text":"Inspect the image"}],"tool_calls":[{"id":" call_image ","name":" inspect_image ","arguments":{"image":{"type":"image","source":"data:image/png;base64,AQID"},"caption":"  preserve whitespace  "},"provider_metadata":{"signature":"signed-call","index":7}}]}"#
    );
    assert_eq!(
        history[1],
        json!({
            "role": "tool",
            "name": "inspect_image",
            "tool_call_id": "call_image",
            "content": INTERRUPTED_TOOL_CALL_CONTENT,
            "is_error": true,
        })
    );
}

#[test]
fn indexed_history_preserves_pending_insertion_order_and_exact_result_matching() {
    let history = assistant_conversation_native_history_to_values(vec![
        native_message(
            "assistant",
            "",
            None,
            Some(vec![
                json!({"id":"z","name":"last","arguments":{}}),
                json!({"id":" a ","name":" first ","arguments":null}),
                json!({"id":"m","name":"middle","arguments":[]}),
                json!({"id":"a","name":"duplicate","arguments":"keep first"}),
            ]),
        ),
        native_message("tool", "not trimmed", Some(" a "), None),
        native_message("tool", "real", Some("a"), None),
        native_message("tool", "duplicate", Some("a"), None),
        native_message("user", "continue", None, None),
    ]);
    assert_eq!(history.len(), 5);
    assert_eq!(history[0]["tool_calls"].as_array().unwrap().len(), 3);
    assert_eq!(history[1]["content"], "real");
    assert_eq!(history[2]["tool_call_id"], "z");
    assert_eq!(history[3]["tool_call_id"], "m");
    assert_eq!(history[2]["is_error"], true);
    assert_eq!(history[3]["content"], INTERRUPTED_TOOL_CALL_CONTENT);
    assert_eq!(history[4]["role"], "user");
}

#[test]
fn indexed_history_can_reuse_call_id_only_after_a_history_boundary() {
    let call = json!({"id":"same","name":"tool","arguments":{}});
    let history = assistant_conversation_native_history_to_values(vec![
        native_message("assistant", "first", None, Some(vec![call.clone()])),
        native_message("user", "boundary", None, None),
        native_message("tool", "stale", Some("same"), None),
        native_message("assistant", "second", None, Some(vec![call])),
        native_message("tool", "fresh", Some("same"), None),
    ]);
    assert_eq!(history.len(), 5);
    assert_eq!(history[1]["is_error"], true);
    assert_eq!(history[2]["role"], "user");
    assert_eq!(history[3]["content"], "second");
    assert_eq!(history[4]["content"], "fresh");
}
