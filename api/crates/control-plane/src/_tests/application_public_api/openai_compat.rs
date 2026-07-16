use control_plane::application_public_api::compat::openai::{
    map_chat_completion_request, map_response_request, OpenAiCompatError,
};
use serde_json::{json, Value};

fn base_request() -> Value {
    json!({
        "model": "provider/custom-model",
        "messages": [
            {"role": "user", "content": "Earlier question"},
            {"role": "assistant", "content": "Earlier answer"},
            {"role": "user", "content": "Final question"}
        ]
    })
}

fn assert_unsupported_feature(request: Value, param: &str) {
    let error = map_chat_completion_request(request).unwrap_err();

    assert_openai_unsupported_feature(error, param);
}

fn assert_openai_unsupported_feature(error: OpenAiCompatError, param: &str) {
    assert_eq!(error.error_type, "invalid_request_error");
    assert_eq!(error.code, "unsupported_feature");
    assert_eq!(error.param.as_deref(), Some(param));
    assert_eq!(
        error.message,
        format!("{param} is not supported by this endpoint")
    );
}

#[test]
fn last_user_text_maps_to_native_query() {
    let native = map_chat_completion_request(base_request()).unwrap();

    assert_eq!(native.query, "Final question");
}

#[test]
fn last_user_image_url_maps_to_native_content_blocks() {
    let native = map_chat_completion_request(json!({
        "model": "gpt-compatible",
        "messages": [
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": "Describe image"},
                    {
                        "type": "image_url",
                        "image_url": {"url": "https://example.com/cat.png"}
                    }
                ]
            }
        ]
    }))
    .unwrap();

    assert_eq!(native.query, "Describe image");
    assert_eq!(native.history.len(), 1);
    assert_eq!(native.history[0]["role"], json!("user"));
    assert_eq!(native.history[0]["content"], json!("Describe image"));
    assert_eq!(
        native.history[0]["content_blocks"],
        json!([
            {"type": "text", "text": "Describe image"},
            {
                "type": "image_url",
                "image_url": {"url": "https://example.com/cat.png"}
            }
        ])
    );
}

#[test]
fn prior_system_message_maps_to_native_system_context() {
    let native = map_chat_completion_request(json!({
        "model": "gpt-compatible",
        "messages": [
            {"role": "system", "content": "Use the support playbook."},
            {"role": "user", "content": "Earlier question"},
            {"role": "assistant", "content": "Earlier answer"},
            {"role": "user", "content": "Final question"}
        ]
    }))
    .unwrap();

    assert_eq!(
        native.system_text().as_deref(),
        Some("Use the support playbook.")
    );
    assert_eq!(
        native.history,
        vec![
            json!({"role": "user", "content": "Earlier question"}),
            json!({"role": "assistant", "content": "Earlier answer"})
        ]
    );
}

#[test]
fn stream_true_maps_to_native_streaming_response_mode() {
    let mut request = base_request();
    request["stream"] = json!(true);

    let native = map_chat_completion_request(request).unwrap();

    assert_eq!(native.response_mode.as_deref(), Some("streaming"));
}

#[test]
fn user_maps_to_native_conversation_user() {
    let mut request = base_request();
    request["user"] = json!("external-user-123");

    let native = map_chat_completion_request(request).unwrap();

    assert_eq!(
        native.conversation.get("user"),
        Some(&json!("external-user-123"))
    );
}

#[test]
fn metadata_maps_to_native_metadata() {
    let mut request = base_request();
    request["metadata"] = json!({
        "trace_id": "trace-123",
        "customer_tier": "enterprise"
    });

    let native = map_chat_completion_request(request).unwrap();

    assert_eq!(
        native.metadata.as_value(),
        json!({
            "trace_id": "trace-123",
            "customer_tier": "enterprise"
        })
    );
}

#[test]
fn responses_instructions_map_to_native_system_context() {
    let native = map_response_request(
        json!({
            "model": "gpt-compatible",
            "instructions": "Use the support playbook.",
            "input": "Final question"
        }),
        None,
    )
    .unwrap();

    assert_eq!(native.query, "Final question");
    assert_eq!(
        native.system_text().as_deref(),
        Some("Use the support playbook.")
    );
    assert!(native.history.is_empty());
}

#[test]
fn responses_input_image_maps_to_native_content_blocks() {
    let native = map_response_request(
        json!({
            "model": "gpt-compatible",
            "input": [
                {
                    "role": "user",
                    "content": [
                        {"type": "input_text", "text": "Describe image"},
                        {
                            "type": "input_image",
                            "image_url": "data:image/png;base64,aW1hZ2U="
                        }
                    ]
                }
            ]
        }),
        None,
    )
    .unwrap();

    assert_eq!(native.query, "Describe image");
    assert_eq!(native.history.len(), 1);
    assert_eq!(
        native.history[0]["content_blocks"][0]["type"],
        json!("text")
    );
    assert_eq!(
        native.history[0]["content_blocks"][1],
        json!({
            "type": "image_url",
            "image_url": {"url": "data:image/png;base64,aW1hZ2U="}
        })
    );
}

#[test]
fn model_maps_exactly_without_validation() {
    let mut request = base_request();
    request["model"] = json!("unregistered/provider:model.with/slashes");

    let native = map_chat_completion_request(request).unwrap();

    assert_eq!(
        native.model.as_deref(),
        Some("unregistered/provider:model.with/slashes")
    );
}

#[test]
fn tools_map_to_native_compatibility_inputs_and_metadata() {
    let mut request = base_request();
    request["tools"] = json!([
        {
            "type": "function",
            "function": {
                "name": "lookup_order",
                "parameters": {"type": "object"}
            }
        }
    ]);
    request["tool_choice"] = json!({"type": "function", "function": {"name": "lookup_order"}});

    let native = map_chat_completion_request(request).unwrap();

    assert_eq!(
        native.inputs.as_value()["tools"][0]["name"],
        json!("lookup_order")
    );
    assert_eq!(
        native.inputs.as_value()["tool_choice"]["function"]["name"],
        json!("lookup_order")
    );
    assert_eq!(
        native.metadata.as_value()["compatibility"]["tool_choice"]["function"]["name"],
        json!("lookup_order")
    );
}

#[test]
fn tool_messages_map_to_native_history() {
    let mut request = base_request();
    request["messages"] = json!([
        {"role": "user", "content": "Find order"},
        {
            "role": "assistant",
            "content": null,
            "tool_calls": [
                {
                    "id": "call_123",
                    "type": "function",
                    "function": {"name": "lookup_order", "arguments": "{\"order_id\":\"order_123\"}"}
                }
            ]
        },
        {"role": "tool", "tool_call_id": "call_123", "content": "{\"status\":\"shipped\"}"}
    ]);

    let native = map_chat_completion_request(request).unwrap();

    assert_eq!(native.query, "Find order");
    assert_eq!(
        native.history,
        vec![
            json!({
                "role": "assistant",
                "content": "",
                "tool_calls": [
                    {
                        "id": "call_123",
                        "type": "function",
                        "function": {"name": "lookup_order", "arguments": "{\"order_id\":\"order_123\"}"}
                    }
                ]
            }),
            json!({
                "role": "tool",
                "content": "{\"status\":\"shipped\"}",
                "tool_call_id": "call_123"
            })
        ]
    );
}

#[test]
fn legacy_function_call_maps_to_native_compatibility_inputs() {
    let mut request = base_request();
    request["function_call"] = json!({"name": "lookup_order"});

    let native = map_chat_completion_request(request).unwrap();

    assert_eq!(
        native.inputs.as_value()["tool_choice"]["name"],
        json!("lookup_order")
    );
}

#[test]
fn audio_output_returns_unsupported_feature() {
    let mut request = base_request();
    request["audio"] = json!({
        "voice": "alloy",
        "format": "mp3"
    });

    assert_unsupported_feature(request, "audio");
}

#[test]
fn multimodal_generation_returns_unsupported_feature() {
    let mut request = base_request();
    request["modalities"] = json!(["text", "audio"]);

    assert_unsupported_feature(request, "modalities");
}
