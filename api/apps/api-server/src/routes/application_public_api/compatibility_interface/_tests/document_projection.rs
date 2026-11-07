use super::*;

fn source_fields(source: &str, name: &str) -> Vec<String> {
    let declaration = source
        .split(&format!("const {name}: &[&str] = &["))
        .nth(1)
        .unwrap()
        .split("];")
        .next()
        .unwrap();
    declaration
        .split('"')
        .enumerate()
        .filter(|(index, _)| index % 2 == 1)
        .map(|(_, field)| field.to_owned())
        .collect()
}

#[test]
fn compatibility_document_field_inventory_tracks_actual_protocol_translation_owners() {
    let openai = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/control-plane/src/application_public_api/compat/openai.rs"
    ));
    let anthropic = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/control-plane/src/application_public_api/compat/anthropic.rs"
    ));
    assert_eq!(
        ChatDocument::FIELDS,
        source_fields(openai, "OPENAI_CHAT_TYPED_ROOT_FIELDS")
    );
    let mut responses = source_fields(openai, "OPENAI_RESPONSES_TYPED_ROOT_FIELDS");
    responses.push("generate".into());
    assert_eq!(ResponsesDocument::FIELDS, responses);
    assert_eq!(
        AnthropicDocument::FIELDS,
        source_fields(anthropic, "ANTHROPIC_TYPED_ROOT_FIELDS")
    );
}

#[test]
fn compatibility_protocol_documents_preserve_wire_business_fields_nulls_and_long_arrays() {
    let content = "complete client content ".repeat(300);
    let chat=ProtocolRequestDocument::chat(&json!({"model":"chat-model","messages":[{"role":"user","content":content}],
        "tools":(0..45).map(|index|json!({"type":"function","function":{"name":format!("tool-{index}"),"parameters":{"type":"object"}}})).collect::<Vec<_>>(),
        "stop":null,"authorization":"ROOT_CANARY"})).unwrap();
    let value = ep::to_value(&chat).unwrap();
    assert_eq!(value["messages"][0]["content"], content);
    assert_eq!(value["tools"].as_array().unwrap().len(), 45);
    assert_eq!(value.get("stop"), Some(&Value::Null));
    assert!(value.get("query").is_none());
    assert!(!value.to_string().contains("ROOT_CANARY"));
    assert!(jsonschema::validator_for(&ChatDocument::schema())
        .unwrap()
        .is_valid(&value));

    let responses=ProtocolRequestDocument::responses(&json!({"model":"response-model","input":[{"type":"message","role":"user","content":content}],
        "previous_response_id":"resp-previous","tools":[{"type":"mcp","server_label":"business server","server_url":"https://example.test/mcp",
            "authorization":"MCP_CANARY","headers":{"Authorization":"HEADER_CANARY"}}]})).unwrap();
    let value = ep::to_value(&responses).unwrap();
    assert_eq!(value["input"][0]["content"], content);
    assert_eq!(value["previous_response_id"], "resp-previous");
    assert_eq!(value["tools"][0]["server_label"], "business server");
    assert!(!value.to_string().contains("CANARY"));

    let anthropic=ProtocolRequestDocument::anthropic(&json!({"model":"claude-model","messages":[{"role":"user","content":[{"type":"text","text":content}]}],
        "mcp_servers":[{"type":"url","name":"business server","url":"https://example.test/mcp","authorization_token":"ANTHROPIC_CANARY"}]})).unwrap();
    let value = ep::to_value(&anthropic).unwrap();
    assert_eq!(value["messages"][0]["content"][0]["text"], content);
    assert_eq!(value["mcp_servers"][0]["name"], "business server");
    assert!(!value.to_string().contains("ANTHROPIC_CANARY"));
    assert!(jsonschema::validator_for(&AnthropicDocument::schema())
        .unwrap()
        .is_valid(&value));
}
