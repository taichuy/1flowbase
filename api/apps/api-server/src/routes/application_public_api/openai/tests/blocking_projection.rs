use super::*;

#[test]
fn openai_chat_tools_are_translated_to_native() {
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
}

// #2036 AC-003: blocking Responses uses formal current-round items, preserving
// custom calls, encrypted reasoning, phases and original call IDs exactly.
#[test]
fn native_blocking_responses_preserves_formal_items_and_round_identity() {
    let mut run = blocking_run(NativeRunStatus::Waiting);
    let round = Uuid::from_u128(0x77777777777777777777777777777777);
    run.metadata["response_round_id"] = json!(round);
    run.required_action = Some(NativeRequiredAction {
        action_type: "submit_tool_outputs".into(),
        payload: json!({"callback_task_id":Uuid::now_v7(),"callback_kind":"llm_tool_calls"}),
    });
    run.tool_calls = Some(json!([{"id":"legacy-shadow","name":"must_not_project","arguments":{}}]));
    let items = vec![
        json!({"id":"rs_native","type":"reasoning","encrypted_content":"opaque","summary":[]}),
        json!({"id":"msg_native","type":"message","role":"assistant","phase":"commentary","status":"completed","content":[{"type":"output_text","text":"Checking files.","annotations":[]}]}),
        json!({"id":"fc_native","type":"function_call","call_id":"original_function","name":"read","arguments":"{\"path\":\"first\"}","status":"completed"}),
        json!({"id":"ct_native","type":"custom_tool_call","call_id":"original_custom","name":"exec","input":"cat next-file","status":"completed"}),
    ];
    let response = to_openai_responses_response_with_native_items(
        run,
        "provider/model".into(),
        Some("resp_previous_round".into()),
        Some(items.clone()),
    )
    .unwrap();
    assert_eq!(response.output, items);
    assert_eq!(response.id, format!("resp_{round}"));
    assert_eq!(
        response.previous_response_id.as_deref(),
        Some("resp_previous_round")
    );
    assert_eq!(response.status, "completed");
    assert_eq!(response.output_text, "Checking files.");
}

#[test]
fn native_blocking_responses_empty_formal_output_does_not_synthesize_a_message() {
    let response = to_openai_responses_response_with_native_items(
        blocking_run(NativeRunStatus::Succeeded),
        "provider/model".into(),
        None,
        Some(vec![]),
    )
    .unwrap();
    assert!(response.output.is_empty());
    assert_eq!(response.output_text, "");
}

#[test]
fn native_responses_resume_command_keeps_tool_results_as_an_array() {
    let results =
        json!([{"tool_call_id":"native_call","content":[{"type":"input_text","text":"result"}]}]);
    let correlated = json!({"tool_results":results});
    let command = openai_resume_command(
        "",
        Uuid::now_v7(),
        PublishedCallbackResumeSource::OpenAiResponses,
        correlated["tool_results"].clone(),
        Some("blocking".into()),
    );
    assert_eq!(command.response_payload, correlated);
    assert!(command.response_payload["tool_results"].is_array());
}

// Issue 2036 blocking regression: the collector must distinguish canonical
// tool events from an observed, but empty, native formal output stream.
#[test]
fn blocking_collector_keeps_canonical_function_tool_fallback() {
    let mut items = None;
    collect_blocking_response_output_item(
        &mut items,
        "tool_call_commit",
        &json!({"id":"call_inventory"}),
    );
    assert!(items.is_none());
    let callback_task_id = Uuid::now_v7();
    let mut run = blocking_run(NativeRunStatus::Waiting);
    run.required_action = Some(NativeRequiredAction {
        action_type: "submit_tool_outputs".into(),
        payload: json!({"callback_task_id":callback_task_id,"callback_kind":"llm_tool_calls"}),
    });
    run.tool_calls = Some(
        json!([{"id":"call_inventory","name":"lookup_inventory","arguments":{"sku":"sku_123"}}]),
    );
    let response =
        to_openai_responses_response_with_native_items(run, "provider/model".into(), None, items)
            .unwrap();
    let payload = serde_json::to_value(response).unwrap();
    assert_eq!(payload["status"], "completed");
    assert_eq!(payload["output"][0]["type"], "function_call");
    assert_eq!(payload["output"][0]["call_id"], "call_inventory");
}

#[test]
fn blocking_collector_observed_native_items_suppresses_empty_output_fallback() {
    let mut items = None;
    collect_blocking_response_output_item(
        &mut items,
        "provider_output_item_added",
        &json!({"item":{"type":"message","id":"msg_native"}}),
    );
    assert_eq!(items, Some(vec![]));
    let response = to_openai_responses_response_with_native_items(
        blocking_run(NativeRunStatus::Succeeded),
        "provider/model".into(),
        None,
        items,
    )
    .unwrap();
    let payload = serde_json::to_value(response).unwrap();
    assert_eq!(payload["output"], json!([]));
    assert_eq!(payload["output_text"], "");
}

#[test]
fn blocking_collector_keeps_native_done_item_order_and_shape() {
    let mut items = None;
    let expected = vec![
        json!({"type":"reasoning","id":"rs_native","encrypted_content":"opaque"}),
        json!({"type":"custom_tool_call","call_id":"call_original","name":"exec","input":"pwd"}),
    ];
    for item in &expected {
        collect_blocking_response_output_item(
            &mut items,
            "provider_output_item_added",
            &json!({"item":item}),
        );
        collect_blocking_response_output_item(
            &mut items,
            "provider_output_item_done",
            &json!({"item":item}),
        );
    }
    assert_eq!(items, Some(expected));
}

#[test]
fn blocking_collector_projects_canonical_callback_completion() {
    let mut items = None;
    collect_blocking_response_output_item(
        &mut items,
        "answer_delta",
        &json!({"delta":"Inventory confirmed"}),
    );
    collect_blocking_response_output_item(&mut items, "flow_completed", &json!({}));
    let mut run = blocking_run(NativeRunStatus::Succeeded);
    run.answer = Some("Inventory confirmed".into());
    let response = to_openai_responses_response_with_native_items(
        run,
        "provider/model".into(),
        Some("resp_previous".into()),
        items,
    )
    .unwrap();
    let payload = serde_json::to_value(response).unwrap();
    assert_eq!(payload["status"], "completed");
    assert_eq!(payload["previous_response_id"], "resp_previous");
    assert_eq!(payload["output_text"], "Inventory confirmed");
    assert_eq!(payload["output"].as_array().unwrap().len(), 1);
    assert_eq!(payload["output"][0]["type"], "message");
    assert_eq!(
        payload["output"][0]["content"][0]["text"],
        "Inventory confirmed"
    );
}

#[test]
fn external_protocol_context_cannot_claim_host_transport_connection_scope() {
    let key = control_plane::orchestration_runtime::HOST_TRANSPORT_CONNECTION_SCOPE_HEADER;
    let mut headers = HeaderMap::new();
    headers.insert(key, "forged-from-http".parse().unwrap());
    headers.insert("session-id", "legitimate-client-session".parse().unwrap());
    let translated = ProtocolContextEnvelope {
        source_protocol: "openai_responses".into(),
        headers: BTreeMap::from([(key.into(), vec!["forged-from-body".into()])]),
        ..Default::default()
    };
    let captured = openai_protocol_context_from_ingress(
        ClientProtocolIngressPolicy::OpenAiResponses,
        None,
        &headers,
        Some(translated),
    )
    .unwrap();
    assert!(!captured.headers.contains_key(key));
    assert_eq!(
        captured.headers["session-id"],
        vec!["legitimate-client-session"]
    );
}

#[tokio::test]
async fn semantic_ambiguous_round_maps_to_openai_conflict() {
    let error: OpenAiRouteError = native::service_error(
        control_plane::errors::ControlPlaneError::Conflict("responses_tool_output_ambiguous_round")
            .into(),
    )
    .into();
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        body["error"]["code"],
        "responses_tool_output_ambiguous_round"
    );
    assert_eq!(body["error"]["type"], "invalid_request_error");
}

#[test]
fn invalid_callback_tool_results_map_to_bad_request() {
    let error = native::service_error(
        control_plane::errors::ControlPlaneError::InvalidInput("tool_results").into(),
    );
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert_eq!(error.code, "tool_results");
}

#[tokio::test]
async fn unary_openai_error_preserves_upstream_object_and_actual_status() {
    use control_plane::application_public_api::native::NativeError;
    for upstream in [
        json!({"message":"rejected\nnext line","type":"vendor_refusal","code":"unknown_vendor_code","param":null,"extra":{"opaque":[null,1]}}),
        json!({"message":null,"type":null,"code":null,"future":true}),
        json!({"extra":[]}),
        json!({"message":"","code":""}),
    ] {
        let mut run = blocking_run(NativeRunStatus::Failed);
        run.error = Some(NativeError {
            code: "unknown_vendor_code".into(),
            message: "fallback".into(),
            details: json!({"upstream_error":upstream,"status_code":422,"raw_body":"original body\n","semantic_terminal":true}),
        });
        let response =
            OpenAiRouteError::from(native::blocking_run_projection_error(&run)).into_response();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload, json!({"error":upstream}));
    }
}

#[tokio::test]
async fn unary_gateway_error_keeps_existing_openai_fallback() {
    let run = blocking_run(NativeRunStatus::Failed);
    let response =
        OpenAiRouteError::from(native::blocking_run_projection_error(&run)).into_response();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        payload,
        json!({"error":{"message":"published run failed","type":"invalid_request_error","param":null,"code":"runtime_error"}})
    );
}

#[tokio::test]
async fn unary_native_error_preserves_unknown_upstream_code_and_details() {
    use control_plane::application_public_api::native::NativeError;
    let upstream = json!({"message":"original\nmessage","code":"future_native_code","type":"vendor_error","extra":null});
    let mut run = blocking_run(NativeRunStatus::Failed);
    let error = NativeError {
        code: "future_native_code".into(),
        message: "original\nmessage".into(),
        details: json!({"upstream_error":upstream,"status_code":409,"raw_body":"original body\n"}),
    };
    run.error = Some(error.clone());
    let response = native::blocking_run_projection_error(&run).into_response();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload, serde_json::to_value(error).unwrap());
}

#[test]
fn chat_resume_unary_usage_uses_immutable_boundary_for_retries_and_old_callbacks() {
    use super::super::usage::{to_chat_response_for_round, ResponseUsageBaseline};
    let baseline = json!({"native_usage_baseline":{"prompt_tokens":7995,"completion_tokens":132,"total_tokens":8127,"input_cache_hit_tokens":512,"cache_read_tokens":512,"reasoning_tokens":100}});
    let cumulative = NativeUsage {
        prompt_tokens: Some(16212),
        completion_tokens: Some(157),
        total_tokens: Some(16369),
        input_cache_hit_tokens: Some(8448),
        cache_read_tokens: Some(8448),
        reasoning_tokens: Some(110),
        ..NativeUsage::default()
    };
    // Fresh admission, attachment and replay all read the same persisted task boundary,
    // even if the current Native snapshot already contains resumed usage.
    for _ in 0..3 {
        let mut run = blocking_run(NativeRunStatus::Succeeded);
        run.usage = Some(cumulative.clone());
        let response = to_chat_response_for_round(
            run.clone(),
            "deepseek-flash".into(),
            "turn".into(),
            &ResponseUsageBaseline::from_callback_payload(&baseline),
        )
        .unwrap();
        let wire = serde_json::to_value(response).unwrap();
        assert_eq!(wire["usage"]["prompt_tokens"], 8217);
        assert_eq!(wire["usage"]["completion_tokens"], 25);
        assert_eq!(wire["usage"]["total_tokens"], 8242);
        assert_eq!(
            wire["usage"]["prompt_tokens_details"]["cached_tokens"],
            7936
        );
        assert_eq!(
            wire["usage"]["completion_tokens_details"]["reasoning_tokens"],
            10
        );
        assert_eq!(run.usage.unwrap().prompt_tokens, Some(16212));
    }
    for payload in [
        json!({}),
        json!({"native_usage_baseline":"invalid"}),
        json!({"native_usage_baseline":{}}),
        json!({"native_usage_baseline":{"unknown":1}}),
    ] {
        let mut run = blocking_run(NativeRunStatus::Succeeded);
        run.usage = Some(cumulative.clone());
        let response = to_chat_response_for_round(
            run,
            "deepseek-flash".into(),
            "old".into(),
            &ResponseUsageBaseline::from_callback_payload(&payload),
        )
        .unwrap();
        assert!(serde_json::to_value(response)
            .unwrap()
            .get("usage")
            .is_none());
    }
    let known_empty =
        ResponseUsageBaseline::from_callback_payload(&json!({"native_usage_baseline":null}));
    assert_eq!(
        known_empty
            .project(Some(&cumulative))
            .unwrap()
            .prompt_tokens,
        16212
    );
    let regressed = NativeUsage {
        prompt_tokens: Some(1),
        ..cumulative
    };
    assert!(ResponseUsageBaseline::from_callback_payload(&baseline)
        .project(Some(&regressed))
        .is_none());
}

#[test]
fn preferred_blocking_projection_uses_selection_and_required_transport_stays_strict() {
    use control_plane::ports::{
        LlmInputRepresentation, LlmInputSelection, ProviderTransportPayload, RuntimeEventEnvelope,
    };
    let required = ProviderTransportPayload::openai_responses(json!({"input":[]})).unwrap();
    assert_eq!(
        compat_sse::ResponsesProjectionMode::from_transport(None),
        compat_sse::ResponsesProjectionMode::SemanticNativeResponses
    );
    assert_eq!(
        compat_sse::ResponsesProjectionMode::from_transport(Some(&required)),
        compat_sse::ResponsesProjectionMode::TransparentProviderResponses
    );
    let preferred = required.preferred();
    assert_eq!(
        compat_sse::ResponsesProjectionMode::from_transport(Some(&preferred)),
        compat_sse::ResponsesProjectionMode::AwaitingHostSelection
    );
    for native in [false, true] {
        let mut run = blocking_run(NativeRunStatus::Succeeded);
        run.answer = Some("blocking marker".into());
        let node = Uuid::now_v7();
        let item = json!({"type":"message","id":"msg_blocking_original","role":"assistant","content":[{"type":"output_text","text":"blocking marker"}]});
        let mut items = None;
        let mut selector = compat_sse::ResponsesProjectionSelector::new(
            compat_sse::ResponsesProjectionMode::from_transport(Some(&preferred)),
        );
        let facts = [
            LlmInputSelection {
                node_id: "llm".into(),
                node_run_id: node,
                representation: if native {
                    LlmInputRepresentation::NativeOpaque
                } else {
                    LlmInputRepresentation::SemanticMapped
                },
            }
            .runtime_event(),
            control_plane::orchestration_runtime::debug_stream_events::provider_output_item_done(
                "llm",
                node,
                0,
                item.clone(),
            ),
        ];
        for (index, fact) in facts.into_iter().enumerate() {
            collect_selected_blocking_response_event(
                &mut items,
                &mut selector,
                run.id,
                &RuntimeEventEnvelope::new(run.id, index as i64, fact),
            );
        }
        let response = serde_json::to_value(
            to_openai_responses_response_with_native_items(run, "fixture".into(), None, items)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(response["output"].as_array().unwrap().len(), 1);
        assert_eq!(
            response["output"][0]["content"][0]["text"],
            "blocking marker"
        );
        if native {
            assert_eq!(response["output"], json!([item]));
        }
    }
}

#[test]
fn responses_unary_usage_is_round_local_and_unknown_baseline_is_null() {
    use super::super::usage::ResponseUsageBaseline;
    let cumulative = NativeUsage {
        prompt_tokens: Some(220),
        completion_tokens: Some(35),
        total_tokens: Some(255),
        cache_read_tokens: Some(150),
        reasoning_tokens: Some(12),
        ..NativeUsage::default()
    };
    let items = vec![
        json!({"type":"message","id":"provider-id","phase":"final_answer","role":"assistant","content":[{"type":"output_text","text":"done","annotations":[]}],"status":"completed"}),
    ];
    for payload in [
        json!({"native_usage_baseline":{"prompt_tokens":100,"completion_tokens":20,"total_tokens":120,"cache_read_tokens":60,"reasoning_tokens":5}}),
        json!({}),
    ] {
        for _ in 0..2 {
            let mut run = blocking_run(NativeRunStatus::Succeeded);
            run.usage = Some(cumulative.clone());
            let response = to_openai_responses_response_for_round(
                run.clone(),
                "provider/model".into(),
                Some("resp_prior".into()),
                Some(items.clone()),
                &ResponseUsageBaseline::from_callback_payload(&payload),
            )
            .unwrap();
            let wire = serde_json::to_value(response).unwrap();
            assert_eq!(wire["output"], json!(items));
            if payload.get("native_usage_baseline").is_some() {
                assert_eq!(
                    wire["usage"],
                    json!({"input_tokens":120,"output_tokens":15,"total_tokens":135,"input_tokens_details":{"cached_tokens":90},"output_tokens_details":{"reasoning_tokens":7}})
                );
            } else {
                assert!(wire["usage"].is_null());
            }
            assert_eq!(run.usage.as_ref().unwrap().prompt_tokens, Some(220));
        }
    }
}
