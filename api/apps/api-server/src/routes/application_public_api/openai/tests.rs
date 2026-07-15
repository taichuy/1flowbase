use super::*;
use control_plane::application_public_api::native::{NativeRequiredAction, NativeRunStatus};
use time::OffsetDateTime;
use uuid::Uuid;

#[test]
fn openai_response_projects_native_tool_calls() {
    let run = NativeRunResult {
        id: Uuid::nil(),
        application_id: Uuid::nil(),
        api_key_id: Uuid::nil(),
        publication_version_id: Uuid::nil(),
        status: NativeRunStatus::Succeeded,
        node_input_payload: json!({}),
        metadata: json!({}),
        answer: None,
        answer_segments: None,
        required_action: None,
        tool_calls: Some(json!([
            {
                "id": "call_123",
                "name": "lookup_order",
                "arguments": {"order_id": "order_123"}
            }
        ])),
        usage: None,
        error: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
    };

    let payload = serde_json::to_value(to_openai_response(
        run,
        "provider/model".into(),
        "chatcmpl-test-tool-call".to_string(),
    ))
    .expect("openai response serializes");

    assert_eq!(payload["id"], json!("chatcmpl-test-tool-call"));
    assert_eq!(payload["choices"][0]["finish_reason"], json!("tool_calls"));
    assert_eq!(
        payload["choices"][0]["message"]["tool_calls"][0]["function"]["name"],
        json!("lookup_order")
    );
    assert_eq!(
        payload["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"],
        json!("{\"order_id\":\"order_123\"}")
    );
}

#[test]
fn openai_response_filters_internal_visible_llm_tool_calls() {
    let callback_task_id = Uuid::from_u128(0xabababababababababababababababab);
    let run = NativeRunResult {
        id: Uuid::nil(),
        application_id: Uuid::nil(),
        api_key_id: Uuid::nil(),
        publication_version_id: Uuid::nil(),
        status: NativeRunStatus::Waiting,
        node_input_payload: json!({}),
        metadata: json!({}),
        answer: Some("visible internal LLM output".to_string()),
        answer_segments: None,
        required_action: Some(NativeRequiredAction {
            action_type: "submit_tool_outputs".to_string(),
            payload: json!({
                "callback_task_id": callback_task_id,
                "callback_kind": "llm_tool_calls"
            }),
        }),
        tool_calls: Some(json!([
            {
                "id": "call_internal",
                "type": "visible_internal_llm_tool",
                "name": "inspect_visible_context",
                "arguments": {"query": "visible"}
            }
        ])),
        usage: None,
        error: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
    };

    let chat_payload = serde_json::to_value(to_openai_response(
        run.clone(),
        "provider/model".into(),
        "chatcmpl-internal".to_string(),
    ))
    .expect("openai chat response serializes");
    let responses_payload = serde_json::to_value(to_openai_responses_response(
        run,
        "provider/model".into(),
        None,
    ))
    .expect("openai responses object serializes");

    assert_eq!(chat_payload["choices"][0]["finish_reason"], json!("stop"));
    assert_eq!(
        chat_payload["choices"][0]["message"]["content"],
        json!("visible internal LLM output")
    );
    assert!(chat_payload["choices"][0]["message"]["tool_calls"].is_null());
    assert_eq!(
        responses_payload["output_text"],
        json!("visible internal LLM output")
    );
    assert_eq!(responses_payload["output"][0]["type"], json!("message"));
    assert!(responses_payload["output"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item["type"] != json!("function_call")));
}

#[test]
fn openai_response_encodes_callback_task_id_into_tool_call_ids() {
    let callback_task_id = Uuid::from_u128(0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
    let run = NativeRunResult {
        id: Uuid::nil(),
        application_id: Uuid::nil(),
        api_key_id: Uuid::nil(),
        publication_version_id: Uuid::nil(),
        status: NativeRunStatus::Waiting,
        node_input_payload: json!({}),
        metadata: json!({}),
        answer: Some("need tool".to_string()),
        answer_segments: None,
        required_action: Some(NativeRequiredAction {
            action_type: "submit_tool_outputs".to_string(),
            payload: json!({ "callback_task_id": callback_task_id }),
        }),
        tool_calls: Some(json!([
            {
                "id": "call_123",
                "name": "lookup_order",
                "arguments": {"order_id": "order_123"}
            }
        ])),
        usage: None,
        error: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
    };

    let payload = serde_json::to_value(to_openai_response(
        run,
        "provider/model".into(),
        "chatcmpl-test-callback".to_string(),
    ))
    .expect("openai response serializes");

    let tool_call_id = payload["choices"][0]["message"]["tool_calls"][0]["id"]
        .as_str()
        .expect("tool call id should be a string");
    assert!(tool_call_id.starts_with(
        control_plane::application_public_api::callback_tool_ids::OPENAI_CALLBACK_TOOL_CALL_PREFIX
    ));
    assert_eq!(
        decode_openai_callback_tool_call_id(tool_call_id),
        Some((callback_task_id, "call_123".to_string()))
    );
}

#[test]
fn openai_chat_tool_resume_request_decodes_tool_messages() {
    let callback_task_id = Uuid::from_u128(0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);
    let external_tool_call_id =
        encode_openai_callback_tool_call_id(callback_task_id, "call_weather");

    let resume = openai_chat_tool_resume_request(&json!({
            "model": "1flowbase",
            "messages": [
                {"role": "assistant", "content": null, "tool_calls": [
                    {"id": external_tool_call_id, "type": "function", "function": {"name": "lookup_weather", "arguments": "{}"}}
                ]},
                {"role": "tool", "tool_call_id": external_tool_call_id, "content": "{\"temperature\":21}"}
            ]
        }))
        .expect("resume request should parse")
        .expect("tool message should resume callback");

    assert_eq!(resume.callback_task_id, callback_task_id);
    assert_eq!(
        resume.tool_results[0]["tool_call_id"],
        json!("call_weather")
    );
    assert_eq!(
        resume.tool_results[0]["content"],
        json!("{\"temperature\":21}")
    );
}

#[test]
fn openai_chat_tool_resume_request_uses_latest_trailing_tool_messages() {
    let previous_callback_task_id = Uuid::from_u128(0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
    let current_callback_task_id = Uuid::from_u128(0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);
    let previous_tool_call_id =
        encode_openai_callback_tool_call_id(previous_callback_task_id, "call_previous");
    let current_tool_call_id =
        encode_openai_callback_tool_call_id(current_callback_task_id, "call_current");

    let resume = openai_chat_tool_resume_request(&json!({
            "model": "1flowbase",
            "messages": [
                {"role": "user", "content": "first"},
                {"role": "assistant", "content": null, "tool_calls": [
                    {"id": previous_tool_call_id, "type": "function", "function": {"name": "lookup_previous", "arguments": "{}"}}
                ]},
                {"role": "tool", "tool_call_id": previous_tool_call_id, "content": "old result"},
                {"role": "assistant", "content": "old answer"},
                {"role": "user", "content": "next"},
                {"role": "assistant", "content": null, "tool_calls": [
                    {"id": current_tool_call_id, "type": "function", "function": {"name": "lookup_current", "arguments": "{}"}}
                ]},
                {"role": "tool", "tool_call_id": current_tool_call_id, "content": "new result"}
            ]
        }))
        .expect("resume request should parse")
        .expect("trailing tool messages should resume callback");

    assert_eq!(resume.callback_task_id, current_callback_task_id);
    assert_eq!(resume.tool_results.as_array().unwrap().len(), 1);
    assert_eq!(
        resume.tool_results[0]["tool_call_id"],
        json!("call_current")
    );
    assert_eq!(resume.tool_results[0]["content"], json!("new result"));
}

#[test]
fn openai_chat_tool_resume_request_ignores_historical_tool_messages() {
    let callback_task_id = Uuid::from_u128(0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
    let external_tool_call_id =
        encode_openai_callback_tool_call_id(callback_task_id, "call_previous");

    let resume = openai_chat_tool_resume_request(&json!({
            "model": "1flowbase",
            "messages": [
                {"role": "user", "content": "first"},
                {"role": "assistant", "content": null, "tool_calls": [
                    {"id": external_tool_call_id, "type": "function", "function": {"name": "lookup_previous", "arguments": "{}"}}
                ]},
                {"role": "tool", "tool_call_id": external_tool_call_id, "content": "old result"},
                {"role": "assistant", "content": "old answer"},
                {"role": "user", "content": "next question"}
            ]
        }))
        .expect("historical tool messages should parse");

    assert!(resume.is_none());
}

#[test]
fn openai_responses_response_projects_native_tool_calls_with_encoded_call_id() {
    let callback_task_id = Uuid::from_u128(0xcccccccccccccccccccccccccccccccc);
    let run = NativeRunResult {
        id: Uuid::nil(),
        application_id: Uuid::nil(),
        api_key_id: Uuid::nil(),
        publication_version_id: Uuid::nil(),
        status: NativeRunStatus::Waiting,
        node_input_payload: json!({}),
        metadata: json!({}),
        answer: Some("".to_string()),
        answer_segments: None,
        required_action: Some(NativeRequiredAction {
            action_type: "submit_tool_outputs".to_string(),
            payload: json!({ "callback_task_id": callback_task_id, "callback_kind": "llm_tool_calls" }),
        }),
        tool_calls: Some(json!([
            {
                "id": "call_inventory",
                "name": "lookup_inventory",
                "arguments": {"sku": "sku_123"}
            }
        ])),
        usage: None,
        error: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
    };

    let payload = serde_json::to_value(to_openai_responses_response(
        run,
        "provider/model".into(),
        Some("resp_previous".into()),
    ))
    .expect("responses object serializes");

    assert_eq!(payload["status"], json!("completed"));
    assert_eq!(payload["output_text"], json!(""));
    assert_eq!(payload["output"][0]["type"], json!("function_call"));
    assert_eq!(payload["output"][0]["name"], json!("lookup_inventory"));
    assert_eq!(
        payload["output"][0]["arguments"],
        json!("{\"sku\":\"sku_123\"}")
    );
    let call_id = payload["output"][0]["call_id"]
        .as_str()
        .expect("call_id should be encoded");
    assert_eq!(
        decode_openai_callback_tool_call_id(call_id),
        Some((callback_task_id, "call_inventory".to_string()))
    );
}

#[test]
fn openai_responses_tool_resume_request_decodes_function_call_outputs() {
    let callback_task_id = Uuid::from_u128(0xdddddddddddddddddddddddddddddddd);
    let call_id = encode_openai_callback_tool_call_id(callback_task_id, "call_inventory");

    let resume = openai_responses_tool_resume_request(&json!({
        "model": "1flowbase",
        "previous_response_id": "resp_11111111-1111-1111-1111-111111111111",
        "input": [
            {
                "type": "function_call_output",
                "call_id": call_id,
                "output": {"stock": 7}
            }
        ]
    }))
    .expect("resume request should parse")
    .expect("function_call_output should resume callback");

    assert_eq!(resume.callback_task_id, callback_task_id);
    assert_eq!(
        resume.tool_results[0]["tool_call_id"],
        json!("call_inventory")
    );
    assert_eq!(resume.tool_results[0]["content"], json!("{\"stock\":7}"));
}

#[test]
fn openai_responses_tool_resume_request_only_reads_trailing_function_call_outputs() {
    let old_callback_task_id = Uuid::from_u128(0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
    let new_callback_task_id = Uuid::from_u128(0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);
    let old_call_id = encode_openai_callback_tool_call_id(old_callback_task_id, "call_old");
    let new_call_id = encode_openai_callback_tool_call_id(new_callback_task_id, "call_new");

    let resume = openai_responses_tool_resume_request(&json!({
        "model": "1flowbase",
        "input": [
            {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "看图"}]},
            {"type": "function_call", "call_id": old_call_id, "name": "shell", "arguments": "{}"},
            {"type": "function_call_output", "call_id": old_call_id, "output": "old result"},
            {"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "继续"}]},
            {"type": "function_call", "call_id": new_call_id, "name": "shell", "arguments": "{}"},
            {"type": "function_call_output", "call_id": new_call_id, "output": "new result"}
        ]
    }))
    .expect("resume request should parse")
    .expect("trailing function_call_output should resume the new callback");

    assert_eq!(resume.callback_task_id, new_callback_task_id);
    assert_eq!(resume.tool_results.as_array().map(Vec::len), Some(1));
    assert_eq!(resume.tool_results[0]["tool_call_id"], json!("call_new"));

    let no_resume = openai_responses_tool_resume_request(&json!({
        "model": "1flowbase",
        "input": [
            {"type": "function_call", "call_id": old_call_id, "name": "shell", "arguments": "{}"},
            {"type": "function_call_output", "call_id": old_call_id, "output": "old result"},
            {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "新问题"}]}
        ]
    }))
    .expect("resume request should parse");
    assert!(
        no_resume.is_none(),
        "a new user turn after historical tool outputs must start a fresh run"
    );
}
