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
    bounded_governance_history(&store).await;
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

// Extends the fixed Root claim/backlog fixture; no second test execution or new required name.
async fn bounded_governance_history(store: &PgControlPlaneStore) {
    use control_plane_contracts::ports::*;
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "Bounded governance")
        .await
        .unwrap()
        .id;
    let other_workspace = Uuid::now_v7();
    let user = Uuid::now_v7();
    sqlx::query("insert into users(id,account,email,password_hash,name,nickname,status) values($1,$2,$3,'x','Bounded','Bounded','active')")
        .bind(user).bind(format!("bounded-{user}")).bind(format!("bounded-{user}@example.test"))
        .execute(store.pool()).await.unwrap();
    let installation = Uuid::now_v7();
    let contribution = "acme.bounded.events";
    sqlx::query("insert into extension_installations(id,category,organization,artifact_id,artifact_version,plugin_id,contract_version,protocol,display_name,source_kind,trust_level,verification_status,desired_state,signature_status,metadata_json,created_by) values($1,'runtime-extensions','acme','bounded','1.0.0','acme.bounded','1flowbase.extension-bus/v1','stdio_json','Bounded','uploaded','unverified','valid','active_requested','missing',$2,$3)")
        .bind(installation).bind(serde_json::json!({"managed":{"module":{"contributions":[{"contribution_id":contribution}]}}}))
        .bind(user).execute(store.pool()).await.unwrap();
    sqlx::query("insert into plugin_contribution_authorization_revisions(installation_id,workspace_id) values($1,$2)")
        .bind(installation).bind(workspace).execute(store.pool()).await.unwrap();
    let subscriber = format!("managed.{workspace}.{contribution}");
    let version = format!(
        "{}:sha256:{}:sha256:{}:1:{installation}",
        Uuid::now_v7(),
        "a".repeat(64),
        "b".repeat(64)
    );
    let ids = (1..=MANAGED_DELIVERY_PAGE_LIMIT + 40)
        .map(|i| Uuid::from_u128(i as u128))
        .collect::<Vec<_>>();
    sqlx::query("insert into lifecycle_outbox(event_id,transaction_id,contract_id,contract_version,canonical_payload,occurred_at,graph_fingerprint,status,delivered_at) select id,id,'model_definition.committed','v1',decode('ff','hex'),now(),'bounded-graph','delivered',now() from unnest($1::uuid[]) id")
        .bind(&ids).execute(store.pool()).await.unwrap();
    sqlx::query("insert into lifecycle_outbox_deliveries(event_id,subscriber_id,handler_id,handler_version,status,delivered_at) select id,$2,$2,$3,'delivered',now() from unnest($1::uuid[]) id")
        .bind(&ids).bind(&subscriber).bind(&version).execute(store.pool()).await.unwrap();
    let mut input = RecordLifecycleFactInput {
        event_id: Uuid::now_v7(),
        transaction_id: Uuid::now_v7(),
        contract_id: "model_definition.committed".into(),
        contract_version: "v1".into(),
        canonical_payload: Vec::new(),
        occurred_at: OffsetDateTime::now_utc(),
        publication: LifecyclePublicationPlan {
            graph_fingerprint: "bounded-graph".into(),
            subscribers: vec![LifecycleSubscriberTarget {
                subscriber_id: subscriber.clone(),
                handler_id: subscriber.clone(),
                handler_version: version.clone(),
            }],
        },
    };
    let fact = extension_contracts::AfterCommitFact::new(
        extension_contracts::LifecycleFactId::new(input.event_id.to_string()).unwrap(),
        extension_contracts::LifecycleTransactionId::new(input.transaction_id.to_string()).unwrap(),
        1,
        ModelDefinitionCommittedFact {
            model_definition_id: Uuid::now_v7(),
            scope_kind: domain::DataModelScopeKind::Workspace,
            scope_id: workspace,
        },
    );
    input.canonical_payload = serde_json::to_vec(&fact).unwrap();
    store.record_lifecycle_fact(&input).await.unwrap();
    let exact = ResumeManagedLifecycleDelivery {
        event_id: input.event_id,
        subscriber_id: subscriber.clone(),
        expected: ManagedFrozenExecutionTarget {
            graph_fingerprint: input.publication.graph_fingerprint.clone(),
            handler_id: subscriber.clone(),
            handler_version: version.clone(),
        },
    };
    // IR-F01 old-data compatibility: an earlier installer could overwrite the manifest while
    // retaining this installation and its delivery rows. No new ownership write can repair it;
    // reads and subsequent revoke/disable must use the original frozen delivery identity.
    sqlx::query("update extension_installations set metadata_json=jsonb_set(metadata_json,'{managed,module,contributions}','[]'::jsonb) where id=$1")
        .bind(installation).execute(store.pool()).await.unwrap();
    let page = store
        .managed_lifecycle_delivery_page(installation, workspace)
        .await
        .unwrap();
    assert!(page.truncated);
    assert_eq!(page.deliveries.len(), MANAGED_DELIVERY_PAGE_LIMIT);
    assert!(page
        .deliveries
        .iter()
        .all(|row| row.event_id != input.event_id));
    assert!(page
        .deliveries
        .iter()
        .all(|row| row.subscriber_id == subscriber));

    // SQL may conservatively admit an unrecognized version, but must not hide malformed
    // legacy ownership merely because its last component happens to name another install.
    let foreign_id = Uuid::now_v7();
    let foreign_subscriber = format!("managed.{workspace}.aaa-retired-contribution");
    let foreign_version = format!("{}:{foreign_id}", version.rsplit_once(':').unwrap().0);
    let overflow = foreign_version.replacen(":1:", ":18446744073709551616:", 1);
    let huge_generation = foreign_version.replacen(":1:", ":999999999999999999999999:", 1);
    let invalid_algorithm = foreign_version.replacen(":sha256:", ":SHA256:", 1);
    for (candidate, expected_legacy) in [
        (foreign_version.clone(), false),
        (overflow, true),
        (huge_generation, true),
        (invalid_algorithm, true),
        (format!("{foreign_version}\n"), true),
        (
            format!("bad-epoch:{}", foreign_version.split_once(':').unwrap().1),
            true,
        ),
    ] {
        assert_eq!(
            managed_handler_installation(&candidate).is_none(),
            expected_legacy
        );
        sqlx::query("insert into lifecycle_outbox_deliveries(event_id,subscriber_id,handler_id,handler_version,status,delivered_at) values($1,$2,$2,$3,'delivered',now()) on conflict(event_id,subscriber_id) do update set handler_version=excluded.handler_version")
            .bind(ids[0]).bind(&foreign_subscriber).bind(&candidate)
            .execute(store.pool()).await.unwrap();
        let observed = store
            .managed_lifecycle_delivery_page(installation, workspace)
            .await
            .unwrap();
        let legacy = observed
            .deliveries
            .iter()
            .find(|row| row.subscriber_id == foreign_subscriber);
        assert_eq!(legacy.is_some(), expected_legacy, "version={candidate}");
        if let Some(legacy) = legacy {
            assert_eq!(legacy.ownership, "unknown_legacy");
        }
    }
    // This auxiliary row only probes the SQL/parser boundary; the genuine old target remains.
    sqlx::query("delete from lifecycle_outbox_deliveries where event_id=$1 and subscriber_id=$2")
        .bind(ids[0])
        .bind(&foreign_subscriber)
        .execute(store.pool())
        .await
        .unwrap();
    assert_eq!(
        store
            .managed_lifecycle_delivery(installation, workspace, &exact)
            .await
            .unwrap()
            .unwrap()
            .canonical_payload,
        input.canonical_payload,
        "one exact target survives unrelated invalid historical payloads beyond the display window"
    );
    assert!(store
        .managed_lifecycle_delivery(installation, other_workspace, &exact)
        .await
        .unwrap()
        .is_none());
    assert!(store
        .managed_lifecycle_delivery(Uuid::now_v7(), workspace, &exact)
        .await
        .unwrap()
        .is_none());
    let mut wrong = exact.clone();
    wrong.expected.handler_version.push('x');
    assert!(store
        .managed_lifecycle_delivery(installation, workspace, &wrong)
        .await
        .unwrap()
        .is_none());
    assert!(store
        .managed_installation_has_backlog(
            installation,
            Some(workspace),
            Some(&[exact.expected.clone()])
        )
        .await
        .unwrap());
    assert!(
        store
            .managed_installation_has_backlog(installation, None, None)
            .await
            .unwrap(),
        "late pending blocks deletion"
    );
    assert!(!store
        .managed_installation_has_backlog(installation, Some(other_workspace), None)
        .await
        .unwrap());
    assert!(!store
        .managed_installation_has_backlog(installation, Some(workspace), Some(&[wrong.expected]))
        .await
        .unwrap());
    assert!(store
        .lifecycle_target_has_backlog(
            workspace,
            "bounded-graph",
            &input.publication.subscribers[0]
        )
        .await
        .unwrap());
    assert!(!store
        .lifecycle_target_has_backlog(
            other_workspace,
            "bounded-graph",
            &input.publication.subscribers[0]
        )
        .await
        .unwrap());

    // Native IDs carry no workspace prefix; only the canonical fact scope distinguishes them.
    let native = LifecycleSubscriberTarget {
        subscriber_id: "native-bounded".into(),
        handler_id: "native-bounded".into(),
        handler_version: "1".into(),
    };
    sqlx::query("insert into lifecycle_outbox_deliveries(event_id,subscriber_id,handler_id,handler_version) values($1,$2,$3,$4)")
        .bind(input.event_id).bind(&native.subscriber_id).bind(&native.handler_id).bind(&native.handler_version)
        .execute(store.pool()).await.unwrap();
    assert!(store
        .lifecycle_target_has_backlog(workspace, "bounded-graph", &native)
        .await
        .unwrap());
    assert!(!store
        .lifecycle_target_has_backlog(other_workspace, "bounded-graph", &native)
        .await
        .unwrap());
    assert!(!store
        .lifecycle_target_has_backlog(workspace, "other-graph", &native)
        .await
        .unwrap());
    sqlx::query("update lifecycle_outbox_deliveries set status='delivered',delivered_at=now() where event_id=$1")
        .bind(input.event_id).execute(store.pool()).await.unwrap();
    assert!(
        !store
            .managed_installation_has_backlog(installation, None, None)
            .await
            .unwrap(),
        "delivered history is excluded from guard work"
    );

    let processed_event = Uuid::now_v7();
    let processed_transaction = Uuid::now_v7();
    let processed: extension_contracts::ManagedEventFact = serde_json::from_value(serde_json::json!({
        "event_id":processed_event,"transaction_id":processed_transaction,
        "contract_id":"acme.composition-a.processed","contract_version":"1","workspace_id":workspace,
        "publisher":{"subject":{"installation_id":installation,"workspace_id":workspace,"contribution_id":contribution},
            "artifact_fingerprint":format!("sha256:{}", "a".repeat(64)),"binding_fingerprint":format!("sha256:{}", "b".repeat(64))},
        "causation_id":input.event_id,"correlation_id":input.event_id,
        "payload":{"model_id":Uuid::now_v7(),"status":"processed","result_reference":null}
    })).unwrap();
    store
        .record_lifecycle_fact(&RecordLifecycleFactInput {
            event_id: processed_event,
            transaction_id: processed_transaction,
            contract_id: processed.contract_id.clone(),
            contract_version: processed.contract_version.clone(),
            canonical_payload: serde_json::to_vec(&processed).unwrap(),
            occurred_at: OffsetDateTime::now_utc(),
            publication: LifecyclePublicationPlan {
                graph_fingerprint: "bounded-graph".into(),
                subscribers: vec![native.clone()],
            },
        })
        .await
        .unwrap();
    assert!(store
        .lifecycle_target_has_backlog(workspace, "bounded-graph", &native)
        .await
        .unwrap());
    assert!(
        !store
            .lifecycle_target_has_backlog(other_workspace, "bounded-graph", &native)
            .await
            .unwrap(),
        "processed identity reads publisher.subject.workspace_id, not a flattened alias"
    );
    assert!(store
        .managed_lifecycle_delivery_page(installation, other_workspace)
        .await
        .unwrap()
        .deliveries
        .is_empty());

    // Owner uncertainty remains conservative even when all prior completed rows exceed the page.
    sqlx::query("update lifecycle_outbox_deliveries set status='pending',delivered_at=null,handler_version='legacy-unknown' where event_id=$1 and subscriber_id=$2")
        .bind(input.event_id).bind(&subscriber).execute(store.pool()).await.unwrap();
    assert!(store
        .managed_installation_has_backlog(installation, Some(workspace), Some(&[]))
        .await
        .unwrap());
    assert!(store
        .managed_installation_has_backlog(installation, None, None)
        .await
        .unwrap());
    assert!(store
        .managed_lifecycle_delivery(installation, workspace, &exact)
        .await
        .unwrap()
        .is_none());

    // An unfinished inspection is explicitly Busy, not an empty target proof.
    sqlx::query("update lifecycle_outbox_deliveries set status='delivered',delivered_at=now() where event_id=$1")
        .bind(input.event_id).execute(store.pool()).await.unwrap();
    let large_ids = (1000..5100).map(Uuid::from_u128).collect::<Vec<_>>();
    sqlx::query("insert into lifecycle_outbox(event_id,transaction_id,contract_id,contract_version,canonical_payload,occurred_at,graph_fingerprint) select id,id,'model_definition.committed','v1',decode('ff','hex'),now(),'bounded-other-graph' from unnest($1::uuid[]) id")
        .bind(&large_ids).execute(store.pool()).await.unwrap();
    sqlx::query("insert into lifecycle_outbox_deliveries(event_id,subscriber_id,handler_id,handler_version) select id,$2,$2,$3 from unnest($1::uuid[]) id")
        .bind(&large_ids).bind(&subscriber).bind(&version).execute(store.pool()).await.unwrap();
    assert!(store
        .managed_installation_has_backlog(installation, Some(workspace), Some(&[exact.expected]))
        .await
        .unwrap_err()
        .is::<ManagedLifecycleBacklogCheckBusy>());

    // F07: the real revoke/disable transaction owners pause all batches atomically.
    sqlx::query("insert into workspaces(id,tenant_id,name) values($1,$2,'Other pause scope')")
        .bind(other_workspace)
        .bind(tenant.id)
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query("insert into plugin_contribution_authorization_revisions(installation_id,workspace_id) values($1,$2)")
        .bind(installation).bind(other_workspace).execute(store.pool()).await.unwrap();
    let other_contribution = "acme.bounded.other";
    sqlx::query("update extension_installations set metadata_json=jsonb_set(metadata_json,'{managed,module,contributions}',(metadata_json#>'{managed,module,contributions}')||jsonb_build_array(jsonb_build_object('contribution_id',$2::text))) where id=$1")
        .bind(installation).bind(other_contribution).execute(store.pool()).await.unwrap();
    let cross_workspace_subscriber = format!("managed.{other_workspace}.{contribution}");
    let other_contribution_subscriber = format!("managed.{workspace}.{other_contribution}");
    for extra_subscriber in [&cross_workspace_subscriber, &other_contribution_subscriber] {
        sqlx::query("insert into lifecycle_outbox_deliveries(event_id,subscriber_id,handler_id,handler_version) values($1,$2,$2,$3)")
            .bind(large_ids[0]).bind(extra_subscriber).bind(&version).execute(store.pool()).await.unwrap();
    }
    let other_installation = Uuid::now_v7();
    let other_version = format!(
        "{}:{other_installation}",
        version.rsplit_once(':').unwrap().0
    );
    assert_eq!(
        managed_handler_installation(&other_version),
        Some(other_installation)
    );
    sqlx::query("update lifecycle_outbox_deliveries set handler_version=$3 where event_id=$1 and subscriber_id=$2")
        .bind(large_ids[1]).bind(&subscriber).bind(&other_version).execute(store.pool()).await.unwrap();
    // A whole later page belongs to another installation; the keyset must still advance.
    sqlx::query("update lifecycle_outbox_deliveries set handler_version=$3 where event_id=any($1) and subscriber_id=$2")
        .bind(&large_ids[768..1024]).bind(&subscriber).bind(&other_version).execute(store.pool()).await.unwrap();
    let known_other_count = 257_i64;
    sqlx::query("update lifecycle_outbox_deliveries set handler_version='legacy-unknown' where event_id=$1 and subscriber_id=$2")
        .bind(large_ids[2]).bind(&subscriber).execute(store.pool()).await.unwrap();
    let claimed_by = Uuid::now_v7();
    let claim_id = Uuid::now_v7();
    sqlx::query("update lifecycle_outbox_deliveries set status='claimed',claimed_by=$3,claimed_at=now(),claim_id=$4,claim_expires_at=now()+interval '30 seconds' where event_id=$1 and subscriber_id=$2")
        .bind(large_ids[0]).bind(&subscriber).bind(claimed_by).bind(claim_id).execute(store.pool()).await.unwrap();
    let authorization = Uuid::now_v7();
    sqlx::query("update plugin_contribution_authorization_revisions set revision=1 where installation_id=$1 and workspace_id=$2")
        .bind(installation).bind(workspace).execute(store.pool()).await.unwrap();
    sqlx::query("insert into plugin_contribution_authorizations(id,installation_id,workspace_id,contribution_id,point_id,permission,resource_scope,permission_contract_id,permission_contract_version,status,granted_by,revision) values($1,$2,$3,$4,'1flowbase.application.runtime-event.after-commit','event.subscribe','{\"kind\":\"workspace\"}'::jsonb,'managed-event','1','active',$5,1)")
        .bind(authorization).bind(installation).bind(workspace).bind(contribution).bind(user)
        .execute(store.pool()).await.unwrap();
    let audit_id = Uuid::now_v7();
    let revoke = RevokeContributionAuthorizationInput {
        installation_id: installation,
        workspace_id: workspace,
        authorization_id: authorization,
        expected_revision: 1,
        actor_user_id: user,
        audit_log: domain::AuditLogRecord {
            id: audit_id,
            workspace_id: Some(workspace),
            actor_user_id: Some(user),
            target_type: "plugin_installation".into(),
            target_id: Some(installation),
            event_code: "extension_center.contribution_authorizations.revoke".into(),
            payload: serde_json::json!({}),
            created_at: OffsetDateTime::now_utc(),
        },
    };
    let failure_trigger = format!(
        r#"
        create function fail_later_pause_batch() returns trigger language plpgsql as $$
        begin
          if new.event_id='{}'::uuid and new.subscriber_id='{}' and new.status='paused' then
            if (select count(*) from lifecycle_outbox_deliveries
                where subscriber_id='{}' and pause_reason=new.pause_reason) < 256 then
              raise exception 'fixture did not observe a completed earlier batch';
            end if;
            raise exception 'injected later pause batch failure';
          end if;
          return new;
        end $$;
        create trigger fail_later_pause_batch before update on lifecycle_outbox_deliveries
          for each row execute function fail_later_pause_batch();
    "#,
        large_ids[512], subscriber, subscriber
    );
    let remove_trigger = "drop trigger fail_later_pause_batch on lifecycle_outbox_deliveries; drop function fail_later_pause_batch();";
    sqlx::raw_sql(&failure_trigger)
        .execute(store.pool())
        .await
        .unwrap();
    let failed = store
        .revoke_contribution_authorization(&revoke)
        .await
        .unwrap_err();
    assert!(
        failed
            .to_string()
            .contains("injected later pause batch failure"),
        "{failed:#}"
    );
    let authority_state:(String,i64)=sqlx::query_as("select a.status,r.revision from plugin_contribution_authorizations a join plugin_contribution_authorization_revisions r using(installation_id,workspace_id) where a.id=$1")
        .bind(authorization).fetch_one(store.pool()).await.unwrap();
    assert_eq!(
        authority_state,
        ("active".into(), 1),
        "grant and revision roll back with earlier pause batches"
    );
    let changed:i64=sqlx::query_scalar("select count(*) from lifecycle_outbox_deliveries where subscriber_id=$1 and pause_reason is not null")
        .bind(&subscriber).fetch_one(store.pool()).await.unwrap();
    assert_eq!(changed, 0);
    let retained_claim: Option<Uuid> = sqlx::query_scalar(
        "select claim_id from lifecycle_outbox_deliveries where event_id=$1 and subscriber_id=$2",
    )
    .bind(large_ids[0])
    .bind(&subscriber)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(retained_claim, Some(claim_id));
    let audit_count: i64 = sqlx::query_scalar("select count(*) from audit_logs where id=$1")
        .bind(audit_id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(audit_count, 0);
    sqlx::raw_sql(remove_trigger)
        .execute(store.pool())
        .await
        .unwrap();
    assert_eq!(
        store
            .revoke_contribution_authorization(&revoke)
            .await
            .unwrap()
            .revision,
        2
    );
    let paused:i64=sqlx::query_scalar("select count(*) from lifecycle_outbox_deliveries where event_id=any($1) and subscriber_id=$2 and status='paused' and pause_reason='authority_revoked' and claimed_by is null and claimed_at is null and claim_id is null and claim_expires_at is null")
        .bind(&large_ids).bind(&subscriber).fetch_one(store.pool()).await.unwrap();
    assert_eq!(
        paused,
        large_ids.len() as i64 - known_other_count,
        "all batches pause, including legacy; known other installation stays excluded"
    );
    for excluded in [&cross_workspace_subscriber, &other_contribution_subscriber] {
        let status: String = sqlx::query_scalar(
            "select status from lifecycle_outbox_deliveries where event_id=$1 and subscriber_id=$2",
        )
        .bind(large_ids[0])
        .bind(excluded)
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert_eq!(
            status, "pending",
            "revoke does not cross workspace or contribution"
        );
    }
    let disable = UpdatePluginDesiredStateInput {
        installation_id: installation,
        desired_state: domain::PluginDesiredState::Disabled,
        actor_user_id: user,
    };
    sqlx::raw_sql(&failure_trigger)
        .execute(store.pool())
        .await
        .unwrap();
    let failed = store.update_desired_state(&disable).await.unwrap_err();
    assert!(
        failed
            .to_string()
            .contains("injected later pause batch failure"),
        "{failed:#}"
    );
    let desired: String =
        sqlx::query_scalar("select desired_state from extension_installations where id=$1")
            .bind(installation)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(
        desired, "active_requested",
        "disable intent rolls back with pause batches"
    );
    let partial:i64=sqlx::query_scalar("select count(*) from lifecycle_outbox_deliveries where pause_reason='installation_inactive'")
        .fetch_one(store.pool()).await.unwrap();
    assert_eq!(partial, 0);
    sqlx::raw_sql(remove_trigger)
        .execute(store.pool())
        .await
        .unwrap();
    assert_eq!(
        store
            .update_desired_state(&disable)
            .await
            .unwrap()
            .desired_state,
        domain::PluginDesiredState::Disabled
    );
    let paused:i64=sqlx::query_scalar("select count(*) from lifecycle_outbox_deliveries where event_id=any($1) and status='paused' and pause_reason='installation_inactive'")
        .bind(&large_ids).fetch_one(store.pool()).await.unwrap();
    assert_eq!(
        paused,
        large_ids.len() as i64 - known_other_count + 2,
        "installation disable includes both historical scopes and all contributions"
    );
    let other_status:(String,Option<String>)=sqlx::query_as("select status,pause_reason from lifecycle_outbox_deliveries where event_id=$1 and subscriber_id=$2")
        .bind(large_ids[1]).bind(&subscriber).fetch_one(store.pool()).await.unwrap();
    assert_eq!(other_status, ("pending".into(), None));
    let untouched:i64=sqlx::query_scalar("select count(*) from lifecycle_outbox_deliveries where event_id=any($1) and status='delivered' and pause_reason is null")
        .bind(&ids).fetch_one(store.pool()).await.unwrap();
    assert_eq!(
        untouched,
        ids.len() as i64,
        "completed history remains unchanged"
    );
}

/// Root #2014: workspace cleanup for a new event never depends on an example contract name.
#[tokio::test]
async fn root_2014_generic_event_history_scope_is_verified_or_conservative() {
    use control_plane_contracts::ports::ManagedLifecycleOutboxRepository;
    let store = store().await;
    let workspace = Uuid::now_v7();
    let event = Uuid::now_v7();
    let transaction = Uuid::now_v7();
    let identity = extension_contracts::ManagedExecutionIdentity::new(
        extension_contracts::ManagedInstallationId::new(Uuid::now_v7().to_string()).unwrap(),
        extension_contracts::ManagedWorkspaceId::new(workspace.to_string()).unwrap(),
        extension_contracts::ContributionId::new("orion.shipments.publish").unwrap(),
        extension_contracts::ManagedArtifactFingerprint::from_bytes(b"artifact"),
        extension_contracts::ManagedBindingFingerprint::from_bytes(b"binding"),
    );
    let fact = extension_contracts::ManagedEventFact {
        event_id: event.to_string(),
        transaction_id: transaction.to_string(),
        contract_id: "orion.shipments.created".into(),
        contract_version: "9".into(),
        workspace_id: workspace.to_string(),
        publisher: identity,
        causation_id: Uuid::now_v7().to_string(),
        correlation_id: Uuid::now_v7().to_string(),
        payload: serde_json::json!({"shipment_id":"s1","destination_code":"NYC","item_count":3}),
    };
    let plan = publication(&["native_observer"]);
    let target = plan.subscribers[0].clone();
    let graph = plan.graph_fingerprint.clone();
    store
        .record_lifecycle_fact(&RecordLifecycleFactInput {
            event_id: event,
            transaction_id: transaction,
            contract_id: fact.contract_id.clone(),
            contract_version: fact.contract_version.clone(),
            canonical_payload: serde_json::to_vec(&fact).unwrap(),
            occurred_at: OffsetDateTime::now_utc(),
            publication: plan,
        })
        .await
        .unwrap();
    assert!(store
        .lifecycle_target_has_backlog(workspace, &graph, &target)
        .await
        .unwrap());
    assert!(!store
        .lifecycle_target_has_backlog(Uuid::now_v7(), &graph, &target)
        .await
        .unwrap());
    sqlx::query("update lifecycle_outbox set canonical_payload=$2 where event_id=$1")
        .bind(event)
        .bind(br#"{"unknown":true}"#.as_slice())
        .execute(store.pool())
        .await
        .unwrap();
    assert!(store
        .lifecycle_target_has_backlog(Uuid::now_v7(), &graph, &target)
        .await
        .unwrap());
}
