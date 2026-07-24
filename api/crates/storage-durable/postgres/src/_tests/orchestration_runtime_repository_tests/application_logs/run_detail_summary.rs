use super::*;

#[tokio::test]
async fn application_run_log_list_uses_summary_projection_without_raw_payload() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-06-20 09:00:00 UTC);
    let run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started_at,
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({ "answer": "ok" }),
            error_payload: None,
            finished_at: Some(started_at + Duration::seconds(2)),
        },
    )
    .await
    .unwrap();

    sqlx::query(
        r#"
        update application_run_log_summaries
        set title = '',
            input_payload = '{"query":"raw summary payload must not affect list title"}'::jsonb
        where flow_run_id = $1
        "#,
    )
    .bind(run.id)
    .execute(store.pool())
    .await
    .unwrap();

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

    assert_eq!(logs.total, 1);
    assert_eq!(logs.items[0].run.id, run.id);
    assert_eq!(logs.items[0].run.title, "Untitled run");
}

#[tokio::test]
async fn application_run_detail_returns_raw_payload_only_for_matching_application_scope() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let other_seeded = seed_runtime_base_with_workspace_name(&store, "Other Runtime").await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-06-20 10:00:00 UTC);
    let run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started_at,
        FlowRunMode::DebugFlowRun,
        None,
    )
    .await;
    let node = seed_node_run_for(
        &store,
        &run,
        "node-raw",
        "llm",
        "Raw Node",
        json!({
            "prompt_messages": [
                { "role": "user", "content": "full raw node input" }
            ],
            "raw_marker": "node-input"
        }),
        started_at + Duration::seconds(1),
    )
    .await;

    sqlx::query(
        r#"
        update flow_runs
        set input_payload = $2,
            output_payload = $3,
            error_payload = $4
        where id = $1
        "#,
    )
    .bind(run.id)
    .bind(json!({
        "query": "full raw flow input",
        "history": [
            { "role": "user", "content": "long history item" }
        ],
        "raw_marker": "flow-input"
    }))
    .bind(json!({
        "answer": "full raw flow output",
        "raw_marker": "flow-output"
    }))
    .bind(Some(json!({
        "code": "RAW_FLOW_ERROR",
        "raw_marker": "flow-error"
    })))
    .execute(store.pool())
    .await
    .unwrap();

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_node_run(
        &store,
        &UpdateNodeRunInput {
            node_run_id: node.id,
            status: NodeRunStatus::Failed,
            output_payload: json!({
                "text": "full raw node output",
                "raw_marker": "node-output"
            }),
            error_payload: Some(json!({
                "code": "RAW_NODE_ERROR",
                "raw_marker": "node-error"
            })),
            metrics_payload: json!({ "usage": { "total_tokens": 9 } }),
            debug_payload: json!({
                "trace": [
                    { "event": "provider_call", "raw_marker": "node-debug" }
                ]
            }),
            finished_at: Some(started_at + Duration::seconds(3)),
        },
    )
    .await
    .unwrap();

    let detail =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_detail(
            &store,
            seeded.application_id,
            run.id,
        )
        .await
        .unwrap()
        .expect("matching application scope should read run detail");

    assert_eq!(
        detail.flow_run.input_payload["raw_marker"],
        json!("flow-input")
    );
    assert_eq!(
        detail.flow_run.output_payload["raw_marker"],
        json!("flow-output")
    );
    assert_eq!(
        detail.flow_run.error_payload.as_ref().unwrap()["raw_marker"],
        json!("flow-error")
    );
    assert_eq!(detail.node_runs.len(), 1);
    assert_eq!(
        detail.node_runs[0].input_payload["raw_marker"],
        json!("node-input")
    );
    assert_eq!(
        detail.node_runs[0].output_payload["raw_marker"],
        json!("node-output")
    );
    assert_eq!(
        detail.node_runs[0].error_payload.as_ref().unwrap()["raw_marker"],
        json!("node-error")
    );
    assert_eq!(
        detail.node_runs[0].debug_payload["trace"][0]["raw_marker"],
        json!("node-debug")
    );

    let cross_application_detail =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_detail(
            &store,
            other_seeded.application_id,
            run.id,
        )
        .await
        .unwrap();
    assert!(cross_application_detail.is_none());
}

#[tokio::test]
async fn terminal_flow_run_writes_static_application_run_log_summary() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-05-24 09:00:00 UTC);
    let run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started_at,
        FlowRunMode::DebugFlowRun,
        None,
    )
    .await;
    let first_node = seed_node_run_for(
        &store,
        &run,
        "node-llm",
        "llm",
        "LLM",
        json!({ "prompt": "总结退款政策" }),
        started_at + Duration::seconds(1),
    )
    .await;
    let _second_node = seed_node_run_for(
        &store,
        &run,
        "node-tool",
        "tool",
        "Tool",
        json!({ "tool_name": "lookup_order" }),
        started_at + Duration::seconds(2),
    )
    .await;

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_node_run(
        &store,
        &UpdateNodeRunInput {
            node_run_id: first_node.id,
            status: NodeRunStatus::Succeeded,
            output_payload: json!({ "answer": "ok" }),
            error_payload: None,
            metrics_payload: json!({
                "usage": {
                    "input_tokens": 13,
                    "output_tokens": 9,
                    "cache_read_tokens": 250
                }
            }),
            debug_payload: json!({}),
            finished_at: Some(started_at + Duration::seconds(3)),
        },
    )
    .await
    .unwrap();
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_callback_task(
        &store,
        &CreateCallbackTaskInput {
            flow_run_id: run.id,
            node_run_id: first_node.id,
            callback_kind: "llm_tool_calls".to_string(),
            request_payload: json!({
                "tool_calls": [
                    { "id": "call-1" },
                    { "id": "call-2" }
                ]
            }),
            external_ref_payload: None,
        },
    )
    .await
    .unwrap();

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({ "answer": "完成" }),
            error_payload: None,
            finished_at: Some(started_at + Duration::seconds(5)),
        },
    )
    .await
    .unwrap();

    sqlx::query(
        r#"
        update node_runs
        set metrics_payload = '{"usage":{"total_tokens":999}}'::jsonb
        where id = $1
        "#,
    )
    .bind(first_node.id)
    .execute(store.pool())
    .await
    .unwrap();

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

    assert_eq!(logs.total, 1);
    assert_eq!(logs.items[0].run.id, run.id);
    assert_eq!(logs.items[0].total_tokens, Some(22));
    assert_eq!(logs.items[0].input_tokens, Some(13));
    assert_eq!(logs.items[0].output_tokens, Some(9));
    assert_eq!(logs.items[0].input_cache_hit_tokens, Some(250));
    let cache_hit_rate = logs.items[0].input_cache_hit_rate.unwrap();
    assert!((cache_hit_rate - (250.0 / 263.0)).abs() < f64::EPSILON);
    assert_eq!(logs.items[0].unique_node_count, 2);
    assert_eq!(logs.items[0].tool_callback_count, 2);
}

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
        &control_plane::ports::CompleteCallbackTaskInput {
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

#[tokio::test]
async fn application_run_detail_hides_failed_imported_stitched_sources_and_boundaries() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let conversation_id = "conversation-import-visibility-fixture";
    let external_user = "claude-code-import-user-fixture";
    let visible_started_at = datetime!(2026-06-24 09:00:00 UTC);
    let hidden_boundary_started_at = datetime!(2026-06-24 09:00:02 UTC);
    let hidden_prior_started_at = datetime!(2026-06-24 09:00:04 UTC);
    let current_started_at = datetime!(2026-06-24 09:00:06 UTC);

    let visible_prior = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        visible_started_at,
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    set_run_external_context(&store, visible_prior.id, external_user, conversation_id).await;
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: visible_prior.id,
            status: FlowRunStatus::Cancelled,
            output_payload: json!({}),
            error_payload: None,
            finished_at: Some(visible_started_at + Duration::seconds(1)),
        },
    )
    .await
    .unwrap();

    let hidden_boundary = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        hidden_boundary_started_at,
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    set_run_external_context(&store, hidden_boundary.id, external_user, conversation_id).await;
    attach_import_job_to_run(&store, &seeded, hidden_boundary.id, "failed").await;
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: hidden_boundary.id,
            status: FlowRunStatus::Failed,
            output_payload: json!({}),
            error_payload: Some(json!({ "message": "hidden imported boundary" })),
            finished_at: Some(hidden_boundary_started_at + Duration::seconds(1)),
        },
    )
    .await
    .unwrap();

    let hidden_prior = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        hidden_prior_started_at,
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    set_run_external_context(&store, hidden_prior.id, external_user, conversation_id).await;
    attach_import_job_to_run(&store, &seeded, hidden_prior.id, "failed").await;
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: hidden_prior.id,
            status: FlowRunStatus::Cancelled,
            output_payload: json!({}),
            error_payload: None,
            finished_at: Some(hidden_prior_started_at + Duration::seconds(1)),
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
    set_run_external_context(&store, current_run.id, external_user, conversation_id).await;
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: current_run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({ "answer": "visible current" }),
            error_payload: None,
            finished_at: Some(current_started_at + Duration::seconds(1)),
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
    let stitched_source_ids = detail
        .stitched_trace
        .iter()
        .map(|trace| trace.source_flow_run.id)
        .collect::<Vec<_>>();

    assert_eq!(stitched_source_ids, vec![visible_prior.id]);
}

#[tokio::test]
async fn application_run_trace_projection_watermark_hides_failed_imported_stitched_sources() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let conversation_id = "conversation-watermark-import-visibility";
    let external_user = "claude-code-watermark-user-fixture";
    let hidden_started_at = datetime!(2026-06-24 10:00:00 UTC);
    let current_started_at = datetime!(2026-06-24 10:00:05 UTC);

    let hidden_prior = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        hidden_started_at,
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    set_run_external_context(&store, hidden_prior.id, external_user, conversation_id).await;
    attach_import_job_to_run(&store, &seeded, hidden_prior.id, "failed").await;
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: hidden_prior.id,
            status: FlowRunStatus::Cancelled,
            output_payload: json!({}),
            error_payload: None,
            finished_at: Some(hidden_started_at + Duration::seconds(1)),
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
    set_run_external_context(&store, current_run.id, external_user, conversation_id).await;
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: current_run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({ "answer": "watermark current" }),
            error_payload: None,
            finished_at: Some(current_started_at + Duration::seconds(1)),
        },
    )
    .await
    .unwrap();

    let source_watermark =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_trace_projection_source_watermark(
            &store,
            seeded.application_id,
            current_run.id,
        )
        .await
        .unwrap()
        .unwrap();

    assert!(
        source_watermark.ends_with("/stitched:0/subagents:0"),
        "failed imported source must not change projection source watermark: {source_watermark}"
    );
}

async fn set_run_external_context(
    store: &PgControlPlaneStore,
    run_id: Uuid,
    external_user: &str,
    conversation_id: &str,
) {
    sqlx::query(
        r#"
        update flow_runs
        set external_user = $2,
            external_conversation_id = $3,
            compatibility_mode = 'anthropic-messages-v1'
        where id = $1
        "#,
    )
    .bind(run_id)
    .bind(external_user)
    .bind(conversation_id)
    .execute(store.pool())
    .await
    .unwrap();
}

async fn attach_import_job_to_run(
    store: &PgControlPlaneStore,
    seeded: &RuntimeSeedState,
    run_id: Uuid,
    status: &str,
) {
    let upload_session_id = Uuid::now_v7();
    let import_job_id = Uuid::now_v7();

    sqlx::query(
        r#"
        insert into run_archive_upload_sessions (
            id,
            scope_id,
            application_id,
            actor_user_id,
            original_filename,
            total_size_bytes,
            received_bytes,
            expected_sha256,
            chunk_size_bytes,
            status,
            completed_at,
            created_by,
            updated_by
        ) values ($1, $2, $3, $4, 'fixture.zip', 8, 8, $5, 8, 'completed', now(), $4, $4)
        "#,
    )
    .bind(upload_session_id)
    .bind(seeded.workspace_id)
    .bind(seeded.application_id)
    .bind(seeded.actor_user_id)
    .bind("0".repeat(64))
    .execute(store.pool())
    .await
    .unwrap();

    sqlx::query(
        r#"
        insert into run_archive_import_jobs (
            id,
            scope_id,
            application_id,
            actor_user_id,
            upload_session_id,
            status,
            archive_version,
            archive_sha256,
            run_count,
            imported_run_count,
            error_payload,
            result_payload,
            started_at,
            finished_at,
            created_by,
            updated_by
        ) values ($1, $2, $3, $4, $5, $6, 1, $7, 1, 1, '{}', '{}', now(), now(), $4, $4)
        "#,
    )
    .bind(import_job_id)
    .bind(seeded.workspace_id)
    .bind(seeded.application_id)
    .bind(seeded.actor_user_id)
    .bind(upload_session_id)
    .bind(status)
    .bind("1".repeat(64))
    .execute(store.pool())
    .await
    .unwrap();

    sqlx::query(
        r#"
        update flow_runs
        set import_job_id = $2,
            import_source_run_id = $3
        where id = $1
        "#,
    )
    .bind(run_id)
    .bind(import_job_id)
    .bind(run_id.to_string())
    .execute(store.pool())
    .await
    .unwrap();
}
