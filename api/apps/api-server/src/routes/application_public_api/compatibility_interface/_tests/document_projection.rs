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

#[test]
fn compatibility_models_keep_both_real_wire_variants_and_every_model() {
    use super::super::models::{CompatibilityModelsInput, CompatibilityModelsOutput};
    let operation = ApiOperationContext {
        interface_id: "compat.openai.models.list".into(),
        method: "GET".into(),
        path: "/v1/models".into(),
    };
    let models=(0..45).map(|index| serde_json::from_value(json!({"id":format!("model-{index}"),"name":format!("Model {index}"),"context_window":128000,"max_context_window":256000,"max_output_tokens":8000,"auto_compact_token_limit":90000,
        "capabilities":{"reasoning":true,"tool_call":true,"multimodal":true,"structured_output":true},"reasoning":{"default_effort":"high","supported_efforts":["low","high"]}})).unwrap()).collect::<Vec<control_plane::application_public_api::model_catalog::AgentModelDescriptor>>();
    for codex in [false, true] {
        let output = CompatibilityModelsOutput {
            codex,
            models: models.clone(),
            publication_created_at: 123,
        };
        let value = output.project_document_output(&operation).unwrap();
        let key = if codex { "models" } else { "data" };
        assert_eq!(value[key].as_array().unwrap().len(), 45);
        if codex {
            assert_eq!(value[key][44]["slug"], "model-44");
            assert_eq!(value[key][44]["limit"]["output"], 8000);
        } else {
            assert_eq!(value[key][44]["id"], "model-44");
            assert_eq!(value[key][44]["created"], 123);
        }
        assert!(jsonschema::validator_for(
            &CompatibilityModelsOutput::document_output_schema(&operation).unwrap()
        )
        .unwrap()
        .is_valid(&value));
    }
    let input = CompatibilityModelsInput {
        client_version: Some("1.0".into()),
    };
    assert_eq!(
        input.project_document_input(&operation).unwrap(),
        json!({"query":{"client_version":"1.0"}})
    );
}
