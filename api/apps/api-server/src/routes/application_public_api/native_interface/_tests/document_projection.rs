use super::*;
use control_plane::application_public_api::native::NativeRunStatus;

fn operation() -> ApiEventOperationContext {
    ApiEventOperationContext {
        interface_id: BLOCKING_INTERFACE_ID.into(),
        method: "POST".into(),
        path: "/api/agent/v1/runs".into(),
    }
}

#[test]
fn native_document_preserves_full_business_input_and_omits_protocol_transport() {
    let text = "complete business content ".repeat(300);
    let mut request: NativeRunRequest = serde_json::from_value(json!({
        "query":text,"inputs":{"env":{"ordinary":"business"},"rows":(0..45).collect::<Vec<_>>()},
        "history":[{"role":"assistant","content":"earlier response"}],
        "attachments":[{"source":"url","value":"https://example.test/document","name":"document"}],
        "conversation":{"id":"conversation-1"},"metadata":{"trace_id":"trace-1"},
        "execution":{"idempotency_key":"operation-1"}
    }))
    .unwrap();
    request.client_protocol_envelope = Some(
        plugin_framework::provider_contract::ProtocolContextEnvelope {
            source_protocol: "native".into(),
            headers: [("authorization".into(), vec!["CREDENTIAL_CANARY".into()])].into(),
            ..Default::default()
        },
    );
    let value = ApplicationNativeRunInput {
        request,
        protocol: TranslationProtocol::Native,
    }
    .project_document_input(&operation())
    .unwrap();
    assert_eq!(value["body"]["query"], text);
    assert_eq!(
        value["body"]["inputs"]["rows"].as_array().unwrap().len(),
        45
    );
    assert_eq!(value["body"]["inputs"]["env"]["ordinary"], "business");
    assert_eq!(value["body"]["attachments"][0]["name"], "document");
    assert_eq!(value["body"]["conversation"]["id"], "conversation-1");
    assert!(value["body"].get("client_protocol_envelope").is_none());
    assert!(!value.to_string().contains("CREDENTIAL_CANARY"));
    assert!(value.get("protocol").is_none());
}

#[test]
fn native_document_uses_response_owner_and_excludes_frozen_secret_slots() {
    let output = ApplicationNativeRunOutput(NativeRunResult {
        id: uuid::Uuid::now_v7(),
        application_id: uuid::Uuid::now_v7(),
        api_key_id: uuid::Uuid::now_v7(),
        publication_version_id: uuid::Uuid::now_v7(),
        status: NativeRunStatus::Succeeded,
        node_input_payload: json!({"env":{"secret":"FROZEN_CANARY"},"__client_protocol_envelope":{"handle":"LOCATOR_CANARY"},
            "sys":{"native_inference_recovery":{"grant":"GRANT_CANARY"},"provider_session_identity":"ROUTING_CANARY","requested_model_id":"model"},
            "business":{"env":{"text":"ordinary nested content"},"rows":(0..45).collect::<Vec<_>>()}}),
        metadata: json!({"title":"actual title"}),
        answer: Some("actual answer".repeat(300)),
        answer_segments: None,
        required_action: None,
        tool_calls: Some(json!([{"name":"business tool"}])),
        usage: None,
        error: None,
        operation_terminal: None,
        created_at: time::OffsetDateTime::now_utc(),
    });
    let value = output.project_document_output(&operation()).unwrap();
    assert_eq!(value["answer"], output.0.answer.as_ref().unwrap().as_str());
    assert_eq!(value["tool_calls"][0]["name"], "business tool");
    assert_eq!(
        value["node_input_payload"]["business"]["rows"]
            .as_array()
            .unwrap()
            .len(),
        45
    );
    assert_eq!(
        value["node_input_payload"]["business"]["env"]["text"],
        "ordinary nested content"
    );
    assert_eq!(
        value["node_input_payload"]["sys"]["requested_model_id"],
        "model"
    );
    assert!(value["created_at"].is_string());
    for canary in [
        "FROZEN_CANARY",
        "LOCATOR_CANARY",
        "GRANT_CANARY",
        "ROUTING_CANARY",
    ] {
        assert!(!value.to_string().contains(canary));
    }
    assert!(value.get("api_key_id").is_none());
    assert!(jsonschema::validator_for(&result_schema())
        .unwrap()
        .is_valid(&value));
    // Legacy business event contracts retain their original projection.
    assert_eq!(
        output.project_for_event().unwrap()["0"]["answer"],
        value["answer"]
    );
    assert!(output.project_for_event().unwrap()["0"]
        .get("node_input_payload")
        .is_none());
}
