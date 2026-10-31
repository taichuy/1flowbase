use super::*;
use crate::application_public_api::native::ResponsesTransportRequirement;

fn inline_history() -> Value {
    json!({
        "model": "gpt-6-luna", "store": false, "reasoning": {"effort": "max"},
        "input": [
            {"role": "user", "content": "Continue from the pruned context."},
            {"type": "function_call", "id": "fc_call_first", "status": "completed",
             "call_id": "call_first", "name": "probe", "arguments": "{\"value\":\"first\"}"},
            {"type": "function_call_output", "call_id": "call_first", "output": "[Old result pruned]"},
            {"type": "function_call", "id": "opaque-projection-id", "status": "completed",
             "call_id": "call_latest", "name": "probe", "arguments": "{\"value\":\"latest\"}"},
            {"type": "function_call_output", "call_id": "call_latest", "output": "LATEST_OK"}
        ]
    })
}

#[test]
fn complete_inline_function_replay_uses_call_identity_not_persisted_item_ids() {
    let mut translated = translate_response_request(inline_history())
        .expect("complete inline function history has a semantic owner");
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
            json!({"role": "user", "content": "Continue from the pruned context."}),
            json!({"role": "assistant", "content": "", "tool_calls": [
                {"id": "call_first", "name": "probe", "arguments": {"value": "first"}}
            ]}),
            json!({"role": "tool", "tool_call_id": "call_first", "content": "[Old result pruned]"}),
            json!({"role": "assistant", "content": "", "tool_calls": [
                {"id": "call_latest", "name": "probe", "arguments": {"value": "latest"}}
            ]}),
            json!({"role": "tool", "tool_call_id": "call_latest", "content": "LATEST_OK"})
        ]
    );
}

#[test]
fn inline_function_projection_metadata_is_optional_and_not_prefix_coupled() {
    let call = inline_history()["input"][1].clone();
    for omitted in [vec!["id"], vec!["status"], vec!["id", "status"]] {
        let mut item = call.clone();
        for field in omitted {
            item.as_object_mut().unwrap().remove(field);
        }
        assert!(!responses_input_item_requires_native_passthrough(&item));
    }
    let mut arbitrary_id = call;
    arbitrary_id["id"] = json!("not-a-gateway-prefix");
    assert!(!responses_input_item_requires_native_passthrough(
        &arbitrary_id
    ));
}

#[test]
fn opaque_unfinished_and_reference_only_items_keep_native_transport() {
    for (field, value) in [
        ("namespace", json!("functions")),
        ("encrypted_content", json!("opaque-state")),
        ("status", json!("in_progress")),
        ("status", json!("incomplete")),
        ("id", json!("")),
        ("id", json!(42)),
        ("provider_extension", json!({"must": "survive"})),
    ] {
        let mut body = inline_history();
        body["input"][1][field] = value;
        assert!(responses_input_item_requires_native_passthrough(
            &body["input"][1]
        ));
        let mut translated = translate_response_request(body.clone())
            .expect("opaque fields must remain in the provider-native representation");
        assert_eq!(
            translated
                .request
                .metadata
                .responses_transport_requirement(),
            ResponsesTransportRequirement::NativePassthrough
        );
        assert_eq!(
            translated
                .request
                .metadata
                .take_provider_transport_payload()
                .expect("native replay payload")
                .wire_body(),
            &body
        );
    }
    for missing in ["call_id", "name", "arguments"] {
        let mut call = inline_history()["input"][1].clone();
        call.as_object_mut().unwrap().remove(missing);
        assert!(responses_input_item_requires_native_passthrough(&call));
    }
    assert!(responses_input_item_requires_native_passthrough(
        &json!({"type": "item_reference", "id": "fc_call_first"})
    ));
    assert!(responses_input_item_requires_native_passthrough(
        &json!({"type": "reasoning", "id": "rs_1", "encrypted_content": "opaque"})
    ));
}

#[test]
fn native_only_request_contract_still_preserves_the_entire_inline_body() {
    let mut body = inline_history();
    body["text"] = json!({"format": {"type": "text"}});
    let mut translated = translate_response_request(body.clone())
        .expect("explicit native-only contracts retain native input");
    assert_eq!(
        translated
            .request
            .metadata
            .responses_transport_requirement(),
        ResponsesTransportRequirement::NativePassthrough
    );
    assert_eq!(
        translated
            .request
            .metadata
            .take_provider_transport_payload()
            .expect("explicit native replay payload")
            .wire_body(),
        &body
    );
}
