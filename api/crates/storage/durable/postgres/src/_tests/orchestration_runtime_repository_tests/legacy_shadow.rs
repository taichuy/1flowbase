use super::*;

async fn seed_legacy_checkpoint_run(
    store: &PgControlPlaneStore,
    seeded: &RuntimeSeedState,
    started_at: OffsetDateTime,
    locator_payload: serde_json::Value,
) -> (domain::FlowRunRecord, domain::CheckpointRecord) {
    let compiled = seed_compiled_plan(store, seeded).await;
    let run = seed_flow_run(store, seeded, &compiled, started_at).await;
    let checkpoint = <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_checkpoint(
        store,
        &CreateCheckpointInput {
            flow_run_id: run.id,
            node_run_id: None,
            status: "waiting_callback".to_string(),
            reason: "legacy context fixture".to_string(),
            locator_payload,
            variable_snapshot: json!({
                "messages": [
                    { "role": "user", "content": "keep order" },
                    { "role": "assistant", "tool_calls": [{ "id": "call-1", "name": "lookup" }] }
                ],
                "usage": { "input_tokens": 13, "output_tokens": 8 }
            }),
            external_ref_payload: None,
        },
    )
    .await
    .unwrap();
    (run, checkpoint)
}

fn shadow_input(
    flow_run_id: Uuid,
    execution: LegacyRuntimeShadowExecution,
) -> ConvertLegacyRuntimeShadowBatchInput {
    ConvertLegacyRuntimeShadowBatchInput {
        application_id: None,
        flow_run_id: Some(flow_run_id),
        after: None,
        limit: 100,
        lock_budget_ms: 25,
        execution,
    }
}

#[tokio::test]
async fn legacy_shadow_preview_is_write_free_and_apply_is_reentrant_with_byte_statistics() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let (run, _) = seed_legacy_checkpoint_run(
        &store,
        &seeded,
        datetime!(2026-08-13 13:00:00 UTC),
        json!({ "node_id": "legacy-node" }),
    )
    .await;

    let preview = store
        .convert_legacy_runtime_shadow_batch(&shadow_input(
            run.id,
            LegacyRuntimeShadowExecution::Preview,
        ))
        .await
        .unwrap();
    assert!(preview.statistics.iter().all(|item| item.source_bytes > 0));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from runtime_legacy_shadow_rows")
            .fetch_one(store.pool())
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from runtime_legacy_shadow_batches")
            .fetch_one(store.pool())
            .await
            .unwrap(),
        0
    );

    let applied = store
        .convert_legacy_runtime_shadow_batch(&shadow_input(
            run.id,
            LegacyRuntimeShadowExecution::Apply,
        ))
        .await
        .unwrap();
    assert!(applied.statistics.iter().any(|item| {
        item.source_kind == LegacyRuntimeShadowSourceKind::CheckpointContext
            && item.shadowed_rows == 1
            && item.canonical_bytes == item.source_bytes
    }));
    let replay = store
        .convert_legacy_runtime_shadow_batch(&shadow_input(
            run.id,
            LegacyRuntimeShadowExecution::Apply,
        ))
        .await
        .unwrap();
    assert!(replay
        .statistics
        .iter()
        .any(|item| item.already_shadowed_rows > 0));
    let duplicate_groups: i64 = sqlx::query_scalar(
        r#"
        select count(*) from (
            select source_table, source_column, source_row_id
              from runtime_legacy_shadow_rows
             group by source_table, source_column, source_row_id
            having count(*) > 1
        ) duplicates
        "#,
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(duplicate_groups, 0);
}

#[tokio::test]
async fn legacy_shadow_cursor_classifies_pending_and_terminal_runs() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let (pending, _) = seed_legacy_checkpoint_run(
        &store,
        &seeded,
        datetime!(2026-08-13 13:10:00 UTC),
        json!({ "node_id": "pending" }),
    )
    .await;
    let (terminal, _) = seed_legacy_checkpoint_run(
        &store,
        &seeded,
        datetime!(2026-08-13 13:11:00 UTC),
        json!({ "node_id": "terminal" }),
    )
    .await;
    sqlx::query("update flow_runs set status = 'succeeded', finished_at = now() where id = $1")
        .bind(terminal.id)
        .execute(store.pool())
        .await
        .unwrap();

    let first = store
        .convert_legacy_runtime_shadow_batch(&ConvertLegacyRuntimeShadowBatchInput {
            application_id: Some(seeded.application_id),
            flow_run_id: None,
            after: None,
            limit: 1,
            lock_budget_ms: 25,
            execution: LegacyRuntimeShadowExecution::Preview,
        })
        .await
        .unwrap();
    assert!(first.has_more);
    let second = store
        .convert_legacy_runtime_shadow_batch(&ConvertLegacyRuntimeShadowBatchInput {
            application_id: Some(seeded.application_id),
            flow_run_id: None,
            after: first.next,
            limit: 100,
            lock_budget_ms: 25,
            execution: LegacyRuntimeShadowExecution::Preview,
        })
        .await
        .unwrap();
    let classifications = first
        .statistics
        .iter()
        .chain(second.statistics.iter())
        .map(|item| item.run_classification)
        .collect::<Vec<_>>();
    assert!(
        classifications.contains(&control_plane::ports::LegacyRuntimeRunClassification::Pending)
    );
    assert!(
        classifications.contains(&control_plane::ports::LegacyRuntimeRunClassification::Terminal)
    );
    assert_ne!(pending.id, terminal.id);
}

#[tokio::test]
async fn legacy_shadow_preserves_mixed_repository_and_archive_source_json_then_rolls_back_only_shadow_rows(
) {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let (run, checkpoint) = seed_legacy_checkpoint_run(
        &store,
        &seeded,
        datetime!(2026-08-13 13:20:00 UTC),
        json!({ "node_id": "legacy" }),
    )
    .await;
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_run_event(
        &store,
        &AppendRunEventInput {
            flow_run_id: run.id,
            node_run_id: None,
            event_type: "legacy_tool_call".to_string(),
            payload: json!({ "tool_calls": [{ "id": "call-1" }], "usage": { "total_tokens": 21 } }),
        },
    )
    .await
    .unwrap();
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_run_event(
        &store,
        &AppendRunEventInput {
            flow_run_id: run.id,
            node_run_id: None,
            event_type: "legacy_tool_result".to_string(),
            payload: json!({ "tool_call_id": "call-1", "content": "ok" }),
        },
    )
    .await
    .unwrap();
    let before_detail = serde_json::to_value(
        store
            .get_application_run_detail(run.application_id, run.id)
            .await
            .unwrap(),
    )
    .unwrap();
    let before_archive_context =
        serde_json::to_value(store.list_context_projections(run.id).await.unwrap()).unwrap();

    store
        .convert_legacy_runtime_shadow_batch(&shadow_input(
            run.id,
            LegacyRuntimeShadowExecution::Apply,
        ))
        .await
        .unwrap();
    let after_detail = serde_json::to_value(
        store
            .get_application_run_detail(run.application_id, run.id)
            .await
            .unwrap(),
    )
    .unwrap();
    let after_archive_context =
        serde_json::to_value(store.list_context_projections(run.id).await.unwrap()).unwrap();
    assert_eq!(after_detail, before_detail);
    assert_eq!(after_archive_context, before_archive_context);

    let rollback = store
        .rollback_legacy_runtime_shadow(&RollbackLegacyRuntimeShadowInput {
            application_id: run.application_id,
            flow_run_id: Some(run.id),
        })
        .await
        .unwrap();
    assert!(rollback.deleted_shadow_rows >= 3);
    assert_eq!(
        store
            .get_checkpoint(run.id, checkpoint.id)
            .await
            .unwrap()
            .unwrap()
            .variable_snapshot,
        json!({
            "messages": [
                { "role": "user", "content": "keep order" },
                { "role": "assistant", "tool_calls": [{ "id": "call-1", "name": "lookup" }] }
            ],
            "usage": { "input_tokens": 13, "output_tokens": 8 }
        })
    );
}

#[tokio::test]
async fn legacy_shadow_skips_unknown_or_mixed_context_ownership_and_reports_differences() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let (unknown_run, _) = seed_legacy_checkpoint_run(
        &store,
        &seeded,
        datetime!(2026-08-13 13:30:00 UTC),
        json!({ "context_version_id": "not-a-provable-owned-version" }),
    )
    .await;
    let unknown = store
        .convert_legacy_runtime_shadow_batch(&shadow_input(
            unknown_run.id,
            LegacyRuntimeShadowExecution::Apply,
        ))
        .await
        .unwrap();
    assert!(unknown
        .differences
        .iter()
        .any(|difference| { difference.reason == "checkpoint_context_ownership_not_legacy" }));

    let (mixed_run, _) = seed_legacy_checkpoint_run(
        &store,
        &seeded,
        datetime!(2026-08-13 13:31:00 UTC),
        json!({ "node_id": "mixed" }),
    )
    .await;
    let native_content = store
        .put_canonical_runtime_content(&PutCanonicalRuntimeContentInput {
            scope_id: seeded.workspace_id,
            application_id: seeded.application_id,
            content: json!({ "native": true }),
        })
        .await
        .unwrap();
    store
        .append_context_version(&AppendContextVersionInput {
            scope_id: seeded.workspace_id,
            application_id: seeded.application_id,
            flow_run_id: mixed_run.id,
            parent_context_version_id: None,
            sequence: 0,
            transition_kind: ContextTransitionKind::Initial,
            transition_actor: ContextTransitionActor::Host,
            declared_compaction_provenance: None,
            actual_content_id: native_content.id,
        })
        .await
        .unwrap();
    let mixed = store
        .convert_legacy_runtime_shadow_batch(&shadow_input(
            mixed_run.id,
            LegacyRuntimeShadowExecution::Apply,
        ))
        .await
        .unwrap();
    assert!(mixed
        .differences
        .iter()
        .any(|difference| difference.reason == "mixed_context_lineage_ownership"));
}
