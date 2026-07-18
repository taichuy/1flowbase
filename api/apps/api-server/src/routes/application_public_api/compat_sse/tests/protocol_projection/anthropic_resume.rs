use super::*;

#[tokio::test]
async fn anthropic_resume_replay_terminal_returns_explicit_unsupported() {
    let mut run = native_run();
    let route_node_run_id = Uuid::from_u128(0x77777777777777777777777777777777);
    let previous_callback_task_id = Uuid::from_u128(0x88888888888888888888888888888888);
    let route_callback_task_id = Uuid::from_u128(0x99999999999999999999999999999999);
    run.status = NativeRunStatus::Waiting;
    run.tool_calls = Some(json!([
        {
            "id": "call_route_read",
            "name": "Read",
            "arguments": {
                "file_path": "uploads/agent-flow-node-detail-icon-aligned.png"
            }
        }
    ]));
    run.required_action = Some(NativeRequiredAction {
        action_type: "submit_tool_outputs".to_string(),
        payload: json!({
            "callback_task_id": route_callback_task_id,
            "callback_kind": "llm_tool_calls",
            "node_run_id": route_node_run_id,
            "tool_calls": run.tool_calls.clone().unwrap()
        }),
    });

    let (base_state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    seed_flow_run_for_compat_sse_test(&base_state, &run).await;
    append_compat_sse_runtime_event(
        &base_state,
        run.id,
        "waiting_callback",
        json!({
            "type": "waiting_callback",
            "run_id": run.id,
            "status": "waiting_callback",
            "callback_task_id": previous_callback_task_id,
            "callback_kind": "llm_tool_calls",
            "node_run_id": route_node_run_id,
            "tool_calls": [
                {
                    "id": "call_previous",
                    "name": "Read",
                    "arguments": { "file_path": "uploads/old.png" }
                }
            ]
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
            "callback_task_id": route_callback_task_id,
            "callback_kind": "llm_tool_calls",
            "node_id": "node-llm-1",
            "node_run_id": route_node_run_id,
            "tool_calls": run.tool_calls.clone().unwrap()
        }),
    )
    .await;

    let subscription_replay = vec![
        RuntimeEventEnvelope::new(run.id, 1, debug_stream_events::flow_started(run.id)),
        RuntimeEventEnvelope::new(
            run.id,
            2,
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
                    "callback_task_id": route_callback_task_id,
                    "callback_kind": "llm_tool_calls",
                    "node_id": "node-llm-1",
                    "node_run_id": route_node_run_id,
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
    let mut mapper = AnthropicStreamMapper::new("1flowbase".to_string());

    tokio::time::timeout(
        Duration::from_secs(2),
        send_compatible_runtime_event_stream(
            state,
            run.clone(),
            ANTHROPIC_SSE_PROJECTION,
            Some(0),
            Some(previous_callback_task_id),
            sender,
            move |run, envelope| mapper.runtime_event_to_sse(run, envelope),
        ),
    )
    .await
    .expect("Anthropic stream should stop at replayed route callback");

    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    let response = completed_compatible_stream(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("event: message_start"), "{body}");
    assert!(body.contains("event: error"), "{body}");
    assert!(body.contains("required_action_not_supported"), "{body}");
    assert!(!body.contains("\"type\":\"tool_use\""), "{body}");
    assert!(!body.contains("\"name\":\"Read\""), "{body}");
    assert!(
        !body.contains("agent-flow-node-detail-icon-aligned.png"),
        "{body}"
    );
    assert!(!body.contains("\"stop_reason\":\"tool_use\""), "{body}");
    assert!(!body.contains("\"stop_reason\":\"end_turn\""), "{body}");
    assert!(!body.contains("event: message_stop"), "{body}");
    assert!(!body.contains("uploads/old.png"), "{body}");
}
