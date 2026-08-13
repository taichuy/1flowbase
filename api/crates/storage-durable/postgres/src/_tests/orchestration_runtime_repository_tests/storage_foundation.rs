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

    sqlx::query("alter table runtime_canonical_contents disable trigger runtime_canonical_contents_reject_update")
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query(
        "update runtime_canonical_contents set content = '{\"tampered\":true}' where id = $1",
    )
    .bind(first.id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query("alter table runtime_canonical_contents enable trigger runtime_canonical_contents_reject_update")
        .execute(store.pool())
        .await
        .unwrap();
    let collision = store
        .put_canonical_runtime_content(&PutCanonicalRuntimeContentInput {
            scope_id: seeded.workspace_id,
            application_id: seeded.application_id,
            content: json!({"fixed": {"alpha": 1, "beta": 2}}),
        })
        .await;
    assert!(collision.is_err());
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
async fn provider_invocation_context_tracks_explicit_and_observed_replacement_epochs() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-08-13 12:15:00 UTC);
    let run = seed_flow_run(&store, &seeded, &compiled, started_at).await;
    let first_span = store
        .append_runtime_span(&AppendRuntimeSpanInput {
            flow_run_id: run.id,
            node_run_id: None,
            parent_span_id: None,
            kind: domain::RuntimeSpanKind::LlmTurn,
            name: "epoch-1".into(),
            status: domain::RuntimeSpanStatus::Succeeded,
            capability_id: None,
            input_ref: None,
            output_ref: None,
            error_payload: None,
            metadata: json!({}),
            started_at,
            finished_at: Some(started_at),
        })
        .await
        .unwrap();
    let first = store
        .append_provider_invocation_context(&AppendProviderInvocationContextInput {
            scope_id: seeded.workspace_id,
            application_id: seeded.application_id,
            flow_run_id: run.id,
            invocation_span_id: first_span.id,
            actual_context: json!({
                "effective_system": ["fixed"],
                "provider_messages": [{ "role": "user", "content": "old" }]
            }),
            context_epoch: json!({ "declaration": "unknown" }),
        })
        .await
        .unwrap();
    let second_span = store
        .append_runtime_span(&AppendRuntimeSpanInput {
            flow_run_id: run.id,
            node_run_id: None,
            parent_span_id: None,
            kind: domain::RuntimeSpanKind::LlmTurn,
            name: "epoch-2".into(),
            status: domain::RuntimeSpanStatus::Succeeded,
            capability_id: None,
            input_ref: None,
            output_ref: None,
            error_payload: None,
            metadata: json!({}),
            started_at,
            finished_at: Some(started_at),
        })
        .await
        .unwrap();
    let second = store
        .append_provider_invocation_context(&AppendProviderInvocationContextInput {
            scope_id: seeded.workspace_id,
            application_id: seeded.application_id,
            flow_run_id: run.id,
            invocation_span_id: second_span.id,
            actual_context: json!({
                "effective_system": ["fixed"],
                "provider_messages": [{ "role": "user", "content": "summary" }]
            }),
            context_epoch: json!({ "declaration": "unknown" }),
        })
        .await
        .unwrap();

    assert_eq!(second.parent_context_version_id, Some(first.id));
    assert_eq!(
        second.transition_kind,
        ContextTransitionKind::ObservedReplacement
    );
    assert_eq!(second.transition_actor, ContextTransitionActor::Host);
    assert!(second.declared_compaction_provenance.is_none());

    let third_span = store
        .append_runtime_span(&AppendRuntimeSpanInput {
            flow_run_id: run.id,
            node_run_id: None,
            parent_span_id: None,
            kind: domain::RuntimeSpanKind::LlmTurn,
            name: "epoch-3-identical".into(),
            status: domain::RuntimeSpanStatus::Succeeded,
            capability_id: None,
            input_ref: None,
            output_ref: None,
            error_payload: None,
            metadata: json!({}),
            started_at,
            finished_at: Some(started_at),
        })
        .await
        .unwrap();
    let third = store
        .append_provider_invocation_context(&AppendProviderInvocationContextInput {
            scope_id: seeded.workspace_id,
            application_id: seeded.application_id,
            flow_run_id: run.id,
            invocation_span_id: third_span.id,
            actual_context: json!({
                "effective_system": ["fixed"],
                "provider_messages": [{ "role": "user", "content": "summary" }]
            }),
            context_epoch: json!({ "declaration": "unknown" }),
        })
        .await
        .unwrap();
    assert_eq!(third.actual_content_id, second.actual_content_id);
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
    let mut illegal_transition = recovery_input.clone();
    illegal_transition.sequence = 1;
    illegal_transition.state_code = RecoveryStateCode::Paused;
    illegal_transition.idempotency_key = "waiting-to-paused-is-illegal".into();
    assert!(!RecoveryStateCode::WaitingCallback.allows_transition_to(RecoveryStateCode::Paused));
    assert!(store
        .append_recovery_history(&illegal_transition)
        .await
        .is_err());
    for (status, finished_at) in [
        (FlowRunStatus::Running, None),
        (FlowRunStatus::Paused, None),
        (FlowRunStatus::Failed, Some(OffsetDateTime::now_utc())),
    ] {
        store
            .update_flow_run(&UpdateFlowRunInput {
                flow_run_id: run.id,
                status,
                output_payload: json!({}),
                error_payload: (status == FlowRunStatus::Failed)
                    .then(|| json!({ "message": "fixture" })),
                finished_at,
            })
            .await
            .unwrap();
    }
    let recorded_states = sqlx::query_scalar::<_, String>(
        "select state_code from flow_run_recovery_history where flow_run_id = $1 order by sequence",
    )
    .bind(run.id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(
        recorded_states,
        vec!["waiting_callback", "running", "paused", "failed"]
    );
    let update =
        sqlx::query("update flow_run_recovery_history set event_sequence = 10 where id = $1")
            .bind(history.id)
            .execute(store.pool())
            .await;
    assert!(update.is_err());
}

#[tokio::test]
async fn resume_claim_reacquire_fences_stale_token_generation_and_payload_conflicts() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let run = seed_flow_run(
        &store,
        &seeded,
        &compiled,
        datetime!(2026-08-13 13:30:00 UTC),
    )
    .await;
    sqlx::query("update flow_runs set status = 'waiting_human' where id = $1")
        .bind(run.id)
        .execute(store.pool())
        .await
        .unwrap();
    let checkpoint = store
        .create_checkpoint(&CreateCheckpointInput {
            flow_run_id: run.id,
            node_run_id: None,
            status: "waiting_human".into(),
            reason: "claim fixture".into(),
            locator_payload: json!({}),
            variable_snapshot: json!({}),
            external_ref_payload: None,
        })
        .await
        .unwrap();
    let input = AcquireResumeClaimInput {
        scope_id: seeded.workspace_id,
        application_id: seeded.application_id,
        flow_run_id: run.id,
        checkpoint_id: checkpoint.id,
        callback_task_id: None,
        kind: ResumeClaimKind::Human,
        request_payload: json!({ "answer": "approved" }),
    };
    let first = store.acquire_resume_claim(&input).await.unwrap();
    assert_eq!(first.disposition, ResumeClaimDisposition::Acquired);
    assert_eq!(first.claim.generation, 0);
    let duplicate = store.acquire_resume_claim(&input).await.unwrap();
    assert_eq!(duplicate.disposition, ResumeClaimDisposition::InProgress);
    let mut conflicting = input.clone();
    conflicting.request_payload = json!({ "answer": "different" });
    assert!(store.acquire_resume_claim(&conflicting).await.is_err());

    sqlx::query("update flow_run_resume_claims set lease_expires_at = now() - interval '1 second' where id = $1")
        .bind(first.claim.id)
        .execute(store.pool())
        .await
        .unwrap();
    let reacquired = store.acquire_resume_claim(&input).await.unwrap();
    assert_eq!(reacquired.disposition, ResumeClaimDisposition::Acquired);
    assert_eq!(reacquired.claim.generation, 1);
    assert_ne!(reacquired.claim.claim_token, first.claim.claim_token);

    for (claim_token, expected_generation) in [
        (first.claim.claim_token, first.claim.generation),
        (reacquired.claim.claim_token, first.claim.generation),
    ] {
        assert!(store
            .finish_resume_claim(&FinishResumeClaimInput {
                claim_id: first.claim.id,
                claim_token,
                expected_generation,
                status: ResumeClaimStatus::Succeeded,
                error_payload: None,
                completed_at: OffsetDateTime::now_utc(),
            })
            .await
            .is_err());
    }
    let finished = store
        .finish_resume_claim(&FinishResumeClaimInput {
            claim_id: reacquired.claim.id,
            claim_token: reacquired.claim.claim_token,
            expected_generation: reacquired.claim.generation,
            status: ResumeClaimStatus::Succeeded,
            error_payload: None,
            completed_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    assert_eq!(finished.status, ResumeClaimStatus::Succeeded);

    let other_run = seed_flow_run(
        &store,
        &seeded,
        &compiled,
        datetime!(2026-08-13 13:31:00 UTC),
    )
    .await;
    let unclaimed_checkpoint = store
        .create_checkpoint(&CreateCheckpointInput {
            flow_run_id: run.id,
            node_run_id: None,
            status: "waiting_human".into(),
            reason: "owner chain fixture".into(),
            locator_payload: json!({}),
            variable_snapshot: json!({}),
            external_ref_payload: None,
        })
        .await
        .unwrap();
    let cross_owner = sqlx::query(
        r#"
        insert into flow_run_resume_claims (
            id, scope_id, application_id, flow_run_id, checkpoint_id, callback_task_id,
            resume_kind, status, request_payload, claim_token, lease_expires_at
        ) values ($1, $2, $3, $4, $5, null, 'human', 'processing', '{}', $6, now())
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(seeded.workspace_id)
    .bind(seeded.application_id)
    .bind(other_run.id)
    .bind(unclaimed_checkpoint.id)
    .bind(Uuid::now_v7())
    .execute(store.pool())
    .await;
    assert!(cross_owner.is_err());
}

#[tokio::test]
async fn persist_waiting_state_rolls_back_run_checkpoint_and_event_on_recovery_failure() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-08-13 13:00:00 UTC);
    let run = seed_flow_run(&store, &seeded, &compiled, started_at).await;
    let node_run = seed_node_run(&store, &run, started_at).await;

    let result = store
        .persist_waiting_state(&PersistWaitingStateInput {
            checkpoint_id: Uuid::now_v7(),
            scope_id: seeded.workspace_id,
            application_id: seeded.application_id,
            flow_run_id: run.id,
            node_run_id: node_run.id,
            expected_status: FlowRunStatus::Running,
            output_payload: json!({ "status": "waiting" }),
            checkpoint_status: "waiting_human".into(),
            checkpoint_reason: "human_input".into(),
            locator_payload: json!({
                "node_id": "node-llm",
                "next_node_index": 1,
                "active_node_ids": ["node-llm"]
            }),
            variable_snapshot: json!({}),
            checkpoint_external_ref_payload: None,
            context_content: json!({ "format": "runtime_snapshot_v1", "variable_pool": {} }),
            parent_context_version_id: Some(Uuid::now_v7()),
            context_transition_kind: ContextTransitionKind::Append,
            recovery_idempotency_key: "rollback-invalid-context".into(),
            resume_claim_id: None,
            resume_claim_token: None,
            waiting_event: AppendRuntimeEventInput {
                flow_run_id: run.id,
                node_run_id: Some(node_run.id),
                span_id: None,
                parent_span_id: None,
                event_type: "flow.waiting_human".into(),
                layer: domain::RuntimeEventLayer::AgentTransition,
                source: domain::RuntimeEventSource::Host,
                trust_level: domain::RuntimeTrustLevel::HostFact,
                item_id: None,
                ledger_ref: None,
                payload: json!({ "status": "waiting_human" }),
                visibility: domain::RuntimeEventVisibility::Workspace,
                durability: domain::RuntimeEventDurability::Durable,
            },
            kind: PersistWaitingKind::Human,
        })
        .await;

    assert!(result.is_err());
    let status: String = sqlx::query_scalar("select status from flow_runs where id = $1")
        .bind(run.id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    let checkpoint_count: i64 =
        sqlx::query_scalar("select count(*) from flow_run_checkpoints where flow_run_id = $1")
            .bind(run.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let event_count: i64 =
        sqlx::query_scalar("select count(*) from runtime_events where flow_run_id = $1")
            .bind(run.id)
            .fetch_one(store.pool())
            .await
            .unwrap();

    assert_eq!(status, FlowRunStatus::Running.as_str());
    assert_eq!(checkpoint_count, 0);
    assert_eq!(event_count, 0);
}
