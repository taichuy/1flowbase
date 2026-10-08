use super::*;

async fn delete_service(
    store: &PgControlPlaneStore,
    scope: Uuid,
    app: Uuid,
) -> (AgentLogsService<PgControlPlaneStore>, domain::ActorContext) {
    let owner: Uuid = sqlx::query_scalar("select created_by from applications where id=$1")
        .bind(app)
        .fetch_one(store.pool())
        .await
        .unwrap();
    let actor =
        domain::ActorContext::root_in_scope(owner, root_tenant_id(store).await, scope, "root");
    (AgentLogsService::new(store.for_actor(actor.clone())), actor)
}
fn range(from: &str, to: &str) -> AgentLogsDeleteScope {
    AgentLogsDeleteScope::TimeRange {
        started_at_from: from.into(),
        started_at_to: to.into(),
    }
}
async fn ingest_turn(
    store: &PgControlPlaneStore,
    scope: Uuid,
    app: Uuid,
    name: &str,
    at: &str,
) -> Uuid {
    let mut first = event(name, 0, AgentLogEventKind::User, Some("question"));
    first.source_task_id = name.into();
    first.occurred_at = at.into();
    AgentLogsService::new(store.clone())
        .ingest(app, scope, Uuid::now_v7(), batch(vec![first]))
        .await
        .unwrap()
        .record_ids[0]
}

#[tokio::test]
async fn agent_logs_delete_half_open_complete_turn_and_all_time_preserve_application() {
    let (store, scope, app) = setup().await;
    let initial_application: serde_json::Value =
        sqlx::query_scalar("select to_jsonb(a) from applications a where id=$1")
            .bind(app)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let owner: Uuid = sqlx::query_scalar("select created_by from applications where id=$1")
        .bind(app)
        .fetch_one(store.pool())
        .await
        .unwrap();
    let key = Uuid::now_v7();
    sqlx::query("insert into api_keys(id,name,token_hash,token_prefix,creator_user_id,tenant_id,scope_kind,scope_id,key_kind,application_id) values($1,'preserved key','delete-fixture-hash','sk-fixture',$2,$3,'workspace',$4,'application_api_key',$5)").bind(key).bind(owner).bind(root_tenant_id(&store).await).bind(scope).bind(app).execute(store.pool()).await.unwrap();
    let accounts: serde_json::Value = sqlx::query_scalar("select coalesce(jsonb_agg(to_jsonb(a) order by id),'[]'::jsonb) from user_credit_accounts a").fetch_one(store.pool()).await.unwrap();
    let ledger: serde_json::Value = sqlx::query_scalar("select coalesce(jsonb_agg(to_jsonb(a) order by id),'[]'::jsonb) from runtime_credit_ledger a").fetch_one(store.pool()).await.unwrap();

    let before = ingest_turn(&store, scope, app, "before", "2026-10-06T23:59:59Z").await;
    let from = ingest_turn(&store, scope, app, "from", "2026-10-07T00:00:00Z").await;
    let within = ingest_turn(&store, scope, app, "within", "2026-10-07T23:59:59Z").await;
    let to = ingest_turn(&store, scope, app, "to", "2026-10-08T00:00:00Z").await;
    let mut late = event(
        "late-event",
        3,
        AgentLogEventKind::Assistant,
        Some("after interval"),
    );
    late.source_task_id = "from".into();
    late.occurred_at = "2026-10-09T00:00:00Z".into();
    AgentLogsService::new(store.clone())
        .ingest(app, scope, Uuid::now_v7(), batch(vec![late]))
        .await
        .unwrap();
    let (service, owner) = delete_service(&store, scope, app).await;
    assert_eq!(
        service
            .delete(
                &owner,
                app,
                range("2026-10-07T00:00:00Z", "2026-10-08T00:00:00Z")
            )
            .await
            .unwrap()
            .deleted_records,
        2
    );
    for id in [from, within] {
        assert!(store
            .application_log_record(app, id)
            .await
            .unwrap()
            .is_none());
    }
    for id in [before, to] {
        assert!(store
            .application_log_record(app, id)
            .await
            .unwrap()
            .is_some());
    }
    assert_eq!(
        service
            .delete(&owner, app, AgentLogsDeleteScope::AllTime {})
            .await
            .unwrap()
            .deleted_records,
        2
    );
    assert_eq!(
        owned_log_counts(&store, app, &[before, from, within, to], &[]).await,
        vec![1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(
        service
            .delete(&owner, app, AgentLogsDeleteScope::AllTime {})
            .await
            .unwrap()
            .deleted_records,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, serde_json::Value>(
            "select to_jsonb(a) from applications a where id=$1"
        )
        .bind(app)
        .fetch_one(store.pool())
        .await
        .unwrap(),
        initial_application
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from api_keys where id=$1 and application_id=$2"
        )
        .bind(key)
        .bind(app)
        .fetch_one(store.pool())
        .await
        .unwrap(),
        1
    );
    assert_eq!(sqlx::query_scalar::<_,serde_json::Value>("select coalesce(jsonb_agg(to_jsonb(a) order by id),'[]'::jsonb) from user_credit_accounts a").fetch_one(store.pool()).await.unwrap(),accounts);
    assert_eq!(sqlx::query_scalar::<_,serde_json::Value>("select coalesce(jsonb_agg(to_jsonb(a) order by id),'[]'::jsonb) from runtime_credit_ledger a").fetch_one(store.pool()).await.unwrap(),ledger);
    // Deleted receipts no longer suppress a corrected full replay.
    let reimported = ingest_turn(&store, scope, app, "from", "2026-10-07T00:00:00Z").await;
    assert_ne!(reimported, from);
    assert_eq!(
        store
            .record_client_trajectory_page(app, reimported, None, 10)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
}

#[tokio::test]
async fn agent_logs_delete_rejects_other_workspace_native_and_ungranted_actor() {
    let (store, scope, app) = setup().await;
    let id = ingest_turn(&store, scope, app, "kept", "2026-10-07T00:00:00Z").await;
    let counts = owned_log_counts(&store, app, &[id], &[]).await;
    let owner: Uuid = sqlx::query_scalar("select created_by from applications where id=$1")
        .bind(app)
        .fetch_one(store.pool())
        .await
        .unwrap();
    let ungranted = domain::ActorContext::scoped_in_scope(
        owner,
        root_tenant_id(&store).await,
        scope,
        "member",
        Vec::<String>::new(),
    );
    assert!(AgentLogsService::new(store.for_actor(ungranted.clone()))
        .delete(&ungranted, app, AgentLogsDeleteScope::AllTime {})
        .await
        .is_err());
    let wrong = seed_workspace(&store, "delete-wrong").await;
    assert!(store
        .delete_agent_logs(app, wrong, &AgentLogsDeleteScope::AllTime {})
        .await
        .is_err());
    let foreign_owner = seed_user(&store, wrong, "delete-foreign").await;
    let foreign_app = store
        .create_application(&CreateApplicationInput {
            actor_user_id: foreign_owner,
            workspace_id: wrong,
            application_type: ApplicationType::AgentLogs,
            workflow_trigger_type: None,
            workflow_trigger_config: None,
            name: "foreign".into(),
            description: "".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();
    let foreign_id = ingest_turn(
        &store,
        wrong,
        foreign_app.id,
        "foreign",
        "2026-10-07T00:00:00Z",
    )
    .await;
    let foreign_counts = owned_log_counts(&store, foreign_app.id, &[foreign_id], &[]).await;
    let (service, owner) = delete_service(&store, scope, app).await;
    assert!(service
        .delete(&owner, foreign_app.id, AgentLogsDeleteScope::AllTime {})
        .await
        .is_err());
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let native_run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        OffsetDateTime::now_utc(),
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    let native_counts = owned_log_counts(
        &store,
        seeded.application_id,
        &[native_run.id],
        &[native_run.id],
    )
    .await;
    assert!(native_counts[1] > 0);

    assert!(store
        .delete_agent_logs(
            seeded.application_id,
            seeded.workspace_id,
            &AgentLogsDeleteScope::AllTime {}
        )
        .await
        .is_err());
    let (native_service, native_owner) =
        delete_service(&store, seeded.workspace_id, seeded.application_id).await;
    assert!(native_service
        .delete(
            &native_owner,
            seeded.application_id,
            AgentLogsDeleteScope::AllTime {}
        )
        .await
        .is_err());
    assert_eq!(owned_log_counts(&store, app, &[id], &[]).await, counts);
    assert_eq!(
        owned_log_counts(&store, foreign_app.id, &[foreign_id], &[]).await,
        foreign_counts
    );
    service
        .delete(&owner, app, AgentLogsDeleteScope::AllTime {})
        .await
        .unwrap();
    assert_eq!(
        owned_log_counts(&store, foreign_app.id, &[foreign_id], &[]).await,
        foreign_counts
    );
    assert_eq!(
        owned_log_counts(
            &store,
            seeded.application_id,
            &[native_run.id],
            &[native_run.id]
        )
        .await,
        native_counts
    );
    // The sealed current workspace may differ from the actor's first membership.
    sqlx::query("insert into workspace_memberships(id,workspace_id,user_id,introduction) values($1,$2,$3,'')").bind(Uuid::now_v7()).bind(wrong).bind(owner.user_id).execute(store.pool()).await.unwrap();
    let switched_actor =
        domain::ActorContext::root_in_scope(owner.user_id, owner.tenant_id, wrong, "root");
    let switched_service = AgentLogsService::new(store.for_actor(switched_actor.clone()));
    assert_eq!(
        switched_service
            .delete(
                &switched_actor,
                foreign_app.id,
                AgentLogsDeleteScope::AllTime {}
            )
            .await
            .unwrap()
            .deleted_records,
        1
    );
}

#[tokio::test]
async fn agent_logs_delete_keeps_legal_shared_body_until_last_record_reference() {
    let (store, scope, app) = setup().await;
    let removed = ingest_turn(&store, scope, app, "remove", "2026-10-07T00:00:00Z").await;
    let kept = ingest_turn(&store, scope, app, "keep", "2026-10-08T00:00:00Z").await;
    let content: Uuid = sqlx::query_scalar(
        "select content_id from client_trajectory_sections where record_id=$1 and section='raw'",
    )
    .bind(removed)
    .fetch_one(store.pool())
    .await
    .unwrap();
    let original: serde_json::Value = sqlx::query_scalar("select runtime_original_json(content,raw_json_payloads,'content') from runtime_canonical_contents where id=$1").bind(content).fetch_one(store.pool()).await.unwrap();
    // A real section on the retained record shares the same scoped canonical content.
    sqlx::query("insert into client_trajectory_sections(id,request_id,step_id,record_id,section,event_sequence,content_id,content_path,body_kind,observed_at,value_hash,value_byte_size) select $1,request_id,step_id,record_id,'shared_body',event_sequence,$2,'{}','content',observed_at,(select content_hash from runtime_canonical_contents where id=$2),(select byte_size from runtime_canonical_contents where id=$2) from client_trajectory_sections where record_id=$3 and section='raw'")
        .bind(Uuid::now_v7()).bind(content).bind(kept).execute(store.pool()).await.unwrap();
    let (service, owner) = delete_service(&store, scope, app).await;
    service
        .delete(
            &owner,
            app,
            range("2026-10-07T00:00:00Z", "2026-10-08T00:00:00Z"),
        )
        .await
        .unwrap();
    let step = store
        .record_client_trajectory_page(app, kept, None, 10)
        .await
        .unwrap()
        .items[0]
        .id;
    let shared = store
        .record_client_trajectory_section(app, kept, step, "shared_body", None, 10)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(shared.items[0].value, original);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from runtime_observation_body_ownership where content_id=$1"
        )
        .bind(content)
        .fetch_one(store.pool())
        .await
        .unwrap(),
        1
    );
    service
        .delete(&owner, app, AgentLogsDeleteScope::AllTime {})
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from runtime_canonical_contents where id=$1")
            .bind(content)
            .fetch_one(store.pool())
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        owned_log_counts(&store, app, &[removed, kept], &[]).await,
        vec![1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
}

#[tokio::test]
async fn agent_logs_delete_deferred_failure_rolls_back_every_owned_directory() {
    let (store, scope, app) = setup().await;
    let id = ingest_turn(&store, scope, app, "rollback", "2026-10-07T00:00:00Z").await;
    let counts = owned_log_counts(&store, app, &[id], &[]).await;
    sqlx::raw_sql("create function reject_log_section_delete() returns trigger language plpgsql as $$ begin raise exception 'fixture deferred delete rejection'; end $$; create constraint trigger fixture_reject_log_delete after delete on client_trajectory_sections deferrable initially deferred for each row execute function reject_log_section_delete();").execute(store.pool()).await.unwrap();
    let (service, owner) = delete_service(&store, scope, app).await;
    assert!(service
        .delete(&owner, app, AgentLogsDeleteScope::AllTime {})
        .await
        .is_err());
    assert_eq!(owned_log_counts(&store, app, &[id], &[]).await, counts);
    assert!(store
        .application_log_record(app, id)
        .await
        .unwrap()
        .is_some());
    let mut source = event("rollback", 0, AgentLogEventKind::User, Some("question"));
    source.source_task_id = "rollback".into();
    source.occurred_at = "2026-10-07T00:00:00Z".into();
    assert_eq!(
        AgentLogsService::new(store.clone())
            .ingest(app, scope, Uuid::now_v7(), batch(vec![source]))
            .await
            .unwrap()
            .duplicate_events,
        1
    );
}

#[tokio::test]
async fn agent_logs_application_shared_exclusive_lock_coordinates_delete_and_ingest() {
    use std::time::Duration;
    let (store, scope, app) = setup().await;
    let mut tx = store.pool().begin().await.unwrap();
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("agent-logs-application:{app}"))
        .execute(&mut *tx)
        .await
        .unwrap();
    let ingest_store = store.clone();
    let mut ingestion = tokio::spawn(async move {
        ingest_turn(&ingest_store, scope, app, "locked", "2026-10-07T00:00:00Z").await
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut ingestion)
            .await
            .is_err()
    );
    tx.commit().await.unwrap();
    let id = tokio::time::timeout(Duration::from_secs(5), &mut ingestion)
        .await
        .unwrap()
        .unwrap();
    let mut shared_tx = store.pool().begin().await.unwrap();
    sqlx::query("select pg_advisory_xact_lock_shared(hashtextextended($1,0))")
        .bind(format!("agent-logs-application:{app}"))
        .execute(&mut *shared_tx)
        .await
        .unwrap();
    // Simulate a transaction ingesting collector source while another source writes.
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("agent-logs:{app}:collector"))
        .execute(&mut *shared_tx)
        .await
        .unwrap();
    let mut parallel = event("parallel", 0, AgentLogEventKind::User, Some("question"));
    parallel.source_task_id = "parallel".into();
    parallel.occurred_at = "2026-10-07T01:00:00Z".into();
    let mut other_source = batch(vec![parallel]);
    other_source.source_id = "independent-collector".into();
    let another = tokio::time::timeout(
        Duration::from_secs(5),
        AgentLogsService::new(store.clone()).ingest(app, scope, Uuid::now_v7(), other_source),
    )
    .await
    .unwrap()
    .unwrap()
    .record_ids[0];
    let (service, owner) = delete_service(&store, scope, app).await;
    let mut deletion = tokio::spawn(async move {
        service
            .delete(&owner, app, AgentLogsDeleteScope::AllTime {})
            .await
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut deletion)
            .await
            .is_err()
    );
    assert!(store
        .application_log_record(app, id)
        .await
        .unwrap()
        .is_some());
    shared_tx.commit().await.unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), &mut deletion)
            .await
            .unwrap()
            .unwrap()
            .unwrap()
            .deleted_records,
        2
    );
    assert_eq!(
        owned_log_counts(&store, app, &[id, another], &[]).await,
        vec![1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
}
