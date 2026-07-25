use super::*;
use axum::body::Bytes;
use control_plane::application_public_api::native::{NativeRequiredAction, NativeRunStatus};
use control_plane::application_public_api::protocol_translation::{
    TranslationDecisionKind, TranslationProtocol, TranslationSafeRepresentation,
};
use control_plane::ports::{
    ProviderTransportPayload, ProviderTransportSlotId, ProviderTransportStore,
};
use storage_ephemeral::MemoryProviderTransportStore;
use time::Duration;
use time::OffsetDateTime;
use uuid::Uuid;

fn blocking_run(status: NativeRunStatus) -> NativeRunResult {
    NativeRunResult {
        id: Uuid::nil(),
        application_id: Uuid::nil(),
        api_key_id: Uuid::nil(),
        publication_version_id: Uuid::nil(),
        status,
        node_input_payload: json!({}),
        metadata: json!({}),
        answer: Some("must-not-successify".to_string()),
        answer_segments: None,
        required_action: None,
        tool_calls: None,
        usage: None,
        error: None,
        operation_terminal: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
    }
}

fn provider_transport_payload(canary: &str) -> ProviderTransportPayload {
    ProviderTransportPayload::openai_responses(json!({
        "model": "gpt-test",
        "input": canary,
    }))
    .expect("fixture provider payload should be valid")
}

#[tokio::test]
async fn d3_p1_generate_and_compact_share_the_flow_run_transport_slot() {
    let store = MemoryProviderTransportStore::new(Duration::minutes(5), 64 * 1024);
    for operation in [
        AiNativeOperation::Generate(domain::AiNativeGenerateProfile::Standard),
        AiNativeOperation::Compact(AiNativeCompactProfile::ResponsesCompact),
    ] {
        let flow_run_id = Uuid::now_v7();
        let payload = provider_transport_payload("D3-P1-ROUTE-STAGING-CANARY");
        let expected_payload = payload.clone();

        let slot = stage_openai_provider_transport(&store, flow_run_id, operation, Some(payload))
            .await
            .expect("route-local staging should succeed")
            .expect("Generate and Compact should receive a sealed transport slot");

        assert!(slot == ProviderTransportSlotId::for_flow_run(flow_run_id));
        assert_eq!(store.get(slot).await.unwrap(), Some(expected_payload));
    }
}

#[tokio::test]
async fn d3_p1_count_tokens_without_payload_does_not_create_a_transport_slot() {
    let store = MemoryProviderTransportStore::new(Duration::minutes(5), 64 * 1024);
    let flow_run_id = Uuid::now_v7();

    let slot = stage_openai_provider_transport(
        &store,
        flow_run_id,
        AiNativeOperation::CountTokens,
        None,
    )
    .await
    .expect("CountTokens staging decision should succeed");

    assert!(slot.is_none());
    assert_eq!(
        store
            .get(ProviderTransportSlotId::for_flow_run(flow_run_id))
            .await
            .unwrap(),
        None
    );
}

#[test]
fn d2_ac_001_openai_malformed_json_uses_the_endpoint_protocol_and_safe_receipt() {
    let sentinel = "D2-OPENAI-MALFORMED-JSON-MUST-NOT-REACH-RECEIPT";
    for protocol in [
        TranslationProtocol::OpenAiChat,
        TranslationProtocol::OpenAiResponses,
    ] {
        let error =
            parse_openai_json_body(Bytes::from(format!("{{\"raw\":\"{sentinel}\"")), protocol)
                .expect_err("malformed OpenAI JSON must be rejected by the adapter boundary");
        let OpenAiRouteError::Compat(error) = error else {
            panic!("malformed JSON must remain an OpenAI adapter error");
        };

        assert_eq!(error.report.protocol, protocol);
        assert_eq!(error.report.decisions.len(), 1);
        let decision = &error.report.decisions[0];
        assert_eq!(decision.source_path, "$.body");
        assert_eq!(decision.kind, TranslationDecisionKind::Rejected);
        assert_eq!(
            decision.effective_value,
            TranslationSafeRepresentation::Present
        );
        assert!(
            !serde_json::to_string(&error.report)
                .expect("receipt should serialize")
                .contains(sentinel),
            "malformed JSON must not be retained in the receipt"
        );
    }
}

#[test]
fn openai_response_projects_native_tool_calls() {
    let callback_task_id = Uuid::from_u128(0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);
    let run = NativeRunResult {
        id: Uuid::nil(),
        application_id: Uuid::nil(),
        api_key_id: Uuid::nil(),
        publication_version_id: Uuid::nil(),
        status: NativeRunStatus::Waiting,
        node_input_payload: json!({}),
        metadata: json!({}),
        answer: None,
        answer_segments: None,
        required_action: Some(NativeRequiredAction {
            action_type: "submit_tool_outputs".to_string(),
            payload: json!({ "callback_task_id": callback_task_id, "callback_kind": "llm_tool_calls" }),
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
        operation_terminal: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
    };

    let payload = serde_json::to_value(
        to_openai_response(
            run,
            "provider/model".into(),
            "chatcmpl-test-tool-call".to_string(),
        )
        .expect("waiting external tool call should project"),
    )
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
        status: NativeRunStatus::Succeeded,
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
        operation_terminal: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
    };

    let chat_payload = serde_json::to_value(
        to_openai_response(
            run.clone(),
            "provider/model".into(),
            "chatcmpl-internal".to_string(),
        )
        .expect("succeeded run should project"),
    )
    .expect("openai chat response serializes");
    let responses_payload = serde_json::to_value(
        to_openai_responses_response(run, "provider/model".into(), None)
            .expect("waiting external tool call should project"),
    )
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
        status: NativeRunStatus::Succeeded,
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
        operation_terminal: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
    };

    let payload = serde_json::to_value(
        to_openai_response(
            run,
            "provider/model".into(),
            "chatcmpl-test-callback".to_string(),
        )
        .expect("succeeded run should project"),
    )
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
        operation_terminal: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
    };

    let payload = serde_json::to_value(
        to_openai_responses_response(run, "provider/model".into(), Some("resp_previous".into()))
            .expect("waiting external tool call should project"),
    )
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
fn d2_ac_004_openai_blocking_terminal_status_matrix() {
    let chat_incomplete = serde_json::to_value(
        to_openai_response(
            blocking_run(NativeRunStatus::Incomplete),
            "provider/model".into(),
            "chatcmpl-incomplete".to_string(),
        )
        .expect("incomplete Chat run should project"),
    )
    .expect("Chat response serializes");
    assert_eq!(
        chat_incomplete["choices"][0]["finish_reason"],
        json!("length")
    );
    assert_eq!(
        chat_incomplete["choices"][0]["message"]["content"],
        json!("must-not-successify")
    );

    let responses_incomplete = serde_json::to_value(
        to_openai_responses_response(
            blocking_run(NativeRunStatus::Incomplete),
            "provider/model".into(),
            None,
        )
        .expect("incomplete Responses run should project"),
    )
    .expect("Responses response serializes");
    assert_eq!(responses_incomplete["status"], json!("incomplete"));
    assert_eq!(
        responses_incomplete["incomplete_details"]["reason"],
        json!("max_output_tokens")
    );
    assert_ne!(responses_incomplete["status"], json!("completed"));

    for status in [NativeRunStatus::Failed, NativeRunStatus::Cancelled] {
        assert!(matches!(
            to_openai_response(
                blocking_run(status),
                "provider/model".into(),
                "chatcmpl-terminal-error".to_string(),
            ),
            Err(OpenAiRouteError::Native(_))
        ));
        assert!(matches!(
            to_openai_responses_response(blocking_run(status), "provider/model".into(), None,),
            Err(OpenAiRouteError::Native(_))
        ));
    }

    assert!(matches!(
        to_openai_response(
            blocking_run(NativeRunStatus::Waiting),
            "provider/model".into(),
            "chatcmpl-waiting".to_string(),
        ),
        Err(OpenAiRouteError::RequiredAction)
    ));
    assert!(matches!(
        to_openai_responses_response(
            blocking_run(NativeRunStatus::Waiting),
            "provider/model".into(),
            None,
        ),
        Err(OpenAiRouteError::RequiredAction)
    ));
}

#[test]
fn openai_continuation_inputs_are_translated_to_native() {
    let chat =
        control_plane::application_public_api::compat::openai::translate_chat_completion_request(
            json!({
                "model": "1flowbase",
                "messages": [{"role": "user", "content": "continue"}],
                "tools": [{"type": "function", "function": {"name": "lookup"}}]
            }),
        )
        .expect("tool definitions should enter a Native run");
    assert_eq!(chat.request.inputs.as_value()["tools"][0]["name"], "lookup");

    let responses =
        control_plane::application_public_api::compat::openai::translate_response_request(json!({
            "model": "1flowbase",
            "input": "continue",
            "previous_response_id": "resp_11111111-1111-1111-1111-111111111111"
        }))
        .expect("previous_response_id should be resolved by the route");
    assert_eq!(responses.request.query, "continue");
}
