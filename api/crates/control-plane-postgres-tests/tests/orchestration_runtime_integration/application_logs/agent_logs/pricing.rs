use super::*;

async fn price(
    store: &PgControlPlaneStore,
    provider: &str,
    model: &str,
    input: &str,
    priority: i32,
    enabled: bool,
    from: &str,
    window: bool,
) {
    sqlx::query(r#"insert into model_pricing_rules(id,provider_code,upstream_model_id,
        input_token_unit_size,input_token_unit_price,output_token_unit_size,output_token_unit_price,
        cache_hit_token_unit_size,cache_hit_token_unit_price,cache_write_token_unit_size,cache_write_token_unit_price,
        currency_code,effective_from,timezone,weekday_mask,priority,enabled,source_kind,extensions,rules,local_time_start,local_time_end)
        values($1,$2,$3,1000,$4::numeric,1000,2.5,1000,0.1,1000,1.5,'USD',$5::timestamptz,'UTC',127,$6,$7,'manual','{}','[]',
        case when $8 then '00:00'::time end,case when $8 then '00:01'::time end)"#)
        .bind(Uuid::now_v7()).bind(provider).bind(model).bind(input).bind(from)
        .bind(priority).bind(enabled).bind(window).execute(store.pool()).await.unwrap();
}
fn usage_event(id: &str, model: &str) -> AgentLogEvent {
    let mut e = event(id, 10, AgentLogEventKind::Usage, None);
    e.provider_code = Some("z-client-provider".into());
    e.model_id = Some(model.into());
    e.usage = Some(AgentLogUsage {
        basis: AgentLogUsageBasis::Delta,
        response_id: Some(id.into()),
        input_tokens: Some(100),
        output_tokens: Some(40),
        input_cache_hit_tokens: Some(20),
        cache_write_tokens: Some(10),
        total_tokens: Some(140),
    });
    e
}
async fn cost(store: &PgControlPlaneStore, app: Uuid, id: Uuid) -> Option<String> {
    sqlx::query_scalar(
        "select total_cost::text from application_run_log_tasks where application_id=$1 and id=$2",
    )
    .bind(app)
    .bind(id)
    .fetch_one(store.pool())
    .await
    .unwrap()
}
async fn assert_cost(store: &PgControlPlaneStore, app: Uuid, id: Uuid, expected: &str) {
    assert!(sqlx::query_scalar::<_, bool>("select total_cost=$3::numeric from application_run_log_tasks where application_id=$1 and id=$2")
        .bind(app).bind(id).bind(expected).fetch_one(store.pool()).await.unwrap());
}
async fn identity_snapshot(store: &PgControlPlaneStore, app: Uuid) -> Value {
    sqlx::query_scalar(r#"select jsonb_build_object(
        'tasks',(select jsonb_agg(to_jsonb(t)-'total_cost'-'cost_breakdown' order by id) from application_run_log_tasks t where application_id=$1),
        'receipts',(select jsonb_agg(to_jsonb(r)-'rated_cost' order by event_id) from application_log_upload_receipts r where application_id=$1),
        'messages',(select jsonb_agg(to_jsonb(m) order by id) from application_run_conversation_message_items m where application_id=$1),
        'bodies',(select jsonb_agg(to_jsonb(c) order by id) from runtime_canonical_contents c where application_id=$1),
        'steps',(select jsonb_agg(to_jsonb(s) order by id) from client_trajectory_steps s where record_id in(select id from application_run_log_tasks where application_id=$1)),
        'sections',(select jsonb_agg(to_jsonb(s) order by id) from client_trajectory_sections s where record_id in(select id from application_run_log_tasks where application_id=$1)),
        'accounts',(select coalesce(jsonb_agg(to_jsonb(a) order by id),'[]'::jsonb) from user_credit_accounts a),
        'credit',(select coalesce(jsonb_agg(to_jsonb(l) order by id),'[]'::jsonb) from runtime_credit_ledger l))"#)
        .bind(app).fetch_one(store.pool()).await.unwrap()
}

#[tokio::test]
async fn agent_logs_model_id_only_uses_stable_first_match_without_changing_native_billing() {
    let (store, scope, app) = setup().await;
    price(
        &store,
        "z-client-provider",
        "priced-model",
        "99",
        100,
        true,
        "2026-01-01",
        false,
    )
    .await;
    price(
        &store,
        "a-first",
        "priced-model",
        "1.25",
        1,
        true,
        "2026-01-01",
        false,
    )
    .await;
    price(
        &store,
        "a-first",
        "priced-model",
        "999",
        9,
        true,
        "2026-01-01",
        true,
    )
    .await;
    price(
        &store,
        "0-disabled",
        "priced-model",
        "999",
        1,
        false,
        "2026-01-01",
        false,
    )
    .await;
    price(
        &store,
        "0-future",
        "priced-model",
        "999",
        1,
        true,
        "2027-01-01",
        false,
    )
    .await;
    price(
        &store,
        "0-prefix",
        "priced-model-extra",
        "999",
        1,
        true,
        "2026-01-01",
        false,
    )
    .await;
    let at = time::OffsetDateTime::parse(
        "2026-10-07T08:00:00Z",
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap();
    let native = control_plane::billing::resolve_pricing_rule(
        &store,
        "z-client-provider",
        "priced-model",
        at,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(native.provider_code, "z-client-provider");
    assert_eq!(native.input_token_unit_price.to_string(), "99");
    let e = usage_event("priced", "priced-model");
    let service = AgentLogsService::new(store.clone());
    let id = service
        .ingest(app, scope, Uuid::now_v7(), batch(vec![e.clone()]))
        .await
        .unwrap()
        .record_ids[0];
    assert_cost(&store, app, id, "0.2045").await;
    let original = store
        .record_client_trajectory_section(
            app,
            id,
            store
                .record_client_trajectory_page(app, id, None, 50)
                .await
                .unwrap()
                .items[0]
                .id,
            "overview",
            None,
            8,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        original.items[0].value["provider_code"],
        json!("z-client-provider")
    );
    assert_eq!(
        service
            .ingest(app, scope, Uuid::now_v7(), batch(vec![e]))
            .await
            .unwrap()
            .duplicate_events,
        1
    );
}

#[tokio::test]
async fn agent_logs_model_exact_missing_price_and_partial_usage_are_not_guessed() {
    let (store, scope, app) = setup().await;
    price(
        &store,
        "a",
        "priced-model-extra",
        "999",
        1,
        true,
        "2026-01-01",
        false,
    )
    .await;
    let service = AgentLogsService::new(store.clone());
    let unknown = service
        .ingest(
            app,
            scope,
            Uuid::now_v7(),
            batch(vec![usage_event("unknown", "priced-model")]),
        )
        .await
        .unwrap()
        .record_ids[0];
    assert_cost(&store, app, unknown, "0").await;
    price(
        &store,
        "a",
        "priced-model",
        "1.25",
        1,
        true,
        "2026-01-01",
        false,
    )
    .await;
    let mut partial = usage_event("partial", "priced-model");
    partial.source_task_id = "partial-turn".into();
    partial.usage.as_mut().unwrap().input_cache_hit_tokens = None;
    let id = service
        .ingest(app, scope, Uuid::now_v7(), batch(vec![partial]))
        .await
        .unwrap()
        .record_ids[0];
    assert_eq!(cost(&store, app, id).await, None);
}

#[tokio::test]
async fn agent_logs_reprice_changes_only_costs_is_idempotent_and_includes_later_appends() {
    let (store, scope, app) = setup().await;
    price(
        &store,
        "a",
        "priced-model",
        "1.25",
        1,
        true,
        "2026-01-01",
        false,
    )
    .await;
    let mut final_event = event(
        "final",
        20,
        AgentLogEventKind::TaskEnd,
        Some("preserved answer"),
    );
    final_event.phase = Some("final_answer".into());
    let original = batch(vec![
        event(
            "system",
            0,
            AgentLogEventKind::System,
            Some("preserved instruction"),
        ),
        event(
            "user",
            1,
            AgentLogEventKind::User,
            Some("preserved question"),
        ),
        usage_event("old", "priced-model"),
        final_event,
    ]);
    let id = store
        .ingest_agent_logs(
            app,
            scope,
            Uuid::now_v7(),
            &original,
            &[None, None, Some("0".into()), None],
        )
        .await
        .unwrap()
        .record_ids[0];
    let owner: Uuid = sqlx::query_scalar("select created_by from applications where id=$1")
        .bind(app)
        .fetch_one(store.pool())
        .await
        .unwrap();
    let foreign = store
        .create_application(&CreateApplicationInput {
            actor_user_id: owner,
            workspace_id: scope,
            application_type: ApplicationType::AgentLogs,
            workflow_trigger_type: None,
            workflow_trigger_config: None,
            name: "Foreign".into(),
            description: "".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap()
        .id;
    let foreign_id = store
        .ingest_agent_logs(
            foreign,
            scope,
            Uuid::now_v7(),
            &original,
            &[None, None, Some("0".into()), None],
        )
        .await
        .unwrap()
        .record_ids[0];
    let before = identity_snapshot(&store, app).await;
    let service = AgentLogsService::new(store.clone());
    let result = service
        .reprice_next_record(app, scope, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.record_id, id);
    assert_eq!(result.changed_events, 1);
    assert_cost(&store, app, id, "0.2045").await;
    assert_cost(&store, foreign, foreign_id, "0").await;
    assert_eq!(before, identity_snapshot(&store, app).await);
    assert_eq!(
        service
            .reprice_next_record(app, scope, None)
            .await
            .unwrap()
            .unwrap()
            .changed_events,
        0
    );
    assert!(service
        .reprice_next_record(app, scope, Some(id))
        .await
        .unwrap()
        .is_none());
    assert!(service
        .reprice_next_record(app, Uuid::now_v7(), None)
        .await
        .is_err());
    assert!(store
        .reprice_agent_log_record(
            app,
            scope,
            id,
            &[
                ("old".into(), Some("999".into())),
                ("missing".into(), Some("1".into()))
            ]
        )
        .await
        .is_err());
    let read = store
        .next_agent_log_pricing_record(app, scope, None)
        .await
        .unwrap()
        .unwrap();
    let mut later = usage_event("new", "priced-model");
    later.sequence = 11;
    service
        .ingest(app, scope, Uuid::now_v7(), batch(vec![later]))
        .await
        .unwrap();
    store
        .reprice_agent_log_record(
            app,
            scope,
            read.record_id,
            &[("old".into(), Some("0.2045".into()))],
        )
        .await
        .unwrap();
    assert_cost(&store, app, id, "0.409").await;
    assert_eq!(
        store
            .application_log_record(app, id)
            .await
            .unwrap()
            .unwrap()
            .total_tokens,
        Some(280)
    );
    sqlx::query("update applications set application_type='agent_flow' where id=$1")
        .bind(foreign)
        .execute(store.pool())
        .await
        .unwrap();
    assert!(service
        .reprice_next_record(foreign, scope, None)
        .await
        .is_err());
    assert!(store
        .reprice_agent_log_record(foreign, scope, foreign_id, &[])
        .await
        .is_err());
}
