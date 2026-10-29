use super::*;
use control_plane_contracts::ports::{
    AppendContextProjectionInput, ContainerObservationInput, ContainerObservationOperation as Op,
};

#[tokio::test]
async fn scoped_observation_resume_reuses_span_context_attempt_and_usage_before_clearing_payload() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let run = seed_flow_run(&store, &seeded, &compiled, OffsetDateTime::now_utc()).await;
    let started = super::container_commits::start(run.id, Uuid::now_v7(), 0);
    let node = store
        .commit_container_transition(&started)
        .await
        .unwrap()
        .unwrap();
    let mut completed = super::container_commits::complete(started);
    completed.observation = Some(ContainerObservationInput {
        observation_key: "terminal-trace".into(),
        trace_payload: json!({"context":"must remain until all writes finish"}),
    });
    store.commit_container_transition(&completed).await.unwrap();
    let pending = store
        .list_pending_container_observations(run.id, None, 1)
        .await
        .unwrap();
    assert_eq!(pending.len(), 1);
    let observation = pending[0].id;
    let span_id = Uuid::now_v7();
    let span = Op::Span(AppendRuntimeSpanInput {
        flow_run_id: run.id,
        node_run_id: Some(node.id),
        parent_span_id: None,
        kind: domain::RuntimeSpanKind::LlmTurn,
        name: "scoped LLM".into(),
        status: domain::RuntimeSpanStatus::Running,
        capability_id: None,
        input_ref: None,
        output_ref: None,
        error_payload: None,
        metadata: json!({}),
        started_at: node.started_at,
        finished_at: None,
    });
    store
        .apply_container_observation(observation, span_id, &span)
        .await
        .unwrap();
    let context_input = AppendProviderInvocationContextInput {
        scope_id: seeded.workspace_id,
        application_id: seeded.application_id,
        flow_run_id: run.id,
        invocation_span_id: span_id,
        actual_context: json!({"provider_messages":[{"role":"user","content":"hello"}]}),
        context_epoch: json!({"declaration":"unknown"}),
    };
    let context = store
        .append_provider_invocation_context(&context_input)
        .await
        .unwrap();
    let projection_id = Uuid::now_v7();
    let projection = Op::ContextProjection(AppendContextProjectionInput {
        flow_run_id: run.id,
        node_run_id: Some(node.id),
        llm_turn_span_id: Some(span_id),
        projection_kind: "managed_full".into(),
        merge_stage_ref: None,
        source_transcript_ref: None,
        source_item_refs: json!([]),
        compaction_event_id: None,
        summary_version: None,
        model_input_ref: "runtime_artifact:inline:fixture".into(),
        model_input_hash: "fixture".into(),
        compacted_summary_ref: None,
        previous_projection_id: None,
        token_estimate: Some(1),
        provider_continuation_metadata: json!({"context_version_id":context.id}),
    });
    store
        .apply_container_observation(observation, projection_id, &projection)
        .await
        .unwrap();
    let attempt_id = Uuid::now_v7();
    let attempt = Op::Attempt(AppendModelFailoverAttemptLedgerInput {
        flow_run_id: run.id,
        node_run_id: Some(node.id),
        llm_turn_span_id: Some(span_id),
        queue_snapshot_id: None,
        attempt_index: 0,
        provider_instance_id: None,
        provider_code: "fixture".into(),
        upstream_model_id: "model".into(),
        protocol: "openai".into(),
        request_ref: None,
        request_hash: None,
        started_at: node.started_at,
        first_token_at: None,
        finished_at: Some(OffsetDateTime::now_utc()),
        status: "succeeded".into(),
        failed_after_first_token: false,
        upstream_request_id: None,
        error_code: None,
        error_message_ref: None,
        usage_ledger_id: None,
        cost_ledger_id: None,
        response_ref: None,
    });
    store
        .apply_container_observation(observation, attempt_id, &attempt)
        .await
        .unwrap();
    let usage_id = Uuid::now_v7();
    let usage = Op::Usage(AppendUsageLedgerInput {
        flow_run_id: run.id,
        node_run_id: Some(node.id),
        span_id: Some(span_id),
        failover_attempt_id: Some(attempt_id),
        provider_instance_id: None,
        gateway_route_id: None,
        model_id: Some("model".into()),
        upstream_model_id: Some("model".into()),
        upstream_request_id: None,
        input_tokens: Some(3),
        cached_input_tokens: None,
        output_tokens: Some(2),
        reasoning_output_tokens: None,
        total_tokens: Some(5),
        input_cache_hit_tokens: None,
        input_cache_miss_tokens: None,
        cache_read_tokens: None,
        cache_write_tokens: None,
        price_snapshot: None,
        cost_snapshot: None,
        usage_status: domain::UsageLedgerStatus::Recorded,
        raw_usage: json!({"input_tokens":3}),
        normalized_usage: json!({"input_tokens":3}),
    });
    store
        .apply_container_observation(observation, usage_id, &usage)
        .await
        .unwrap();
    // Crash point: usage exists, but its link and the receipt completion do not.
    // Re-run all ordinary observation builders with the same scoped write IDs.
    for (id, operation) in [
        (span_id, &span),
        (projection_id, &projection),
        (attempt_id, &attempt),
        (usage_id, &usage),
    ] {
        store
            .apply_container_observation(observation, id, operation)
            .await
            .unwrap();
    }
    assert_eq!(
        store
            .append_provider_invocation_context(&context_input)
            .await
            .unwrap()
            .id,
        context.id
    );
    store
        .link_usage_ledger_to_model_failover_attempt(&LinkUsageLedgerToModelFailoverAttemptInput {
            failover_attempt_id: attempt_id,
            usage_ledger_id: usage_id,
        })
        .await
        .unwrap();
    // A committed trace can be reconciled after global failure/cancellation,
    // without reopening ordinary event admission or changing business status.
    sqlx::query("update flow_runs set status='failed' where id=$1")
        .bind(run.id)
        .execute(store.pool())
        .await
        .unwrap();
    let event_input = AppendRunEventInput {
        flow_run_id: run.id,
        node_run_id: Some(node.id),
        event_type: "provider_usage_recorded".into(),
        payload: json!({"tokens":5}),
    };
    assert!(store.append_run_event(&event_input).await.is_err());
    let event_id = Uuid::now_v7();
    store
        .apply_container_observation(observation, event_id, &Op::RunEvent(event_input.clone()))
        .await
        .unwrap();
    store
        .apply_container_observation(observation, event_id, &Op::RunEvent(event_input))
        .await
        .unwrap();
    let counts:(i64,i64,i64,i64)=sqlx::query_as("select (select count(*) from runtime_spans where node_run_id=$1),(select count(*) from runtime_context_projections where llm_turn_span_id=$2),(select count(*) from model_failover_attempt_ledger where node_run_id=$1),(select count(*) from runtime_usage_ledger where node_run_id=$1)")
        .bind(node.id).bind(span_id).fetch_one(store.pool()).await.unwrap();
    assert_eq!(counts, (1, 1, 1, 1));
    sqlx::query("create function reject_observation_completion() returns trigger language plpgsql as $$ begin raise exception 'injected receipt failure'; end $$").execute(store.pool()).await.unwrap();
    sqlx::query("create trigger reject_observation_completion before update on flow_container_observations for each row execute function reject_observation_completion()").execute(store.pool()).await.unwrap();
    let refs =
        json!({"context_projection_ref":format!("runtime_context_projection:{projection_id}")});
    assert!(store
        .finish_container_observation(observation, &refs)
        .await
        .is_err());
    assert_eq!(
        store
            .list_pending_container_observations(run.id, None, 1)
            .await
            .unwrap()
            .len(),
        1
    );
    let unchanged = store
        .get_container_node_run(run.id, completed.execution_identity.execution_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unchanged.status, NodeRunStatus::Succeeded);
    assert!(unchanged
        .debug_payload
        .get("context_projection_ref")
        .is_none());
    sqlx::query("drop trigger reject_observation_completion on flow_container_observations")
        .execute(store.pool())
        .await
        .unwrap();
    store
        .finish_container_observation(observation, &refs)
        .await
        .unwrap();
    store
        .finish_container_observation(observation, &refs)
        .await
        .unwrap();
    assert!(store
        .list_pending_container_observations(run.id, None, 1)
        .await
        .unwrap()
        .is_empty());
    let cleared:bool=sqlx::query_scalar("select trace_payload is null and raw_json_payloads='{}' and completed_at is not null from flow_container_observations where id=$1").bind(observation).fetch_one(store.pool()).await.unwrap();
    assert!(cleared);
    let finished = store
        .get_container_node_run(run.id, completed.execution_identity.execution_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(finished.status, NodeRunStatus::Succeeded);
    assert_eq!(finished.output_payload, json!({"result":"committed"}));
    assert_eq!(
        finished.debug_payload["context_projection_ref"],
        refs["context_projection_ref"]
    );
}
