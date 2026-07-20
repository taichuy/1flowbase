use super::*;
use control_plane::application_public_api::run_service::{
    ApplicationPublishedFlowRunRepository, ApplicationPublishedRunControlRepository,
};

#[tokio::test]
async fn orchestration_runtime_repository_round_trips_canonical_published_public_run_metadata() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let api_key_id = seed_application_api_key(&store, &seeded).await;
    let publication_version_id = Uuid::now_v7();
    let started_at = datetime!(2026-05-09 23:55:00 UTC);

    let created = <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_flow_run(
        &store,
        &CreateFlowRunInput {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_id: seeded.flow_id,
            flow_draft_id: seeded.draft_id,
            compiled_plan_id: compiled.id,
            debug_session_id: "published-public-run".to_string(),
            flow_schema_version: compiled.schema_version.clone(),
            document_hash: compiled.document_hash.clone(),
            run_mode: FlowRunMode::PublishedApiRun,
            target_node_id: None,
            title: "Customer hello".to_string(),
            status: FlowRunStatus::Running,
            input_payload: json!({ "message": "hello" }),
            started_at,
            api_key_id: Some(api_key_id),
            publication_version_id: Some(publication_version_id),
            external_user: Some("external-user-1".to_string()),
            external_conversation_id: Some("conversation-1".to_string()),
            external_trace_id: Some("trace-1".to_string()),
            compatibility_mode: None,
            idempotency_key: Some("idem-1".to_string()),
        },
    )
    .await
    .unwrap();

    assert_eq!(created.run_mode, FlowRunMode::PublishedApiRun);
    assert_eq!(created.run_mode.as_str(), "published_api_run");
    assert_eq!(created.api_key_id, Some(api_key_id));
    assert_eq!(created.publication_version_id, Some(publication_version_id));
    assert_eq!(created.title, "Customer hello");
    assert_eq!(created.external_user.as_deref(), Some("external-user-1"));
    assert_eq!(
        created.external_conversation_id.as_deref(),
        Some("conversation-1")
    );
    assert_eq!(created.external_trace_id.as_deref(), Some("trace-1"));
    assert!(created.compatibility_mode.is_none());
    assert_eq!(created.idempotency_key.as_deref(), Some("idem-1"));

    let fetched = <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_flow_run(
        &store,
        seeded.application_id,
        created.id,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(fetched.api_key_id, Some(api_key_id));
    assert_eq!(fetched.publication_version_id, Some(publication_version_id));
    assert_eq!(fetched.title, "Customer hello");
    assert_eq!(fetched.external_user.as_deref(), Some("external-user-1"));
    assert_eq!(
        fetched.external_conversation_id.as_deref(),
        Some("conversation-1")
    );
    assert_eq!(fetched.external_trace_id.as_deref(), Some("trace-1"));
    assert!(fetched.compatibility_mode.is_none());
    assert_eq!(fetched.idempotency_key.as_deref(), Some("idem-1"));

    let completed = <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: created.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({ "ok": true }),
            error_payload: None,
            finished_at: Some(started_at + Duration::seconds(3)),
        },
    )
    .await
    .unwrap();
    assert_eq!(completed.api_key_id, Some(api_key_id));
    assert_eq!(
        completed.publication_version_id,
        Some(publication_version_id)
    );
    assert_eq!(completed.title, "Customer hello");
    assert_eq!(completed.external_user.as_deref(), Some("external-user-1"));
    assert_eq!(
        completed.external_conversation_id.as_deref(),
        Some("conversation-1")
    );
    assert_eq!(completed.external_trace_id.as_deref(), Some("trace-1"));
    assert!(completed.compatibility_mode.is_none());
    assert_eq!(completed.idempotency_key.as_deref(), Some("idem-1"));
}

#[tokio::test]
async fn workflow_schedule_run_create_or_get_is_atomic_across_concurrent_callers() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let idempotency_key = "workflow-schedule:test:34200".to_string();

    let input = CreateFlowRunInput {
        actor_user_id: seeded.actor_user_id,
        application_id: seeded.application_id,
        flow_id: seeded.flow_id,
        flow_draft_id: seeded.draft_id,
        compiled_plan_id: compiled.id,
        debug_session_id: String::new(),
        flow_schema_version: compiled.schema_version.clone(),
        document_hash: compiled.document_hash.clone(),
        run_mode: FlowRunMode::WorkflowScheduleRun,
        target_node_id: None,
        title: "Scheduled workflow".to_string(),
        status: FlowRunStatus::Queued,
        input_payload: json!({ "node-workflow-start": { "customer_id": "C-42" } }),
        started_at: datetime!(2026-06-30 09:30:00 UTC),
        api_key_id: None,
        publication_version_id: None,
        external_user: None,
        external_conversation_id: None,
        external_trace_id: Some(format!("workflow-schedule:{}", seeded.application_id)),
        compatibility_mode: None,
        idempotency_key: Some(idempotency_key.clone()),
    };
    let (first, second) = tokio::join!(
        <PgControlPlaneStore as ApplicationPublishedFlowRunRepository>::create_published_flow_run(
            &store, &input,
        ),
        <PgControlPlaneStore as ApplicationPublishedFlowRunRepository>::create_published_flow_run(
            &store, &input,
        ),
    );
    let first = first.unwrap();
    let second = second.unwrap();
    let fetched = <PgControlPlaneStore as ApplicationPublishedFlowRunRepository>::find_published_flow_run_by_idempotency_key(
        &store,
        seeded.application_id,
        None,
        &idempotency_key,
    )
    .await
    .unwrap()
    .expect("schedule run should be found without an api_key_id");
    let persisted_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from flow_runs
        where application_id = $1
          and run_mode = 'workflow_schedule_run'
          and idempotency_key = $2
        "#,
    )
    .bind(seeded.application_id)
    .bind(&idempotency_key)
    .fetch_one(store.pool())
    .await
    .unwrap();

    assert_ne!(first.created, second.created);
    assert_eq!(first.flow_run.id, second.flow_run.id);
    assert_eq!(fetched.id, first.flow_run.id);
    assert_eq!(fetched.api_key_id, None);
    assert_eq!(fetched.run_mode, FlowRunMode::WorkflowScheduleRun);
    assert!(fetched.compatibility_mode.is_none());
    assert_eq!(persisted_count, 1);
}

#[tokio::test]
async fn published_run_stream_state_projects_status_usage_and_latest_pending_callback() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-07-16 15:00:00 UTC);
    let run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started_at,
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    let node_run = seed_node_run(&store, &run, started_at).await;
    sqlx::query(
        r#"
        update node_runs
        set metrics_payload = $2,
            output_payload = $3
        where id = $1
        "#,
    )
    .bind(node_run.id)
    .bind(json!({
        "usage": {
            "input_tokens": 21,
            "output_tokens": 8
        },
        "large_unrelated_metrics": "must not be projected"
    }))
    .bind(json!({
        "usage": {
            "prompt_tokens": 999,
            "completion_tokens": 999
        },
        "large_unrelated_output": "must not be projected"
    }))
    .execute(store.pool())
    .await
    .unwrap();
    let first_pending =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_callback_task(
            &store,
            &CreateCallbackTaskInput {
                flow_run_id: run.id,
                node_run_id: node_run.id,
                callback_kind: "llm_tool_calls".to_string(),
                request_payload: json!({ "tool_calls": [{ "id": "toolu_first" }] }),
                external_ref_payload: None,
            },
        )
        .await
        .unwrap();
    let latest_pending =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_callback_task(
            &store,
            &CreateCallbackTaskInput {
                flow_run_id: run.id,
                node_run_id: node_run.id,
                callback_kind: "llm_tool_calls".to_string(),
                request_payload: json!({
                    "tool_calls": [{ "id": "toolu_latest" }],
                    "large_unrelated_request": "must not be projected"
                }),
                external_ref_payload: None,
            },
        )
        .await
        .unwrap();

    let stream_state =
        <PgControlPlaneStore as ApplicationPublishedRunControlRepository>::get_published_run_stream_state(
            &store,
            seeded.application_id,
            run.id,
        )
        .await
        .unwrap()
        .expect("published run stream state should exist");
    let wrong_application =
        <PgControlPlaneStore as ApplicationPublishedRunControlRepository>::get_published_run_stream_state(
            &store,
            Uuid::now_v7(),
            run.id,
        )
        .await
        .unwrap();

    assert_eq!(stream_state.status, FlowRunStatus::Running);
    assert_eq!(stream_state.node_usages.len(), 1);
    assert_eq!(
        stream_state.node_usages[0].metrics_usage,
        Some(json!({ "input_tokens": 21, "output_tokens": 8 }))
    );
    assert_eq!(
        stream_state.node_usages[0].output_usage,
        Some(json!({ "prompt_tokens": 999, "completion_tokens": 999 }))
    );
    assert_ne!(first_pending.id, latest_pending.id);
    let pending_callback = stream_state
        .latest_pending_callback
        .expect("latest pending callback should be projected");
    assert_eq!(pending_callback.id, latest_pending.id);
    assert_eq!(pending_callback.request_payload, None);
    assert_eq!(
        pending_callback.tool_calls,
        Some(json!([{ "id": "toolu_latest" }]))
    );
    assert!(wrong_application.is_none());
}

#[tokio::test]
async fn data_model_side_effect_receipts_upsert_and_get_by_workspace_key() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-05-08 10:00:00 UTC);
    let run = seed_flow_run(&store, &seeded, &compiled, started_at).await;
    let node_run = seed_node_run_for(
        &store,
        &run,
        "node-create",
        "data_model_create",
        "Data Model Create",
        json!({ "payload": { "title": "Order A" } }),
        started_at,
    )
    .await;
    let payload_hash = "sha256:test".to_string();
    let idempotency_key = format!(
        "data_model:{}:{}:{}:{}:{}:{}:{}",
        seeded.workspace_id,
        seeded.application_id,
        seeded.draft_id,
        run.id,
        node_run.node_id,
        "create",
        payload_hash
    );
    let input = UpsertDataModelSideEffectReceiptInput {
        workspace_id: seeded.workspace_id,
        application_id: seeded.application_id,
        draft_id: seeded.draft_id,
        flow_run_id: run.id,
        node_run_id: node_run.id,
        node_id: node_run.node_id.clone(),
        action: "create".to_string(),
        model_code: "orders".to_string(),
        record_id: Some("record-1".to_string()),
        deleted_id: None,
        affected_count: 1,
        idempotency_key: idempotency_key.clone(),
        payload_hash,
        actor_user_id: seeded.actor_user_id,
        scope_id: seeded.workspace_id,
        status: "succeeded".to_string(),
        output_payload: json!({
            "record": {
                "id": "record-1",
                "title": "Order A"
            }
        }),
    };

    let claim =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::claim_data_model_side_effect_receipt(
            &store, &input,
        )
        .await
        .unwrap();
    assert!(claim.claimed);
    assert_eq!(claim.record.status, "pending");

    let first =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::upsert_data_model_side_effect_receipt(
            &store, &input,
        )
        .await
        .unwrap();
    assert_eq!(first.id, claim.record.id);
    assert_eq!(first.status, "succeeded");
    let mut replay_input = input.clone();
    replay_input.record_id = Some("record-2".to_string());
    replay_input.output_payload = json!({
        "record": {
            "id": "record-2",
            "title": "Order B"
        }
    });
    let replay =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::upsert_data_model_side_effect_receipt(
            &store,
            &replay_input,
        )
        .await
        .unwrap();
    let loaded =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_data_model_side_effect_receipt(
            &store,
            seeded.workspace_id,
            &idempotency_key,
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(replay.id, first.id);
    assert_eq!(loaded.id, first.id);
    assert_eq!(loaded.record_id.as_deref(), Some("record-1"));
    assert_eq!(loaded.output_payload["record"]["id"], json!("record-1"));
    assert!(
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_data_model_side_effect_receipt(
            &store,
            Uuid::now_v7(),
            &idempotency_key,
        )
        .await
        .unwrap()
        .is_none()
    );
}
