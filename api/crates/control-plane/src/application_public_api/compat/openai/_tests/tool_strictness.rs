use super::*;

fn optional_permission_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "value": {"type": "string"},
            "sandbox_permissions": {"type": "string"},
            "justification": {"type": "string"}
        },
        "required": ["value"],
        "additionalProperties": false
    })
}

#[test]
fn responses_tool_strictness_preserves_false_and_omission_in_native_inputs() {
    for strict in [Some(false), None] {
        let schema = optional_permission_schema();
        let mut tool = json!({
            "type": "function", "name": "probe", "description": "Echo value",
            "parameters": schema
        });
        let mut expected = json!({
            "name": "probe", "description": "Echo value",
            "input_schema": schema, "source": "client"
        });
        if let Some(strict) = strict {
            tool["strict"] = json!(strict);
            expected["strict"] = json!(strict);
        }
        let translated = translate_response_request(json!({
            "model": "1flowbase", "input": "hi", "tools": [tool]
        }))
        .expect("semantic Responses tools should map to Native inputs");

        assert_eq!(
            translated.request.inputs.as_value()["tools"],
            json!([expected])
        );
        assert_eq!(
            translated.request.metadata.responses_transport_requirement(),
            crate::application_public_api::native::ResponsesTransportRequirement::SemanticCompatible
        );
    }
}

#[test]
fn chat_tool_strictness_preserves_explicit_bools_and_omission_in_native_inputs() {
    for strict in [Some(false), Some(true), None] {
        let schema = optional_permission_schema();
        let mut function = json!({
            "name": "probe", "description": "Echo value", "parameters": schema
        });
        let mut expected = json!({
            "name": "probe", "description": "Echo value",
            "input_schema": schema, "source": "client"
        });
        if let Some(strict) = strict {
            function["strict"] = json!(strict);
            expected["strict"] = json!(strict);
        }
        let translated = translate_chat_completion_request(json!({
            "model": "1flowbase", "messages": [{"role": "user", "content": "hi"}],
            "tools": [{"type": "function", "function": function}]
        }))
        .expect("Chat function tools should map to Native inputs");

        assert_eq!(
            translated.request.inputs.as_value()["tools"],
            json!([expected])
        );
    }
}

#[test]
fn responses_tool_strictness_true_keeps_existing_native_transport_and_wire_body() {
    let body = json!({
        "model": "1flowbase", "input": "hi",
        "tools": [{
            "type": "function", "name": "probe", "strict": true,
            "parameters": optional_permission_schema()
        }]
    });
    let mut translated = translate_response_request(body.clone())
        .expect("strict Responses tools should retain native transport");

    assert_eq!(
        translated
            .request
            .metadata
            .responses_transport_requirement(),
        crate::application_public_api::native::ResponsesTransportRequirement::NativePassthrough
    );
    assert!(translated.request.inputs.as_value().get("tools").is_none());
    let payload = translated
        .request
        .metadata
        .take_provider_transport_payload()
        .expect("native transport should retain the original wire body");
    assert_eq!(payload.wire_body(), &body);
}
