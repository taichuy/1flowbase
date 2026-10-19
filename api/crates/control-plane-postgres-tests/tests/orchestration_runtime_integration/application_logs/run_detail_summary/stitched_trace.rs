use super::*;

#[tokio::test]
async fn application_run_detail_stitches_prior_conversation_tool_trace() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let conversation_id = "conversation-stitch-fixture";
    let external_user = "claude-code-user-fixture";
    let prior_started_at = datetime!(2026-05-24 09:00:00 UTC);
    let current_started_at = datetime!(2026-05-24 09:00:10 UTC);
    let prior_run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        prior_started_at,
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    let prior_node = seed_node_run_for(
        &store,
        &prior_run,
        "node-llm",
        "llm",
        "LLM",
        json!({ "prompt": "route image" }),
        prior_started_at + Duration::seconds(1),
    )
    .await;
    let callback_task =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_callback_task(
            &store,
            &CreateCallbackTaskInput {
                flow_run_id: prior_run.id,
                node_run_id: prior_node.id,
                callback_kind: "llm_tool_calls".to_string(),
                request_payload: json!({
                    "tool_calls": [
                        { "id": "call_image", "name": "image_llm" }
                    ]
                }),
                external_ref_payload: None,
            },
        )
        .await
        .unwrap();
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::complete_callback_task(
        &store,
        &control_plane_contracts::ports::CompleteCallbackTaskInput {
            callback_task_id: callback_task.id,
            response_payload: json!({
                "tool_results": [
                    {
                        "tool_call_id": "call_image",
                        "name": "image_llm",
                        "content": "{\"answer\":\"route ok\"}",
                        "execution": { "status": "succeeded" }
                    }
                ]
            }),
            completed_at: prior_started_at + Duration::seconds(4),
        },
    )
    .await
    .unwrap();
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_node_run(
        &store,
        &UpdateNodeRunInput {
            node_run_id: prior_node.id,
            status: NodeRunStatus::Succeeded,
            output_payload: json!({ "usage": { "total_tokens": 33520 } }),
            error_payload: None,
            metrics_payload: json!({}),
            debug_payload: json!({
                "llm_rounds": [
                    {
                        "round_index": 0,
                        "assistant": {
                            "role": "assistant",
                            "content": "need image route",
                            "tool_calls": [
                                { "id": "call_image", "name": "image_llm" }
                            ]
                        }
                    },
                    {
                        "round_index": 1,
                        "tool_results": [
                            {
                                "tool_call_id": "call_image",
                                "name": "image_llm",
                                "content": "{\"answer\":\"route ok\"}"
                            }
                        ]
                    },
                    {
                        "round_index": 2,
                        "assistant": {
                            "role": "assistant",
                            "content": "main resumed"
                        }
                    }
                ]
            }),
            finished_at: Some(prior_started_at + Duration::seconds(5)),
        },
    )
    .await
    .unwrap();
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_runtime_events(
        &store,
        &[
            AppendRuntimeEventInput {
                flow_run_id: prior_run.id,
                node_run_id: Some(prior_node.id),
                span_id: None,
                parent_span_id: None,
                event_type: "visible_internal_llm_tool_waiting_callback".into(),
                layer: domain::RuntimeEventLayer::AgentTransition,
                source: domain::RuntimeEventSource::Host,
                trust_level: domain::RuntimeTrustLevel::HostFact,
                item_id: None,
                ledger_ref: None,
                payload: json!({
                    "tool_call_id": "call_image",
                    "tool_name": "image_llm",
                    "main_node_id": "node-llm",
                    "target_node_id": "node-llm-image",
                    "node_id": "node-llm-image",
                    "node_alias": "Image LLM",
                    "arguments": { "prompt": "route image" }
                }),
                visibility: domain::RuntimeEventVisibility::Workspace,
                durability: domain::RuntimeEventDurability::Durable,
            },
            AppendRuntimeEventInput {
                flow_run_id: prior_run.id,
                node_run_id: Some(prior_node.id),
                span_id: None,
                parent_span_id: None,
                event_type: "visible_internal_llm_tool_completed".into(),
                layer: domain::RuntimeEventLayer::AgentTransition,
                source: domain::RuntimeEventSource::Host,
                trust_level: domain::RuntimeTrustLevel::HostFact,
                item_id: None,
                ledger_ref: None,
                payload: json!({
                    "tool_call_id": "call_image",
                    "tool_name": "image_llm",
                    "main_node_id": "node-llm",
                    "target_node_id": "node-llm-image",
                    "node_id": "node-llm-image",
                    "node_alias": "Image LLM",
                    "provider_route": { "model": "image-route-v1" }
                }),
                visibility: domain::RuntimeEventVisibility::Workspace,
                durability: domain::RuntimeEventDurability::Durable,
            },
        ],
    )
    .await
    .unwrap();
    sqlx::query(
        r#"
        update flow_runs
        set external_user = $2,
            external_conversation_id = $3,
            compatibility_mode = 'anthropic-messages-v1'
        where id = $1
        "#,
    )
    .bind(prior_run.id)
    .bind(external_user)
    .bind(conversation_id)
    .execute(store.pool())
    .await
    .unwrap();
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: prior_run.id,
            status: FlowRunStatus::Cancelled,
            output_payload: json!({}),
            error_payload: None,
            finished_at: Some(prior_started_at + Duration::seconds(6)),
        },
    )
    .await
    .unwrap();

    let other_user_run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        prior_started_at + Duration::seconds(7),
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    sqlx::query(
        r#"
        update flow_runs
        set external_user = 'other-claude-code-user',
            external_conversation_id = $2,
            compatibility_mode = 'anthropic-messages-v1'
        where id = $1
        "#,
    )
    .bind(other_user_run.id)
    .bind(conversation_id)
    .execute(store.pool())
    .await
    .unwrap();
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: other_user_run.id,
            status: FlowRunStatus::Cancelled,
            output_payload: json!({}),
            error_payload: None,
            finished_at: Some(prior_started_at + Duration::seconds(8)),
        },
    )
    .await
    .unwrap();

    let current_run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        current_started_at,
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    sqlx::query(
        r#"
        update flow_runs
        set external_user = $2,
            external_conversation_id = $3,
            compatibility_mode = 'anthropic-messages-v1'
        where id = $1
        "#,
    )
    .bind(current_run.id)
    .bind(external_user)
    .bind(conversation_id)
    .execute(store.pool())
    .await
    .unwrap();
    let current_node = seed_node_run_for(
        &store,
        &current_run,
        "node-llm",
        "llm",
        "LLM",
        json!({ "prompt": "final answer" }),
        current_started_at + Duration::seconds(1),
    )
    .await;
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_node_run(
        &store,
        &UpdateNodeRunInput {
            node_run_id: current_node.id,
            status: NodeRunStatus::Succeeded,
            output_payload: json!({ "answer": "final" }),
            error_payload: None,
            metrics_payload: json!({}),
            debug_payload: json!({}),
            finished_at: Some(current_started_at + Duration::seconds(2)),
        },
    )
    .await
    .unwrap();
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: current_run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({ "answer": "final" }),
            error_payload: None,
            finished_at: Some(current_started_at + Duration::seconds(3)),
        },
    )
    .await
    .unwrap();

    let detail =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_detail(
            &store,
            seeded.application_id,
            current_run.id,
        )
        .await
        .unwrap()
        .unwrap();

    assert!(detail.callback_tasks.is_empty());
    assert_eq!(detail.stitched_trace.len(), 1);
    let stitched_trace = &detail.stitched_trace[0];
    assert_eq!(stitched_trace.source_flow_run.id, prior_run.id);
    assert_eq!(stitched_trace.node_runs[0].id, prior_node.id);
    assert_eq!(stitched_trace.callback_tasks[0].id, callback_task.id);
    assert!(stitched_trace
        .runtime_events
        .iter()
        .any(|event| event.event_type == "visible_internal_llm_tool_completed"));

    let logs =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_application_run_logs_page(
            &store,
            seeded.application_id,
            ListApplicationRunsPageInput {
                page: 1,
                page_size: 20,
                created_after: None,
                sort_by: Some("created_at".to_string()),
                sort_order: Some("desc".to_string()),
            },
        )
        .await
        .unwrap();
    let current_summary = logs
        .items
        .iter()
        .find(|summary| summary.run.id == current_run.id)
        .expect("current run summary should exist");
    assert_eq!(current_summary.tool_callback_count, 0);
}
