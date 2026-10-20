use super::*;

#[tokio::test]
async fn live_provider_reasoning_signature_delta_is_transient_in_runtime_event_stream() {
    let service = OrchestrationRuntimeService::for_tests_with_provider_events(vec![
        plugin_framework::provider_contract::ProviderStreamEvent::ReasoningDelta {
            delta: "先分析".into(),
        },
        plugin_framework::provider_contract::ProviderStreamEvent::ReasoningSignatureDelta {
            signature: "opaque-signature-fixture".into(),
        },
        plugin_framework::provider_contract::ProviderStreamEvent::TextDelta {
            delta: "结果".into(),
        },
    ]);
    let seeded = service.seed_application_with_flow("Support Agent").await;
    let stream =
        std::sync::Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service = service.with_runtime_event_stream(stream.clone());

    let detail = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: serde_json::json!({ "node-start": { "query": "hello" } }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();

    let detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: detail.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    let llm_node = detail
        .node_runs
        .iter()
        .find(|node_run| node_run.node_id == "node-llm")
        .expect("llm node run should be persisted");
    assert_eq!(llm_node.output_payload["text"], "<think>先分析</think>结果");
    assert!(llm_node.output_payload.get("reasoning_content").is_none());
    assert!(llm_node.output_payload.get("attempts").is_none());
    assert!(llm_node.output_payload.get("event_count").is_none());
    assert!(llm_node.output_payload.get("provider_code").is_none());
    assert_eq!(
        llm_node.output_payload["provider_route"]["provider_code"],
        "fixture_provider"
    );
    assert!(llm_node.debug_payload.get("reasoning_content").is_none());
    assert!(llm_node.debug_payload.get("provider_route").is_none());

    let events = stream.events();
    assert!(events
        .iter()
        .any(|event| event.event_type == "reasoning_delta" && event.payload["text"] == "先分析"));
    let signature_event = events
        .iter()
        .find(|event| event.event_type == "reasoning_signature_delta")
        .expect("reasoning signature should reach the live runtime stream");
    assert_eq!(
        signature_event.payload["signature"],
        "opaque-signature-fixture"
    );
    assert_eq!(
        signature_event.durability,
        RuntimeEventDurability::Ephemeral
    );
    assert!(!signature_event.persist_required);
    assert!(!signature_event.trace_visible);
    assert!(events.iter().any(|event| event.event_type == "text_delta"));
}

#[tokio::test]
async fn live_provider_text_delta_with_think_tags_is_split_into_reasoning_and_answer() {
    let service = OrchestrationRuntimeService::for_tests_with_provider_events(vec![
        plugin_framework::provider_contract::ProviderStreamEvent::TextDelta {
            delta: "<think>先分析".into(),
        },
        plugin_framework::provider_contract::ProviderStreamEvent::TextDelta {
            delta: "用户问题</think>正式回答".into(),
        },
    ]);
    let seeded = service.seed_application_with_flow("Support Agent").await;
    let stream =
        std::sync::Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service = service.with_runtime_event_stream(stream.clone());

    let detail = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: serde_json::json!({ "node-start": { "query": "hello" } }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();

    service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: detail.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    let events = stream.events();
    let reasoning_text = events
        .iter()
        .filter(|event| event.event_type == "reasoning_delta")
        .filter(|event| event.payload["presentation"]["kind"].as_str() == Some("answer"))
        .filter_map(|event| event.payload["text"].as_str())
        .collect::<String>();
    let answer_text = events
        .iter()
        .filter(|event| event.event_type == "text_delta")
        .filter(|event| event.payload["presentation"]["kind"].as_str() == Some("answer"))
        .filter_map(|event| event.payload["text"].as_str())
        .collect::<String>();

    assert_eq!(reasoning_text, "先分析用户问题");
    assert_eq!(answer_text, "正式回答");
    assert!(!events.iter().any(|event| {
        event
            .payload
            .get("text")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|text| text.contains("<think>") || text.contains("</think>"))
    }));
}

#[tokio::test]
async fn fast_stream_provider_events_are_durably_persisted_to_runtime_observability() {
    use plugin_framework::provider_contract::{
        ProviderFinishReason, ProviderStreamEvent, ProviderToolCall, ProviderUsage,
    };

    let service = OrchestrationRuntimeService::for_tests_with_provider_events(vec![
        ProviderStreamEvent::TextDelta {
            delta: "hello".to_string(),
        },
        ProviderStreamEvent::ToolCallCommit {
            call: ProviderToolCall {
                id: "call-1".to_string(),
                name: "lookup_policy".to_string(),
                arguments: json!({ "query": "refund" }),
                provider_metadata: json!({}),
            },
        },
        ProviderStreamEvent::UsageSnapshot {
            usage: ProviderUsage {
                input_tokens: Some(10),
                output_tokens: Some(5),
                reasoning_tokens: None,
                input_cache_hit_tokens: None,
                input_cache_miss_tokens: None,
                cache_read_tokens: None,
                cache_write_by_ttl_seconds: None,
                cache_write_tokens: None,
                total_tokens: Some(15),
            },
        },
        ProviderStreamEvent::Finish {
            reason: ProviderFinishReason::Stop,
        },
    ]);
    let seeded = service.seed_application_with_flow("Support Agent").await;
    let stream =
        std::sync::Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service = service.with_runtime_event_stream(stream.clone());

    let detail = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: serde_json::json!({ "node-start": { "query": "hello" } }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();

    let completed_detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: detail.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    assert_eq!(completed_detail.flow_run.output_payload["answer"], "hello");
    let live_events = stream.events();
    assert!(
        live_events
            .iter()
            .any(|event| event.event_type == "text_delta" && event.payload["text"] == "hello"),
        "actual provider content must remain observable on the live stream"
    );

    let context_index = live_events
        .iter()
        .position(|event| event.event_type == "context_snapshot")
        .expect("AI Gateway should project canonical context before Provider invocation");
    let usage_index = live_events
        .iter()
        .position(|event| event.event_type == "usage_snapshot")
        .expect("Provider usage should remain available for billing observability");
    assert!(context_index < usage_index);
    let context_snapshot = &live_events[context_index];
    assert_eq!(context_snapshot.source, RuntimeEventSource::Runtime);
    assert!(context_snapshot.payload["input_tokens"].as_u64().unwrap() > 0);
    assert_eq!(
        context_snapshot.payload["effective_context_window"],
        json!(128_000)
    );
    assert_eq!(
        context_snapshot.payload["measurement"]["method"],
        json!("generic_estimate")
    );
    assert_eq!(
        context_snapshot.payload["measurement"]["accuracy"],
        json!("estimated")
    );

    let live_usage = live_events
        .iter()
        .find(|event| event.event_type == "usage_snapshot")
        .expect("typed provider usage should be projected to the live runtime stream");
    assert_eq!(live_usage.payload["usage"]["input_tokens"], json!(10));
    assert_eq!(live_usage.payload["usage"]["total_tokens"], json!(15));

    let runtime_event_types = service
        .list_runtime_events(detail.flow_run.id, 0)
        .await
        .into_iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    assert!(
        !runtime_event_types.iter().any(|event_type| matches!(
            event_type.as_str(),
            "text_delta" | "reasoning_delta" | "tool_call_delta" | "mcp_call_delta"
        )),
        "provider fragments must not become durable runtime_events: {runtime_event_types:?}"
    );
    assert!(
        runtime_event_types
            .iter()
            .any(|event_type| event_type == "tool_call_commit"),
        "provider tool commits should still be written to durable runtime_events: {runtime_event_types:?}"
    );
    assert!(
        runtime_event_types
            .iter()
            .any(|event_type| event_type == "usage_snapshot"),
        "provider usage snapshots should still be written to durable runtime_events: {runtime_event_types:?}"
    );
    assert!(
        runtime_event_types
            .iter()
            .any(|event_type| event_type == "context_snapshot"),
        "AI Gateway context snapshots should be recoverable from durable runtime_events: {runtime_event_types:?}"
    );
    assert!(
        runtime_event_types
            .iter()
            .any(|event_type| event_type == "finish"),
        "provider finish events should still be written to durable runtime_events: {runtime_event_types:?}"
    );

    assert!(
        runtime_event_types
            .iter()
            .any(|event_type| event_type == "provider_semantic_step"),
        "complete native invocation facts must survive fragment removal"
    );
    assert!(runtime_event_types
        .iter()
        .any(|event_type| event_type == "flow_finished"));
    let capability_invocations = service
        .list_capability_invocations(detail.flow_run.id)
        .await;
    assert!(
        capability_invocations
            .iter()
            .any(|invocation| invocation.capability_id.contains("lookup_policy")),
        "provider tool commits should still create capability intent records: {capability_invocations:?}"
    );
}

#[tokio::test]
async fn provider_error_after_live_delta_drains_runtime_event_stream_forwarding() {
    let service = OrchestrationRuntimeService::for_tests_with_live_events_then_error(vec![
        plugin_framework::provider_contract::ProviderStreamEvent::TextDelta {
            delta: "partial before error".to_string(),
        },
    ]);
    let seeded = service.seed_application_with_flow("Support Agent").await;
    let stream =
        std::sync::Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service = service.with_runtime_event_stream(stream.clone());

    let detail = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: serde_json::json!({ "node-start": { "query": "hello" } }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();

    let failed_detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: detail.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    assert_eq!(failed_detail.flow_run.status, domain::FlowRunStatus::Failed);
    let error_payload = failed_detail
        .flow_run
        .error_payload
        .as_ref()
        .expect("provider failure must retain its canonical error");
    assert_eq!(
        error_payload["error_code"],
        json!("provider_invalid_response")
    );
    assert_eq!(
        error_payload["message"],
        json!("provider failed after live events"),
        "the durable error must keep the provider runtime message"
    );
    let event_types = stream
        .events()
        .into_iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    let text_delta_index = event_types
        .iter()
        .position(|event_type| event_type == "text_delta")
        .expect("text_delta should be appended before provider error returns");
    let flow_failed_index = event_types
        .iter()
        .position(|event_type| event_type == "flow_failed")
        .expect("failed run should append flow_failed");
    assert!(
        text_delta_index < flow_failed_index,
        "text_delta should be drained before flow_failed: {event_types:?}"
    );
}

#[tokio::test]
async fn provider_error_after_live_delta_does_not_project_failure_as_answer() {
    let service = OrchestrationRuntimeService::for_tests_with_live_events_then_error(vec![
        plugin_framework::provider_contract::ProviderStreamEvent::TextDelta {
            delta: "partial before error".to_string(),
        },
    ]);
    let seeded = service.seed_application_with_flow("Support Agent").await;
    let stream =
        std::sync::Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service = service.with_runtime_event_stream(stream.clone());

    let detail = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: serde_json::json!({ "node-start": { "query": "hello" } }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();

    let failed_detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: detail.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    assert_eq!(failed_detail.flow_run.status, domain::FlowRunStatus::Failed);
    assert!(
        failed_detail
            .flow_run
            .output_payload
            .get("answer")
            .is_none(),
        "provider failure text must not become an Answer"
    );
    let llm_node = node_run(&failed_detail, "node-llm");
    assert_eq!(llm_node.status, domain::NodeRunStatus::Failed);
    assert!(llm_node.output_payload.get("text").is_none());
    assert!(llm_node.output_payload.get("usage").is_none());
    assert!(llm_node.output_payload.get("tool_calls").is_none());
    assert_eq!(
        llm_node.error_payload.as_ref().unwrap()["error_code"],
        json!("provider_invalid_response")
    );
    assert!(failed_detail
        .node_runs
        .iter()
        .all(|node_run| node_run.node_id != "node-answer"));
    let durable_events = service
        .list_runtime_events(failed_detail.flow_run.id, 0)
        .await;
    assert_eq!(
        durable_events
            .iter()
            .filter(|event| event.event_type == "flow_failed")
            .count(),
        1
    );
    assert!(durable_events
        .iter()
        .all(|event| event.event_type != "flow_finished"));
    let live_events = stream.events();
    let presented_answer = live_events
        .iter()
        .filter(|event| event.event_type == "text_delta")
        .filter(|event| event.payload["presentation"]["kind"] == json!("answer"))
        .filter_map(|event| event.payload["text"].as_str())
        .collect::<String>();
    assert_eq!(presented_answer, "partial before error");
}

#[tokio::test]
async fn successful_live_debug_run_emits_flow_lifecycle_and_closes_runtime_stream() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Support Agent").await;
    let stream =
        std::sync::Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service = service.with_runtime_event_stream(stream.clone());

    let detail = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: serde_json::json!({ "node-start": { "query": "hello" } }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();

    service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: detail.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    let event_types = stream
        .events()
        .into_iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    assert!(event_types
        .iter()
        .any(|event_type| event_type == "flow_started"));
    assert!(event_types
        .iter()
        .any(|event_type| event_type == "flow_finished"));
    let durable_event_types = service
        .list_runtime_events(detail.flow_run.id, 0)
        .await
        .into_iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    assert!(
        durable_event_types
            .iter()
            .any(|event_type| event_type == "flow_started"),
        "flow lifecycle should be durable: {durable_event_types:?}"
    );
    assert!(
        durable_event_types
            .iter()
            .any(|event_type| event_type == "flow_finished"),
        "flow lifecycle should be durable: {durable_event_types:?}"
    );
    let node_finished_events = stream
        .events()
        .into_iter()
        .filter(|event| event.event_type == "node_finished")
        .collect::<Vec<_>>();
    assert!(!node_finished_events.is_empty());
    for event in node_finished_events {
        assert!(
            event.payload.get("debug_payload").is_none(),
            "runtime stream must not expose persisted debug payload"
        );
        assert_no_pending_debug_ref(&event.payload);
    }
    assert_eq!(
        stream.close_calls(),
        vec![(
            detail.flow_run.id,
            crate::ports::RuntimeEventCloseReason::Finished
        )]
    );
}
