use super::*;

#[tokio::test]
async fn trace_projection_repository_queries_root_children_content_and_status() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-06-18 09:00:00 UTC);
    let run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started_at,
        FlowRunMode::DebugFlowRun,
        None,
    )
    .await;
    let root_trace_node_id =
        control_plane::orchestration_runtime::trace_projection::trace_node_id_for_locator(
            run.id,
            "run:test/node:root",
        );
    let child_trace_node_id =
        control_plane::orchestration_runtime::trace_projection::trace_node_id_for_locator(
            run.id,
            "run:test/node:root/tool:weather",
        );

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::replace_application_run_trace_projection(
        &store,
        &control_plane::ports::ReplaceApplicationRunTraceProjectionInput {
            flow_run_id: run.id,
            projection_version: 1,
            source_watermark: "node_runs:2/runtime_events:0".to_string(),
            nodes: vec![
                control_plane::ports::ApplicationRunTraceNodeProjectionInput {
                    trace_node_id: root_trace_node_id,
                    parent_trace_node_id: None,
                    stable_locator: "run:test/node:root".to_string(),
                    node_kind: "node_run".to_string(),
                    owner_kind: Some("node_run".to_string()),
                    owner_id: Some("root-node-run".to_string()),
                    order_key: "000001".to_string(),
                    node_id: Some("node-root".to_string()),
                    node_type: Some("llm".to_string()),
                    node_mode: None,
                    node_alias: "Root LLM".to_string(),
                    status: "succeeded".to_string(),
                    started_at,
                    finished_at: Some(started_at + Duration::seconds(1)),
                    duration_ms: Some(1000),
                    metrics_payload: json!({ "usage": { "total_tokens": 13 } }),
                    has_children: true,
                    child_count: 1,
                    has_content: true,
                    content_ref: None,
                    source_flow_run_id: None,
                    source_trace_node_id: None,
                    parent_callback_task_id: None,
                    parent_tool_call_id: None,
                    trace_relation_kind: None,
                },
                control_plane::ports::ApplicationRunTraceNodeProjectionInput {
                    trace_node_id: child_trace_node_id,
                    parent_trace_node_id: Some(root_trace_node_id),
                    stable_locator: "run:test/node:root/tool:weather".to_string(),
                    node_kind: "tool_callback".to_string(),
                    owner_kind: Some("tool_call".to_string()),
                    owner_id: Some("call-weather".to_string()),
                    order_key: "000001/000001".to_string(),
                    node_id: None,
                    node_type: Some("tool".to_string()),
                    node_mode: Some("route".to_string()),
                    node_alias: "weather".to_string(),
                    status: "succeeded".to_string(),
                    started_at: started_at + Duration::milliseconds(100),
                    finished_at: Some(started_at + Duration::milliseconds(900)),
                    duration_ms: Some(800),
                    metrics_payload: json!({}),
                    has_children: false,
                    child_count: 0,
                    has_content: true,
                    content_ref: Some("artifact:tool-weather".to_string()),
                    source_flow_run_id: None,
                    source_trace_node_id: None,
                    parent_callback_task_id: None,
                    parent_tool_call_id: None,
                    trace_relation_kind: None,
                },
            ],
            contents: vec![control_plane::ports::ApplicationRunTraceNodeContentProjectionInput {
                trace_node_id: child_trace_node_id,
                content_kind: "tool_callback".to_string(),
                payload: json!({
                    "tool_call_id": "call-weather",
                    "payload": { "temperature": 22 }
                }),
                source_refs: json!([
                    {
                        "source_kind": "callback_task",
                        "source_locator": "callback-task:test"
                    }
                ]),
            }],
        },
    )
    .await
    .unwrap();

    let status =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_trace_projection_status(
            &store,
            run.id,
            1,
        )
        .await
        .unwrap()
        .expect("projection status should be stored");
    assert_eq!(
        status.status,
        domain::ApplicationRunTraceProjectionStatus::Succeeded
    );
    assert_eq!(status.source_watermark, "node_runs:2/runtime_events:0");

    let flow_run_scope_id: Uuid =
        sqlx::query_scalar("select scope_id from flow_runs where id = $1")
            .bind(run.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(flow_run_scope_id, seeded.workspace_id);

    let (status_id, status_scope_id): (Uuid, Uuid) = sqlx::query_as(
        r#"
        select id, scope_id
        from application_run_trace_projection_statuses
        where flow_run_id = $1
          and projection_version = $2
        "#,
    )
    .bind(run.id)
    .bind(1)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(status_scope_id, flow_run_scope_id);
    assert_ne!(status_id, run.id);

    let node_physical_rows: Vec<(Uuid, Uuid, Uuid)> = sqlx::query_as(
        r#"
        select trace_node_id, id, scope_id
        from application_run_trace_nodes
        where flow_run_id = $1
        order by order_key
        "#,
    )
    .bind(run.id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(node_physical_rows.len(), 2);
    for (trace_node_id, node_id, node_scope_id) in node_physical_rows {
        assert_eq!(node_scope_id, flow_run_scope_id);
        assert_ne!(node_id, trace_node_id);
    }

    let (content_id, content_flow_run_id, content_scope_id): (Uuid, Uuid, Uuid) = sqlx::query_as(
        r#"
        select id, flow_run_id, scope_id
        from application_run_trace_node_contents
        where trace_node_id = $1
        "#,
    )
    .bind(child_trace_node_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(content_flow_run_id, run.id);
    assert_eq!(content_scope_id, flow_run_scope_id);
    assert_ne!(content_id, child_trace_node_id);

    let roots =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_application_run_trace_roots(
            &store, run.id,
        )
        .await
        .unwrap();
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].trace_node_id, root_trace_node_id);
    assert_eq!(roots[0].stable_locator, "run:test/node:root");
    assert_eq!(roots[0].child_count, 1);

    let children_page =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_application_run_trace_children_page(
            &store,
            control_plane::ports::ListApplicationRunTraceChildrenPageInput {
                flow_run_id: run.id,
                parent_trace_node_id: root_trace_node_id,
                page_size: 20,
                cursor: None,
            },
        )
        .await
        .unwrap();
    let children = children_page.items;
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].trace_node_id, child_trace_node_id);
    assert_eq!(children[0].parent_trace_node_id, Some(root_trace_node_id));
    assert_eq!(children[0].node_mode.as_deref(), Some("route"));
    assert!(!children_page.has_more);
    assert!(children_page.next_cursor.is_none());

    let locator_match =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_trace_node_by_locator(
            &store,
            run.id,
            "run:test/node:root/tool:weather",
        )
        .await
        .unwrap()
        .expect("stable locator should find child node");
    assert_eq!(locator_match.trace_node_id, child_trace_node_id);

    let content =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_trace_node_content(
            &store,
            run.id,
            child_trace_node_id,
        )
        .await
        .unwrap()
        .expect("content should be stored separately from summary");
    assert_eq!(content.content_kind, "tool_callback");
    assert_eq!(content.payload["tool_call_id"], json!("call-weather"));
    assert_eq!(content.payload["payload"]["temperature"], json!(22));

    let index_names: Vec<(String, String)> = sqlx::query_as(
        r#"
        select tablename, indexname
        from pg_indexes
        where schemaname = current_schema()
          and tablename in (
              'application_run_trace_projection_statuses',
              'application_run_trace_nodes',
              'application_run_trace_node_contents'
          )
        order by tablename, indexname
        "#,
    )
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert!(index_names
        .iter()
        .any(|(_, name)| name == "application_run_trace_nodes_children_idx"));
    assert!(index_names
        .iter()
        .any(|(_, name)| name == "application_run_trace_nodes_stable_locator_idx"));
    assert!(index_names.iter().any(|(_, name)| {
        name == "application_run_trace_projection_statuses_scope_created_id_idx"
    }));
    assert!(index_names
        .iter()
        .any(|(_, name)| name == "application_run_trace_nodes_scope_created_id_idx"));
    assert!(index_names
        .iter()
        .any(|(_, name)| { name == "application_run_trace_node_contents_scope_created_id_idx" }));
}

#[tokio::test]
async fn trace_projection_repository_paginates_children_by_stable_order() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-06-18 11:00:00 UTC);
    let run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started_at,
        FlowRunMode::DebugFlowRun,
        None,
    )
    .await;
    let root_trace_node_id =
        control_plane::orchestration_runtime::trace_projection::trace_node_id_for_locator(
            run.id,
            "run:test/node:root",
        );
    let mut nodes = vec![
        control_plane::ports::ApplicationRunTraceNodeProjectionInput {
            trace_node_id: root_trace_node_id,
            parent_trace_node_id: None,
            stable_locator: "run:test/node:root".to_string(),
            node_kind: "node_run".to_string(),
            owner_kind: Some("node_run".to_string()),
            owner_id: Some("root-node-run".to_string()),
            order_key: "000001".to_string(),
            node_id: Some("node-root".to_string()),
            node_type: Some("llm".to_string()),
            node_mode: None,
            node_alias: "Root LLM".to_string(),
            status: "succeeded".to_string(),
            started_at,
            finished_at: Some(started_at + Duration::seconds(1)),
            duration_ms: Some(1000),
            metrics_payload: json!({}),
            has_children: true,
            child_count: 5,
            has_content: false,
            content_ref: None,
            source_flow_run_id: None,
            source_trace_node_id: None,
            parent_callback_task_id: None,
            parent_tool_call_id: None,
            trace_relation_kind: None,
        },
    ];
    for index in 0..5 {
        let stable_locator = format!("run:test/node:root/tool:{index:02}");
        nodes.push(control_plane::ports::ApplicationRunTraceNodeProjectionInput {
            trace_node_id:
                control_plane::orchestration_runtime::trace_projection::trace_node_id_for_locator(
                    run.id,
                    &stable_locator,
                ),
            parent_trace_node_id: Some(root_trace_node_id),
            stable_locator,
            node_kind: "tool_callback".to_string(),
            owner_kind: Some("tool_call".to_string()),
            owner_id: Some(format!("call-{index:02}")),
            order_key: format!("000001/{index:06}"),
            node_id: None,
            node_type: Some("tool".to_string()),
            node_mode: None,
            node_alias: format!("tool_{index:02}"),
            status: "succeeded".to_string(),
            started_at: started_at + Duration::milliseconds(index * 100),
            finished_at: Some(started_at + Duration::milliseconds(index * 100 + 50)),
            duration_ms: Some(50),
            metrics_payload: json!({}),
            has_children: false,
            child_count: 0,
            has_content: false,
            content_ref: None,
            source_flow_run_id: None,
            source_trace_node_id: None,
            parent_callback_task_id: None,
            parent_tool_call_id: None,
            trace_relation_kind: None,
        });
    }

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::replace_application_run_trace_projection(
        &store,
        &control_plane::ports::ReplaceApplicationRunTraceProjectionInput {
            flow_run_id: run.id,
            projection_version: 1,
            source_watermark: "node_runs:6/runtime_events:0".to_string(),
            nodes,
            contents: vec![],
        },
    )
    .await
    .unwrap();

    let first_page =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_application_run_trace_children_page(
            &store,
            control_plane::ports::ListApplicationRunTraceChildrenPageInput {
                flow_run_id: run.id,
                parent_trace_node_id: root_trace_node_id,
                page_size: 2,
                cursor: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(first_page.items.len(), 2);
    assert_eq!(first_page.items[0].node_alias, "tool_00");
    assert_eq!(first_page.items[1].node_alias, "tool_01");
    assert!(first_page.has_more);
    assert!(first_page.next_cursor.is_some());

    let second_page =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_application_run_trace_children_page(
            &store,
            control_plane::ports::ListApplicationRunTraceChildrenPageInput {
                flow_run_id: run.id,
                parent_trace_node_id: root_trace_node_id,
                page_size: 2,
                cursor: first_page.next_cursor,
            },
        )
        .await
        .unwrap();
    assert_eq!(second_page.items.len(), 2);
    assert_eq!(second_page.items[0].node_alias, "tool_02");
    assert_eq!(second_page.items[1].node_alias, "tool_03");
    assert!(second_page.has_more);

    let last_page =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_application_run_trace_children_page(
            &store,
            control_plane::ports::ListApplicationRunTraceChildrenPageInput {
                flow_run_id: run.id,
                parent_trace_node_id: root_trace_node_id,
                page_size: 2,
                cursor: second_page.next_cursor,
            },
        )
        .await
        .unwrap();
    assert_eq!(last_page.items.len(), 1);
    assert_eq!(last_page.items[0].node_alias, "tool_04");
    assert!(!last_page.has_more);
    assert!(last_page.next_cursor.is_none());
}

#[tokio::test]
async fn trace_projection_failed_status_preserves_diagnostics() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-06-18 10:00:00 UTC);
    let run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started_at,
        FlowRunMode::DebugFlowRun,
        None,
    )
    .await;

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::upsert_application_run_trace_projection_status(
        &store,
        &control_plane::ports::UpsertApplicationRunTraceProjectionStatusInput {
            flow_run_id: run.id,
            projection_version: 1,
            status: domain::ApplicationRunTraceProjectionStatus::Failed,
            source_watermark: "node_runs:1/runtime_events:3".to_string(),
            attempt_count: 2,
            last_attempt_at: Some(started_at + Duration::seconds(5)),
            last_success_at: None,
            diagnostic: Some(domain::ApplicationRunTraceProjectionDiagnostic {
                last_error_code: Some("invalid_locator".to_string()),
                last_error_stage: Some("build_locator".to_string()),
                last_error_source_kind: Some("runtime_event".to_string()),
                last_error_source_locator: Some("event:42".to_string()),
                last_error_message: Some("locator source missing stable id".to_string()),
                last_error_ref: Some("req-trace-projection-1".to_string()),
                retriable: true,
            }),
        },
    )
    .await
    .unwrap();

    let status =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_trace_projection_status(
            &store,
            run.id,
            1,
        )
        .await
        .unwrap()
        .expect("failed projection status should be stored");

    assert_eq!(
        status.status,
        domain::ApplicationRunTraceProjectionStatus::Failed
    );
    assert_eq!(status.attempt_count, 2);
    assert_eq!(status.last_error_stage.as_deref(), Some("build_locator"));
    assert_eq!(
        status.last_error_source_locator.as_deref(),
        Some("event:42")
    );
    assert_eq!(
        status.last_error_ref.as_deref(),
        Some("req-trace-projection-1")
    );
    assert!(status.retriable);

    let (status_id, status_scope_id): (Uuid, Uuid) = sqlx::query_as(
        r#"
        select statuses.id, statuses.scope_id
        from application_run_trace_projection_statuses statuses
        where statuses.flow_run_id = $1
          and statuses.projection_version = $2
        "#,
    )
    .bind(run.id)
    .bind(1)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(status_scope_id, seeded.workspace_id);
    assert_ne!(status_id, run.id);
}
