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
    delta.provider_code = Some("fixture-provider".into());
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
