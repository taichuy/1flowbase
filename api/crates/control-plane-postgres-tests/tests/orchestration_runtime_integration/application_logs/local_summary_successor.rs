use super::*;
use control_plane_contracts::{
    application_public_runtime::ApplicationPublishedFlowRunRepository,
    ports::ApplicationRunLogContext,
};

fn published_input(
    seeded: &RuntimeSeedState,
    compiled: &domain::CompiledPlanRecord,
    api_key_id: Uuid,
    external_user: &str,
    thread_id: &str,
    turn_id: &str,
    idempotency_key: &str,
) -> CreateFlowRunInput {
    CreateFlowRunInput {
        application_run_log_context: Some(ApplicationRunLogContext {
            identity_status: "identified".into(),
            protocol: Some("openai_responses".into()),
            call_kind: Some("generate".into()),
            request_kind: Some("turn".into()),
            thread_id: Some(thread_id.into()),
            turn_id: Some(turn_id.into()),
            ..Default::default()
        }),
        actor_user_id: seeded.actor_user_id,
        application_id: seeded.application_id,
        flow_id: seeded.flow_id,
        flow_draft_id: seeded.draft_id,
        compiled_plan_id: compiled.id,
        debug_session_id: String::new(),
        flow_schema_version: compiled.schema_version.clone(),
        document_hash: compiled.document_hash.clone(),
        run_mode: FlowRunMode::PublishedApiRun,
        target_node_id: None,
        title: "Codex turn".into(),
        status: FlowRunStatus::Queued,
        input_payload: json!({"node-start":{"query":"Codex turn"}}),
        started_at: OffsetDateTime::now_utc(),
        api_key_id: Some(api_key_id),
        publication_version_id: None,
        assistant_conversation_id: None,
        external_user: Some(external_user.into()),
        external_conversation_id: None,
        external_trace_id: None,
        compatibility_mode: Some("openai-responses-v1".into()),
        idempotency_key: Some(idempotency_key.into()),
    }
}

async fn make_waiting_callback(
    store: &PgControlPlaneStore,
    run: &domain::FlowRunRecord,
) -> (domain::NodeRunRecord, domain::CallbackTaskRecord) {
    let started_at = OffsetDateTime::now_utc();
    let node = store
        .create_node_run(&CreateNodeRunInput {
            flow_run_id: run.id,
            node_id: "node-llm".into(),
            node_type: "llm".into(),
            node_alias: "LLM".into(),
            status: NodeRunStatus::WaitingCallback,
            input_payload: json!({}),
            debug_payload: json!({}),
            started_at,
        })
        .await
        .unwrap();
    store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::WaitingCallback,
            output_payload: json!({}),
            error_payload: None,
            finished_at: None,
        })
        .await
        .unwrap();
    let callback = store
        .create_callback_task(&CreateCallbackTaskInput {
            flow_run_id: run.id,
            node_run_id: node.id,
            callback_kind: "llm_tool_calls".into(),
            request_payload: json!({"tool_calls":[{"id":"call-2050"}]}),
            external_ref_payload: None,
        })
        .await
        .unwrap();
    (node, callback)
}

#[tokio::test]
async fn issue_2050_local_summary_atomically_supersedes_exact_callback_lineage() {
    let database = isolated_database().await;
    let store = PgControlPlaneStore::new(database.connect().await.unwrap());
    run_migrations(store.pool()).await.unwrap();
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let api_key_id = seed_application_api_key(&store, &seeded).await;

    let predecessor_input = published_input(
        &seeded,
        &compiled,
        api_key_id,
        "codex-user",
        "thread-2050",
        "turn-2050",
        "issue-2050-predecessor",
    );
    let predecessor = ApplicationPublishedFlowRunRepository::create_published_flow_run(
        &store,
        &predecessor_input,
    )
    .await
    .unwrap()
    .flow_run;
    let (node, callback) = make_waiting_callback(&store, &predecessor).await;
    let attempt = store
        .record_flow_run_callback_resume_attempt(&RecordFlowRunCallbackResumeAttemptInput {
            flow_run_id: predecessor.id,
            callback_task_id: callback.id,
            source: "responses".into(),
            response_payload: json!({"output":"late"}),
            idempotency_key: "issue-2050-active-attempt".into(),
        })
        .await
        .unwrap()
        .attempt;

    let mut foreign_input = published_input(
        &seeded,
        &compiled,
        api_key_id,
        "codex-user",
        "thread-2050",
        "foreign-turn",
        "issue-2050-foreign",
    );
    let foreign =
        ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &foreign_input)
            .await
            .unwrap()
            .flow_run;
    let (_, foreign_callback) = make_waiting_callback(&store, &foreign).await;

    let mut successor_input = predecessor_input.clone();
    successor_input.idempotency_key = Some("issue-2050-successor".into());
    successor_input.title = "Local summary".into();
    successor_input.started_at += Duration::seconds(1);
    successor_input.application_run_log_context = Some(ApplicationRunLogContext {
        identity_status: "identified".into(),
        protocol: Some("openai_responses".into()),
        call_kind: Some("compact".into()),
        request_kind: Some("compaction".into()),
        thread_id: Some("thread-2050".into()),
        turn_id: Some("turn-2050".into()),
        ..Default::default()
    });
    let successor = ApplicationPublishedFlowRunRepository::create_published_flow_run_superseding_callback_predecessors(
        &store,
        &successor_input,
    )
    .await
    .unwrap();
    let replay = ApplicationPublishedFlowRunRepository::create_published_flow_run_superseding_callback_predecessors(
        &store,
        &successor_input,
    )
    .await
    .unwrap();

    let statuses: (String, String, String, String, String) = sqlx::query_as(
        r#"
        select
          (select status from flow_runs where id=$1),
          (select status from node_runs where id=$2),
          (select status from flow_run_callback_tasks where id=$3),
          (select status from flow_run_callback_resume_attempts where id=$4),
          (select status from flow_runs where id=$5)
        "#,
    )
    .bind(predecessor.id)
    .bind(node.id)
    .bind(callback.id)
    .bind(attempt.id)
    .bind(foreign.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    let reason: Option<String> =
        sqlx::query_scalar("select error_payload->>'reason' from flow_runs where id=$1")
            .bind(predecessor.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let foreign_callback_status: String =
        sqlx::query_scalar("select status from flow_run_callback_tasks where id=$1")
            .bind(foreign_callback.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let late_completion = store
        .complete_callback_task(&CompleteCallbackTaskInput {
            callback_task_id: callback.id,
            response_payload: json!({"output":"too late"}),
            completed_at: OffsetDateTime::now_utc(),
        })
        .await;

    assert_eq!(
        statuses,
        (
            "cancelled".into(),
            "cancelled".into(),
            "cancelled".into(),
            "cancelled".into(),
            "waiting_callback".into(),
        )
    );
    assert_eq!(reason.as_deref(), Some("local_summary_superseded"));
    assert_eq!(foreign_callback_status, "pending");
    assert!(late_completion.is_err(), "late callback must fail closed");
    assert!(successor.created);
    assert!(!replay.created);
    assert_eq!(successor.flow_run.id, replay.flow_run.id);

    foreign_input.idempotency_key = Some("issue-2050-standard-generate".into());
    let standard =
        ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &foreign_input)
            .await
            .unwrap();
    assert!(
        standard.created,
        "ordinary Generate must use the plain create path"
    );
}
