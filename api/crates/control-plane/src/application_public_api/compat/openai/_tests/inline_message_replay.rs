use super::super::projection::{function_call_items, output_item, OutputKind, PresentedOutput};
use super::*;
use crate::application_public_api::native::ResponsesTransportRequirement;

const START: &str = "Start the synthetic replay.";
const CONTINUE: &str = "Continue the synthetic replay.";

fn inline_message(text: &str) -> Value {
    output_item(
        Uuid::from_u128(1),
        OutputKind::Message,
        Some(text.to_owned()),
    )
}

fn message_request(message: Value) -> Value {
    json!({
        "model": "gpt-6-luna", "store": false,
        "input": [
            {"role": "user", "content": START},
            message,
            {"role": "user", "content": CONTINUE}
        ]
    })
}

fn assert_semantic_message_replay(message: Value, text: &str) {
    assert!(!responses_input_item_requires_native_passthrough(&message));
    let mut translated = translate_response_request(message_request(message))
        .expect("complete inline assistant text has a semantic owner");
    assert_eq!(
        translated
            .request
            .metadata
            .responses_transport_requirement(),
        ResponsesTransportRequirement::SemanticCompatible
    );
    assert!(translated
        .request
        .metadata
        .take_provider_transport_payload()
        .is_none());
    assert_eq!(translated.request.query, CONTINUE);
    assert_eq!(
        translated.request.history,
        vec![
            json!({"role": "user", "content": START}),
            json!({"role": "assistant", "content": text})
        ]
    );
    translated
        .report
        .ensure_consistent()
        .expect("valid receipt");
}

fn assert_native_replay(body: Value) {
    let expected_bytes = serde_json::to_vec(&body).expect("serializable fixture");
    let mut translated = translate_response_request(body.clone())
        .expect("unsupported inline data must retain native transport");
    assert_eq!(
        translated
            .request
            .metadata
            .responses_transport_requirement(),
        ResponsesTransportRequirement::NativePassthrough
    );
    let payload = translated
        .request
        .metadata
        .take_provider_transport_payload()
        .expect("original native replay payload");
    assert_eq!(payload.wire_body(), &body);
    assert_eq!(
        serde_json::to_vec(payload.wire_body()).expect("serializable native payload"),
        expected_bytes
    );
    translated
        .report
        .ensure_consistent()
        .expect("valid receipt");
}

#[test]
fn complete_inline_assistant_projection_roundtrips_streaming_and_nonstreaming() {
    let text = "  synthetic first line\nsecond line: \"quoted\" λ\0  ";
    let unary = inline_message(text);
    let mut streaming = PresentedOutput::default();
    streaming.push(OutputKind::Message, "  synthetic first line\n");
    streaming.push(OutputKind::Message, "second line: \"quoted\" λ\0  ");
    let streamed = streaming.items(Uuid::from_u128(1));
    assert_eq!(streamed, vec![unary.clone()]);
    assert_semantic_message_replay(unary, text);
    for message in streamed {
        assert_semantic_message_replay(message, text);
    }
}

#[test]
fn inline_assistant_delivery_metadata_is_optional_empty_and_not_prefix_coupled() {
    for id in [
        None,
        Some("opaque/upstream-id"),
        Some("  arbitrary delivery λ  "),
    ] {
        for completed in [false, true] {
            for metadata in [
                vec![],
                vec!["annotations"],
                vec!["logprobs"],
                vec!["annotations", "logprobs"],
            ] {
                let mut message = inline_message("UNCHANGED");
                message
                    .as_object_mut()
                    .expect("message object")
                    .remove("id");
                if let Some(id) = id {
                    message["id"] = json!(id);
                }
                if completed {
                    message["status"] = json!("completed");
                }
                for field in metadata {
                    message["content"][0][field] = json!([]);
                }
                assert_semantic_message_replay(message, "UNCHANGED");
            }
        }
    }
}

#[test]
fn inline_assistant_supported_text_forms_keep_existing_projection_order() {
    for (content, expected) in [
        (json!("plain text"), "plain text"),
        (json!(""), ""),
        (
            json!([{"type": "text", "text": "text alias"}]),
            "text alias",
        ),
        (
            json!([{"type": "input_text", "text": "input alias"}]),
            "input alias",
        ),
        (json!([{"type": "output_text", "text": ""}]), ""),
        (
            json!([
                {"type": "output_text", "text": " first ", "annotations": []},
                {"type": "output_text", "text": "second\n", "logprobs": []}
            ]),
            " first \nsecond\n",
        ),
    ] {
        let mut message = inline_message("unused");
        message["content"] = content;
        message["status"] = json!("completed");
        assert_semantic_message_replay(message.clone(), expected);
        message
            .as_object_mut()
            .expect("message object")
            .remove("type");
        assert_semantic_message_replay(message, expected);
    }
}

#[test]
fn mixed_inline_assistant_function_and_pruned_outputs_keep_text_and_call_ownership() {
    let calls = [
        json!({"id": "call_old", "name": "probe", "arguments": {"value": "old"}}),
        json!({"id": "call_latest", "name": "probe", "arguments": {"value": "latest"}}),
    ];
    let projected_calls = function_call_items(calls.iter());
    let mut later_message = inline_message("Use the retained latest result.");
    later_message["id"] = json!("unrelated-message-delivery-id");
    later_message["status"] = json!("completed");
    later_message["content"][0]["annotations"] = json!([]);
    later_message["content"][0]["logprobs"] = json!([]);
    let mut translated = translate_response_request(json!({
        "model": "gpt-6-luna", "store": false,
        "input": [
            {"type": "message", "role": "user", "content": [{"type": "input_text", "text": START}]},
            inline_message("Inspect the synthetic result."),
            projected_calls[0],
            {"type": "function_call_output", "call_id": "call_old", "output": "[Old result pruned]"},
            later_message,
            projected_calls[1],
            {"type": "function_call_output", "call_id": "call_latest", "output": "LATEST_OK\n"}
        ]
    }))
    .expect("complete mixed pruned history has a semantic owner");
    assert_eq!(
        translated
            .request
            .metadata
            .responses_transport_requirement(),
        ResponsesTransportRequirement::SemanticCompatible
    );
    assert!(translated
        .request
        .metadata
        .take_provider_transport_payload()
        .is_none());
    assert!(translated.request.query.is_empty());
    assert_eq!(
        translated.request.history,
        vec![
            json!({"role": "user", "content": START}),
            json!({"role": "assistant", "content": "Inspect the synthetic result."}),
            json!({"role": "assistant", "content": "", "tool_calls": [
                {"id": "call_old", "name": "probe", "arguments": {"value": "old"}}
            ]}),
            json!({"role": "tool", "tool_call_id": "call_old", "content": "[Old result pruned]"}),
            json!({"role": "assistant", "content": "Use the retained latest result."}),
            json!({"role": "assistant", "content": "", "tool_calls": [
                {"id": "call_latest", "name": "probe", "arguments": {"value": "latest"}}
            ]}),
            json!({"role": "tool", "tool_call_id": "call_latest", "content": "LATEST_OK\n"})
        ]
    );
    translated
        .report
        .ensure_consistent()
        .expect("valid receipt");
}

#[test]
fn malformed_or_unfinished_message_delivery_metadata_remains_native_byte_equivalent() {
    for (field, value) in [
        ("id", json!("")),
        ("id", json!(" \t\n")),
        ("id", Value::Null),
        ("id", json!(42)),
        ("id", json!(false)),
        ("id", json!([])),
        ("id", json!({})),
        ("status", json!("in_progress")),
        ("status", json!("incomplete")),
        ("status", json!("failed")),
        ("status", json!("")),
        ("status", json!("completed ")),
        ("status", Value::Null),
        ("status", json!(42)),
        ("status", json!(false)),
        ("status", json!([])),
        ("status", json!({})),
        ("phase", json!("final_answer")),
        ("phase", Value::Null),
        ("annotations", json!([])),
        ("logprobs", json!([])),
        ("encrypted_content", json!("opaque-state")),
        ("provider_extension", json!({"retain": true})),
    ] {
        let mut message = inline_message("RETAIN ALL FIELDS");
        message[field] = value;
        assert!(
            responses_input_item_requires_native_passthrough(&message),
            "{field}"
        );
        assert_native_replay(message_request(message));
    }
}

#[test]
fn malformed_incomplete_reference_and_nonassistant_messages_remain_native_byte_equivalent() {
    for content in [
        Value::Null,
        json!(42),
        json!({}),
        json!([]),
        json!(["partial"]),
        json!([{"type": "output_text"}]),
        json!([{"type": "output_text", "text": Value::Null}]),
        json!([{"type": "output_text", "text": 42}]),
        json!([{"text": "missing type"}]),
        json!([{"type": "output_text", "text": "partial"}, false]),
        json!([{"type": "refusal", "refusal": "opaque refusal"}]),
        json!([{"type": "input_image", "image_url": "https://example.invalid/synthetic.png"}]),
    ] {
        let mut message = inline_message("unused");
        message["content"] = content;
        assert!(responses_input_item_requires_native_passthrough(&message));
        assert_native_replay(message_request(message));
    }
    for role in [
        json!("user"),
        json!("system"),
        json!("developer"),
        json!("tool"),
        json!("Assistant"),
        Value::Null,
        json!(42),
    ] {
        let mut message = inline_message("RETAIN ROLE");
        message["role"] = role;
        assert!(responses_input_item_requires_native_passthrough(&message));
        assert_native_replay(message_request(message));
    }
    for missing in ["content", "role"] {
        let mut message = inline_message("RETAIN REFERENCE");
        message
            .as_object_mut()
            .expect("message object")
            .remove(missing);
        assert!(responses_input_item_requires_native_passthrough(&message));
        assert_native_replay(message_request(message));
    }
    for item in [
        json!({"type": "message", "id": "message-reference-only"}),
        json!({"type": "item_reference", "id": "message-reference-only"}),
        json!({"type": "reasoning", "id": "opaque-reasoning", "summary": [], "encrypted_content": "opaque"}),
        json!({"type": "unknown_native_item", "id": "opaque-item", "provider_extension": {"retain": true}}),
    ] {
        assert!(responses_input_item_requires_native_passthrough(&item));
        assert_native_replay(message_request(item));
    }
}

#[test]
fn rich_or_malformed_output_text_fields_remain_native_byte_equivalent() {
    for (field, value) in [
        (
            "annotations",
            json!([{"type": "url_citation", "url": "https://example.invalid/source", "start_index": 0, "end_index": 3}]),
        ),
        ("annotations", Value::Null),
        ("annotations", json!({})),
        ("annotations", json!("")),
        (
            "logprobs",
            json!([{"token": "synthetic", "logprob": -0.25}]),
        ),
        ("logprobs", Value::Null),
        ("logprobs", json!({})),
        ("logprobs", json!(0)),
        ("phase", json!("final_answer")),
        ("phase", Value::Null),
        ("refusal", json!("opaque refusal")),
        ("provider_extension", json!({"retain": true})),
    ] {
        let mut message = inline_message("RETAIN RICH TEXT");
        message["content"][0][field] = value;
        assert!(
            responses_input_item_requires_native_passthrough(&message),
            "{field}"
        );
        assert_native_replay(message_request(message.clone()));
        message
            .as_object_mut()
            .expect("message object")
            .remove("id");
        assert!(
            responses_input_item_requires_native_passthrough(&message),
            "{field}"
        );
        assert_native_replay(message_request(message));
    }
}

#[test]
fn semantic_output_text_validator_accepts_only_empty_response_metadata_arrays() {
    for field in ["annotations", "logprobs"] {
        for value in [
            json!([]),
            Value::Null,
            json!({}),
            json!(""),
            json!(42),
            json!(false),
            json!([{"retain": true}]),
        ] {
            let accepted = value == json!([]);
            let mut part = json!({"type": "output_text", "text": "UNCHANGED"});
            part[field] = value;
            let mut report = TranslationReportBuilder::new(TranslationProtocol::OpenAiResponses);
            let result = validate_responses_content_parts(&json!([part]), 1, &mut report);
            assert_eq!(result.is_ok(), accepted, "{field}");
            report.ensure_consistent().expect("valid receipt");
        }
    }
    for kind in ["text", "input_text", "output_text"] {
        for field in ["annotations", "logprobs", "provider_extension"] {
            let mut part = json!({"type": kind, "text": "KEEP GUARDS"});
            part[field] = json!([]);
            let error = translate_chat_completion_request(json!({
                "model": "gpt-6-luna",
                "messages": [{"role": "user", "content": [part.clone()]}]
            }))
            .expect_err("Responses-only empty metadata must not widen Chat validation");
            assert_eq!(error.param.as_deref(), Some("messages"));
            if kind != "output_text" || field == "provider_extension" {
                let mut report =
                    TranslationReportBuilder::new(TranslationProtocol::OpenAiResponses);
                validate_responses_content_parts(&json!([part.clone()]), 1, &mut report)
                    .expect_err("other content-part unknown fields remain rejected");
                let mut message = inline_message("unused");
                message["content"] = json!([part]);
                assert_native_replay(message_request(message));
            }
        }
    }
}

#[test]
fn empty_output_metadata_does_not_expand_nonassistant_message_classification() {
    for role in ["user", "system", "developer"] {
        for field in ["annotations", "logprobs"] {
            let mut message = json!({"type": "message", "role": role,
                "content": [{"type": "output_text", "text": "RETAIN ROLE"}]});
            message["content"][0][field] = json!([]);
            assert!(responses_input_item_requires_native_passthrough(&message));
            assert_native_replay(message_request(message));
        }
    }
}

#[test]
fn semantic_message_validator_does_not_bypass_unknown_or_invalid_delivery_fields() {
    for (field, value) in [
        ("id", json!("")),
        ("id", json!(42)),
        ("status", json!("incomplete")),
        ("status", Value::Null),
        ("phase", json!("final_answer")),
        ("provider_extension", json!({"retain": true})),
    ] {
        let mut message = inline_message("KEEP GUARDS");
        message[field] = value;
        let body = message_request(message);
        let mut report = TranslationReportBuilder::new(TranslationProtocol::OpenAiResponses);
        validate_responses_input(&body["input"], false, &mut report)
            .expect_err("delivery metadata is not a general unknown-field exemption");
        report.ensure_consistent().expect("valid receipt");
    }
}

#[test]
fn supported_bare_assistant_messages_and_media_keep_existing_semantic_projection() {
    for content in [
        json!("bare text"),
        json!([{"type": "output_text", "text": "bare text"}]),
    ] {
        assert_semantic_message_replay(
            json!({"role": "assistant", "content": content}),
            "bare text",
        );
    }
    assert_semantic_message_replay(json!({"role": "assistant", "content": []}), "");
    let message = json!({"type": "message", "role": "assistant", "content": [
        {"type": "output_text", "text": "Synthetic image context."},
        {"type": "input_image", "image_url": "https://example.invalid/synthetic.png", "detail": "low"}
    ]});
    assert!(!responses_input_item_requires_native_passthrough(&message));
    let mut translated = translate_response_request(message_request(message))
        .expect("existing bare media projection remains semantic");
    assert_eq!(
        translated
            .request
            .metadata
            .responses_transport_requirement(),
        ResponsesTransportRequirement::SemanticCompatible
    );
    assert!(translated
        .request
        .metadata
        .take_provider_transport_payload()
        .is_none());
    assert_eq!(translated.request.query, CONTINUE);
    assert_eq!(
        translated.request.history,
        vec![
            json!({"role": "user", "content": START}),
            json!({"role": "assistant", "content": "Synthetic image context.", "content_blocks": [
                {"type": "text", "text": "Synthetic image context."},
                {"type": "image_url", "image_url": {"url": "https://example.invalid/synthetic.png", "detail": "low"}}
            ]})
        ]
    );
}

#[test]
fn native_request_or_sibling_keeps_complete_inline_messages_byte_equivalent() {
    let mut message = inline_message("RETAIN ENTIRE BODY");
    message["status"] = json!("completed");
    message["content"][0]["annotations"] = json!([]);
    for (field, value) in [
        ("text", json!({"format": {"type": "text"}})),
        ("store", json!(true)),
        ("truncation", json!("auto")),
    ] {
        let mut body = message_request(message.clone());
        body[field] = value;
        assert_native_replay(body);
    }
    let mut body = message_request(message);
    body["input"].as_array_mut().expect("input array").insert(2,
        json!({"type": "unknown_native_item", "id": "opaque-sibling", "provider_extension": {"retain": true}}));
    assert_native_replay(body);
}
