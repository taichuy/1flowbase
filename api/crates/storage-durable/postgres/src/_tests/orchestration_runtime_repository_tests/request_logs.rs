use super::*;

fn request_log(
    scope_id: Uuid,
    attempt_id: Uuid,
    started_at: OffsetDateTime,
    status: &str,
    output_tokens: Option<i64>,
) -> ProviderRequestLogTask {
    ProviderRequestLogTask {
        scope_id,
        attempt_id,
        flow_run_id: Uuid::now_v7(),
        application_id: Some(Uuid::now_v7()),
        conversation_id: Some("conversation-1".into()),
        application_name: "Runtime App Snapshot".into(),
        attempt_index: 1,
        provider_instance_id: Some(Uuid::now_v7()),
        provider_instance_display_name: Some("Provider Snapshot".into()),
        provider_code: "fixture_provider".into(),
        protocol: "openai_compatible".into(),
        upstream_model_id: "gpt-5.4-mini".into(),
        reasoning_effort: Some("high".into()),
        status: status.into(),
        error_code: None,
        failed_after_first_token: false,
        input_tokens: Some(120),
        output_tokens,
        total_tokens: output_tokens.map(|v| v + 120),
        started_at,
        first_token_at: output_tokens
            .filter(|v| *v > 0)
            .map(|_| started_at + Duration::milliseconds(80)),
        finished_at: Some(started_at + Duration::milliseconds(250)),
        time_to_first_token_ms: output_tokens.filter(|v| *v > 0).map(|_| 80),
        total_duration_ms: Some(250),
    }
}

fn query(scope_id: Uuid, page: i64, page_size: i64) -> ListModelProviderRequestLogsPageInput {
    ListModelProviderRequestLogsPageInput {
        scope_id,
        application_name: None,
        provider_instance_id: None,
        model_id: None,
        status: None,
        zero_output_only: false,
        started_after: None,
        started_before: None,
        page,
        page_size,
    }
}

#[tokio::test]
async fn provider_request_logs_batch_insert_is_idempotent_and_queryable() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let scope_id = Uuid::now_v7();
    let attempt_id = Uuid::now_v7();
    let at = datetime!(2026-07-11 10:00:00 UTC);
    let row = request_log(scope_id, attempt_id, at, "empty_response", Some(0));
    store
        .insert_model_provider_request_logs_batch(&[row.clone(), row.clone()])
        .await
        .unwrap();
    let mut input = query(scope_id, 1, 20);
    input.provider_instance_id = row.provider_instance_id;
    input.model_id = Some("gpt-5.4-mini".into());
    input.status = Some("empty_response".into());
    input.zero_output_only = true;
    input.started_after = Some(at - Duration::seconds(1));
    input.started_before = Some(at + Duration::seconds(1));
    let page=<PgControlPlaneStore as OrchestrationRuntimeRepository>::list_model_provider_request_logs_page(&store,input).await.unwrap();
    assert_eq!(page.total_count, 1);
    assert_eq!(page.items[0].attempt_id, attempt_id);
    assert_eq!(page.items[0].application_id, row.application_id);
    assert_eq!(page.items[0].conversation_id, row.conversation_id);
    assert_eq!(page.items[0].application_name, "Runtime App Snapshot");
}

#[tokio::test]
async fn provider_request_logs_scope_filters_and_paginates_flat_records() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let scope = Uuid::now_v7();
    let other = Uuid::now_v7();
    let at = datetime!(2026-07-11 11:00:00 UTC);
    store
        .insert_model_provider_request_logs_batch(&[
            request_log(scope, Uuid::now_v7(), at, "succeeded", Some(8)),
            request_log(
                scope,
                Uuid::now_v7(),
                at + Duration::seconds(1),
                "failed",
                None,
            ),
            request_log(
                other,
                Uuid::now_v7(),
                at + Duration::seconds(2),
                "succeeded",
                Some(9),
            ),
        ])
        .await
        .unwrap();
    let page=<PgControlPlaneStore as OrchestrationRuntimeRepository>::list_model_provider_request_logs_page(&store,query(scope,2,1)).await.unwrap();
    assert_eq!(page.total_count, 2);
    assert_eq!(page.items[0].status, "succeeded");
}

#[tokio::test]
async fn provider_request_logs_do_not_project_existing_attempt_ledgers() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-07-11 12:00:00 UTC);
    let run = seed_flow_run(&store, &seeded, &compiled, started_at).await;
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_model_failover_attempt_ledger(
        &store,
        &AppendModelFailoverAttemptLedgerInput {
            flow_run_id: run.id,
            node_run_id: None,
            llm_turn_span_id: None,
            queue_snapshot_id: None,
            attempt_index: 1,
            is_retry: false,
            retry_reason: None,
            provider_instance_id: None,
            provider_code: "legacy".into(),
            upstream_model_id: "legacy-model".into(),
            protocol: "openai_compatible".into(),
            request_ref: None,
            request_hash: None,
            started_at,
            first_token_at: None,
            finished_at: Some(started_at + Duration::seconds(1)),
            status: "succeeded".into(),
            failed_after_first_token: false,
            upstream_request_id: None,
            error_code: None,
            error_message_ref: None,
            usage_ledger_id: None,
            cost_ledger_id: None,
            response_ref: None,
        },
    )
    .await
    .unwrap();
    let page=<PgControlPlaneStore as OrchestrationRuntimeRepository>::list_model_provider_request_logs_page(&store,query(seeded.workspace_id,1,20)).await.unwrap();
    assert!(page.items.is_empty());
}
