use super::*;

#[tokio::test]
async fn anthropic_live_flow_started_is_not_duplicated_before_waiting_tool_use() {
    let mut run = native_run();
    let node_run_id = Uuid::from_u128(0x77777777777777777777777777777777);
    let callback_task_id = Uuid::from_u128(0x99999999999999999999999999999999);
    run.status = NativeRunStatus::Waiting;
    run.tool_calls = Some(json!([
        {
            "id": "toolu_next",
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
        "flow_started",
        json!({
            "type": "flow_started",
            "run_id": run.id,
            "status": "running"
        }),
    )
    .await;
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

    let runtime_event_stream = Arc::new(
        ReplayBeforeFallbackRuntimeEventStream::with_closed_subscription_replay(
            vec![RuntimeEventEnvelope::new(
                run.id,
                1,
                debug_stream_events::flow_started(run.id),
            )],
            Vec::new(),
            RuntimeEventCloseReason::WaitingCallback,
        ),
    );
    let state = Arc::new(ApiState {
        test_resources: base_state.test_resources.clone(),
        store: base_state.store.clone(),
        console_policy_reader: Arc::clone(&base_state.console_policy_reader),
        system_backup: base_state.system_backup.clone(),
        system_maintenance: base_state.system_maintenance.clone(),
        authenticator_registry: base_state.authenticator_registry.clone(),
        settings_feature_registry: base_state.settings_feature_registry.clone(),
        console_operation_registry: base_state.console_operation_registry.clone(),
        infrastructure: base_state.infrastructure.clone(),
        extension_boot_snapshot: base_state.extension_boot_snapshot.clone(),
        console_surface_registry: base_state.console_surface_registry.clone(),
        file_storage_registry: base_state.file_storage_registry.clone(),
        runtime_engine: base_state.runtime_engine.clone(),
        provider_runtime: base_state.provider_runtime.clone(),
        process_started_at: base_state.process_started_at,
        runtime_activity: base_state.runtime_activity.clone(),
        assistant_conversation_events: base_state.assistant_conversation_events.clone(),
        notification_service: base_state.notification_service.clone(),
        notification_events: base_state.notification_events.clone(),
        assistant_executions: base_state.assistant_executions.clone(),
        assistant_client_sessions: base_state.assistant_client_sessions.clone(),
        api_runtime_profile: base_state.api_runtime_profile.clone(),
        runtime_host_system: base_state.runtime_host_system.clone(),
        runtime_process_sampler: base_state.runtime_process_sampler.clone(),
        official_plugin_source: base_state.official_plugin_source.clone(),
        official_mcp_bundle_source: base_state.official_mcp_bundle_source.clone(),
        official_extension_catalog_source: base_state.official_extension_catalog_source.clone(),
        official_i18n_catalog_update_service: base_state
            .official_i18n_catalog_update_service
            .clone(),
        official_model_pricing_catalog_index_url: base_state
            .official_model_pricing_catalog_index_url
            .clone(),
        model_billing_require_provider_usage: base_state.model_billing_require_provider_usage,
        api_node_id: base_state.api_node_id.clone(),
        provider_install_root: base_state.provider_install_root.clone(),
        application_template_root: String::new(),
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
        bootstrap_workspace_id: base_state.bootstrap_workspace_id,
        bootstrap_workspace_name: base_state.bootstrap_workspace_name.clone(),
    });
    let (sender, mut receiver) = mpsc::channel(32);
    let mut mapper = AnthropicStreamMapper::new("1flowbase".to_string());

    tokio::time::timeout(
        Duration::from_secs(2),
        send_compatible_runtime_event_stream(
            state,
            run.clone(),
            ANTHROPIC_SSE_PROJECTION,
            Some(0),
            None,
            sender,
            move |run, envelope| mapper.runtime_event_to_sse(run, envelope),
        ),
    )
    .await
    .expect("compatible stream should stop at durable waiting callback");

    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert_eq!(body.matches("event: message_start").count(), 1, "{body}");
    assert!(body.contains("prior node answer"), "{body}");
    assert!(body.contains("lookup_next"), "{body}");
    assert!(body.contains("event: message_stop"), "{body}");
    assert!(body.contains("\"stop_reason\":\"tool_use\""), "{body}");
    assert!(!body.contains("required_action_not_supported"), "{body}");
}

#[tokio::test]
async fn anthropic_same_answer_presentation_from_live_and_durable_is_emitted_once() {
    let mut run = native_run();
    run.answer = Some("answer exactly once".to_string());

    let (base_state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    seed_flow_run_for_compat_sse_test(&base_state, &run).await;
    let answer_delta = debug_stream_events::answer_text_delta(
        "node-answer",
        "answer exactly once".to_string(),
        0,
        Some("node-llm"),
        None,
        Some("text"),
    );
    let runtime_event_stream = Arc::new(
        ReplayBeforeFallbackRuntimeEventStream::with_subscription_replay(
            vec![
                RuntimeEventEnvelope::new(run.id, 1, debug_stream_events::flow_started(run.id)),
                RuntimeEventEnvelope::new(run.id, 2, answer_delta),
                RuntimeEventEnvelope::new(
                    run.id,
                    3,
                    debug_stream_events::flow_finished(
                        run.id,
                        json!({ "answer": "answer exactly once" }),
                    ),
                ),
            ],
            Vec::new(),
        ),
    );
    let state = Arc::new(ApiState {
        test_resources: base_state.test_resources.clone(),
        store: base_state.store.clone(),
        console_policy_reader: Arc::clone(&base_state.console_policy_reader),
        system_backup: base_state.system_backup.clone(),
        system_maintenance: base_state.system_maintenance.clone(),
        authenticator_registry: base_state.authenticator_registry.clone(),
        settings_feature_registry: base_state.settings_feature_registry.clone(),
        console_operation_registry: base_state.console_operation_registry.clone(),
        infrastructure: base_state.infrastructure.clone(),
        extension_boot_snapshot: base_state.extension_boot_snapshot.clone(),
        console_surface_registry: base_state.console_surface_registry.clone(),
        file_storage_registry: base_state.file_storage_registry.clone(),
        runtime_engine: base_state.runtime_engine.clone(),
        provider_runtime: base_state.provider_runtime.clone(),
        process_started_at: base_state.process_started_at,
        runtime_activity: base_state.runtime_activity.clone(),
        assistant_conversation_events: base_state.assistant_conversation_events.clone(),
        notification_service: base_state.notification_service.clone(),
        notification_events: base_state.notification_events.clone(),
        assistant_executions: base_state.assistant_executions.clone(),
        assistant_client_sessions: base_state.assistant_client_sessions.clone(),
        api_runtime_profile: base_state.api_runtime_profile.clone(),
        runtime_host_system: base_state.runtime_host_system.clone(),
        runtime_process_sampler: base_state.runtime_process_sampler.clone(),
        official_plugin_source: base_state.official_plugin_source.clone(),
        official_mcp_bundle_source: base_state.official_mcp_bundle_source.clone(),
        official_extension_catalog_source: base_state.official_extension_catalog_source.clone(),
        official_i18n_catalog_update_service: base_state
            .official_i18n_catalog_update_service
            .clone(),
        official_model_pricing_catalog_index_url: base_state
            .official_model_pricing_catalog_index_url
            .clone(),
        model_billing_require_provider_usage: base_state.model_billing_require_provider_usage,
        api_node_id: base_state.api_node_id.clone(),
        provider_install_root: base_state.provider_install_root.clone(),
        application_template_root: String::new(),
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
        bootstrap_workspace_id: base_state.bootstrap_workspace_id,
        bootstrap_workspace_name: base_state.bootstrap_workspace_name.clone(),
    });
    let (sender, mut receiver) = mpsc::channel(32);
    let mut mapper = AnthropicStreamMapper::new("1flowbase".to_string());

    let _ = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        send_compatible_runtime_event_stream(
            state,
            run,
            ANTHROPIC_SSE_PROJECTION,
            Some(0),
            None,
            sender,
            move |run, envelope| mapper.runtime_event_to_sse(run, envelope),
        ),
    )
    .await;

    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert_eq!(body.matches("answer exactly once").count(), 1, "{body}");
}

#[test]
fn anthropic_delta_payload_maps_reasoning_to_thinking_delta() {
    let payload = anthropic_delta_payload(0, "reasoning_delta", "先分析用户问题".to_string());

    let (event_name, payload) = payload.expect("reasoning delta should map to Anthropic thinking");
    assert_eq!(event_name, "content_block_delta");
    assert_eq!(payload["delta"]["type"], json!("thinking_delta"));
    assert_eq!(payload["delta"]["thinking"], json!("先分析用户问题"));
}

#[tokio::test]
async fn anthropic_completed_stream_projects_thinking_and_visible_text() {
    let mut run = native_run();
    run.status = NativeRunStatus::Succeeded;
    run.answer = Some("<think>先分析</think>\n最终回答".to_string());
    let response = test_projected_events_response(anthropic_completed_run_to_sse(&run, "claude"));
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("\"type\":\"thinking\""), "{body}");
    assert!(body.contains("\"type\":\"thinking_delta\""), "{body}");
    assert!(body.contains("\"thinking\":\"先分析\""), "{body}");
    assert!(body.contains("\"type\":\"text_delta\""), "{body}");
    assert!(body.contains("\"text\":\"\\n最终回答\""), "{body}");
    assert!(!body.contains("<think>"), "{body}");
}

#[tokio::test]
async fn issue_1743_anthropic_stream_uses_canonical_reasoning_and_visible_text() {
    let mut run = native_run();
    run.status = NativeRunStatus::Succeeded;
    run.answer = Some("<think>旧思考</think>旧回答".to_string());
    run.answer_segments = Some(vec![
        AnswerProjectionSegment::reasoning("结构化思考"),
        AnswerProjectionSegment::message("结构化回答"),
    ]);

    let response = test_projected_events_response(anthropic_completed_run_to_sse(&run, "claude"));
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("\"type\":\"thinking_delta\""), "{body}");
    assert!(body.contains("\"thinking\":\"结构化思考\""), "{body}");
    assert!(body.contains("\"text\":\"结构化回答\""), "{body}");
    assert!(!body.contains("旧思考"), "{body}");
    assert!(!body.contains("旧回答"), "{body}");
}

#[tokio::test]
async fn anthropic_live_answer_reasoning_delta_projects_thinking_delta() {
    let run = native_run();
    let mut mapper = AnthropicStreamMapper::new("1flowbase".to_string());
    let reasoning_events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::answer_reasoning_delta(
                "node-answer",
                "private reasoning".to_string(),
                0,
                Some("node-llm"),
                None,
                Some("text"),
            ),
        ),
    );
    let text_events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            2,
            debug_stream_events::answer_text_delta(
                "node-answer",
                "visible answer".to_string(),
                1,
                Some("node-llm"),
                None,
                Some("text"),
            ),
        ),
    );

    let response = test_projected_events_response([reasoning_events, text_events].concat());
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("\"type\":\"thinking\""), "{body}");
    assert!(body.contains("\"type\":\"thinking_delta\""), "{body}");
    assert!(
        body.contains("\"thinking\":\"private reasoning\""),
        "{body}"
    );
    assert!(body.contains("\"text\":\"visible answer\""), "{body}");
    assert_eq!(body.matches("event: content_block_start").count(), 2);
}

#[test]
fn anthropic_projects_answer_presentation_delta_not_provider_raw_delta() {
    let run = native_run();
    let mut mapper = AnthropicStreamMapper::new("1flowbase".to_string());
    let provider_events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::text_delta(
                "node-llm",
                Uuid::from_u128(0x55555555555555555555555555555555),
                "provider raw".to_string(),
            ),
        ),
    );
    let presentation_events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            2,
            debug_stream_events::answer_text_delta(
                "node-answer",
                "answer presentation".to_string(),
                0,
                Some("node-llm"),
                None,
                Some("text"),
            ),
        ),
    );

    assert!(provider_events.is_empty());
    assert_eq!(presentation_events.len(), 2);
}

#[tokio::test]
async fn anthropic_terminal_does_not_reconstruct_answer_text() {
    let run = native_run();
    let mut mapper = AnthropicStreamMapper::new("1flowbase".to_string());
    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_finished(run.id, json!({ "answer": "最终回答" })),
        ),
    );

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(!body.contains("\"type\":\"text_delta\""), "{body}");
    assert!(!body.contains("最终回答"), "{body}");
    assert!(body.contains("event: message_stop"), "{body}");
}

#[tokio::test]
async fn d2_ac_008_anthropic_failed_terminal_with_partial_output_remains_error() {
    let mut run = native_run();
    run.status = NativeRunStatus::Failed;
    run.answer = Some("must-not-replay".to_string());
    run.error = Some(NativeError {
        code: "runtime_error".to_string(),
        message: "safe canonical failure".to_string(),
        details: json!({}),
    });
    let mut mapper = AnthropicStreamMapper::new("1flowbase".to_string());
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
    assert!(body.contains("event: error"), "{body}");
    assert!(!body.contains("event: message_stop"), "{body}");
    assert!(!body.contains("\"stop_reason\":\"end_turn\""), "{body}");
}

#[tokio::test]
async fn d2_ac_008_anthropic_incomplete_terminal_does_not_reconstruct_text() {
    let mut run = native_run();
    run.status = NativeRunStatus::Incomplete;
    run.answer = Some("output limit partial".to_string());
    let mut mapper = AnthropicStreamMapper::new("1flowbase".to_string());
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

    assert!(!body.contains("output limit partial"), "{body}");
    assert!(body.contains("\"stop_reason\":\"max_tokens\""), "{body}");
    assert!(body.contains("event: message_stop"), "{body}");
    assert!(!body.contains("\"stop_reason\":\"end_turn\""), "{body}");
}

#[tokio::test]
async fn d2_ac_004_anthropic_cancelled_terminal_is_error_without_message_stop() {
    let mut run = native_run();
    run.status = NativeRunStatus::Cancelled;
    run.answer = Some("must-not-replay".to_string());
    let mut mapper = AnthropicStreamMapper::new("1flowbase".to_string());
    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(run.id, 1, debug_stream_events::flow_cancelled(run.id)),
    );

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("event: error"), "{body}");
    assert!(body.contains("published run cancelled"), "{body}");
    assert!(!body.contains("must-not-replay"), "{body}");
    assert!(!body.contains("event: message_stop"), "{body}");
    assert!(!body.contains("\"stop_reason\""), "{body}");
}

#[tokio::test]
async fn ac_003_anthropic_waiting_callback_projects_tool_use_and_message_stop() {
    let mut run = native_run();
    run.status = NativeRunStatus::Waiting;
    let mut mapper = AnthropicStreamMapper::new("1flowbase".to_string());
    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            RuntimeEventPayload {
                event_type: "waiting_callback".to_string(),
                source: RuntimeEventSource::Runtime,
                durability: RuntimeEventDurability::DurableRequired,
                persist_required: true,
                trace_visible: true,
                payload: json!({
                    "callback_kind": "llm_tool_calls",
                    "callback_task_id": Uuid::nil(),
                    "tool_calls": [{"id": "toolu_lookup", "name": "lookup", "arguments": {"query": "order"}}]
                }),
            },
        ),
    );

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("event: content_block_start"), "{body}");
    assert!(body.contains("\"type\":\"tool_use\""), "{body}");
    assert!(body.contains("toolu_lookup"), "{body}");
    assert!(body.contains("\"type\":\"input_json_delta\""), "{body}");
    assert!(body.contains("\"stop_reason\":\"tool_use\""), "{body}");
    assert!(body.contains("event: message_stop"), "{body}");
    assert!(!body.contains("required_action_not_supported"), "{body}");
}

#[tokio::test]
async fn anthropic_completed_stream_includes_usage_for_claude_code_cost_and_context() {
    let mut run = native_run();
    run.status = NativeRunStatus::Succeeded;
    run.answer = Some("Final answer".to_string());
    run.usage = Some(NativeUsage {
        prompt_tokens: Some(11),
        completion_tokens: Some(7),
        total_tokens: Some(18),
        input_cache_hit_tokens: Some(3),
        cache_write_tokens: Some(2),
        ..Default::default()
    });

    let response =
        test_projected_events_response(anthropic_completed_run_to_sse(&run, "1flowbase"));
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("event: message_start"), "{body}");
    assert!(body.contains("\"input_tokens\":11"), "{body}");
    assert!(body.contains("\"cache_read_input_tokens\":3"), "{body}");
    assert!(body.contains("\"cache_creation_input_tokens\":2"), "{body}");
    assert!(body.contains("event: message_delta"), "{body}");
    assert!(body.contains("\"output_tokens\":7"), "{body}");
}
