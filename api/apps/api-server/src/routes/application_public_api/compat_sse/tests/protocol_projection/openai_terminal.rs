use super::*;

#[tokio::test]
async fn openai_chat_live_answer_delta_is_not_duplicated_before_waiting_becomes_unsupported() {
    let mut run = native_run();
    let node_run_id = Uuid::from_u128(0x77777777777777777777777777777777);
    let callback_task_id = Uuid::from_u128(0x99999999999999999999999999999999);
    run.status = NativeRunStatus::Waiting;
    run.tool_calls = Some(json!([
        {
            "id": "call_next",
            "name": "lookup_next",
            "arguments": { "query": "next" }
        }
    ]));
    run.required_action = Some(NativeRequiredAction {
        action_type: "submit_tool_outputs".to_string(),
        payload: json!({
            "callback_task_id": callback_task_id,
            "callback_kind": "llm_tool_calls",
            "node_run_id": node_run_id,
            "tool_calls": run.tool_calls.clone().unwrap()
        }),
    });

    let (base_state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    seed_flow_run_for_compat_sse_test(&base_state, &run).await;
    append_compat_sse_runtime_event(
        &base_state,
        run.id,
        "text_delta",
        json!({
            "type": "text_delta",
            "event_type": "text_delta",
            "node_id": "node-answer",
            "text": "prior node answer",
            "presentation": {
                "kind": "answer",
                "answer_node_id": "node-answer",
                "source_node_id": "node-llm",
                "source_node_run_id": node_run_id,
                "source_output_key": "text",
                "segment_index": 0
            }
        }),
    )
    .await;
    append_compat_sse_runtime_event(
        &base_state,
        run.id,
        "waiting_callback",
        json!({
            "type": "waiting_callback",
            "run_id": run.id,
            "status": "waiting_callback",
            "callback_task_id": callback_task_id,
            "callback_kind": "llm_tool_calls",
            "node_run_id": node_run_id,
            "tool_calls": run.tool_calls.clone().unwrap()
        }),
    )
    .await;

    let subscription_replay = vec![
        RuntimeEventEnvelope::new(run.id, 1, debug_stream_events::flow_started(run.id)),
        RuntimeEventEnvelope::new(
            run.id,
            2,
            debug_stream_events::answer_text_delta(
                "node-answer",
                "prior node answer".to_string(),
                0,
                Some("node-llm"),
                Some(node_run_id),
                Some("text"),
            ),
        ),
        RuntimeEventEnvelope::new(
            run.id,
            3,
            RuntimeEventPayload {
                event_type: "waiting_callback".to_string(),
                source: RuntimeEventSource::Runtime,
                durability: RuntimeEventDurability::DurableRequired,
                persist_required: true,
                trace_visible: true,
                payload: json!({
                    "type": "waiting_callback",
                    "run_id": run.id,
                    "status": "waiting_callback",
                    "callback_task_id": callback_task_id,
                    "callback_kind": "llm_tool_calls",
                    "node_run_id": node_run_id,
                    "tool_calls": run.tool_calls.clone().unwrap()
                }),
            },
        ),
    ];
    let runtime_event_stream = Arc::new(
        ReplayBeforeFallbackRuntimeEventStream::with_subscription_replay(
            subscription_replay,
            Vec::new(),
        ),
    );
    let state = Arc::new(ApiState {
        store: base_state.store.clone(),
        settings_feature_registry: base_state.settings_feature_registry.clone(),
        console_operation_registry: base_state.console_operation_registry.clone(),
        infrastructure: base_state.infrastructure.clone(),
        console_surface_registry: base_state.console_surface_registry.clone(),
        file_storage_registry: base_state.file_storage_registry.clone(),
        runtime_engine: base_state.runtime_engine.clone(),
        provider_runtime: base_state.provider_runtime.clone(),
        process_started_at: base_state.process_started_at,
        runtime_activity: base_state.runtime_activity.clone(),
        api_runtime_profile: base_state.api_runtime_profile.clone(),
        plugin_runner_system: base_state.plugin_runner_system.clone(),
        official_plugin_source: base_state.official_plugin_source.clone(),
        official_agent_flow_template_source: base_state.official_agent_flow_template_source.clone(),
        official_mcp_bundle_source: base_state.official_mcp_bundle_source.clone(),
        api_node_id: base_state.api_node_id.clone(),
        provider_install_root: base_state.provider_install_root.clone(),
        provider_secret_master_key: base_state.provider_secret_master_key.clone(),
        host_extension_dropin_root: base_state.host_extension_dropin_root.clone(),
        allow_unverified_filesystem_dropins: base_state.allow_unverified_filesystem_dropins,
        allow_uploaded_host_extensions: base_state.allow_uploaded_host_extensions,
        session_store: base_state.session_store.clone(),
        runtime_event_stream,
        api_docs: base_state.api_docs.clone(),
        cookie_name: base_state.cookie_name.clone(),
        cookie_secure: base_state.cookie_secure,
        session_ttl_days: base_state.session_ttl_days,
        bootstrap_workspace_name: base_state.bootstrap_workspace_name.clone(),
    });
    let (sender, mut receiver) = mpsc::channel(32);
    let mut mapper =
        OpenAiChatStreamMapper::new("1flowbase".to_string(), "chatcmpl-test".to_string(), true);

    tokio::time::timeout(
        Duration::from_secs(2),
        send_compatible_runtime_event_stream(
            state,
            run.clone(),
            OPENAI_CHAT_SSE_PROJECTION,
            Some(0),
            None,
            sender,
            move |run, envelope| mapper.runtime_event_to_sse(run, envelope),
        ),
    )
    .await
    .expect("compatible stream should stop at replayed waiting callback");

    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert_eq!(body.matches("prior node answer").count(), 1, "{body}");
    assert!(body.contains("required_action_not_supported"), "{body}");
    assert!(!body.contains("lookup_next"), "{body}");
    assert!(!body.contains("\"finish_reason\":\"stop\""), "{body}");
    assert!(!body.contains("\"finish_reason\":\"tool_calls\""), "{body}");
    assert!(!body.contains("\"finish_reason\":\"length\""), "{body}");
    assert!(!body.contains("[DONE]"), "{body}");
}

#[test]
fn openai_responses_resume_terminal_answer_fallback_emits_output_delta() {
    let run = native_run();
    let mut mapper = OpenAiResponseStreamMapper::new("1flowbase".to_string(), None, true);
    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_finished(run.id, json!({ "answer": "最终回答" })),
        ),
    );

    // output_item.added + output_text.delta + output_item.done + response.completed
    assert_eq!(events.len(), 4);
}

#[tokio::test]
async fn d2_ac_008_openai_chat_failed_terminal_with_partial_output_remains_error() {
    let mut run = native_run();
    run.status = NativeRunStatus::Failed;
    run.answer = Some("must-not-replay".to_string());
    run.error = Some(NativeError {
        code: "runtime_error".to_string(),
        message: "safe canonical failure".to_string(),
        details: json!({}),
    });
    let mut mapper =
        OpenAiChatStreamMapper::new("1flowbase".to_string(), "chatcmpl-test".to_string(), true);
    let mut events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::answer_text_delta(
                "node-answer",
                "real partial delta".to_string(),
                0,
                Some("node-llm"),
                None,
                Some("text"),
            ),
        ),
    );
    events.extend(mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            2,
            debug_stream_events::flow_failed(
                run.id,
                json!({
                    "message": "provider raw secret",
                    "answer_segments": [{"kind": "reasoning", "text": "must-not-replay"}]
                }),
            ),
        ),
    ));

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("real partial delta"), "{body}");
    assert!(body.contains("safe canonical failure"), "{body}");
    assert!(!body.contains("must-not-replay"), "{body}");
    assert!(!body.contains("provider raw secret"), "{body}");
    assert!(!body.contains("\"finish_reason\":\"stop\""), "{body}");
    assert!(!body.contains("[DONE]"), "{body}");
}

#[tokio::test]
async fn d2_ac_008_openai_responses_failed_terminal_with_partial_output_remains_failed() {
    let mut run = native_run();
    run.status = NativeRunStatus::Failed;
    run.answer = Some("must-not-replay".to_string());
    run.error = Some(NativeError {
        code: "runtime_error".to_string(),
        message: "safe canonical failure".to_string(),
        details: json!({}),
    });
    let mut mapper = OpenAiResponseStreamMapper::new("1flowbase".to_string(), None, true);
    let mut events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::answer_text_delta(
                "node-answer",
                "real partial delta".to_string(),
                0,
                Some("node-llm"),
                None,
                Some("text"),
            ),
        ),
    );
    events.extend(mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            2,
            debug_stream_events::flow_failed(
                run.id,
                json!({
                    "message": "provider raw secret",
                    "answer_segments": [{"kind": "reasoning", "text": "must-not-replay"}]
                }),
            ),
        ),
    ));

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("event: response.output_text.delta"), "{body}");
    assert!(body.contains("real partial delta"), "{body}");
    assert!(body.contains("safe canonical failure"), "{body}");
    assert!(!body.contains("must-not-replay"), "{body}");
    assert!(!body.contains("provider raw secret"), "{body}");
    assert!(body.contains("event: response.failed"), "{body}");
    assert!(!body.contains("event: response.completed"), "{body}");
}

#[tokio::test]
async fn d2_ac_008_openai_chat_incomplete_terminal_uses_length_and_done() {
    let mut run = native_run();
    run.status = NativeRunStatus::Incomplete;
    run.answer = Some("output limit partial".to_string());
    let mut mapper =
        OpenAiChatStreamMapper::new("1flowbase".to_string(), "chatcmpl-test".to_string(), true);
    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_incomplete(
                run.id,
                json!({ "answer": "output limit partial" }),
            ),
        ),
    );

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("output limit partial"), "{body}");
    assert!(body.contains("\"finish_reason\":\"length\""), "{body}");
    assert!(body.contains("[DONE]"), "{body}");
    assert!(!body.contains("\"finish_reason\":\"stop\""), "{body}");
}

#[tokio::test]
async fn d2_ac_008_openai_responses_incomplete_terminal_uses_response_incomplete() {
    let mut run = native_run();
    run.status = NativeRunStatus::Incomplete;
    run.answer = Some("output limit partial".to_string());
    let mut mapper = OpenAiResponseStreamMapper::new("1flowbase".to_string(), None, true);
    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_incomplete(
                run.id,
                json!({ "answer": "output limit partial" }),
            ),
        ),
    );

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("output limit partial"), "{body}");
    assert!(body.contains("event: response.incomplete"), "{body}");
    assert!(body.contains("\"status\":\"incomplete\""), "{body}");
    assert!(!body.contains("event: response.completed"), "{body}");
}

#[tokio::test]
async fn d2_ac_004_openai_chat_cancelled_terminal_is_error_without_done() {
    let mut run = native_run();
    run.status = NativeRunStatus::Cancelled;
    run.answer = Some("must-not-replay".to_string());
    let mut mapper =
        OpenAiChatStreamMapper::new("1flowbase".to_string(), "chatcmpl-test".to_string(), true);
    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(run.id, 1, debug_stream_events::flow_cancelled(run.id)),
    );

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("\"code\":\"run_cancelled\""), "{body}");
    assert!(!body.contains("must-not-replay"), "{body}");
    assert!(!body.contains("\"finish_reason\""), "{body}");
    assert!(!body.contains("[DONE]"), "{body}");
}

#[tokio::test]
async fn d2_ac_004_openai_responses_cancelled_terminal_is_failed_without_completed() {
    let mut run = native_run();
    run.status = NativeRunStatus::Cancelled;
    run.answer = Some("must-not-replay".to_string());
    let mut mapper = OpenAiResponseStreamMapper::new("1flowbase".to_string(), None, true);
    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(run.id, 1, debug_stream_events::flow_cancelled(run.id)),
    );

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("event: response.failed"), "{body}");
    assert!(body.contains("\"code\":\"run_cancelled\""), "{body}");
    assert!(!body.contains("must-not-replay"), "{body}");
    assert!(!body.contains("event: response.completed"), "{body}");
}

#[tokio::test]
async fn d2_ac_004_openai_waiting_terminal_is_adapter_unsupported_without_success_signal() {
    let mut run = native_run();
    run.status = NativeRunStatus::Waiting;
    let waiting = RuntimeEventPayload {
        event_type: "waiting_callback".to_string(),
        source: RuntimeEventSource::Runtime,
        durability: RuntimeEventDurability::DurableRequired,
        persist_required: true,
        trace_visible: true,
        payload: json!({
            "callback_kind": "llm_tool_calls",
            "callback_task_id": Uuid::nil(),
            "tool_calls": [{"id": "must-not-project", "name": "lookup", "arguments": {}}]
        }),
    };
    let mut chat =
        OpenAiChatStreamMapper::new("1flowbase".to_string(), "chatcmpl-test".to_string(), true);
    let chat_body = {
        let response = test_projected_events_response(
            chat.runtime_event_to_sse(&run, RuntimeEventEnvelope::new(run.id, 1, waiting.clone())),
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        String::from_utf8(body.to_vec()).unwrap()
    };
    assert!(
        chat_body.contains("required_action_not_supported"),
        "{chat_body}"
    );
    assert!(!chat_body.contains("must-not-project"), "{chat_body}");
    assert!(!chat_body.contains("\"finish_reason\""), "{chat_body}");
    assert!(!chat_body.contains("[DONE]"), "{chat_body}");

    let mut responses = OpenAiResponseStreamMapper::new("1flowbase".to_string(), None, true);
    let response = test_projected_events_response(
        responses.runtime_event_to_sse(&run, RuntimeEventEnvelope::new(run.id, 1, waiting)),
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();
    assert!(body.contains("event: response.failed"), "{body}");
    assert!(body.contains("required_action_not_supported"), "{body}");
    assert!(!body.contains("must-not-project"), "{body}");
    assert!(!body.contains("event: response.completed"), "{body}");
}
