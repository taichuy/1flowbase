use super::*;

#[tokio::test]
async fn agent_logs_native_model_snapshot_keeps_the_native_summary_owner() {
    let db = isolated_database().await;
    let store = PgControlPlaneStore::new(db.connect().await.unwrap());
    run_migrations(store.pool()).await.unwrap();
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        OffsetDateTime::now_utc(),
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    sqlx::query("update application_run_log_summaries set requested_model_id='native-model',reasoning_effort='high' where flow_run_id=$1")
        .bind(run.id).execute(store.pool()).await.unwrap();
    // A direct task write cannot override the native anchor's request snapshot.
    sqlx::query("update application_run_log_tasks set requested_model_id='other-model',reasoning_effort='low' where id=$1")
        .bind(run.id).execute(store.pool()).await.unwrap();
    let fields: (Option<String>, Option<String>) = sqlx::query_as(
        "select requested_model_id,reasoning_effort from application_run_log_tasks where id=$1",
    )
    .bind(run.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(fields, (Some("native-model".into()), Some("high".into())));
}

#[tokio::test]
async fn agent_logs_project_source_model_effort_and_one_compressed_turn() {
    let (store, scope, app) = setup().await;
    let service = AgentLogsService::new(store.clone());
    let key = Uuid::now_v7();
    let mut context = event("context", 4, AgentLogEventKind::Context, None);
    context.model_id = Some("gpt-6.1-sol".into());
    context.reasoning_effort = Some("medium".into());
    let mut inherited = event("inherited", 8, AgentLogEventKind::Context, None);
    inherited.model_id = Some("parent-model".into());
    inherited.reasoning_effort = Some("high".into());
    inherited.inherited = true;
    let mut final_answer = event(
        "final",
        7,
        AgentLogEventKind::Assistant,
        Some("final reply"),
    );
    final_answer.phase = Some("final_answer".into());
    final_answer.model_id = None;
    let mut interim = event(
        "interim",
        5,
        AgentLogEventKind::Assistant,
        Some("progress only"),
    );
    interim.model_id = None;
    let first = service
        .ingest(
            app,
            scope,
            key,
            batch(vec![
                event("system-a", 1, AgentLogEventKind::System, Some("system one")),
                event("system-b", 2, AgentLogEventKind::System, Some("system two")),
                event("user", 3, AgentLogEventKind::User, Some("question")),
                context,
            ]),
        )
        .await
        .unwrap();
    let id = first.record_ids[0];
    // Later transport batches and out-of-order inheritance must not erase source fields.
    service
        .ingest(
            app,
            scope,
            key,
            batch(vec![inherited, final_answer, interim]),
        )
        .await
        .unwrap();
    let fields: (Option<String>, Option<String>) = sqlx::query_as(
        "select requested_model_id,reasoning_effort from application_run_log_tasks where id=$1",
    )
    .bind(id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(fields, (Some("gpt-6.1-sol".into()), Some("medium".into())));
    let record = store
        .application_log_record(app, id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        record
            .messages
            .iter()
            .map(|m| m.role.as_str())
            .collect::<Vec<_>>(),
        vec!["system", "user", "assistant"]
    );
    assert_eq!(record.messages[0].content, "system one\n\nsystem two");
    assert_eq!(record.messages[1].content, "question");
    assert_eq!(record.messages[2].content, "final reply");
    assert_eq!(record.total_tokens, None);
    assert_eq!(record.cost_breakdown.total_cost, None);
    let trajectory = store
        .record_client_trajectory_page(app, id, None, 20)
        .await
        .unwrap();
    assert_eq!(trajectory.items.len(), 7);
    let interim_step = trajectory
        .items
        .iter()
        .find(|s| s.item_id.as_deref() == Some("interim"))
        .unwrap();
    let raw = store
        .record_client_trajectory_section(app, id, interim_step.id, "raw", None, 20)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(raw.items[0].value["content"], "progress only");
}
