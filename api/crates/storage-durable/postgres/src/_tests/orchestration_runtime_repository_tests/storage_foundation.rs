use super::*;

#[tokio::test]
async fn canonical_content_deduplicates_canonical_json_within_application() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;

    let first = store
        .put_canonical_runtime_content(&PutCanonicalRuntimeContentInput {
            scope_id: seeded.workspace_id,
            application_id: seeded.application_id,
            content: json!({"fixed": {"alpha": 1, "beta": 2}}),
        })
        .await
        .unwrap();
    let reordered = store
        .put_canonical_runtime_content(&PutCanonicalRuntimeContentInput {
            scope_id: seeded.workspace_id,
            application_id: seeded.application_id,
            content: json!({"fixed": {"beta": 2, "alpha": 1}}),
        })
        .await
        .unwrap();

    assert_eq!(first.id, reordered.id);
    assert_eq!(first.content_hash, reordered.content_hash);
    assert_eq!(first.byte_size, 30);
    let count: i64 = sqlx::query_scalar(
        "select count(*) from runtime_canonical_contents where application_id = $1",
    )
    .bind(seeded.application_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(count, 1);
    let update = sqlx::query("update runtime_canonical_contents set content = '{}' where id = $1")
        .bind(first.id)
        .execute(store.pool())
        .await;
    assert!(update.is_err());
}

#[tokio::test]
async fn context_versions_reuse_projection_lineage_and_bind_runtime_span_invocation() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-08-13 12:00:00 UTC);
    let run = seed_flow_run(&store, &seeded, &compiled, started_at).await;
    let content = store
        .put_canonical_runtime_content(&PutCanonicalRuntimeContentInput {
            scope_id: seeded.workspace_id,
            application_id: seeded.application_id,
            content: json!({"messages": [{"role": "user", "content": "hello"}]}),
        })
        .await
        .unwrap();
    let initial = store
        .append_context_version(&AppendContextVersionInput {
            scope_id: seeded.workspace_id,
            application_id: seeded.application_id,
            flow_run_id: run.id,
            parent_context_version_id: None,
            sequence: 0,
            transition_kind: ContextTransitionKind::Initial,
            transition_actor: ContextTransitionActor::Host,
            declared_compaction_provenance: None,
            actual_content_id: content.id,
        })
        .await
        .unwrap();
    let compacted = store
        .append_context_version(&AppendContextVersionInput {
            scope_id: seeded.workspace_id,
            application_id: seeded.application_id,
            flow_run_id: run.id,
            parent_context_version_id: Some(initial.id),
            sequence: 1,
            transition_kind: ContextTransitionKind::DeclaredCompaction,
            transition_actor: ContextTransitionActor::Client,
            declared_compaction_provenance: Some(
                json!({"source": "client", "request_id": "req-1"}),
            ),
            actual_content_id: content.id,
        })
        .await
        .unwrap();
    let span = store
        .append_runtime_span(&AppendRuntimeSpanInput {
            flow_run_id: run.id,
            node_run_id: None,
            parent_span_id: None,
            kind: domain::RuntimeSpanKind::LlmTurn,
            name: "invocation".into(),
            status: domain::RuntimeSpanStatus::Running,
            capability_id: None,
            input_ref: None,
            output_ref: None,
            error_payload: None,
            metadata: json!({}),
            started_at,
            finished_at: None,
        })
        .await
        .unwrap();
    let binding = store
        .bind_invocation_context(&BindInvocationContextInput {
            invocation_span_id: span.id,
            scope_id: seeded.workspace_id,
            application_id: seeded.application_id,
            flow_run_id: run.id,
            context_version_id: compacted.id,
        })
        .await
        .unwrap();

    assert_eq!(compacted.parent_context_version_id, Some(initial.id));
    assert_eq!(compacted.actual_content_id, initial.actual_content_id);
    assert_eq!(binding.invocation_span_id, span.id);
    assert_eq!(binding.context_version_id, compacted.id);
}

#[tokio::test]
async fn recovery_history_is_append_only_and_coordinates_do_not_embed_content() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let run = seed_flow_run(
        &store,
        &seeded,
        &compiled,
        datetime!(2026-08-13 12:30:00 UTC),
    )
    .await;
    let content = store
        .put_canonical_runtime_content(&PutCanonicalRuntimeContentInput {
            scope_id: seeded.workspace_id,
            application_id: seeded.application_id,
            content: json!({"fixed_context": "stored_once"}),
        })
        .await
        .unwrap();
    let context = store
        .append_context_version(&AppendContextVersionInput {
            scope_id: seeded.workspace_id,
            application_id: seeded.application_id,
            flow_run_id: run.id,
            parent_context_version_id: None,
            sequence: 0,
            transition_kind: ContextTransitionKind::Initial,
            transition_actor: ContextTransitionActor::Host,
            declared_compaction_provenance: None,
            actual_content_id: content.id,
        })
        .await
        .unwrap();
    let recovery_input = AppendRecoveryHistoryInput {
        scope_id: seeded.workspace_id,
        application_id: seeded.application_id,
        flow_run_id: run.id,
        node_run_id: None,
        sequence: 0,
        state_code: RecoveryStateCode::WaitingCallback,
        coordinate: RecoveryCoordinate {
            node_sequence: 3,
            iteration_index: 2,
            attempt_index: 1,
            resume_sequence: 4,
            event_sequence: 9,
        },
        context_version_id: context.id,
        recovery_content_id: Some(content.id),
        idempotency_key: "wait-callback-1".into(),
    };
    let history = store
        .append_recovery_history(&recovery_input)
        .await
        .unwrap();
    let replay = store
        .append_recovery_history(&recovery_input)
        .await
        .unwrap();

    assert_eq!(history.id, replay.id);
    assert_eq!(history.coordinate.resume_sequence, 4);
    assert_eq!(history.recovery_content_id, Some(content.id));
    let mut invalid_coordinate = recovery_input.clone();
    invalid_coordinate.sequence = 1;
    invalid_coordinate.idempotency_key = "negative-attempt".into();
    invalid_coordinate.coordinate.attempt_index = -1;
    assert!(store
        .append_recovery_history(&invalid_coordinate)
        .await
        .is_err());
    let update =
        sqlx::query("update flow_run_recovery_history set event_sequence = 10 where id = $1")
            .bind(history.id)
            .execute(store.pool())
            .await;
    assert!(update.is_err());
}
