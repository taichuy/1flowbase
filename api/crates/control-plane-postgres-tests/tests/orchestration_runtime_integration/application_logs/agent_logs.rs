use super::*;
use control_plane::agent_logs::AgentLogsService;
use control_plane_contracts::ports::*;

fn event(id: &str, sequence: i64, kind: AgentLogEventKind, content: Option<&str>) -> AgentLogEvent {
    AgentLogEvent {
        event_id: id.into(),
        source_session_id: "session".into(),
        source_task_id: "actual-turn".into(),
        parent_source_task_id: None,
        sequence,
        occurred_at: "2026-10-07T08:00:00Z".into(),
        kind,
        content: content.map(Into::into),
        phase: None,
        name: None,
        call_id: None,
        model_id: Some("unknown-model".into()),
        reasoning_effort: None,
        provider_code: Some("unknown-provider".into()),
        usage: None,
        inherited: false,
        raw: json!({"original":id}),
    }
}
fn batch(events: Vec<AgentLogEvent>) -> AgentLogsBatch {
    AgentLogsBatch {
        schema_version: AGENT_LOGS_SCHEMA_VERSION.into(),
        source_id: "collector".into(),
        source_client: "codex".into(),
        events,
    }
}

#[path = "agent_logs/field_projection.rs"]
mod field_projection;
#[path = "agent_logs/deletion.rs"]
mod deletion;
#[path = "agent_logs/bulk_lifecycle.rs"]
mod bulk_lifecycle;
#[path = "agent_logs/pricing.rs"]
mod pricing;
async fn setup() -> (PgControlPlaneStore, Uuid, Uuid) {
    let db = isolated_database().await;
    let store = PgControlPlaneStore::new(db.connect().await.unwrap());
    run_migrations(store.pool()).await.unwrap();
    let scope = seed_workspace(&store, "agent-logs").await;
    let owner = seed_user(&store, scope, "collector-owner").await;
    let app = store
        .create_application(&CreateApplicationInput {
            actor_user_id: owner,
            workspace_id: scope,
            application_type: ApplicationType::AgentLogs,
            workflow_trigger_type: None,
            workflow_trigger_config: None,
            name: "Agent logs".into(),
            description: "fixture".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();
    (store, scope, app.id)
}
async fn owned_log_counts(
    store: &PgControlPlaneStore,
    application_id: Uuid,
    record_ids: &[Uuid],
    flow_ids: &[Uuid],
) -> Vec<i64> {
    // Retain the original ownership IDs so deleted parents cannot hide orphaned directories.
    sqlx::query_scalar(
        r#"select array[
            (select count(*) from applications where id=$1),
            (select count(*) from flow_runs where application_id=$1),
            (select count(*) from application_run_log_summaries where application_id=$1),
            (select count(*) from application_run_log_tasks where application_id=$1),
            (select count(*) from application_run_conversation_message_items where application_id=$1),
            (select count(*) from client_trajectory_captures where record_id=any($2) or flow_run_id=any($3)),
            (select count(*) from client_trajectory_steps where record_id=any($2) or flow_run_id=any($3)),
            (select count(*) from client_trajectory_sections where record_id=any($2) or flow_run_id=any($3)),
            (select count(*) from application_log_upload_receipts where application_id=$1),
            (select count(*) from runtime_canonical_contents where application_id=$1),
            (select count(*) from runtime_observation_body_ownership o join runtime_canonical_contents c on c.id=o.content_id where c.application_id=$1)
        ]"#,
    )
    .bind(application_id)
    .bind(record_ids)
    .bind(flow_ids)
    .fetch_one(store.pool())
    .await
    .unwrap()
}

#[tokio::test]
async fn agent_logs_application_delete_cascades_owned_facts_and_preserves_foreign_scope() {
    let (store, scope, app) = setup().await;
    let owner: Uuid = sqlx::query_scalar("select created_by from applications where id=$1")
        .bind(app)
        .fetch_one(store.pool())
        .await
        .unwrap();
    let foreign_app = store
        .create_application(&CreateApplicationInput {
            actor_user_id: owner,
            workspace_id: scope,
            application_type: ApplicationType::AgentLogs,
            workflow_trigger_type: None,
            workflow_trigger_config: None,
            name: "Foreign identical source".into(),
            description: "fixture".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();
    let mut completion = event(
        "final",
        4,
        AgentLogEventKind::TaskEnd,
        Some("declared final"),
    );
    completion.phase = Some("final_answer".into());
    let input = batch(vec![
        event(
            "system",
            1,
            AgentLogEventKind::System,
            Some("effective system"),
        ),
        event("user", 2, AgentLogEventKind::User, Some("question")),
        event(
            "tool",
            3,
            AgentLogEventKind::ToolResult,
            Some("observed tool output"),
        ),
        completion.clone(),
    ]);
    let service = AgentLogsService::new(store.clone());
    let id = service
        .ingest(app, scope, Uuid::now_v7(), input.clone())
        .await
        .unwrap()
        .record_ids[0];
    let foreign_id = service
        .ingest(foreign_app.id, scope, Uuid::now_v7(), input)
        .await
        .unwrap()
        .record_ids[0];
    assert_ne!(id, foreign_id);
    let expected = vec![1, 0, 0, 1, 3, 1, 4, 12, 4, 4, 4];
    assert_eq!(owned_log_counts(&store, app, &[id], &[]).await, expected);
    let foreign_before = owned_log_counts(&store, foreign_app.id, &[foreign_id], &[]).await;
    assert_eq!(foreign_before, expected);

    let wrong_scope = seed_workspace(&store, "wrong-delete-workspace").await;
    let wrong = store
        .delete_application(&DeleteApplicationInput {
            actor_user_id: owner,
            workspace_id: wrong_scope,
            application_id: app,
        })
        .await
        .unwrap_err();
    assert!(matches!(
        wrong.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::NotFound("application"))
    ));
    assert_eq!(owned_log_counts(&store, app, &[id], &[]).await, expected);
    assert_eq!(
        owned_log_counts(&store, foreign_app.id, &[foreign_id], &[]).await,
        foreign_before
    );

    let command = DeleteApplicationInput {
        actor_user_id: owner,
        workspace_id: scope,
        application_id: app,
    };
    store.delete_application(&command).await.unwrap();
    assert_eq!(owned_log_counts(&store, app, &[id], &[]).await, vec![0; 11]);
    assert!(store
        .application_log_record(app, id)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        owned_log_counts(&store, foreign_app.id, &[foreign_id], &[]).await,
        foreign_before
    );
    let foreign = store
        .application_log_record(foreign_app.id, foreign_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(foreign.messages.len(), 3);
    assert_eq!(foreign.messages.last().unwrap().content, "declared final");
    let page = store
        .record_client_trajectory_page(foreign_app.id, foreign_id, None, 8)
        .await
        .unwrap();
    assert_eq!(page.items.len(), 4);
    let raw = store
        .record_client_trajectory_section(
            foreign_app.id,
            foreign_id,
            page.items.last().unwrap().id,
            "raw",
            None,
            8,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        raw.items[0].value,
        serde_json::to_value(completion).unwrap()
    );
    let repeated = store.delete_application(&command).await.unwrap_err();
    assert!(matches!(
        repeated.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::NotFound("application"))
    ));
    assert_eq!(
        owned_log_counts(&store, foreign_app.id, &[foreign_id], &[]).await,
        foreign_before
    );
}

#[tokio::test]
async fn agent_logs_application_delete_preserves_native_flow_first_canonical_cleanup() {
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
    let request_id = Uuid::now_v7();
    let step_id = Uuid::now_v7();
    let at = "2026-10-07T08:00:00Z";
    let step = ClientTrajectoryStep {
        id: step_id,
        request_id,
        sequence: 1,
        created_at: at.into(),
        category: "assistant".into(),
        name: "Native observed assistant".into(),
        namespace: None,
        preview: "Native final".into(),
        parameters_preview: None,
        result_preview: None,
        status: "observed".into(),
        origin: "native".into(),
        protocol: "responses".into(),
        transport: ClientTrajectoryTransport::Http,
        flow_run_id: Some(run.id),
        node_run_id: None,
        parent_id: None,
        call_id: None,
        item_id: Some("native-final".into()),
        response_id: Some("native-response".into()),
        turn_id: None,
        related_step_id: None,
        available_sections: vec!["result".into()],
    };
    for fact in [
        ClientTrajectoryFact::Integrity {
            status: "pending".into(),
            dropped_count: 0,
            persist_failed_count: 0,
        },
        ClientTrajectoryFact::Step {
            step: Box::new(step),
        },
        ClientTrajectoryFact::Section {
            step_id,
            section: "result".into(),
            value: json!({"content": "Native final"}),
        },
    ] {
        store
            .append_client_trajectory(&AppendClientTrajectoryInput {
                flow_run_id: run.id,
                node_run_id: None,
                request_id,
                observed_at: at.into(),
                fact,
            })
            .await
            .unwrap();
    }
    let before = owned_log_counts(&store, seeded.application_id, &[run.id], &[run.id]).await;
    for index in [0, 1, 2, 3, 5, 6, 7, 9] {
        assert!(
            before[index] > 0,
            "Native ownership precondition {index}: {before:?}"
        );
    }
    store
        .delete_application(&DeleteApplicationInput {
            actor_user_id: seeded.actor_user_id,
            workspace_id: seeded.workspace_id,
            application_id: seeded.application_id,
        })
        .await
        .unwrap();
    assert_eq!(
        owned_log_counts(&store, seeded.application_id, &[run.id], &[run.id]).await,
        vec![0; 11]
    );
}

#[tokio::test]
async fn agent_logs_atomic_replay_conflict_no_flow_no_credit_three_layers_and_usage_basis() {
    let (store, scope, app) = setup().await;
    let key = Uuid::now_v7();
    let service = AgentLogsService::new(store.clone());
    let mut system = event(
        "system",
        1,
        AgentLogEventKind::System,
        Some("effective system"),
    );
    system.inherited = true;
    let user = event("user", 2, AgentLogEventKind::User, Some("current question"));
    let interim = event(
        "interim",
        3,
        AgentLogEventKind::Assistant,
        Some("private progress"),
    );
    let mut final_answer = event(
        "final",
        4,
        AgentLogEventKind::Assistant,
        Some("final answer"),
    );
    final_answer.phase = Some("final_answer".into());
    let mut usage = event("usage", 5, AgentLogEventKind::Usage, None);
    usage.usage = Some(AgentLogUsage {
        basis: AgentLogUsageBasis::Delta,
        response_id: Some("response".into()),
        input_tokens: Some(5),
        output_tokens: Some(7),
        input_cache_hit_tokens: Some(0),
        cache_write_tokens: Some(0),
        total_tokens: Some(12),
    });
    let mut snapshot = usage.clone();
    snapshot.event_id = "snapshot".into();
    snapshot.sequence = 6;
    snapshot.usage.as_mut().unwrap().basis = AgentLogUsageBasis::Cumulative;
    snapshot.usage.as_mut().unwrap().total_tokens = Some(999);
    let input = batch(vec![system, user, interim, final_answer, usage, snapshot]);
    let before_flow: i64 = sqlx::query_scalar("select count(*) from flow_runs")
        .fetch_one(store.pool())
        .await
        .unwrap();
    let before_credit: i64 = sqlx::query_scalar("select count(*) from runtime_credit_ledger")
        .fetch_one(store.pool())
        .await
        .unwrap();
    let receipt = service
        .ingest(app, scope, key, input.clone())
        .await
        .unwrap();
    assert_eq!(receipt.accepted_events, 6);
    let id = receipt.record_ids[0];
    let record = store
        .application_log_record(app, id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.native_run_id, None);
    assert_eq!(record.total_tokens, Some(12));
    assert_eq!(record.cost_breakdown.total_cost.as_deref(), Some("0"));
    assert_eq!(
        record
            .messages
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>(),
        vec!["effective system", "current question", "final answer"]
    );
    let page = store
        .record_client_trajectory_page(app, id, None, 50)
        .await
        .unwrap();
    assert_eq!(page.items.len(), 6);
    assert!(page
        .items
        .iter()
        .all(|s| s.flow_run_id.is_none() && s.transport == ClientTrajectoryTransport::File));
    let interim_step = page
        .items
        .iter()
        .find(|s| s.item_id.as_deref() == Some("interim"))
        .unwrap();
    let body = store
        .record_client_trajectory_section(app, id, interim_step.id, "result", None, 8)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(body.items[0].value, json!("private progress"));
    let replay = service
        .ingest(app, scope, key, input.clone())
        .await
        .unwrap();
    assert_eq!(replay.accepted_events, 0);
    assert_eq!(replay.duplicate_events, 6);
    let mut conflict = input.events[0].clone();
    conflict.content = Some("different".into());
    let mut earlier = event(
        "must-rollback",
        0,
        AgentLogEventKind::Context,
        Some("rollback"),
    );
    earlier.source_task_id = "other-turn".into();
    assert!(service
        .ingest(app, scope, key, batch(vec![earlier, conflict]))
        .await
        .unwrap_err()
        .to_string()
        .contains("agent_logs_event_conflict"));
    let count: i64 = sqlx::query_scalar(
        "select count(*) from application_log_upload_receipts where application_id=$1",
    )
    .bind(app)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(count, 6);
    assert!(store
        .application_log_record(Uuid::now_v7(), id)
        .await
        .unwrap()
        .is_none());
    assert!(store
        .record_client_trajectory_section(Uuid::now_v7(), id, interim_step.id, "raw", None, 8)
        .await
        .unwrap()
        .is_none());
    assert!(service
        .ingest(app, Uuid::now_v7(), key, input)
        .await
        .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from flow_runs")
            .fetch_one(store.pool())
            .await
            .unwrap(),
        before_flow
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from runtime_credit_ledger")
            .fetch_one(store.pool())
            .await
            .unwrap(),
        before_credit
    );
}
#[tokio::test]
async fn agent_logs_partial_usage_and_out_of_order_final_are_not_fabricated() {
    let (store, scope, app) = setup().await;
    let service = AgentLogsService::new(store.clone());
    let key = Uuid::now_v7();
    let mut sparse = event("sparse", 4, AgentLogEventKind::Usage, None);
    sparse.usage = Some(AgentLogUsage {
        basis: AgentLogUsageBasis::Cumulative,
        response_id: Some("known-response".into()),
        total_tokens: Some(20),
        input_tokens: None,
        output_tokens: None,
        input_cache_hit_tokens: Some(4),
        cache_write_tokens: None,
    });
    let id = service
        .ingest(app, scope, key, batch(vec![sparse]))
        .await
        .unwrap()
        .record_ids[0];
    let r = store
        .application_log_record(app, id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(r.total_tokens, Some(20));
    assert_eq!(r.input_tokens, None);
    assert_eq!(r.output_tokens, None);
    assert_eq!(r.input_cache_hit_tokens, Some(4));
    assert!(r.messages.is_empty());
    let mut newer = event(
        "new-final",
        10,
        AgentLogEventKind::Assistant,
        Some("new final"),
    );
    newer.phase = Some("final_answer".into());
    let mut older = newer.clone();
    older.event_id = "old-final".into();
    older.sequence = 8;
    older.content = Some("old final".into());
    service
        .ingest(app, scope, key, batch(vec![newer]))
        .await
        .unwrap();
    service
        .ingest(app, scope, key, batch(vec![older]))
        .await
        .unwrap();
    let r = store
        .application_log_record(app, id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(r.messages.last().unwrap().content, "new final");
    assert_eq!(r.total_tokens, Some(20));
}

#[tokio::test]
async fn agent_logs_completion_final_is_single_replay_safe_and_sequence_ordered() {
    let (store, scope, app) = setup().await;
    let service = AgentLogsService::new(store.clone());
    let key = Uuid::now_v7();
    let mut completion = event(
        "completion-final",
        10,
        AgentLogEventKind::TaskEnd,
        Some("declared final"),
    );
    completion.phase = Some("final_answer".into());
    let id = service
        .ingest(app, scope, key, batch(vec![completion.clone()]))
        .await
        .unwrap()
        .record_ids[0];
    let completion_only = store
        .application_log_record(app, id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completion_only.outcome, "final_answer_observed");
    assert_eq!(completion_only.messages.len(), 1);
    assert_eq!(completion_only.messages[0].role, "assistant");
    assert_eq!(completion_only.messages[0].content, "declared final");
    assert_eq!(completion_only.messages[0].sequence, 10);

    let user = event("question", 1, AgentLogEventKind::User, Some("question"));
    let ordinary = event(
        "ordinary",
        2,
        AgentLogEventKind::Assistant,
        Some("progress"),
    );
    let mut overlap = event(
        "assistant-final",
        8,
        AgentLogEventKind::Assistant,
        Some("declared final"),
    );
    overlap.phase = Some("final_answer".into());
    let mut older_completion = completion.clone();
    older_completion.event_id = "older-completion".into();
    older_completion.sequence = 3;
    older_completion.content = Some("older final".into());
    service
        .ingest(
            app,
            scope,
            key,
            batch(vec![overlap.clone(), ordinary.clone(), user.clone()]),
        )
        .await
        .unwrap();
    service
        .ingest(app, scope, key, batch(vec![older_completion.clone()]))
        .await
        .unwrap();
    let replay = service
        .ingest(
            app,
            scope,
            key,
            batch(vec![
                completion.clone(),
                older_completion,
                overlap,
                ordinary,
                user,
            ]),
        )
        .await
        .unwrap();
    assert_eq!(replay.accepted_events, 0);
    assert_eq!(replay.duplicate_events, 5);
    let record = store
        .application_log_record(app, id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.outcome, "final_answer_observed");
    assert_eq!(record.messages.len(), 2);
    assert_eq!(record.messages[0].role, "user");
    assert_eq!(record.messages[0].content, "question");
    assert_eq!(record.messages[1].role, "assistant");
    assert_eq!(record.messages[1].content, "declared final");
    assert_eq!(record.messages[1].sequence, 10);
    let task: (String, Option<String>) =
        sqlx::query_as("select status,final_output from application_run_log_tasks where id=$1")
            .bind(id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(task, ("succeeded".into(), Some("declared final".into())));
    let page = store
        .record_client_trajectory_page(app, id, None, 20)
        .await
        .unwrap();
    assert_eq!(page.items.len(), 5);
    assert_eq!(
        page.items
            .iter()
            .map(|s| s.item_id.as_deref().unwrap())
            .collect::<Vec<_>>(),
        vec![
            "question",
            "ordinary",
            "older-completion",
            "assistant-final",
            "completion-final"
        ]
    );
    let completion_step = page.items.last().unwrap();
    let raw = store
        .record_client_trajectory_section(app, id, completion_step.id, "raw", None, 8)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        raw.items[0].value,
        serde_json::to_value(completion).unwrap()
    );
}

#[tokio::test]
async fn agent_logs_undeclared_empty_cancelled_and_inherited_completions_have_no_final() {
    let (store, scope, app) = setup().await;
    let service = AgentLogsService::new(store.clone());
    let key = Uuid::now_v7();
    for (case, phase, content, inherited, status, outcome) in [
        (
            "undeclared",
            None,
            Some("arbitrary text"),
            false,
            "succeeded",
            "no_final_answer",
        ),
        (
            "no-content",
            Some("final_answer"),
            None,
            false,
            "succeeded",
            "no_final_answer",
        ),
        (
            "empty",
            Some("final_answer"),
            Some(""),
            false,
            "succeeded",
            "no_final_answer",
        ),
        (
            "blank",
            Some("final_answer"),
            Some(" \n "),
            false,
            "succeeded",
            "no_final_answer",
        ),
        (
            "cancelled",
            Some("cancelled"),
            Some("must not be final"),
            false,
            "cancelled",
            "no_final_answer",
        ),
        (
            "inherited",
            Some("final_answer"),
            Some("historic final"),
            true,
            "running",
            "in_progress",
        ),
    ] {
        let mut ordinary = event(
            &format!("{case}-assistant"),
            1,
            AgentLogEventKind::Assistant,
            Some("arbitrary assistant"),
        );
        ordinary.source_task_id = case.into();
        let mut completion = event(
            &format!("{case}-completion"),
            2,
            AgentLogEventKind::TaskEnd,
            content,
        );
        completion.source_task_id = case.into();
        completion.phase = phase.map(Into::into);
        completion.inherited = inherited;
        let id = service
            .ingest(app, scope, key, batch(vec![ordinary, completion]))
            .await
            .unwrap()
            .record_ids[0];
        let record = store
            .application_log_record(app, id)
            .await
            .unwrap()
            .unwrap();
        assert!(
            record.messages.is_empty(),
            "{case} must not manufacture a final"
        );
        assert_eq!(record.outcome, outcome, "{case}");
        let task: (String, Option<String>) =
            sqlx::query_as("select status,final_output from application_run_log_tasks where id=$1")
                .bind(id)
                .fetch_one(store.pool())
                .await
                .unwrap();
        assert_eq!(task, (status.into(), None), "{case}");
        let page = store
            .record_client_trajectory_page(app, id, None, 20)
            .await
            .unwrap();
        assert_eq!(page.items.len(), 2, "{case} must retain original facts");
    }
}

#[tokio::test]
async fn agent_logs_real_application_key_authentication_cannot_generate() {
    let (store, scope, app) = setup().await;
    let owner: Uuid = sqlx::query_scalar("select created_by from applications where id=$1")
        .bind(app)
        .fetch_one(store.pool())
        .await
        .unwrap();
    let token = "sk-agent-logs-fixture-secret";
    let key = Uuid::now_v7();
    sqlx::query("insert into api_keys(id,name,token_hash,token_prefix,creator_user_id,tenant_id,scope_kind,scope_id,key_kind,application_id) values($1,'log fixture',$2,'sk-agent',$3,$4,'workspace',$5,'application_api_key',$6)")
        .bind(key).bind(control_plane::auth::hash_api_key_token(token)).bind(owner).bind(root_tenant_id(&store).await).bind(scope).bind(app).execute(store.pool()).await.unwrap();
    let auth = control_plane::application_public_api::api_keys::ApplicationApiKeyService::new(
        store.clone(),
    );
    let actor = auth.authenticate_bearer_token(token).await.unwrap();
    assert_eq!(actor.application_id, app);
    assert_eq!(actor.api_key_id, key);
    assert!(auth.authenticate_bearer_token("sk-wrong").await.is_err());
    let before: i64 = sqlx::query_scalar("select count(*) from flow_runs")
        .fetch_one(store.pool())
        .await
        .unwrap();
    let native =
        control_plane::application_public_api::run_service::ApplicationPublishedRunService::new(
            store.clone(),
        );
    let request =
        serde_json::from_value(json!({"query":"must not execute","model":"any-model"})).unwrap();
    assert!(native.start_native_run_for_actor(actor,request,control_plane::application_public_api::protocol_translation::TranslationProtocol::Native).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from flow_runs")
            .fetch_one(store.pool())
            .await
            .unwrap(),
        before
    );
    sqlx::query("update api_keys set enabled=false where id=$1")
        .bind(key)
        .execute(store.pool())
        .await
        .unwrap();
    assert!(auth.authenticate_bearer_token(token).await.is_err());
}

#[tokio::test]
async fn agent_logs_response_delta_excludes_legacy_cumulative_and_exact_cache_cost_is_neutral() {
    let (store, scope, app) = setup().await;
    sqlx::query("insert into model_pricing_rules(id,provider_code,upstream_model_id,input_token_unit_size,input_token_unit_price,output_token_unit_size,output_token_unit_price,cache_hit_token_unit_size,cache_hit_token_unit_price,cache_write_token_unit_size,cache_write_token_unit_price,currency_code,effective_from,timezone,weekday_mask,priority,enabled,source_kind,extensions,rules) values($1,'fixture-provider','fixture-model',1000,1.25,1000,2.5,1000,0.1,1000,1.5,'USD','2026-01-01'::timestamptz,'UTC',127,10,true,'manual','{}','[]')")
        .bind(Uuid::now_v7()).execute(store.pool()).await.unwrap();
    let balances:serde_json::Value=sqlx::query_scalar("select coalesce(jsonb_agg(to_jsonb(a) order by id),'[]'::jsonb) from user_credit_accounts a").fetch_one(store.pool()).await.unwrap();
    let credits: i64 = sqlx::query_scalar("select count(*) from runtime_credit_ledger")
        .fetch_one(store.pool())
        .await
        .unwrap();
    let mut delta = event("delta-response", 10, AgentLogEventKind::Usage, None);
    delta.provider_code = Some("unrelated-client-profile".into());
    delta.model_id = Some("fixture-model".into());
    delta.usage = Some(AgentLogUsage {
        basis: AgentLogUsageBasis::Delta,
        response_id: Some("R".into()),
        input_tokens: Some(100),
        output_tokens: Some(40),
        input_cache_hit_tokens: Some(20),
        cache_write_tokens: Some(10),
        total_tokens: Some(140),
    });
    let mut legacy = delta.clone();
    legacy.event_id = "historical-token-count".into();
    legacy.sequence = 11;
    legacy.usage.as_mut().unwrap().basis = AgentLogUsageBasis::Cumulative;
    legacy.usage.as_mut().unwrap().response_id = None;
    legacy.usage.as_mut().unwrap().total_tokens = Some(700);
    legacy.raw = json!({"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":700}}}});
    let service = AgentLogsService::new(store.clone());
    let id = service
        .ingest(app, scope, Uuid::now_v7(), batch(vec![delta, legacy]))
        .await
        .unwrap()
        .record_ids[0];
    let record = store
        .application_log_record(app, id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.total_tokens, Some(140));
    assert_eq!(record.input_tokens, Some(100));
    assert_eq!(
        record
            .cost_breakdown
            .total_cost
            .as_deref()
            .map(|v| v.trim_end_matches('0')),
        Some("0.2045")
    );
    assert!(sqlx::query_scalar::<_, bool>(
        "select total_cost=0.2045 from application_run_log_tasks where id=$1"
    )
    .bind(id)
    .fetch_one(store.pool())
    .await
    .unwrap());
    let page = store
        .record_client_trajectory_page(app, id, None, 50)
        .await
        .unwrap();
    let historical = page
        .items
        .iter()
        .find(|s| s.item_id.as_deref() == Some("historical-token-count"))
        .unwrap();
    let preserved = store
        .record_client_trajectory_section(app, id, historical.id, "overview", None, 8)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        preserved.items[0].value["usage"]["total_tokens"],
        json!(700)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from runtime_credit_ledger")
            .fetch_one(store.pool())
            .await
            .unwrap(),
        credits
    );
    assert_eq!(sqlx::query_scalar::<_,serde_json::Value>("select coalesce(jsonb_agg(to_jsonb(a) order by id),'[]'::jsonb) from user_credit_accounts a").fetch_one(store.pool()).await.unwrap(),balances);
}
#[tokio::test]
async fn agent_logs_equal_sequence_pages_are_lossless_and_cancelled_is_not_success() {
    let (store, scope, app) = setup().await;
    let service = AgentLogsService::new(store.clone());
    let mut end = event("abort", 10, AgentLogEventKind::TaskEnd, None);
    end.phase = Some("cancelled".into());
    let id = service
        .ingest(
            app,
            scope,
            Uuid::now_v7(),
            batch(vec![
                event("first", 10, AgentLogEventKind::ToolResult, Some("first")),
                event("second", 10, AgentLogEventKind::ToolResult, Some("second")),
                end,
            ]),
        )
        .await
        .unwrap()
        .record_ids[0];
    let mut cursor = None;
    let mut seen = std::collections::BTreeSet::new();
    loop {
        let page = store
            .record_client_trajectory_page(app, id, cursor, 1)
            .await
            .unwrap();
        for item in page.items {
            assert!(seen.insert(item.item_id.unwrap()));
        }
        match page.next_cursor {
            Some(RecordClientTrajectoryCursor::Imported(next)) => cursor = Some(next),
            None => break,
            _ => panic!("imported numeric cursor"),
        }
    }
    assert_eq!(
        seen,
        std::collections::BTreeSet::from([
            "first".to_owned(),
            "second".to_owned(),
            "abort".to_owned()
        ])
    );
    let status: String =
        sqlx::query_scalar("select status from application_run_log_tasks where id=$1")
            .bind(id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(status, "cancelled");
}

#[tokio::test]
async fn agent_logs_migration_preserves_native_task_and_flow_ownership() {
    let db = isolated_database().await;
    let store = PgControlPlaneStore::new(db.connect().await.unwrap());
    let before = sqlx::migrate!("../storage/durable/postgres/migrations")
        .iter()
        .filter(|m| m.version < 20261007110000)
        .cloned()
        .collect::<Vec<_>>();
    sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(before),
        ..sqlx::migrate::Migrator::DEFAULT
    }
    .run(store.pool())
    .await
    .unwrap();
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
    let old_summary_exists: bool = sqlx::query_scalar(
        "select exists(select 1 from application_run_log_summaries where flow_run_id=$1)",
    )
    .bind(run.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert!(old_summary_exists, "Native summary must precede migration");
    let old: Value =
        sqlx::query_scalar("select to_jsonb(t) from application_run_log_tasks t where id=$1")
            .bind(run.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    run_migrations(store.pool()).await.unwrap();
    let preserved:Value=sqlx::query_scalar("select to_jsonb(t)-array['source_kind','source_id','source_client','source_session_id','source_task_id','parent_source_task_id','native_run_id','cost_breakdown'] from application_run_log_tasks t where id=$1").bind(run.id).fetch_one(store.pool()).await.unwrap();
    assert_eq!(preserved, old);
    let record = store
        .application_log_record(seeded.application_id, run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.native_run_id, Some(run.id));
    assert_eq!(record.source_kind, "native");
    let native_page = store
        .record_client_trajectory_page(seeded.application_id, run.id, None, 1)
        .await
        .unwrap();
    assert!(native_page.next_cursor.is_none());

    let imported_app = store
        .create_application(&CreateApplicationInput {
            actor_user_id: seeded.actor_user_id,
            workspace_id: seeded.workspace_id,
            application_type: ApplicationType::AgentLogs,
            workflow_trigger_type: None,
            workflow_trigger_config: None,
            name: "Imported lifecycle isolation".into(),
            description: "fixture".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();
    let receipt = AgentLogsService::new(store.clone())
        .ingest(
            imported_app.id,
            seeded.workspace_id,
            Uuid::now_v7(),
            batch(vec![event(
                "imported",
                1,
                AgentLogEventKind::User,
                Some("retained"),
            )]),
        )
        .await
        .unwrap();
    let imported_id = receipt.record_ids[0];
    sqlx::query("delete from flow_runs where id=$1")
        .bind(run.id)
        .execute(store.pool())
        .await
        .unwrap();
    assert!(store
        .application_log_record(seeded.application_id, run.id)
        .await
        .unwrap()
        .is_none());
    let fresh = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        OffsetDateTime::now_utc(),
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    let owner: Uuid =
        sqlx::query_scalar("select native_run_id from application_run_log_tasks where id=$1")
            .bind(fresh.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(owner, fresh.id);
    let deleted_summary =
        sqlx::query("delete from application_run_log_summaries where flow_run_id=$1")
            .bind(fresh.id)
            .execute(store.pool())
            .await
            .unwrap();
    assert_eq!(deleted_summary.rows_affected(), 1);
    assert!(store
        .application_log_record(seeded.application_id, fresh.id)
        .await
        .unwrap()
        .is_none());
    let fresh_flow_exists: bool =
        sqlx::query_scalar("select exists(select 1 from flow_runs where id=$1)")
            .bind(fresh.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert!(
        fresh_flow_exists,
        "summary deletion must own the task cascade"
    );

    let fresh_flow_owned = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        OffsetDateTime::now_utc(),
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    let fresh_flow_record = store
        .application_log_record(seeded.application_id, fresh_flow_owned.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fresh_flow_record.native_run_id, Some(fresh_flow_owned.id));
    sqlx::query("delete from flow_runs where id=$1")
        .bind(fresh_flow_owned.id)
        .execute(store.pool())
        .await
        .unwrap();
    assert!(store
        .application_log_record(seeded.application_id, fresh_flow_owned.id)
        .await
        .unwrap()
        .is_none());
    let imported = store
        .application_log_record(imported_app.id, imported_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(imported.source_kind, "imported");
    assert_eq!(imported.native_run_id, None);
    assert_eq!(imported.messages[0].content, "retained");
    let imported_fake_summary_exists: bool = sqlx::query_scalar(
        "select exists(select 1 from application_run_log_summaries where flow_run_id=$1)",
    )
    .bind(imported_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert!(!imported_fake_summary_exists);
}
