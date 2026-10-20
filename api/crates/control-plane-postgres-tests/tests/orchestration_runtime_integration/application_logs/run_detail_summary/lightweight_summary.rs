use super::*;

#[tokio::test]
async fn application_run_lightweight_reads_preserve_contract_order_without_unrelated_payloads() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-08-13 08:00:00 UTC);
    let large_payload = "x".repeat(1_000_000);
    let callback_request_payload = json!({
        "tool_calls": [{
            "id": "call-1",
            "name": "lookup",
            "arguments": { "large_context": large_payload.clone() }
        }],
        "fixed_context_copy": large_payload.clone()
    });
    let run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started_at,
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    let node = seed_node_run_for(
        &store,
        &run,
        "node-tool",
        "llm",
        "Tool caller",
        json!({}),
        started_at,
    )
    .await;
    let callback = <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_callback_task(
        &store,
        &CreateCallbackTaskInput {
            flow_run_id: run.id,
            node_run_id: node.id,
            callback_kind: "llm_tool_calls".to_string(),
            request_payload: callback_request_payload.clone(),
            external_ref_payload: Some(json!({ "large_external_context": true })),
        },
    )
    .await
    .unwrap();
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::complete_callback_task(
        &store,
        &CompleteCallbackTaskInput {
            callback_task_id: callback.id,
            response_payload: json!({
                "tool_results": [{
                    "tool_call_id": "call-1",
                    "name": "lookup",
                    "content": large_payload.clone()
                }]
            }),
            completed_at: started_at + Duration::seconds(2),
        },
    )
    .await
    .unwrap();
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::WaitingCallback,
            output_payload: json!({ "answer": "waiting" }),
            error_payload: None,
            finished_at: None,
        },
    )
    .await
    .unwrap();
    let checkpoint = <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_checkpoint(
        &store,
        &CreateCheckpointInput {
            flow_run_id: run.id,
            node_run_id: Some(node.id),
            status: "waiting_callback".to_string(),
            reason: "waiting".to_string(),
            locator_payload: json!({ "node_id": "node-tool", "next_node_index": 1 }),
            variable_snapshot: json!({ "large_snapshot": "detail-only" }),
            external_ref_payload: Some(json!({ "detail": true })),
        },
    )
    .await
    .unwrap();
    for event_type in [
        "public_run_resume_requested",
        "unrelated_runtime_fact",
        "flow_run_resumed",
    ] {
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_run_event(
            &store,
            &AppendRunEventInput {
                flow_run_id: run.id,
                node_run_id: Some(node.id),
                event_type: event_type.to_string(),
                payload: json!({ "event_type": event_type }),
            },
        )
        .await
        .unwrap();
    }
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_run_event(
        &store,
        &AppendRunEventInput {
            flow_run_id: run.id,
            node_run_id: None,
            event_type: "unrelated_runtime_fact".to_string(),
            payload: json!({ "large": "not a node detail" }),
        },
    )
    .await
    .unwrap();

    let projection_source =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_trace_projection_source(
            &store,
            seeded.application_id,
            run.id,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(projection_source.event_count, 4);
    assert_eq!(
        projection_source.event_counts_by_node_run.get(&node.id),
        Some(&3)
    );
    assert_eq!(
        projection_source
            .checkpoint_counts_by_node_run
            .get(&node.id),
        Some(&1)
    );
    let projection =
        control_plane::orchestration_runtime::trace_projection::build_application_run_trace_projection_from_source(
            &projection_source,
        )
        .unwrap();
    let node_content = projection
        .contents
        .iter()
        .find(|content| content.content_kind == "node_run")
        .unwrap();
    assert_eq!(
        node_content.payload["payload_index"]["event_count"],
        json!(3)
    );
    assert_eq!(
        node_content.payload["payload_index"]["checkpoint_count"],
        json!(1)
    );
    assert_eq!(
        projection.source_watermark,
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_trace_projection_source_watermark(
            &store,
            seeded.application_id,
            run.id,
        )
        .await
        .unwrap()
        .unwrap()
    );

    let overview =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_overview(
            &store,
            seeded.application_id,
            run.id,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(overview.waiting_node_id.as_deref(), Some("node-tool"));
    assert_eq!(overview.waiting_node_run_id, Some(node.id));
    assert_eq!(overview.tool_callback_count, 1);
    assert_eq!(overview.flow_run.id, run.id);
    assert_eq!(overview.flow_run.application_id, seeded.application_id);
    assert!(
        store
            .get_flow_run_metadata(Uuid::now_v7(), run.id)
            .await
            .unwrap()
            .is_none(),
        "metadata lookup must retain application authorization scope"
    );

    let timeline = <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_resume_timeline(
        &store,
        seeded.application_id,
        run.id,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        timeline.callback_tasks[0].request_payload,
        callback_request_payload
    );
    assert_eq!(
        timeline.callback_tasks[0].response_payload,
        Some(json!({
            "tool_results": [{
                "tool_call_id": "call-1",
                "name": "lookup",
                "content": large_payload
            }]
        }))
    );
    assert_eq!(
        timeline
            .events
            .iter()
            .map(|event| event.event_type.as_str())
            .collect::<Vec<_>>(),
        vec!["public_run_resume_requested", "flow_run_resumed"]
    );
    assert!(timeline.events[0].sequence < timeline.events[1].sequence);

    let summary = <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_resume_timeline_summary(
        &store,
        seeded.application_id,
        run.id,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(summary.callback_tasks.len(), 1);
    assert_eq!(summary.callback_tasks[0].id, callback.id);
    assert_eq!(summary.callback_tasks[0].callback_kind, "llm_tool_calls");
    assert_eq!(summary.events.len(), 2);

    let checkpoints = <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_application_run_trace_checkpoints(
        &store,
        seeded.application_id,
        run.id,
        vec![node.id],
    )
    .await
    .unwrap();
    assert_eq!(checkpoints, vec![checkpoint]);
    let events =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_application_run_trace_events(
            &store,
            seeded.application_id,
            run.id,
            vec![node.id],
        )
        .await
        .unwrap();
    assert_eq!(events.len(), 3);
    assert!(events
        .windows(2)
        .all(|pair| pair[0].sequence < pair[1].sequence));
}

#[test]
fn lightweight_summary_queries_do_not_select_callback_or_checkpoint_payload_columns() {
    let source = include_str!("../../../../../storage/durable/postgres/src/orchestration_runtime_repository/detail_queries.rs");
    let overview = source
        .split("pub(super) async fn overview_tool_callback_count")
        .nth(1)
        .unwrap()
        .split("pub(super) async fn latest_overview_waiting_node")
        .next()
        .unwrap();
    for payload_column in [
        "request_payload",
        "response_payload",
        "external_ref_payload",
    ] {
        assert!(!overview.contains(payload_column));
    }

    let callbacks = source
        .split("pub(super) async fn list_resume_timeline_callback_summaries")
        .nth(1)
        .unwrap()
        .split("pub(super) async fn list_resume_timeline_event_summaries")
        .next()
        .unwrap();
    for payload_column in [
        "request_payload",
        "response_payload",
        "external_ref_payload",
    ] {
        assert!(!callbacks.contains(payload_column));
    }

    let events = source
        .split("pub(super) async fn list_resume_timeline_event_summaries")
        .nth(1)
        .unwrap()
        .split("pub(super) async fn list_trace_checkpoints_for_node_runs")
        .next()
        .unwrap();
    assert!(!events.contains("flow_run_callback_tasks"));
    let projected_events = events
        .split("projected_events as")
        .nth(1)
        .unwrap()
        .split("legacy_events as")
        .next()
        .unwrap();
    assert!(!projected_events.contains("payload"));
    assert!(projected_events.contains("resume_timeline_description"));

    let legacy_events = events
        .split("legacy_events as")
        .nth(1)
        .unwrap()
        .split("select * from projected_events")
        .next()
        .unwrap();
    assert!(legacy_events.contains("events.payload"));
    assert!(legacy_events.contains("not events.resume_timeline_description_projected"));
}

#[tokio::test]
async fn resume_timeline_summary_uses_each_events_own_identifier_projection() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-08-13 18:00:00 UTC);
    let run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started_at,
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    let node = seed_node_run_for(
        &store,
        &run,
        "node-resume",
        "llm",
        "Resume",
        json!({}),
        started_at,
    )
    .await;
    let nearest_callback =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_callback_task(
            &store,
            &CreateCallbackTaskInput {
                flow_run_id: run.id,
                node_run_id: node.id,
                callback_kind: "external_callback".to_string(),
                request_payload: json!({}),
                external_ref_payload: None,
            },
        )
        .await
        .unwrap();

    let callback_identifier = Uuid::now_v7().to_string();
    let resume_identifier = "  resume-exact-format  ".to_string();
    for payload in [
        json!({}),
        json!({ "callback_task_id": callback_identifier }),
        json!({
            "resume_request_id": resume_identifier,
            "callback_task_id": nearest_callback.id.to_string()
        }),
    ] {
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_run_event(
            &store,
            &AppendRunEventInput {
                flow_run_id: run.id,
                node_run_id: Some(node.id),
                event_type: "public_run_resume_requested".to_string(),
                payload,
            },
        )
        .await
        .unwrap();
    }
    let legacy_resume_identifier = Uuid::now_v7().to_string();
    let legacy_event = <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_run_event(
        &store,
        &AppendRunEventInput {
            flow_run_id: run.id,
            node_run_id: Some(node.id),
            event_type: "flow_run_resumed".to_string(),
            payload: json!({
                "resume_request_id": legacy_resume_identifier,
                "callback_task_id": nearest_callback.id.to_string()
            }),
        },
    )
    .await
    .unwrap();
    sqlx::query(
        r#"
        update flow_run_events
        set resume_timeline_description = null,
            resume_timeline_description_projected = false
        where id = $1
        "#,
    )
    .bind(legacy_event.id)
    .execute(store.pool())
    .await
    .unwrap();

    let summary = <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_resume_timeline_summary(
        &store,
        seeded.application_id,
        run.id,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        summary
            .events
            .iter()
            .map(|event| event.description.as_deref())
            .collect::<Vec<_>>(),
        vec![
            None,
            Some(callback_identifier.as_str()),
            Some(resume_identifier.as_str()),
            Some(legacy_resume_identifier.as_str()),
        ]
    );

    let projected_rows = sqlx::query_as::<_, (Option<String>, bool)>(
        r#"
        select resume_timeline_description, resume_timeline_description_projected
        from flow_run_events
        where flow_run_id = $1 and id <> $2
        order by sequence asc
        "#,
    )
    .bind(run.id)
    .bind(legacy_event.id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(
        projected_rows,
        vec![
            (None, true),
            (Some(callback_identifier), true),
            (Some(resume_identifier), true),
        ]
    );
}
