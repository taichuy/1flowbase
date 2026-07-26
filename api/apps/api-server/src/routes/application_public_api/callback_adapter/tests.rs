use super::*;
use control_plane::application_public_api::callback_tool_ids::{
    encode_anthropic_callback_tool_use_id, encode_openai_callback_tool_call_id,
};

fn callback(value: u128) -> Uuid {
    Uuid::from_u128(value)
}

#[test]
fn chat_parallel_results_correlate_only_with_the_preceding_assistant_turn() {
    let task = callback(1);
    let first = encode_openai_callback_tool_call_id(task, "call_first");
    let second = encode_openai_callback_tool_call_id(task, "call_second");
    let request = json!({
        "messages": [
            {"role": "user", "content": "old body"},
            {"role": "assistant", "content": "old response", "tool_calls": [
                {"id": first, "type": "function", "function": {"name": "first", "arguments": "{}"}},
                {"id": second, "type": "function", "function": {"name": "second", "arguments": "{}"}}
            ]},
            {"role": "tool", "tool_call_id": first, "content": "A"},
            {"role": "tool", "tool_call_id": second, "content": "B"}
        ]
    });

    let correlated = correlate_openai_chat_callback(&request)
        .expect("parallel callback should be valid")
        .expect("parallel callback should resume");
    assert_eq!(correlated.callback_task_id, task);
    assert_eq!(
        correlated.tool_results,
        json!([
            {"tool_call_id": "call_first", "content": "A"},
            {"tool_call_id": "call_second", "content": "B"}
        ])
    );
    assert!(correlated
        .tool_results
        .to_string()
        .find("old body")
        .is_none());
    assert!(correlated
        .tool_results
        .to_string()
        .find("old response")
        .is_none());
}

#[test]
fn chat_orphan_callback_marker_is_invalid_but_paired_stale_history_with_new_text_is_fresh() {
    let task = callback(2);
    let call_id = encode_openai_callback_tool_call_id(task, "call_stale");
    let orphan = json!({"messages": [
        {"role": "tool", "tool_call_id": call_id, "content": "isolated"}
    ]});
    assert_eq!(
        correlate_openai_chat_callback(&orphan)
            .expect_err("isolated callback output must fail")
            .param,
        "messages"
    );

    let historical = json!({"messages": [
        {"role": "assistant", "tool_calls": [{"id": call_id}]},
        {"role": "tool", "tool_call_id": call_id, "content": "stale"},
        {"role": "user", "content": "new text"}
    ]});
    assert!(correlate_openai_chat_callback(&historical)
        .expect("historical marker is valid history")
        .is_none());
}

#[test]
fn responses_previous_response_preserves_only_current_parallel_results() {
    let task = callback(3);
    let first = encode_openai_callback_tool_call_id(task, "call_a");
    let second = encode_openai_callback_tool_call_id(task, "call_b");
    let request = json!({"input": [
        {"type": "function_call_output", "call_id": first, "output": "A"},
        {"type": "function_call_output", "call_id": second, "output": {"value": "B"}}
    ]});
    let correlated = correlate_openai_responses_callback(&request, Some("resp_first"))
        .expect("Responses callback should be valid")
        .expect("Responses callback should resume");
    assert_eq!(correlated.callback_task_id, task);
    assert_eq!(
        correlated.tool_results,
        json!([
            {"tool_call_id": "call_a", "content": "A"},
            {"tool_call_id": "call_b", "content": "{\"value\":\"B\"}"}
        ])
    );
}

#[test]
fn responses_complete_history_without_previous_response_correlates_parallel_results() {
    let task = callback(9);
    let first = encode_openai_callback_tool_call_id(task, "call_a");
    let second = encode_openai_callback_tool_call_id(task, "call_b");
    let request = json!({"input": [
        {"role": "user", "content": "run both"},
        {"type": "function_call", "call_id": first, "name": "a", "arguments": "{}"},
        {"type": "function_call", "call_id": second, "name": "b", "arguments": "{}"},
        {"type": "function_call_output", "call_id": first, "output": "A"},
        {"type": "function_call_output", "call_id": second, "output": "B"}
    ]});

    let correlated = correlate_openai_responses_callback(&request, None)
        .expect("complete Responses history should be valid")
        .expect("complete Responses history should resume");
    assert_eq!(correlated.callback_task_id, task);
    assert_eq!(
        correlated.tool_results,
        json!([
            {"tool_call_id": "call_a", "content": "A"},
            {"tool_call_id": "call_b", "content": "B"}
        ])
    );
}

#[test]
fn responses_complete_history_without_previous_response_must_match_outputs_exactly() {
    let task = callback(10);
    let first = encode_openai_callback_tool_call_id(task, "call_a");
    let second = encode_openai_callback_tool_call_id(task, "call_b");
    let mismatched = encode_openai_callback_tool_call_id(task, "call_other");
    let request = json!({"input": [
        {"type": "function_call", "call_id": first},
        {"type": "function_call", "call_id": second},
        {"type": "function_call_output", "call_id": mismatched, "output": "other"}
    ]});

    assert_eq!(
        correlate_openai_responses_callback(&request, None)
            .expect_err("mismatched Responses history must fail")
            .message,
        "callback tool results must cover the preceding assistant tool calls exactly"
    );
}

#[test]
fn responses_orphan_output_without_previous_response_is_invalid() {
    let call_id = encode_openai_callback_tool_call_id(callback(11), "call_orphan");
    let request = json!({"input": [
        {"type": "function_call_output", "call_id": call_id, "output": "isolated"}
    ]});

    assert_eq!(
        correlate_openai_responses_callback(&request, None)
            .expect_err("orphan Responses output must fail")
            .param,
        "input"
    );
}

#[test]
fn responses_new_text_after_a_stale_marker_is_a_fresh_turn() {
    let call_id = encode_openai_callback_tool_call_id(callback(4), "call_old");
    let request = json!({"input": [
        {"type": "function_call_output", "call_id": call_id, "output": "old"},
        {"role": "user", "content": [{"type": "input_text", "text": "new text"}]}
    ]});
    assert!(
        correlate_openai_responses_callback(&request, Some("resp_old"))
            .expect("stale marker plus new text should parse")
            .is_none()
    );
}

#[test]
fn anthropic_parallel_results_must_match_one_preceding_callback_group() {
    let task = callback(5);
    let first = encode_anthropic_callback_tool_use_id(task, "toolu_a");
    let second = encode_anthropic_callback_tool_use_id(task, "toolu_b");
    let request = json!({"messages": [
        {"role": "assistant", "content": [
            {"type": "tool_use", "id": first, "name": "a", "input": {}},
            {"type": "tool_use", "id": second, "name": "b", "input": {}}
        ]},
        {"role": "user", "content": [
            {"type": "tool_result", "tool_use_id": first, "content": "A"},
            {"type": "tool_result", "tool_use_id": second, "content": "B", "is_error": true}
        ]}
    ]});
    let correlated = correlate_anthropic_callback(&request)
        .expect("Anthropic callback should be valid")
        .expect("Anthropic callback should resume");
    assert_eq!(correlated.callback_task_id, task);
    assert_eq!(
        correlated.tool_results,
        json!([
            {"tool_call_id": "toolu_a", "content": "A"},
            {"tool_call_id": "toolu_b", "content": "B", "is_error": true}
        ])
    );
}

#[test]
fn mixed_callback_task_ids_are_rejected() {
    let first = encode_openai_callback_tool_call_id(callback(6), "call_a");
    let second = encode_openai_callback_tool_call_id(callback(7), "call_b");
    let request = json!({"input": [
        {"type": "function_call_output", "call_id": first, "output": "A"},
        {"type": "function_call_output", "call_id": second, "output": "B"}
    ]});
    assert_eq!(
        correlate_openai_responses_callback(&request, Some("resp_previous"))
            .expect_err("mixed callback tasks must fail")
            .message,
        "tool results must belong to one callback task"
    );
}

#[test]
fn exact_callback_payload_retries_are_stable() {
    let task = callback(8);
    let call_id = encode_openai_callback_tool_call_id(task, "call_retry");
    let request = json!({"messages": [
        {"role": "assistant", "tool_calls": [{"id": call_id}]},
        {"role": "tool", "tool_call_id": call_id, "content": "same payload"}
    ]});

    let first = correlate_openai_chat_callback(&request)
        .expect("first callback payload should be valid")
        .expect("first callback payload should resume");
    let retry = correlate_openai_chat_callback(&request)
        .expect("retry callback payload should be valid")
        .expect("retry callback payload should resume");
    assert_eq!(retry, first);
}
