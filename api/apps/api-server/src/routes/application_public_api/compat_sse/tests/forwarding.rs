use super::super::protocol_mappers::{
    anthropic_delta_payload, openai_delta_chunk_payload, openai_finish_chunk_payload,
    openai_response_function_call_output_items, openai_tool_call_chunk_payload,
    AnthropicStreamMapper, OpenAiChatStreamMapper, OpenAiResponseStreamMapper,
};
use super::super::*;
use super::support::*;
use crate::{
    app_state::ApiState,
    host_infrastructure::LocalRuntimeEventStream,
    routes::application_public_api::sse::{
        send_native_runtime_event_stream, IncludeWorkflowEvents,
    },
};
use axum::response::IntoResponse;
use control_plane::{
    application_public_api::native::{AnswerProjectionSegment, NativeError, NativeRequiredAction},
    ports::{
        OrchestrationRuntimeRepository, RuntimeEventCloseReason, RuntimeEventDurability,
        RuntimeEventPayload, RuntimeEventSource, RuntimeEventStream, RuntimeEventStreamPolicy,
        UpdateFlowRunInput,
    },
};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio::sync::mpsc;
use uuid::Uuid;

async fn native_sse_body_from_replay(
    base_state: &ApiState,
    run: NativeRunResult,
    replay: Vec<RuntimeEventEnvelope>,
    from_sequence: Option<i64>,
) -> String {
    let runtime_event_stream = Arc::new(
        ReplayBeforeFallbackRuntimeEventStream::with_closed_subscription_replay(
            replay,
            Vec::new(),
            RuntimeEventCloseReason::Incomplete,
        ),
    );
    let mut state = (*base_state).clone();
    state.runtime_event_stream = runtime_event_stream;
    let (sender, mut receiver) = mpsc::channel(32);

    tokio::time::timeout(
        Duration::from_secs(2),
        send_native_runtime_event_stream(
            Arc::new(state),
            run,
            IncludeWorkflowEvents::None,
            from_sequence,
            None,
            sender,
        ),
    )
    .await
    .expect("Native SSE replay should stop at an incomplete terminal");

    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    let response = axum::response::sse::Sse::new(tokio_stream::iter(events)).into_response();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Native SSE response body should be readable");
    String::from_utf8(body.to_vec()).expect("Native SSE response body should be UTF-8")
}

async fn running_run_with_closed_stream() -> (NativeRunResult, Arc<ApiState>) {
    let (base_state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let run = native_run();
    seed_flow_run_for_compat_sse_test(&base_state, &run).await;
    base_state
        .store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id: run.id,
            status: domain::FlowRunStatus::Running,
            output_payload: json!({ "answer": "partial output" }),
            error_payload: None,
            finished_at: None,
        })
        .await
        .expect("seed a published running run");

    let runtime_event_stream = Arc::new(LocalRuntimeEventStream::new());
    runtime_event_stream
        .open_run(run.id, RuntimeEventStreamPolicy::debug_default())
        .await
        .expect("open the live stream");
    runtime_event_stream
        .close_run(run.id, RuntimeEventCloseReason::Failed)
        .await
        .expect("close the live stream without a terminal");
    let mut state = (*base_state).clone();
    state.runtime_event_stream = runtime_event_stream;
    (run, Arc::new(state))
}

async fn sse_body(
    events: Vec<Result<axum::response::sse::Event, std::convert::Infallible>>,
) -> String {
    let response = axum::response::sse::Sse::new(tokio_stream::iter(events)).into_response();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("SSE response body should be readable");
    String::from_utf8(body.to_vec()).expect("SSE response body should be UTF-8")
}

#[tokio::test]
async fn d2_ac_008_native_eof_fallback_finalizes_running_winner_before_projection() {
    let (run, state) = running_run_with_closed_stream().await;
    let (sender, mut receiver) = mpsc::channel(32);

    send_native_runtime_event_stream(
        state.clone(),
        run.clone(),
        IncludeWorkflowEvents::None,
        None,
        None,
        sender,
    )
    .await;

    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    let body = sse_body(events).await;
    let recovered = state
        .store
        .get_flow_run(run.application_id, run.id)
        .await
        .expect("read durable run")
        .expect("seeded run remains present");
    let runtime_events = state
        .store
        .list_runtime_events(run.id, 0)
        .await
        .expect("read durable runtime events");

    assert_eq!(recovered.status, domain::FlowRunStatus::Failed);
    assert_eq!(
        recovered.error_payload,
        Some(json!({
            "code": "stream_terminal_missing",
            "message": "runtime event stream ended without a terminal event"
        }))
    );
    assert_eq!(
        runtime_events
            .iter()
            .filter(|event| event.event_type == "flow_failed")
            .count(),
        1,
        "EOF recovery must write one durable canonical failure"
    );
    assert!(body.contains("event: run.failed"), "{body}");
    assert!(!body.contains("event: run.completed"), "{body}");
}

#[tokio::test]
async fn d2_ac_008_compatible_eof_fallback_projects_recovered_failed_winner() {
    let (run, state) = running_run_with_closed_stream().await;
    let (sender, _receiver) = mpsc::channel(32);
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mapper_seen = seen.clone();

    send_compatible_runtime_event_stream(
        state.clone(),
        run.clone(),
        OPENAI_CHAT_SSE_PROJECTION,
        None,
        None,
        sender,
        move |winner, envelope| {
            mapper_seen
                .lock()
                .expect("mapper observation lock should be available")
                .push((format!("{:?}", winner.status), envelope.event_type));
            Vec::new()
        },
    )
    .await;

    let recovered = state
        .store
        .get_flow_run(run.application_id, run.id)
        .await
        .expect("read durable run")
        .expect("seeded run remains present");
    let runtime_events = state
        .store
        .list_runtime_events(run.id, 0)
        .await
        .expect("read durable runtime events");

    assert_eq!(recovered.status, domain::FlowRunStatus::Failed);
    assert_eq!(
        runtime_events
            .iter()
            .filter(|event| event.event_type == "flow_failed")
            .count(),
        1,
        "EOF recovery must write one durable canonical failure"
    );
    assert!(
        seen.lock()
            .expect("mapper observation lock should be available")
            .iter()
            .any(|(status, event_type)| status == "Failed" && event_type == "flow_failed"),
        "compatible mapper must receive the reloaded durable failed winner"
    );
}

#[test]
fn live_answer_chunks_claim_the_same_coalesced_durable_delta_once() {
    let run = native_run();
    let node_run_id = Uuid::from_u128(0x55555555555555555555555555555555);
    let mut first = RuntimeEventEnvelope::new(
        run.id,
        1,
        debug_stream_events::answer_text_delta(
            "node-answer",
            "最终".to_string(),
            0,
            Some("node-llm"),
            Some(node_run_id),
            Some("text"),
        ),
    );
    let mut second = RuntimeEventEnvelope::new(
        run.id,
        2,
        debug_stream_events::answer_text_delta(
            "node-answer",
            "回答".to_string(),
            0,
            Some("node-llm"),
            Some(node_run_id),
            Some("text"),
        ),
    );
    let mut durable = RuntimeEventEnvelope::new(
        run.id,
        3,
        debug_stream_events::answer_text_delta(
            "node-answer",
            "最终回答".to_string(),
            0,
            Some("node-llm"),
            Some(node_run_id),
            Some("text"),
        ),
    );
    durable.payload["event_ids"] = json!([format!("{}:1", run.id), format!("{}:2", run.id)]);
    let mut stats = CompatibleStreamStats::default();

    assert!(stats.claim_runtime_event(&mut first));
    assert!(stats.claim_runtime_event(&mut second));
    assert!(!stats.claim_runtime_event(&mut durable));
}

#[test]
fn live_answer_chunks_claim_multiple_coalesced_durable_deltas_once() {
    let run = native_run();
    let node_run_id = Uuid::from_u128(0x55555555555555555555555555555555);
    let chunks = [
        "现在三层",
        "的现状清楚了。关键发现：",
        "\n- 页面层已接 DesignHoverFrame，需换",
        "蓝。\n- 区块层：#66e",
        "0ad / #00c875，",
        "统一重构。",
    ];
    let mut live = chunks
        .iter()
        .enumerate()
        .map(|(index, text)| {
            RuntimeEventEnvelope::new(
                run.id,
                index as i64 + 1,
                debug_stream_events::answer_text_delta(
                    "node-answer",
                    (*text).to_string(),
                    0,
                    Some("node-llm"),
                    Some(node_run_id),
                    Some("text"),
                ),
            )
        })
        .collect::<Vec<_>>();
    let mut stats = CompatibleStreamStats::default();

    for event in &mut live {
        assert!(stats.claim_runtime_event(event));
    }

    let durable_batches = [
        (vec![0], chunks[0].to_string()),
        (vec![1, 2], format!("{}{}", chunks[1], chunks[2])),
        (vec![3], chunks[3].to_string()),
        (vec![4, 5], format!("{}{}", chunks[4], chunks[5])),
    ];
    let replayed = durable_batches
        .into_iter()
        .enumerate()
        .filter_map(|(index, (source_indexes, text))| {
            let mut durable = RuntimeEventEnvelope::new(
                run.id,
                index as i64 + 100,
                debug_stream_events::answer_text_delta(
                    "node-answer",
                    text,
                    0,
                    Some("node-llm"),
                    Some(node_run_id),
                    Some("text"),
                ),
            );
            durable.payload["event_ids"] = json!(source_indexes
                .into_iter()
                .map(|source_index| live[source_index].event_id.clone())
                .collect::<Vec<_>>());
            stats
                .claim_runtime_event(&mut durable)
                .then_some(durable.text.unwrap_or_default())
        })
        .collect::<String>();

    assert!(replayed.is_empty(), "durable replay leaked: {replayed}");
}

#[test]
fn live_answer_prefix_claims_only_missing_suffix_across_durable_batches() {
    let run = native_run();
    let node_run_id = Uuid::from_u128(0x55555555555555555555555555555555);
    let chunks = ["实时", "前缀", "补齐", "完成"];
    let mut source_events = chunks
        .iter()
        .enumerate()
        .map(|(index, text)| {
            RuntimeEventEnvelope::new(
                run.id,
                index as i64 + 1,
                debug_stream_events::answer_text_delta(
                    "node-answer",
                    (*text).to_string(),
                    0,
                    Some("node-llm"),
                    Some(node_run_id),
                    Some("text"),
                ),
            )
        })
        .collect::<Vec<_>>();
    let mut stats = CompatibleStreamStats::default();
    assert!(stats.claim_runtime_event(&mut source_events[0]));
    assert!(stats.claim_runtime_event(&mut source_events[1]));

    let durable_batches = [
        (vec![0], chunks[0].to_string()),
        (vec![1, 2], format!("{}{}", chunks[1], chunks[2])),
        (vec![3], chunks[3].to_string()),
    ];
    let reconciled = durable_batches
        .into_iter()
        .enumerate()
        .filter_map(|(index, (source_indexes, text))| {
            let mut durable = RuntimeEventEnvelope::new(
                run.id,
                index as i64 + 100,
                debug_stream_events::answer_text_delta(
                    "node-answer",
                    text,
                    0,
                    Some("node-llm"),
                    Some(node_run_id),
                    Some("text"),
                ),
            );
            durable.payload["event_ids"] = json!(source_indexes
                .into_iter()
                .map(|source_index| source_events[source_index].event_id.clone())
                .collect::<Vec<_>>());
            stats
                .claim_runtime_event(&mut durable)
                .then_some(durable.text.unwrap_or_default())
        })
        .collect::<String>();

    assert_eq!(reconciled, "补齐完成");
}

#[test]
fn live_answer_chunks_are_not_treated_as_cumulative_snapshots() {
    let run = native_run();
    let node_run_id = Uuid::from_u128(0x55555555555555555555555555555555);
    let mut first = RuntimeEventEnvelope::new(
        run.id,
        1,
        debug_stream_events::answer_text_delta(
            "node-answer",
            "a".to_string(),
            0,
            Some("node-llm"),
            Some(node_run_id),
            Some("text"),
        ),
    );
    let mut second = RuntimeEventEnvelope::new(
        run.id,
        2,
        debug_stream_events::answer_text_delta(
            "node-answer",
            "ab".to_string(),
            0,
            Some("node-llm"),
            Some(node_run_id),
            Some("text"),
        ),
    );
    let mut stats = CompatibleStreamStats::default();

    assert!(stats.claim_runtime_event(&mut first));
    assert!(stats.claim_runtime_event(&mut second));
    assert_eq!(second.text.as_deref(), Some("ab"));
}

#[tokio::test]
async fn d1_ac_007_native_sse_initial_and_durable_replay_keep_incomplete_distinct_from_completed() {
    let (base_state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let mut run = native_run();
    run.status = NativeRunStatus::Incomplete;
    run.answer = Some("partial output at the limit".to_string());

    let initial_stream = native_sse_body_from_replay(
        &base_state,
        run.clone(),
        vec![
            RuntimeEventEnvelope::new(run.id, 1, debug_stream_events::flow_started(run.id)),
            RuntimeEventEnvelope::new(
                run.id,
                2,
                debug_stream_events::flow_incomplete(
                    run.id,
                    json!({ "answer": "partial output at the limit" }),
                ),
            ),
        ],
        None,
    )
    .await;
    let replay_stream = native_sse_body_from_replay(
        &base_state,
        run.clone(),
        vec![RuntimeEventEnvelope::new(
            run.id,
            2,
            debug_stream_events::flow_incomplete(
                run.id,
                json!({ "answer": "partial output at the limit" }),
            ),
        )],
        Some(1),
    )
    .await;

    for body in [initial_stream, replay_stream] {
        assert_eq!(body.matches("event: run.incomplete").count(), 1, "{body}");
        assert!(body.contains("\"status\":\"incomplete\""), "{body}");
        assert!(body.contains("partial output at the limit"), "{body}");
        assert!(!body.contains("event: run.completed"), "{body}");
        assert!(!body.contains("\"status\":\"succeeded\""), "{body}");
    }
}

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
        test_database: base_state.test_database.clone(),
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
    let response = completed_compatible_stream(events);
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
                RuntimeEventEnvelope::new(run.id, 2, answer_delta.clone()),
                RuntimeEventEnvelope::new(run.id, 3, answer_delta),
                RuntimeEventEnvelope::new(
                    run.id,
                    4,
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
        test_database: base_state.test_database.clone(),
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

    send_compatible_runtime_event_stream(
        state,
        run,
        ANTHROPIC_SSE_PROJECTION,
        Some(0),
        None,
        sender,
        move |run, envelope| mapper.runtime_event_to_sse(run, envelope),
    )
    .await;

    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    let response = completed_compatible_stream(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert_eq!(body.matches("answer exactly once").count(), 1, "{body}");
}

#[test]
fn openai_delta_chunk_maps_reasoning_to_reasoning_content() {
    let chat_completion_id = "chatcmpl-test";
    let payload = openai_delta_chunk_payload(
        &native_run(),
        "deepseek-v4-pro",
        chat_completion_id,
        "reasoning_delta",
        "先分析用户问题".to_string(),
    )
    .expect("reasoning delta should map to an OpenAI-compatible chunk");

    assert_eq!(payload["id"], json!(chat_completion_id));
    assert_eq!(
        payload["choices"][0]["delta"]["reasoning_content"],
        json!("先分析用户问题")
    );
    assert_eq!(payload["choices"][0]["delta"].get("content"), None);
}

#[tokio::test]
async fn openai_terminal_fallback_projects_structured_answer_segments() {
    let mut run = native_run();
    run.answer = Some("<think>旧思考</think>旧回答".to_string());
    run.answer_segments = Some(vec![
        AnswerProjectionSegment::reasoning("结构化思考"),
        AnswerProjectionSegment::message("结构化回答"),
    ]);
    let mut mapper = OpenAiChatStreamMapper::new(
        "deepseek-v4-pro".to_string(),
        "chatcmpl-test".to_string(),
        true,
    );

    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_finished(run.id, json!({})),
        ),
    );
    let response = completed_compatible_stream(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(
        body.contains("\"reasoning_content\":\"结构化思考\""),
        "{body}"
    );
    assert!(body.contains("\"content\":\"结构化回答\""), "{body}");
    assert!(!body.contains("旧思考"), "{body}");
    assert!(!body.contains("旧回答"), "{body}");
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
    let response = completed_compatible_stream(anthropic_completed_run_to_sse(&run, "claude"));
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
async fn anthropic_completed_stream_uses_structured_answer_segments_for_thinking_and_text() {
    let mut run = native_run();
    run.status = NativeRunStatus::Succeeded;
    run.answer = Some("<think>旧思考</think>旧回答".to_string());
    run.answer_segments = Some(vec![
        AnswerProjectionSegment::reasoning("结构化思考"),
        AnswerProjectionSegment::message("结构化回答"),
    ]);

    let response = completed_compatible_stream(anthropic_completed_run_to_sse(&run, "claude"));
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

    let response = completed_compatible_stream([reasoning_events, text_events].concat());
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
async fn anthropic_terminal_answer_fallback_emits_text_before_stop() {
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

    let response = completed_compatible_stream(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("\"type\":\"text_delta\""), "{body}");
    assert!(body.contains("\"text\":\"最终回答\""), "{body}");
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

    let response = completed_compatible_stream(events);
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
async fn d2_ac_008_anthropic_incomplete_terminal_uses_max_tokens_and_message_stop() {
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

    let response = completed_compatible_stream(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("output limit partial"), "{body}");
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

    let response = completed_compatible_stream(events);
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

    let response = completed_compatible_stream(events);
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

#[test]
fn openai_waiting_callback_maps_to_tool_call_chunk() {
    let callback_task_id = Uuid::from_u128(0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
    let chat_completion_id = "chatcmpl-tool-test";
    let payload = openai_tool_call_chunk_payload(
        &native_run(),
        "1flowbase",
        chat_completion_id,
        &json!({
            "callback_kind": "llm_tool_calls",
            "callback_task_id": callback_task_id,
            "tool_calls": [
                {
                    "id": "call_weather",
                    "name": "lookup_weather",
                    "arguments": {"city": "Hangzhou"}
                }
            ]
        }),
    )
    .expect("LLM callback should map to OpenAI tool call chunk");

    assert_eq!(payload["id"], json!(chat_completion_id));
    assert_eq!(
        payload["choices"][0]["delta"]["tool_calls"][0]["function"]["name"],
        json!("lookup_weather")
    );
    assert_eq!(
        payload["choices"][0]["delta"]["tool_calls"][0]["function"]["arguments"],
        json!("{\"city\":\"Hangzhou\"}")
    );
    let call_id = payload["choices"][0]["delta"]["tool_calls"][0]["id"]
        .as_str()
        .expect("tool call id should be encoded");
    assert!(call_id.contains("call_weather"));
}

#[test]
fn openai_chat_completion_id_changes_for_callback_resume() {
    let run_id = Uuid::from_u128(0x11111111111111111111111111111111);
    let callback_task_id = Uuid::from_u128(0x22222222222222222222222222222222);

    assert_ne!(
        openai_chat_completion_id_from_run_id(run_id),
        openai_chat_completion_id_from_callback_task(run_id, callback_task_id)
    );
}

#[test]
fn openai_responses_waiting_callback_maps_to_function_call_item() {
    let callback_task_id = Uuid::from_u128(0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);
    let output = openai_response_function_call_output_items(&json!({
        "callback_kind": "llm_tool_calls",
        "callback_task_id": callback_task_id,
        "tool_calls": [
            {
                "id": "call_inventory",
                "name": "lookup_inventory",
                "arguments": {"sku": "sku_123"}
            }
        ]
    }))
    .expect("LLM callback should map to Responses function_call output");

    assert_eq!(output[0]["type"], json!("function_call"));
    assert_eq!(output[0]["name"], json!("lookup_inventory"));
    assert_eq!(output[0]["arguments"], json!("{\"sku\":\"sku_123\"}"));
    assert!(output[0]["call_id"]
        .as_str()
        .expect("call id should be encoded")
        .contains("call_inventory"));
}

#[test]
fn openai_finish_chunk_uses_deepseek_compatible_terminal_shape() {
    let payload = openai_finish_chunk_payload(&native_run(), "1flowbase", "chatcmpl-test", "stop");

    assert_eq!(payload["choices"][0]["delta"]["content"], json!(""));
    assert_eq!(payload["choices"][0]["delta"]["role"], Value::Null);
    assert_eq!(payload["choices"][0]["finish_reason"], json!("stop"));
    assert_eq!(payload["usage"]["prompt_tokens"], json!(0));
    assert_eq!(payload["usage"]["completion_tokens"], json!(0));
    assert_eq!(payload["usage"]["total_tokens"], json!(0));
}

#[test]
fn openai_chat_resume_terminal_answer_fallback_emits_content_before_finish() {
    let run = native_run();
    let mut mapper =
        OpenAiChatStreamMapper::new("1flowbase".to_string(), "chatcmpl-test".to_string(), true);
    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_finished(run.id, json!({ "answer": "最终回答" })),
        ),
    );

    assert_eq!(events.len(), 3);
}

#[tokio::test]
async fn openai_chat_resume_terminal_answer_fallback_projects_thinking_delta() {
    let run = native_run();
    let mut mapper =
        OpenAiChatStreamMapper::new("1flowbase".to_string(), "chatcmpl-test".to_string(), true);

    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_finished(
                run.id,
                json!({ "answer": "<think>先分析</think>\n最终回答" }),
            ),
        ),
    );

    let response = completed_compatible_stream(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("\"reasoning_content\":\"先分析\""), "{body}");
    assert!(body.contains("\"content\":\"\\n最终回答\""), "{body}");
    assert!(!body.contains("<think>"), "{body}");
    assert!(body.contains("[DONE]"), "{body}");
}

#[tokio::test]
async fn openai_responses_resume_terminal_answer_fallback_projects_thinking_delta() {
    let run = native_run();
    let mut mapper = OpenAiResponseStreamMapper::new("1flowbase".to_string(), None, true);
    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_finished(
                run.id,
                json!({ "answer": "<think>先分析</think>\n最终回答" }),
            ),
        ),
    );

    let response = completed_compatible_stream(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(
        body.contains("event: response.reasoning_text.delta"),
        "{body}"
    );
    assert!(body.contains("\"delta\":\"先分析\""), "{body}");
    assert!(body.contains("event: response.output_text.delta"), "{body}");
    assert!(body.contains("\"delta\":\"\\n最终回答\""), "{body}");
    assert!(!body.contains("<think>"), "{body}");
    assert!(body.contains("event: response.completed"), "{body}");
}

#[tokio::test]
async fn openai_response_completed_event_includes_usage() {
    let mut run = native_run();
    run.usage = Some(NativeUsage {
        prompt_tokens: Some(11),
        completion_tokens: Some(7),
        total_tokens: Some(18),
        ..Default::default()
    });
    let mut mapper = OpenAiResponseStreamMapper::new("1flowbase".to_string(), None, true);
    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_finished(run.id, json!({ "answer": "Final answer" })),
        ),
    );

    let response = completed_compatible_stream(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("event: response.completed"), "{body}");
    assert!(body.contains("\"usage\""), "{body}");
    assert!(body.contains("\"input_tokens\":11"), "{body}");
    assert!(body.contains("\"output_tokens\":7"), "{body}");
    assert!(body.contains("\"total_tokens\":18"), "{body}");
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

    let response = completed_compatible_stream(anthropic_completed_run_to_sse(&run, "1flowbase"));
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

#[tokio::test]
async fn openai_chat_terminal_answer_fallback_decodes_artifact_preview_answer() {
    let run = native_run();
    let mut mapper =
        OpenAiChatStreamMapper::new("1flowbase".to_string(), "chatcmpl-test".to_string(), true);
    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_finished(
                run.id,
                json!({
                    "answer": {
                        "__runtime_debug_artifact": true,
                        "artifact_ref": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                        "is_truncated": false,
                        "preview": "\"最终回答\""
                    }
                }),
            ),
        ),
    );

    let response = completed_compatible_stream(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("\"content\":\"最终回答\""), "{body}");
    assert!(body.contains("\"finish_reason\":\"stop\""), "{body}");
    assert!(body.contains("[DONE]"), "{body}");
}

#[test]
fn openai_chat_terminal_answer_fallback_ignores_provider_raw_delta() {
    let run = native_run();
    let mut mapper =
        OpenAiChatStreamMapper::new("1flowbase".to_string(), "chatcmpl-test".to_string(), true);
    let text_events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::text_delta("node-llm", run.id, "已流式输出".to_string()),
        ),
    );
    let terminal_events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            2,
            debug_stream_events::flow_finished(run.id, json!({ "answer": "最终回答" })),
        ),
    );

    assert!(text_events.is_empty());
    assert_eq!(terminal_events.len(), 3);
}

#[test]
fn compatible_stream_stats_count_answer_content_bytes_once_for_terminal_fallback() {
    let run = native_run();
    let mut stats = CompatibleStreamStats::default();
    let answer_delta = RuntimeEventEnvelope::new(
        run.id,
        1,
        debug_stream_events::answer_text_delta(
            "node-answer",
            "已输出".to_string(),
            0,
            Some("node-llm"),
            None,
            Some("text"),
        ),
    );
    stats.record_sent_runtime_event(&run, &answer_delta, true);

    let terminal_event = RuntimeEventEnvelope::new(
        run.id,
        2,
        debug_stream_events::flow_finished(run.id, json!({ "answer": "最终回答" })),
    );
    stats.record_sent_runtime_event(&run, &terminal_event, true);

    assert!(stats.emitted_content());
    assert_eq!(stats.emitted_content_bytes, "已输出".len());
}

#[test]
fn openai_chat_projects_answer_presentation_delta_not_provider_raw_delta() {
    let run = native_run();
    let mut mapper =
        OpenAiChatStreamMapper::new("1flowbase".to_string(), "chatcmpl-test".to_string(), true);
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
            RuntimeEventPayload {
                event_type: "text_delta".to_string(),
                source: RuntimeEventSource::Runtime,
                durability: RuntimeEventDurability::DurableRequired,
                persist_required: true,
                trace_visible: false,
                payload: json!({
                    "type": "text_delta",
                    "node_run_id": Uuid::from_u128(0x66666666666666666666666666666666),
                    "node_id": "node-answer",
                    "text": "answer presentation",
                    "presentation": { "kind": "answer" }
                }),
            },
        ),
    );

    assert!(provider_events.is_empty());
    assert_eq!(presentation_events.len(), 1);
}
