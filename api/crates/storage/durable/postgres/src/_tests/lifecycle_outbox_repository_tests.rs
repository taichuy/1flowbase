use control_plane_contracts::ports::{
    LifecycleOutboxRepository, LifecycleOutboxStatus, LifecyclePublicationPlan,
    LifecycleSubscriberTarget, RecordLifecycleFactInput,
};
use storage_durable_postgres::{run_migrations, PgControlPlaneStore};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

fn publication(subscribers: &[&str]) -> LifecyclePublicationPlan {
    LifecyclePublicationPlan {
        graph_fingerprint: "graph-v1".to_string(),
        subscribers: subscribers
            .iter()
            .map(|subscriber| LifecycleSubscriberTarget {
                subscriber_id: (*subscriber).to_string(),
                handler_id: format!("handler-{subscriber}"),
                handler_version: "v1".to_string(),
            })
            .collect(),
    }
}

async fn store() -> PgControlPlaneStore {
    let database = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    PgControlPlaneStore::new(pool)
}

#[tokio::test]
async fn lcf_005_outbox_claim_retry_delivery_and_idempotency_are_durable() {
    let store = store().await;
    let input = RecordLifecycleFactInput {
        event_id: Uuid::now_v7(),
        transaction_id: Uuid::now_v7(),
        contract_id: "model_definition.committed".to_string(),
        contract_version: "v1".to_string(),
        canonical_payload: br#"{"model_definition_id":"model-1"}"#.to_vec(),
        occurred_at: OffsetDateTime::from_unix_timestamp_nanos(1_700_000_000_123_456_789).unwrap(),
        publication: publication(&["a"]),
    };
    let first = store.record_lifecycle_fact(&input).await.unwrap();
    let replay = store.record_lifecycle_fact(&input).await.unwrap();
    assert_eq!(first, replay);
    assert_eq!(first.occurred_at.nanosecond(), 123_456_000);

    let worker = Uuid::now_v7();
    let lease = Duration::seconds(30);
    let claimed = store
        .claim_lifecycle_facts(worker, 10, lease)
        .await
        .unwrap();
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].status, LifecycleOutboxStatus::Claimed);
    assert_eq!(claimed[0].attempt_count, 1);

    store
        .retry_lifecycle_fact(
            input.event_id,
            "a",
            worker,
            claimed
                .iter()
                .find(|r| r.subscriber_id == "a")
                .unwrap()
                .claim_id
                .unwrap(),
            OffsetDateTime::now_utc() - Duration::seconds(1),
            "subscriber unavailable",
        )
        .await
        .unwrap();
    let claimed_again = store
        .claim_lifecycle_facts(worker, 10, lease)
        .await
        .unwrap();
    assert_eq!(claimed_again[0].attempt_count, 2);
    let delivered = store
        .mark_lifecycle_fact_delivered(
            input.event_id,
            "a",
            worker,
            claimed_again[0].claim_id.unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delivered.status, LifecycleOutboxStatus::Delivered);
    assert!(store
        .claim_lifecycle_facts(worker, 10, lease)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn lcf_qaf_stale_claim_is_recovered_by_a_new_worker() {
    let store = store().await;
    let input = RecordLifecycleFactInput {
        event_id: Uuid::now_v7(),
        transaction_id: Uuid::now_v7(),
        contract_id: "model_definition.committed".to_string(),
        contract_version: "v1".to_string(),
        canonical_payload: b"recovery".to_vec(),
        occurred_at: OffsetDateTime::now_utc(),
        publication: publication(&["a"]),
    };
    store.record_lifecycle_fact(&input).await.unwrap();
    let crashed_worker = Uuid::now_v7();
    let original = store
        .claim_lifecycle_facts(crashed_worker, 1, Duration::seconds(30))
        .await
        .unwrap();
    sqlx::query("update lifecycle_outbox_deliveries set claimed_at = now() - interval '31 seconds', claim_expires_at = now() - interval '1 second' where event_id = $1")
        .bind(input.event_id)
        .execute(store.pool())
        .await
        .unwrap();

    let recovery_worker = Uuid::now_v7();
    let recovered = store
        .claim_lifecycle_facts(recovery_worker, 1, Duration::seconds(30))
        .await
        .unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].claimed_by, Some(recovery_worker));
    assert_eq!(recovered[0].attempt_count, 2);
    assert!(store
        .mark_lifecycle_fact_delivered(
            input.event_id,
            "a",
            crashed_worker,
            original[0].claim_id.unwrap()
        )
        .await
        .is_err());
}

#[tokio::test]
async fn lcf_005_same_event_id_with_different_fact_is_rejected() {
    let store = store().await;
    let input = RecordLifecycleFactInput {
        event_id: Uuid::now_v7(),
        transaction_id: Uuid::now_v7(),
        contract_id: "model_definition.committed".to_string(),
        contract_version: "v1".to_string(),
        canonical_payload: b"first".to_vec(),
        occurred_at: OffsetDateTime::now_utc(),
        publication: publication(&["a"]),
    };
    store.record_lifecycle_fact(&input).await.unwrap();
    let error = store
        .record_lifecycle_fact(&RecordLifecycleFactInput {
            canonical_payload: b"different".to_vec(),
            ..input
        })
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("conflicts with a different fact"));
}

#[tokio::test]
async fn lcf_qaf_subscribers_complete_and_retry_independently() {
    let store = store().await;
    let input = RecordLifecycleFactInput {
        event_id: Uuid::now_v7(),
        transaction_id: Uuid::now_v7(),
        contract_id: "model_definition.committed".to_string(),
        contract_version: "v1".to_string(),
        canonical_payload: b"independent".to_vec(),
        occurred_at: OffsetDateTime::now_utc(),
        publication: publication(&["a", "b"]),
    };
    store.record_lifecycle_fact(&input).await.unwrap();
    let worker = Uuid::now_v7();
    let claimed = store
        .claim_lifecycle_facts(worker, 2, Duration::seconds(30))
        .await
        .unwrap();
    assert_eq!(claimed.len(), 2);
    store
        .mark_lifecycle_fact_delivered(
            input.event_id,
            "a",
            worker,
            claimed
                .iter()
                .find(|r| r.subscriber_id == "a")
                .unwrap()
                .claim_id
                .unwrap(),
        )
        .await
        .unwrap();
    store
        .retry_lifecycle_fact(
            input.event_id,
            "b",
            worker,
            claimed
                .iter()
                .find(|r| r.subscriber_id == "b")
                .unwrap()
                .claim_id
                .unwrap(),
            OffsetDateTime::now_utc() - Duration::seconds(1),
            "subscriber b failed",
        )
        .await
        .unwrap();
    let event_status: String =
        sqlx::query_scalar("select status from lifecycle_outbox where event_id = $1")
            .bind(input.event_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(event_status, "pending");
    let retry = store
        .claim_lifecycle_facts(worker, 2, Duration::seconds(30))
        .await
        .unwrap();
    assert_eq!(retry.len(), 1);
    assert_eq!(retry[0].subscriber_id, "b");
    store
        .mark_lifecycle_fact_delivered(input.event_id, "b", worker, retry[0].claim_id.unwrap())
        .await
        .unwrap();
    let event_status: String =
        sqlx::query_scalar("select status from lifecycle_outbox where event_id = $1")
            .bind(input.event_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(event_status, "delivered");
}

// Root #2007 AC-007 / AUTH-09: same worker UUID cannot reuse a previous claim token.
#[tokio::test]
async fn root_2007_ac_007_claim_fencing_and_paused_backlog() {
    use control_plane_contracts::ports::{LifecycleClaimLost, LifecycleDeliveryPauseReason};
    let store = store().await;
    let input = RecordLifecycleFactInput {
        event_id: Uuid::now_v7(),
        transaction_id: Uuid::now_v7(),
        contract_id: "acme.composition-a.processed".into(),
        contract_version: "1".into(),
        canonical_payload: b"fenced".to_vec(),
        occurred_at: OffsetDateTime::now_utc(),
        publication: publication(&["b", "c"]),
    };
    store.record_lifecycle_fact(&input).await.unwrap();
    let worker = Uuid::now_v7();
    let original = store
        .claim_lifecycle_facts(worker, 2, Duration::seconds(30))
        .await
        .unwrap();
    let old = original.iter().find(|r| r.subscriber_id == "b").unwrap();
    let c = original.iter().find(|r| r.subscriber_id == "c").unwrap();
    store
        .mark_lifecycle_fact_delivered(input.event_id, "c", worker, c.claim_id.unwrap())
        .await
        .unwrap();
    sqlx::query("update lifecycle_outbox_deliveries set claimed_at=now()-interval '31 seconds', claim_expires_at=now()-interval '1 second' where event_id=$1 and subscriber_id='b'").bind(input.event_id).execute(store.pool()).await.unwrap();
    assert!(store
        .mark_lifecycle_fact_delivered(input.event_id, "b", worker, old.claim_id.unwrap())
        .await
        .unwrap_err()
        .downcast_ref::<LifecycleClaimLost>()
        .is_some());
    let current = store
        .claim_lifecycle_facts(worker, 2, Duration::seconds(30))
        .await
        .unwrap();
    assert_eq!(current.len(), 1);
    assert_ne!(current[0].claim_id, old.claim_id);
    for error in [
        store
            .mark_lifecycle_fact_delivered(input.event_id, "b", worker, old.claim_id.unwrap())
            .await
            .unwrap_err(),
        store
            .retry_lifecycle_fact(
                input.event_id,
                "b",
                worker,
                old.claim_id.unwrap(),
                OffsetDateTime::now_utc(),
                "old retry",
            )
            .await
            .unwrap_err(),
        store
            .pause_lifecycle_fact(
                input.event_id,
                "b",
                worker,
                old.claim_id.unwrap(),
                LifecycleDeliveryPauseReason::AuthorityRevoked,
            )
            .await
            .unwrap_err(),
    ] {
        assert!(error.downcast_ref::<LifecycleClaimLost>().is_some());
    }
    let persisted: Uuid = sqlx::query_scalar(
        "select claim_id from lifecycle_outbox_deliveries where event_id=$1 and subscriber_id='b'",
    )
    .bind(input.event_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(Some(persisted), current[0].claim_id);
    let paused = store
        .pause_lifecycle_fact(
            input.event_id,
            "b",
            worker,
            persisted,
            LifecycleDeliveryPauseReason::FrozenGraphUnavailable,
        )
        .await
        .unwrap();
    assert_eq!(paused.status, LifecycleOutboxStatus::Paused);
    assert_eq!(
        paused.pause_reason,
        Some(LifecycleDeliveryPauseReason::FrozenGraphUnavailable)
    );
    assert!(paused.claim_id.is_none());
    assert!(paused.paused_at.is_some());
    // Reconstructing the repository (restart) does not delete or automatically retry a paused target.
    let restarted = PgControlPlaneStore::new(store.pool().clone());
    assert!(restarted
        .claim_lifecycle_facts(Uuid::now_v7(), 10, Duration::seconds(30))
        .await
        .unwrap()
        .is_empty());
    let states:Vec<(String,String)>=sqlx::query_as("select subscriber_id,status from lifecycle_outbox_deliveries where event_id=$1 order by subscriber_id").bind(input.event_id).fetch_all(store.pool()).await.unwrap();
    assert_eq!(
        states,
        vec![
            ("b".into(), "paused".into()),
            ("c".into(), "delivered".into())
        ]
    );
    let parent: String =
        sqlx::query_scalar("select status from lifecycle_outbox where event_id=$1")
            .bind(input.event_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(parent, "pending");
}

#[tokio::test]
async fn root_2007_ac_006_transaction_targets() {
    use control_plane_contracts::ports::{
        CreateModelDefinitionInput, LifecyclePublicationCatalog, ModelDefinitionRepository,
    };
    let store = store().await.with_lifecycle_publication_catalog(
        LifecyclePublicationCatalog::new([(
            ("model_definition.committed".into(), "v1".into()),
            publication(&["b", "c"]),
        )])
        .unwrap(),
    );
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "Transaction targets")
        .await
        .unwrap();
    // Deferred failure occurs at commit, after the real Create owner has inserted fact + targets.
    sqlx::raw_sql("create function reject_root_2007_rollback() returns trigger language plpgsql as $$ begin if new.code = 'root_2007_rollback' then raise exception 'root 2007 injected commit failure'; end if; return new; end $$; create constraint trigger root_2007_commit_failure after insert on model_definitions deferrable initially deferred for each row execute function reject_root_2007_rollback();").execute(store.pool()).await.unwrap();
    let mut input = CreateModelDefinitionInput {
        actor_user_id: Uuid::nil(),
        scope_kind: domain::DataModelScopeKind::Workspace,
        scope_id: workspace.id,
        data_source_instance_id: None,
        source_kind: domain::DataModelSourceKind::MainSource,
        external_resource_key: None,
        external_table_id: None,
        external_capability_snapshot: None,
        template_provider: "root-fixture".into(),
        template_code: "committed".into(),
        template_version: "1".into(),
        code: "root_2007_rollback".into(),
        title: "Commit rollback".into(),
        description: None,
        status: domain::DataModelStatus::Published,
        protection: domain::DataModelProtection::default(),
    };
    assert!(
        ModelDefinitionRepository::create_model_definition(&store, &input)
            .await
            .is_err()
    );
    let counts:(i64,i64,i64)=sqlx::query_as("select (select count(*) from model_definitions where code='root_2007_rollback'), (select count(*) from lifecycle_outbox), (select count(*) from lifecycle_outbox_deliveries)").fetch_one(store.pool()).await.unwrap();
    assert_eq!(counts, (0, 0, 0));
    input.code = "root_2007_commit".into();
    let model = ModelDefinitionRepository::create_model_definition(&store, &input)
        .await
        .unwrap();
    let worker = Uuid::now_v7();
    let records = store
        .claim_lifecycle_facts(worker, 10, Duration::seconds(30))
        .await
        .unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].event_id, records[1].event_id);
    assert_eq!(records[0].graph_fingerprint, "graph-v1");
    let fact: extension_contracts::AfterCommitFact<
        control_plane_contracts::ports::ModelDefinitionCommittedFact,
    > = serde_json::from_slice(&records[0].canonical_payload).unwrap();
    assert_eq!(fact.payload().model_definition_id, model.id);
    let b = records.iter().find(|r| r.subscriber_id == "b").unwrap();
    let c = records.iter().find(|r| r.subscriber_id == "c").unwrap();
    store
        .mark_lifecycle_fact_delivered(c.event_id, "c", worker, c.claim_id.unwrap())
        .await
        .unwrap();
    store
        .retry_lifecycle_fact(
            b.event_id,
            "b",
            worker,
            b.claim_id.unwrap(),
            OffsetDateTime::now_utc() - Duration::seconds(1),
            "B failed after C completed",
        )
        .await
        .unwrap();
    let retry = store
        .claim_lifecycle_facts(worker, 10, Duration::seconds(30))
        .await
        .unwrap();
    assert_eq!(retry.len(), 1);
    assert_eq!(retry[0].subscriber_id, "b");
    store
        .mark_lifecycle_fact_delivered(b.event_id, "b", worker, retry[0].claim_id.unwrap())
        .await
        .unwrap();
    let status: String =
        sqlx::query_scalar("select status from lifecycle_outbox where event_id=$1")
            .bind(b.event_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(status, "delivered");
}

#[tokio::test]
async fn root_2007_ac_007_delivery_migration_preserves_old_targets() {
    let database = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    for migration in [
        include_str!("../../migrations/20260828100000_create_lifecycle_outbox.sql"),
        include_str!("../../migrations/20260828190000_add_lifecycle_subscriber_deliveries.sql"),
    ] {
        sqlx::raw_sql(migration).execute(&pool).await.unwrap();
    }
    let event = Uuid::now_v7();
    sqlx::query("insert into lifecycle_outbox(event_id,transaction_id,contract_id,contract_version,canonical_payload,occurred_at,graph_fingerprint) values ($1,$1,'old.fact','v1','old',now(),'old-graph')").bind(event).execute(&pool).await.unwrap();
    sqlx::query("insert into lifecycle_outbox_deliveries(event_id,subscriber_id,handler_id,handler_version,status,claimed_by,claimed_at,delivered_at,attempt_count) values ($1,'pending','old-handler','old-version','pending',null,null,null,0),($1,'claimed','old-handler','old-version','claimed',$1,now(),null,2),($1,'delivered','old-handler','old-version','delivered',null,null,now(),3)").bind(event).execute(&pool).await.unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations/20260908030000_managed_event_delivery_state.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let rows:Vec<(String,String,String,i32)>=sqlx::query_as("select subscriber_id,status,handler_version,attempt_count from lifecycle_outbox_deliveries order by subscriber_id").fetch_all(&pool).await.unwrap();
    assert_eq!(
        rows,
        vec![
            ("claimed".into(), "pending".into(), "old-version".into(), 2),
            (
                "delivered".into(),
                "delivered".into(),
                "old-version".into(),
                3
            ),
            ("pending".into(), "pending".into(), "old-version".into(), 0)
        ]
    );
    let store = PgControlPlaneStore::new(pool);
    let claims = store
        .claim_lifecycle_facts(Uuid::now_v7(), 10, Duration::seconds(30))
        .await
        .unwrap();
    assert_eq!(claims.len(), 2);
    assert_ne!(claims[0].claim_id, claims[1].claim_id);
    assert!(claims.iter().all(|r| r.graph_fingerprint == "old-graph"));
}

#[tokio::test]
async fn root_2007_ac_007_concurrent_ack_rollup() {
    let store = store().await;
    let input = RecordLifecycleFactInput {
        event_id: Uuid::now_v7(),
        transaction_id: Uuid::now_v7(),
        contract_id: "concurrent.fact".into(),
        contract_version: "1".into(),
        canonical_payload: b"same-fact".to_vec(),
        occurred_at: OffsetDateTime::now_utc(),
        publication: publication(&["b", "c"]),
    };
    store.record_lifecycle_fact(&input).await.unwrap();
    let worker = Uuid::now_v7();
    let claims = store
        .claim_lifecycle_facts(worker, 2, Duration::seconds(30))
        .await
        .unwrap();
    let (first, second) = tokio::join!(
        store.mark_lifecycle_fact_delivered(
            input.event_id,
            &claims[0].subscriber_id,
            worker,
            claims[0].claim_id.unwrap()
        ),
        store.mark_lifecycle_fact_delivered(
            input.event_id,
            &claims[1].subscriber_id,
            worker,
            claims[1].claim_id.unwrap()
        ),
    );
    first.unwrap();
    second.unwrap();
    let state: String = sqlx::query_scalar("select status from lifecycle_outbox where event_id=$1")
        .bind(input.event_id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(state, "delivered");
}
