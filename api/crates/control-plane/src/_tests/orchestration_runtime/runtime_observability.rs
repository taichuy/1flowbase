use control_plane::orchestration_runtime::{
    ContinueFlowDebugRunCommand, OrchestrationRuntimeService, StartFlowDebugRunCommand,
};
use control_plane::runtime_observability::{
    coalesce_provider_stream_events, item_kind_for_event, provider_stream_event_type,
};
use observability::{RuntimeBusEvent, RuntimeEventBus};
use plugin_framework::provider_contract::{ProviderMcpCall, ProviderStreamEvent, ProviderToolCall};
use time::OffsetDateTime;
use uuid::Uuid;

#[test]
fn runtime_event_folds_to_debug_stream_part_with_trust_level() {
    let event = domain::RuntimeEventRecord {
        id: Uuid::now_v7(),
        flow_run_id: Uuid::now_v7(),
        node_run_id: None,
        span_id: None,
        parent_span_id: None,
        sequence: 1,
        event_type: "text_delta".into(),
        layer: domain::RuntimeEventLayer::RuntimeItem,
        source: domain::RuntimeEventSource::Host,
        trust_level: domain::RuntimeTrustLevel::HostFact,
        item_id: None,
        ledger_ref: None,
        payload: serde_json::json!({ "delta": "hello" }),
        visibility: domain::RuntimeEventVisibility::Workspace,
        durability: domain::RuntimeEventDurability::Durable,
        created_at: OffsetDateTime::now_utc(),
    };

    let part = control_plane::runtime_observability::debug_read_model::fold_event_to_debug_part(
        event.flow_run_id,
        &event,
    )
    .unwrap();

    assert_eq!(part.part_type, "text");
    assert_eq!(part.trust_level, domain::RuntimeTrustLevel::HostFact);
    assert_eq!(part.payload["payload"]["delta"], serde_json::json!("hello"));
}

#[test]
fn debug_read_model_maps_cancelled_terminal_event_to_status_part() {
    let run_id = Uuid::now_v7();
    let event = domain::RuntimeEventRecord {
        id: Uuid::now_v7(),
        flow_run_id: run_id,
        node_run_id: None,
        span_id: None,
        parent_span_id: None,
        sequence: 1,
        event_type: "flow_cancelled".to_string(),
        layer: domain::RuntimeEventLayer::AgentTransition,
        source: domain::RuntimeEventSource::Host,
        trust_level: domain::RuntimeTrustLevel::HostFact,
        item_id: None,
        ledger_ref: None,
        payload: serde_json::json!({ "status": "cancelled", "reason": "manual_stop" }),
        visibility: domain::RuntimeEventVisibility::Workspace,
        durability: domain::RuntimeEventDurability::Durable,
        created_at: OffsetDateTime::now_utc(),
    };

    let part = control_plane::runtime_observability::debug_read_model::fold_event_to_debug_part(
        run_id, &event,
    )
    .expect("flow_cancelled should fold to a debug part");

    assert_eq!(part.part_type, "status");
    assert_eq!(part.payload["event_type"], "flow_cancelled");
}

#[test]
fn debug_read_model_excludes_provider_raw_by_default() {
    let run_id = Uuid::now_v7();
    let event = domain::RuntimeEventRecord {
        id: Uuid::now_v7(),
        flow_run_id: run_id,
        node_run_id: None,
        span_id: None,
        parent_span_id: None,
        sequence: 1,
        event_type: "text_delta".to_string(),
        layer: domain::RuntimeEventLayer::ProviderRaw,
        source: domain::RuntimeEventSource::Host,
        trust_level: domain::RuntimeTrustLevel::HostFact,
        item_id: None,
        ledger_ref: None,
        payload: serde_json::json!({ "delta": "raw" }),
        visibility: domain::RuntimeEventVisibility::Workspace,
        durability: domain::RuntimeEventDurability::Durable,
        created_at: OffsetDateTime::now_utc(),
    };

    let part = control_plane::runtime_observability::debug_read_model::fold_event_to_debug_part(
        run_id, &event,
    );

    assert!(part.is_none());
}

#[tokio::test]
async fn external_opaque_boundary_marks_external_agent_event_as_durable_workspace_fact() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Support Agent").await;
    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: serde_json::json!({
                "node-start": { "query": "请总结退款政策" }
            }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();

    let event = service
        .mark_external_opaque_boundary(
            started.flow_run.id,
            serde_json::json!({ "reason": "external local tool execution not observed" }),
        )
        .await;
    let events = service.list_runtime_events(started.flow_run.id, 0).await;

    assert_eq!(event.event_type, "external_agent_opaque_boundary_marked");
    assert_eq!(event.layer, domain::RuntimeEventLayer::Diagnostic);
    assert_eq!(event.source, domain::RuntimeEventSource::ExternalAgent);
    assert_eq!(event.trust_level, domain::RuntimeTrustLevel::ExternalOpaque);
    assert_eq!(event.visibility, domain::RuntimeEventVisibility::Workspace);
    assert_eq!(event.durability, domain::RuntimeEventDurability::Durable);
    assert_eq!(
        event.payload["reason"],
        "external local tool execution not observed"
    );
    assert!(events.iter().any(|record| record.id == event.id));
}

#[tokio::test]
async fn flow_debug_run_shadow_writes_runtime_spans_and_provider_events() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Support Agent").await;

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: serde_json::json!({
                "node-start": { "query": "请总结退款政策" }
            }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    let detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    let spans = service.list_runtime_spans(detail.flow_run.id).await;
    let events = service.list_runtime_events(detail.flow_run.id, 0).await;

    assert!(spans
        .iter()
        .any(|span| span.kind == domain::RuntimeSpanKind::Flow));
    assert!(spans
        .iter()
        .any(|span| span.kind == domain::RuntimeSpanKind::LlmTurn));
    assert!(events.iter().any(|event| event.event_type == "text_delta"));
    assert!(events
        .iter()
        .any(|event| event.layer == domain::RuntimeEventLayer::ProviderRaw));
}

#[tokio::test]
async fn provider_events_returned_by_runtime_are_persisted_before_debug_run_finishes() {
    let service = OrchestrationRuntimeService::for_tests_with_provider_events(vec![
        ProviderStreamEvent::TextDelta {
            delta: "hello ".into(),
        },
        ProviderStreamEvent::TextDelta {
            delta: "world".into(),
        },
        ProviderStreamEvent::UsageSnapshot {
            usage: plugin_framework::provider_contract::ProviderUsage {
                input_tokens: Some(2),
                output_tokens: Some(3),
                total_tokens: Some(5),
                ..Default::default()
            },
        },
        ProviderStreamEvent::Finish {
            reason: plugin_framework::provider_contract::ProviderFinishReason::Stop,
        },
    ]);
    let seeded = service.seed_application_with_flow("Support Agent").await;

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: serde_json::json!({
                "node-start": { "query": "请总结退款政策" }
            }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    let detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    let run_events = service.list_run_events(detail.flow_run.id);
    let runtime_events = service.list_runtime_events(detail.flow_run.id, 0).await;
    let provider_run_event_types: Vec<_> = run_events
        .iter()
        .filter(|event| {
            ["text_delta", "usage_snapshot", "finish"].contains(&event.event_type.as_str())
        })
        .map(|event| event.event_type.as_str())
        .collect();
    let provider_runtime_event_types: Vec<_> = runtime_events
        .iter()
        .filter(|event| event.layer == domain::RuntimeEventLayer::ProviderRaw)
        .map(|event| event.event_type.as_str())
        .collect();

    assert_eq!(provider_runtime_event_types, provider_run_event_types);
    assert_eq!(detail.flow_run.status, domain::FlowRunStatus::Succeeded);
    assert!(detail
        .node_runs
        .iter()
        .all(|node_run| node_run.status == domain::NodeRunStatus::Succeeded));
    assert_eq!(
        provider_run_event_types,
        vec![
            provider_stream_event_type(&ProviderStreamEvent::TextDelta {
                delta: String::new()
            }),
            provider_stream_event_type(&ProviderStreamEvent::UsageSnapshot {
                usage: plugin_framework::provider_contract::ProviderUsage {
                    input_tokens: None,
                    output_tokens: None,
                    total_tokens: None,
                    ..Default::default()
                }
            }),
            provider_stream_event_type(&ProviderStreamEvent::Finish {
                reason: plugin_framework::provider_contract::ProviderFinishReason::Stop,
            }),
        ]
    );
    assert_eq!(provider_runtime_event_types, provider_run_event_types);
    assert!(run_events.iter().any(|event| {
        event.event_type == "text_delta" && event.payload["delta"] == "hello world"
    }));
}

#[tokio::test]
async fn live_debug_persists_llm_debug_payload_without_polluting_public_outputs() {
    let service = OrchestrationRuntimeService::for_tests_with_provider_events(vec![
        ProviderStreamEvent::ToolCallCommit {
            call: ProviderToolCall {
                id: "call-1".into(),
                name: "lookup_order".into(),
                arguments: serde_json::json!({ "order_id": "A-1" }),
                provider_metadata: serde_json::json!({}),
            },
        },
        ProviderStreamEvent::McpCallCommit {
            call: ProviderMcpCall {
                id: "mcp-1".into(),
                server: "orders".into(),
                method: "lookup".into(),
                arguments: serde_json::json!({ "order_id": "A-1" }),
            },
        },
        ProviderStreamEvent::UsageSnapshot {
            usage: plugin_framework::provider_contract::ProviderUsage {
                input_tokens: Some(2),
                output_tokens: Some(3),
                total_tokens: Some(5),
                ..Default::default()
            },
        },
        ProviderStreamEvent::Finish {
            reason: plugin_framework::provider_contract::ProviderFinishReason::Stop,
        },
    ]);
    let seeded = service.seed_application_with_flow("Support Agent").await;

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: serde_json::json!({
                "node-start": { "query": "请查询订单" }
            }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    let detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    let llm_node = detail
        .node_runs
        .iter()
        .find(|node_run| node_run.node_id == "node-llm")
        .expect("llm node run should be persisted");

    assert_eq!(
        llm_node.output_payload["usage"],
        llm_node.metrics_payload["usage"]
    );
    assert_eq!(llm_node.output_payload.get("route"), None);
    assert!(llm_node.output_payload.get("provider_route").is_some());
    assert!(llm_node.output_payload.get("finish_reason").is_some());
    assert_eq!(llm_node.output_payload.get("provider_events"), None);
    assert_eq!(llm_node.output_payload.get("tool_calls"), None);
    assert!(llm_node.output_payload.get("text").is_some());
    assert_eq!(
        llm_node.metrics_payload["usage"]["total_tokens"],
        serde_json::json!(5)
    );
    assert_eq!(
        llm_node.debug_payload["assistant_message"]["content"],
        "echo:gpt-5.4-mini:请查询订单"
    );
    assert!(llm_node.debug_payload["provider_events"]
        .as_array()
        .is_some_and(|events| events.len() >= 4));

    assert_eq!(
        detail.flow_run.output_payload["answer"],
        serde_json::json!("echo:gpt-5.4-mini:请查询订单")
    );
    assert_eq!(
        detail.flow_run.output_payload["sys"]["workflow_run_id"],
        serde_json::json!(detail.flow_run.id.to_string())
    );
    assert_eq!(detail.flow_run.output_payload["env"], serde_json::json!({}));
    for forbidden_key in [
        "node-start",
        "node-llm",
        "debug_payload",
        "provider_events",
        "tool_calls",
        "mcp_calls",
    ] {
        assert_eq!(detail.flow_run.output_payload.get(forbidden_key), None);
    }
}

#[tokio::test]
async fn provider_tool_commit_is_recorded_as_intent_not_execution() {
    let service = OrchestrationRuntimeService::for_tests_with_provider_events(vec![
        ProviderStreamEvent::ToolCallCommit {
            call: ProviderToolCall {
                id: "call-1".into(),
                name: "lookup_order".into(),
                arguments: serde_json::json!({ "order_id": "A-1" }),
                provider_metadata: serde_json::json!({}),
            },
        },
        ProviderStreamEvent::McpCallCommit {
            call: ProviderMcpCall {
                id: "mcp-1".into(),
                server: "orders".into(),
                method: "lookup".into(),
                arguments: serde_json::json!({ "order_id": "A-1" }),
            },
        },
    ]);
    let seeded = service.seed_application_with_flow("Support Agent").await;

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: serde_json::json!({
                "node-start": { "query": "请查询订单" }
            }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    let detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    let events = service.list_runtime_events(detail.flow_run.id, 0).await;
    let intents = events
        .iter()
        .filter(|event| {
            event.event_type == "capability_call_requested"
                && event.layer == domain::RuntimeEventLayer::Capability
        })
        .collect::<Vec<_>>();

    assert_eq!(intents.len(), 2);
    assert!(intents.iter().all(|event| {
        event.payload["provider_only_intent"] == serde_json::json!(true)
            && event.payload["requested_by"] == "model"
            && event.payload["call"]["arguments"]["order_id"] == "A-1"
    }));
    assert!(intents
        .iter()
        .any(|event| { event.payload["capability_id"] == "host_tool:model:lookup_order@runtime" }));
    assert!(intents
        .iter()
        .any(|event| { event.payload["capability_id"] == "mcp_tool:mcp:orders:lookup@runtime" }));
    assert!(!events.iter().any(|event| {
        event.layer == domain::RuntimeEventLayer::Capability
            && matches!(
                event.event_type.as_str(),
                "capability_call_executed" | "capability_call_completed"
            )
    }));
}

#[test]
fn provider_stream_events_are_coalesced_and_published_to_bus() {
    let bus = RuntimeEventBus::new(16);
    let mut receiver = bus.subscribe();

    let events = coalesce_provider_stream_events(
        &bus,
        &[
            ProviderStreamEvent::TextDelta {
                delta: "hel".into(),
            },
            ProviderStreamEvent::TextDelta { delta: "lo".into() },
            ProviderStreamEvent::ReasoningDelta {
                delta: "think".into(),
            },
            ProviderStreamEvent::Finish {
                reason: plugin_framework::provider_contract::ProviderFinishReason::Stop,
            },
        ],
        32,
    )
    .unwrap();

    assert_eq!(
        events[0],
        ProviderStreamEvent::TextDelta {
            delta: "hello".into()
        }
    );
    assert_eq!(
        events[1],
        ProviderStreamEvent::ReasoningDelta {
            delta: "think".into()
        }
    );
    assert_eq!(
        receiver.try_recv().unwrap(),
        RuntimeBusEvent::TextDelta {
            delta: "hello".into()
        }
    );
    assert_eq!(
        receiver.try_recv().unwrap(),
        RuntimeBusEvent::ReasoningDelta {
            delta: "think".into()
        }
    );
}

#[tokio::test]
async fn provider_text_deltas_are_coalesced_before_durable_write() {
    let service = OrchestrationRuntimeService::for_tests_with_provider_events(vec![
        ProviderStreamEvent::TextDelta {
            delta: "hel".into(),
        },
        ProviderStreamEvent::TextDelta { delta: "lo".into() },
        ProviderStreamEvent::UsageSnapshot {
            usage: plugin_framework::provider_contract::ProviderUsage {
                input_tokens: Some(1),
                output_tokens: Some(1),
                total_tokens: Some(2),
                ..Default::default()
            },
        },
        ProviderStreamEvent::Finish {
            reason: plugin_framework::provider_contract::ProviderFinishReason::Stop,
        },
    ]);
    let seeded = service.seed_application_with_flow("Support Agent").await;

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: serde_json::json!({
                "node-start": { "query": "请总结退款政策" }
            }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    let detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    let events = service.list_runtime_events(detail.flow_run.id, 0).await;
    let text_deltas = events
        .iter()
        .filter(|event| {
            event.event_type == "text_delta"
                && event.layer == domain::RuntimeEventLayer::ProviderRaw
        })
        .collect::<Vec<_>>();

    assert_eq!(text_deltas.len(), 1);
    assert_eq!(text_deltas[0].payload["delta"], "hello");
}

#[tokio::test]
async fn llm_turn_records_context_projection_and_usage_ledger() {
    let service = OrchestrationRuntimeService::for_tests_with_provider_events(vec![
        ProviderStreamEvent::TextDelta {
            delta: "hello".into(),
        },
        ProviderStreamEvent::UsageSnapshot {
            usage: plugin_framework::provider_contract::ProviderUsage {
                input_tokens: Some(9),
                cache_read_tokens: Some(3),
                output_tokens: Some(2),
                reasoning_tokens: Some(1),
                total_tokens: Some(12),
                ..Default::default()
            },
        },
        ProviderStreamEvent::Finish {
            reason: plugin_framework::provider_contract::ProviderFinishReason::Stop,
        },
    ]);
    let seeded = service.seed_application_with_flow("Support Agent").await;

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: serde_json::json!({
                "node-start": { "query": "请总结退款政策" }
            }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    let detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    let projections = service.list_context_projections(detail.flow_run.id).await;
    let usage = service.list_usage_ledger(detail.flow_run.id).await;

    assert_eq!(projections.len(), 1);
    assert_eq!(projections[0].projection_kind, "managed_full");
    assert!(projections[0].model_input_hash.starts_with("sha256:"));
    assert_eq!(
        projections[0].model_input_ref,
        format!(
            "runtime_artifact:inline:{}",
            projections[0].model_input_hash
        )
    );
    assert_eq!(usage.len(), 1);
    assert_eq!(usage[0].input_tokens, Some(9));
    assert_eq!(usage[0].cache_read_tokens, Some(3));
    assert_eq!(usage[0].usage_status, domain::UsageLedgerStatus::Recorded);
}

#[tokio::test]
async fn llm_turn_records_failover_attempt_and_links_usage_ledger() {
    let service = OrchestrationRuntimeService::for_tests_with_provider_events(vec![
        ProviderStreamEvent::TextDelta {
            delta: "hello".into(),
        },
        ProviderStreamEvent::UsageSnapshot {
            usage: plugin_framework::provider_contract::ProviderUsage {
                input_tokens: Some(9),
                output_tokens: Some(2),
                total_tokens: Some(11),
                ..Default::default()
            },
        },
        ProviderStreamEvent::Finish {
            reason: plugin_framework::provider_contract::ProviderFinishReason::Stop,
        },
    ]);
    let seeded = service.seed_application_with_flow("Support Agent").await;

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: serde_json::json!({
                "node-start": { "query": "请总结退款政策" }
            }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    let detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    let attempts = service
        .list_model_failover_attempt_ledger(detail.flow_run.id)
        .await;
    let usage = service.list_usage_ledger(detail.flow_run.id).await;

    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].attempt_index, 0);
    assert_eq!(attempts[0].status, "succeeded");
    assert_eq!(attempts[0].provider_code, "fixture_provider");
    assert_eq!(attempts[0].upstream_model_id, "gpt-5.4-mini");
    assert_eq!(usage.len(), 1);
    assert_eq!(usage[0].failover_attempt_id, Some(attempts[0].id));
    assert_eq!(attempts[0].usage_ledger_id, Some(usage[0].id));
}

#[tokio::test]
async fn llm_turn_records_first_output_token_from_reasoning_delta() {
    let service = OrchestrationRuntimeService::for_tests_with_provider_events(vec![
        ProviderStreamEvent::ReasoningDelta {
            delta: "先思考".into(),
        },
        ProviderStreamEvent::TextDelta {
            delta: "再回答".into(),
        },
        ProviderStreamEvent::Finish {
            reason: plugin_framework::provider_contract::ProviderFinishReason::Stop,
        },
    ]);
    let seeded = service.seed_application_with_flow("Support Agent").await;

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: serde_json::json!({
                "node-start": { "query": "请总结退款政策" }
            }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    let detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    let llm_node = detail
        .node_runs
        .iter()
        .find(|node_run| node_run.node_id == "node-llm")
        .expect("llm node run should exist");
    let attempts = service
        .list_model_failover_attempt_ledger(detail.flow_run.id)
        .await;

    assert_eq!(attempts.len(), 1);
    assert!(attempts[0].first_token_at.is_some());
    assert!(llm_node.metrics_payload["first_token_at"].is_string());
    assert!(llm_node.metrics_payload["time_to_first_token_ms"].is_number());
    assert!(llm_node.metrics_payload["attempts"][0]["first_token_at"].is_string());
    assert!(llm_node.metrics_payload["attempts"][0]["time_to_first_token_ms"].is_number());
}

#[tokio::test]
async fn failover_queue_records_each_attempt_and_links_usage_to_winner() {
    let service =
        OrchestrationRuntimeService::for_tests_with_fail_before_token_models(vec!["gpt-5.4-mini"]);
    let seeded = service.seed_application_with_flow("Support Agent").await;
    let primary_instance_id = service.default_provider_instance_id();
    let backup_instance_id = service.seed_provider_instance(
        "fixture_provider",
        "Fixture Backup",
        true,
        domain::ModelProviderInstanceStatus::Ready,
        vec!["backup-model"],
    );

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: serde_json::json!({
                "node-start": { "query": "请总结退款政策" }
            }),
            document_snapshot: Some(failover_queue_document(
                seeded.flow_id,
                primary_instance_id,
                backup_instance_id,
            )),
            debug_session_id: None,
        })
        .await
        .unwrap();
    let detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    let attempts = service
        .list_model_failover_attempt_ledger(detail.flow_run.id)
        .await;
    let usage = service.list_usage_ledger(detail.flow_run.id).await;

    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].attempt_index, 0);
    assert_eq!(attempts[0].status, "failed");
    assert_eq!(attempts[0].provider_instance_id, Some(primary_instance_id));
    assert_eq!(attempts[0].upstream_model_id, "gpt-5.4-mini");
    assert_eq!(attempts[0].usage_ledger_id, None);
    assert_eq!(attempts[1].attempt_index, 1);
    assert_eq!(attempts[1].status, "succeeded");
    assert_eq!(attempts[1].provider_instance_id, Some(backup_instance_id));
    assert_eq!(attempts[1].upstream_model_id, "backup-model");
    assert_eq!(usage.len(), 1);
    assert_eq!(usage[0].failover_attempt_id, Some(attempts[1].id));
    assert_eq!(attempts[1].usage_ledger_id, Some(usage[0].id));
}

#[tokio::test]
async fn all_failed_failover_attempts_do_not_receive_usage_ledger_link() {
    let service = OrchestrationRuntimeService::for_tests_with_fail_before_token_models(vec![
        "gpt-5.4-mini",
        "backup-model",
    ]);
    let seeded = service.seed_application_with_flow("Support Agent").await;
    let primary_instance_id = service.default_provider_instance_id();
    let backup_instance_id = service.seed_provider_instance(
        "fixture_provider",
        "Fixture Backup",
        true,
        domain::ModelProviderInstanceStatus::Ready,
        vec!["backup-model"],
    );

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: serde_json::json!({
                "node-start": { "query": "请总结退款政策" }
            }),
            document_snapshot: Some(failover_queue_document(
                seeded.flow_id,
                primary_instance_id,
                backup_instance_id,
            )),
            debug_session_id: None,
        })
        .await
        .unwrap();
    let detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    let attempts = service
        .list_model_failover_attempt_ledger(detail.flow_run.id)
        .await;
    let usage = service.list_usage_ledger(detail.flow_run.id).await;

    assert_eq!(attempts.len(), 2);
    assert!(attempts.iter().all(|attempt| attempt.status == "failed"));
    assert!(attempts
        .iter()
        .all(|attempt| attempt.usage_ledger_id.is_none()));
    assert_eq!(usage.len(), 1);
    assert_eq!(usage[0].failover_attempt_id, None);
    assert_eq!(
        usage[0].usage_status,
        domain::UsageLedgerStatus::UnavailableError
    );
}

fn failover_queue_document(
    flow_id: Uuid,
    primary_instance_id: Uuid,
    backup_instance_id: Uuid,
) -> serde_json::Value {
    serde_json::json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": { "flowId": flow_id.to_string(), "name": "Support Agent", "description": "", "tags": [] },
        "graph": {
            "nodes": [
                {
                    "id": "node-start",
                    "type": "start",
                    "alias": "Start",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 0, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {},
                    "outputs": []
                },
                {
                    "id": "node-llm",
                    "type": "llm",
                    "alias": "LLM",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 240, "y": 0 },
                    "configVersion": 1,
                    "config": {
                        "retry_enabled": true,
                        "max_retries": 1,
                        "model_provider": {
                            "routing_mode": "failover_queue",
                            "queue_template_id": "queue-template-1",
                            "queue_snapshot_id": "queue-snapshot-1",
                            "queue_targets": [
                                {
                                    "provider_instance_id": primary_instance_id.to_string(),
                                    "provider_code": "fixture_provider",
                                    "protocol": "openai_compatible",
                                    "upstream_model_id": "gpt-5.4-mini"
                                },
                                {
                                    "provider_instance_id": backup_instance_id.to_string(),
                                    "provider_code": "fixture_provider",
                                    "protocol": "openai_compatible",
                                    "upstream_model_id": "backup-model"
                                }
                            ]
                        }
                    },
                    "bindings": {
                        "prompt_messages": { "kind": "prompt_messages", "value": [{ "id": "user-1", "role": "user", "content": { "kind": "templated_text", "value": "{{node-start.query}}" } }] }
                    },
                    "outputs": [{ "key": "text", "title": "模型输出", "valueType": "string" }]
                }
            ],
            "edges": [
                { "id": "edge-start-llm", "source": "node-start", "target": "node-llm", "sourceHandle": null, "targetHandle": null, "containerId": null, "points": [] }
            ]
        },
        "editor": { "viewport": { "x": 0, "y": 0, "zoom": 1 }, "annotations": [], "activeContainerPath": [] }
    })
}

#[tokio::test]
async fn provider_events_fold_into_runtime_items() {
    let service = OrchestrationRuntimeService::for_tests_with_provider_events(vec![
        ProviderStreamEvent::TextDelta {
            delta: "hello".into(),
        },
        ProviderStreamEvent::Finish {
            reason: plugin_framework::provider_contract::ProviderFinishReason::Stop,
        },
    ]);
    let seeded = service.seed_application_with_flow("Support Agent").await;

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: serde_json::json!({
                "node-start": { "query": "请总结退款政策" }
            }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    let detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    let items = service.list_runtime_items(detail.flow_run.id).await;

    assert!(items
        .iter()
        .any(|item| item.kind == domain::RuntimeItemKind::Message));
    assert!(items
        .iter()
        .any(|item| item.trust_level == domain::RuntimeTrustLevel::HostFact));
    assert_eq!(
        item_kind_for_event("capability_call_requested"),
        None,
        "capability request events must not be folded into runtime items"
    );
}

#[tokio::test]
async fn tool_call_commit_creates_capability_invocation_request() {
    let service = OrchestrationRuntimeService::for_tests_with_provider_events(vec![
        ProviderStreamEvent::ToolCallCommit {
            call: ProviderToolCall {
                id: "call-1".into(),
                name: "lookup_order".into(),
                arguments: serde_json::json!({ "order_id": "A-1" }),
                provider_metadata: serde_json::json!({}),
            },
        },
    ]);
    let seeded = service.seed_application_with_flow("Support Agent").await;

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: serde_json::json!({
                "node-start": { "query": "请查询订单" }
            }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    let detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    let invocations = service
        .list_capability_invocations(detail.flow_run.id)
        .await;

    assert_eq!(invocations.len(), 1);
    assert_eq!(
        invocations[0].capability_id,
        "host_tool:model:lookup_order@runtime"
    );
    assert_eq!(invocations[0].authorization_status, "requested");
    assert_eq!(invocations[0].requester_kind, "model");
    assert!(invocations[0]
        .arguments_ref
        .as_deref()
        .is_some_and(|value| value.starts_with("runtime_artifact:inline:")));
}

#[test]
fn capability_ids_are_canonical_across_sources() {
    assert_eq!(
        control_plane::capability_runtime::host_tool_capability_id("search"),
        "host_tool:model:search@runtime"
    );
    assert_eq!(
        control_plane::capability_runtime::mcp_tool_capability_id("github", "create_issue"),
        "mcp_tool:mcp:github:create_issue@runtime"
    );
    assert_eq!(
        control_plane::capability_runtime::skill_action_capability_id(
            "builtin", "coding", "review", "1"
        ),
        "skill_action:builtin:coding:review@1"
    );
    assert_eq!(
        control_plane::capability_runtime::workflow_tool_capability_id("app-1", "flow-1", "3"),
        "workflow_tool:app-1:flow-1@3"
    );
    assert_eq!(
        control_plane::capability_runtime::approval_capability_id("policy-1", "2"),
        "approval:policy:policy-1@2"
    );
    assert_eq!(
        control_plane::capability_runtime::subagent_capability_id("builtin", "reviewer", "1"),
        "system_agent:builtin:reviewer@1"
    );
}
