use super::*;

async fn seed_file_storage(
    store: &PgControlPlaneStore,
    actor_user_id: Uuid,
) -> domain::FileStorageRecord {
    let storage_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into file_storages (
            id,
            code,
            title,
            driver_type,
            enabled,
            is_default,
            config_json,
            rule_json,
            created_by,
            updated_by
        )
        values ($1, $2, 'Runtime Debug', 'local', true, false, $3, '{}'::jsonb, $4, $4)
        "#,
    )
    .bind(storage_id)
    .bind(format!("runtime_debug_{}", storage_id.simple()))
    .bind(json!({ "root_path": "/tmp/1flowbase-runtime-debug-test" }))
    .bind(actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();

    <PgControlPlaneStore as control_plane::ports::FileManagementRepository>::get_file_storage(
        store, storage_id,
    )
    .await
    .unwrap()
    .unwrap()
}

#[tokio::test]
async fn runtime_debug_artifacts_are_scoped_and_payload_previews_are_persisted() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-05-08 09:40:00 UTC);
    let run = seed_flow_run(&store, &seeded, &compiled, started_at).await;
    let node_run = seed_node_run(&store, &run, started_at).await;
    let run_event = <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_run_event(
        &store,
        &AppendRunEventInput {
            flow_run_id: run.id,
            node_run_id: Some(node_run.id),
            event_type: "text_delta".into(),
            payload: json!({ "text": "x".repeat(128) }),
        },
    )
    .await
    .unwrap();
    let storage = seed_file_storage(&store, seeded.actor_user_id).await;
    let artifact_id = Uuid::now_v7();

    let artifact =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_runtime_debug_artifact(
            &store,
            &CreateRuntimeDebugArtifactInput {
                artifact_id,
                workspace_id: seeded.workspace_id,
                application_id: seeded.application_id,
                flow_run_id: Some(run.id),
                node_run_id: Some(node_run.id),
                run_event_id: Some(run_event.id),
                artifact_kind: "node_output_payload".into(),
                content_type: "application/json".into(),
                original_size_bytes: 4096,
                preview_size_bytes: 512,
                storage_id: storage.id,
                storage_ref: "runtime-debug/test/artifact.json".into(),
                retention_state: "active".into(),
            },
        )
        .await
        .unwrap();

    assert_eq!(artifact.id, artifact_id);
    assert_eq!(artifact.workspace_id, seeded.workspace_id);
    assert_eq!(artifact.application_id, seeded.application_id);
    assert_eq!(artifact.flow_run_id, Some(run.id));
    assert_eq!(artifact.node_run_id, Some(node_run.id));
    assert_eq!(artifact.run_event_id, Some(run_event.id));

    let loaded =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_runtime_debug_artifact(
            &store,
            &GetRuntimeDebugArtifactInput {
                workspace_id: seeded.workspace_id,
                application_id: seeded.application_id,
                artifact_id,
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.storage_ref, "runtime-debug/test/artifact.json");
    assert!(
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_runtime_debug_artifact(
            &store,
            &GetRuntimeDebugArtifactInput {
                workspace_id: Uuid::now_v7(),
                application_id: seeded.application_id,
                artifact_id,
            },
        )
        .await
        .unwrap()
        .is_none()
    );
    let pending_delete_artifact_id = Uuid::now_v7();
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_runtime_debug_artifact(
        &store,
        &CreateRuntimeDebugArtifactInput {
            artifact_id: pending_delete_artifact_id,
            workspace_id: seeded.workspace_id,
            application_id: seeded.application_id,
            flow_run_id: Some(run.id),
            node_run_id: Some(node_run.id),
            run_event_id: Some(run_event.id),
            artifact_kind: "node_output_payload".into(),
            content_type: "application/json".into(),
            original_size_bytes: 1024,
            preview_size_bytes: 128,
            storage_id: storage.id,
            storage_ref: "runtime-debug/test/pending.json".into(),
            retention_state: "pending_delete".into(),
        },
    )
    .await
    .unwrap();
    assert!(
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_runtime_debug_artifact(
            &store,
            &GetRuntimeDebugArtifactInput {
                workspace_id: seeded.workspace_id,
                application_id: seeded.application_id,
                artifact_id: pending_delete_artifact_id,
            },
        )
        .await
        .unwrap()
        .is_none()
    );
    let deleted_artifact_id = Uuid::now_v7();
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_runtime_debug_artifact(
        &store,
        &CreateRuntimeDebugArtifactInput {
            artifact_id: deleted_artifact_id,
            workspace_id: seeded.workspace_id,
            application_id: seeded.application_id,
            flow_run_id: Some(run.id),
            node_run_id: Some(node_run.id),
            run_event_id: Some(run_event.id),
            artifact_kind: "node_output_payload".into(),
            content_type: "application/json".into(),
            original_size_bytes: 1024,
            preview_size_bytes: 128,
            storage_id: storage.id,
            storage_ref: "runtime-debug/test/deleted.json".into(),
            retention_state: "deleted".into(),
        },
    )
    .await
    .unwrap();
    assert!(
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_runtime_debug_artifact(
            &store,
            &GetRuntimeDebugArtifactInput {
                workspace_id: seeded.workspace_id,
                application_id: seeded.application_id,
                artifact_id: deleted_artifact_id,
            },
        )
        .await
        .unwrap()
        .is_none()
    );

    let flow_run =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run_payloads(
            &store,
            &UpdateFlowRunPayloadsInput {
                flow_run_id: run.id,
                input_payload: json!({
                    "__runtime_debug_artifact": true,
                    "artifact_ref": artifact_id.to_string(),
                    "is_truncated": true
                }),
                output_payload: json!({}),
                error_payload: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        flow_run.input_payload["artifact_ref"],
        artifact_id.to_string()
    );

    let node_run =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_node_run_payloads(
            &store,
            &UpdateNodeRunPayloadsInput {
                node_run_id: node_run.id,
                input_payload: json!({}),
                output_payload: json!({
                    "__runtime_debug_artifact": true,
                    "artifact_ref": artifact_id.to_string(),
                    "is_truncated": true
                }),
                error_payload: None,
                metrics_payload: json!({}),
                debug_payload: json!({}),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        node_run.output_payload["artifact_ref"],
        artifact_id.to_string()
    );

    let run_event =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_run_event_payload(
            &store,
            &UpdateRunEventPayloadInput {
                run_event_id: run_event.id,
                payload: json!({
                    "__runtime_debug_artifact": true,
                    "artifact_ref": artifact_id.to_string(),
                    "is_truncated": true
                }),
            },
        )
        .await
        .unwrap();
    assert_eq!(run_event.payload["artifact_ref"], artifact_id.to_string());
}
