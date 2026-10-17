use super::*;
use control_plane_contracts::ports::{
    DerivedLifecyclePublicationRepository, LifecycleOutboxRepository,
};
use time::Duration;

#[tokio::test]
async fn rejected_command_fact_survives_zero_targets_and_completed_target_transport() {
    use control_plane_contracts::ports::BillingRepository;
    for targeted in [false, true] {
        let store = fixture()
            .await
            .with_lifecycle_publication_catalog(if targeted {
                catalog()
            } else {
                LifecyclePublicationCatalog::default()
            });
        let tenant = store.upsert_root_tenant().await.unwrap();
        let workspace = Uuid::now_v7();
        sqlx::query(
            "insert into workspaces(id,tenant_id,name) values($1,$2,'Rejected fact fixture')",
        )
        .bind(workspace)
        .bind(tenant.id)
        .execute(store.pool())
        .await
        .unwrap();
        store
            .record_credit_command_rejected(
                workspace,
                "fixture.rejected-plugin",
                "charge",
                "保留拒绝原因",
                "reject:targets",
            )
            .await
            .unwrap();
        let original:(Uuid,serde_json::Value,OffsetDateTime,Option<OffsetDateTime>) = sqlx::query_as(
            "select event_id,payload,created_at,published_at from credit_event_outbox where workspace_id=$1 and event_type='CreditCommandRejected' and account_id is null")
            .bind(workspace).fetch_one(store.pool()).await.unwrap();
        assert_eq!(
            original.1,
            serde_json::json!({"actor_plugin_id":"fixture.rejected-plugin","command":"charge","reason":"保留拒绝原因","idempotency_key":"reject:targets"})
        );
        assert_eq!(original.3, Some(original.2));
        let facts:Vec<(Uuid,Vec<u8>)> = sqlx::query_as("select event_id,canonical_payload from lifecycle_outbox where contract_id='credit.integration_notification'")
            .fetch_all(store.pool()).await.unwrap();
        assert_eq!(facts.len(), if targeted { 1 } else { 0 });
        if let Some((event, payload)) = facts.first() {
            let fact: AfterCommitFact<CreditNotificationFact> =
                serde_json::from_slice(payload).unwrap();
            assert_eq!(fact.fact_id().as_str(), original.0.to_string());
            assert_eq!(fact.payload().payload, original.1);
            let claims = store
                .claim_lifecycle_facts(Uuid::now_v7(), 10, Duration::seconds(30))
                .await
                .unwrap();
            assert_eq!(claims.len(), 2);
            ack(&store, &claims[0]).await;
            assert!(!body(&store, *event).await.0.is_empty());
            ack(&store, &claims[1]).await;
            let compacted = body(&store, *event).await;
            assert!(compacted.0.is_empty());
            assert_eq!(compacted.1.unwrap().len(), 32);
        }
        let retained:(Uuid,serde_json::Value,OffsetDateTime,Option<OffsetDateTime>)=sqlx::query_as(
            "select event_id,payload,created_at,published_at from credit_event_outbox where event_id=$1")
            .bind(original.0).fetch_one(store.pool()).await.unwrap();
        assert_eq!(
            retained, original,
            "transport completion cannot discard the independent rejected fact"
        );
    }
}

async fn publish(store: &PgControlPlaneStore, input: &PublishOptionalNotificationInput) -> Uuid {
    match store.publish_optional_notification(input).await.unwrap() {
        OptionalNotificationPublicationOutcome::Enqueued {
            event_id,
            target_count: 2,
        } => event_id,
        outcome => panic!("expected two frozen targets, got {outcome:?}"),
    }
}
async fn body(store: &PgControlPlaneStore, event: Uuid) -> (Vec<u8>, Option<Vec<u8>>) {
    sqlx::query_as("select canonical_payload,notification_payload_digest from lifecycle_outbox where event_id=$1")
        .bind(event).fetch_one(store.pool()).await.unwrap()
}
async fn ack(store: &PgControlPlaneStore, record: &LifecycleOutboxRecord) {
    store
        .mark_lifecycle_fact_delivered(
            record.event_id,
            &record.subscriber_id,
            record.claimed_by.unwrap(),
            record.claim_id.unwrap(),
        )
        .await
        .unwrap();
}
fn recorded_input(
    event: Uuid,
    input: &PublishOptionalNotificationInput,
) -> RecordLifecycleFactInput {
    RecordLifecycleFactInput {
        event_id: event,
        transaction_id: input.transaction_id,
        contract_id: input.contract.contract_id().into(),
        contract_version: input.contract.contract_version().into(),
        canonical_payload: input.canonical_payload.clone(),
        occurred_at: input.occurred_at,
        publication: catalog()
            .plan_for(
                input.contract.contract_id(),
                input.contract.contract_version(),
            )
            .unwrap()
            .clone(),
    }
}

#[tokio::test]
async fn large_optional_body_survives_retry_pause_then_final_ack_compacts_and_replay_stays_complete()
 {
    let store = fixture()
        .await
        .with_lifecycle_publication_catalog(catalog());
    let mut input = input();
    // Align to milliseconds so the later +1 microsecond mismatch has the SAME typed
    // millisecond timestamp: only the immutable repository time check can reject it.
    input.occurred_at = OffsetDateTime::from_unix_timestamp_nanos(
        input.occurred_at.unix_timestamp_nanos() / 1_000_000 * 1_000_000,
    )
    .unwrap();
    let mut envelope: serde_json::Value = serde_json::from_slice(&input.canonical_payload).unwrap();
    envelope["payload"]["payload"]["large_source_body"] =
        serde_json::json!("原文large-body".repeat(110_000));
    input.canonical_payload = serde_json::to_vec(&envelope).unwrap();
    assert!(input.canonical_payload.len() > 1024 * 1024);
    let event = publish(&store, &input).await;
    let expected_digest = Sha256::digest(&input.canonical_payload).to_vec();
    assert_eq!(
        body(&store, event).await,
        (
            input.canonical_payload.clone(),
            Some(expected_digest.clone())
        )
    );
    let worker = Uuid::now_v7();
    let claims = store
        .claim_lifecycle_facts(worker, 10, Duration::seconds(30))
        .await
        .unwrap();
    assert_eq!(claims.len(), 2);
    let a = claims.iter().find(|r| r.subscriber_id == "a").unwrap();
    let b = claims.iter().find(|r| r.subscriber_id == "b").unwrap();
    ack(&store, a).await;
    assert_eq!(body(&store, event).await.0, input.canonical_payload);
    store
        .retry_lifecycle_fact(
            event,
            "b",
            worker,
            b.claim_id.unwrap(),
            OffsetDateTime::now_utc(),
            "recipient failed",
        )
        .await
        .unwrap();
    assert_eq!(body(&store, event).await.0, input.canonical_payload);
    let retried = store
        .claim_lifecycle_facts(worker, 10, Duration::seconds(30))
        .await
        .unwrap();
    assert_eq!(retried.len(), 1);
    store
        .pause_lifecycle_fact(
            event,
            "b",
            worker,
            retried[0].claim_id.unwrap(),
            LifecycleDeliveryPauseReason::FrozenHandlerUnavailable,
        )
        .await
        .unwrap();
    assert_eq!(body(&store, event).await.0, input.canonical_payload);
    // Explicit fixture recovery, retaining the same frozen target and canonical fact.
    sqlx::query("update lifecycle_outbox_deliveries set status='pending',available_at=now(),pause_reason=null,paused_at=null where event_id=$1 and subscriber_id='b'")
        .bind(event).execute(store.pool()).await.unwrap();
    let final_claim = store
        .claim_lifecycle_facts(worker, 10, Duration::seconds(30))
        .await
        .unwrap();
    assert_eq!(final_claim.len(), 1);
    ack(&store, &final_claim[0]).await;
    assert_eq!(body(&store, event).await, (vec![], Some(expected_digest)));
    assert_eq!(publish(&store, &input).await, event);
    assert!(
        store
            .claim_lifecycle_facts(worker, 10, Duration::seconds(30))
            .await
            .unwrap()
            .is_empty()
    );

    let record = recorded_input(event, &input);
    for mutation in [
        "body",
        "transaction",
        "timestamp",
        "graph",
        "target",
        "contract",
    ] {
        let mut conflict = record.clone();
        match mutation {
            "body" => conflict.canonical_payload.push(b' '),
            "transaction" => conflict.transaction_id = Uuid::now_v7(),
            "timestamp" => conflict.occurred_at += Duration::microseconds(1),
            "graph" => conflict.publication.graph_fingerprint = "different-frozen-graph".into(),
            "target" => conflict.publication.subscribers[0].handler_version = "v2".into(),
            "contract" => conflict.contract_version = "v2".into(),
            _ => unreachable!(),
        }
        assert!(
            store
                .record_derived_lifecycle_fact(&conflict)
                .await
                .is_err(),
            "accepted changed {mutation}"
        );
    }
    let mut changed_time = input.clone();
    changed_time.occurred_at += Duration::microseconds(1);
    assert!(
        store
            .publish_optional_notification(&changed_time)
            .await
            .is_err(),
        "optional publisher must not replace input time with stored time"
    );
    let mut changed_body = input.clone();
    changed_body.canonical_payload.push(b' '); // Same valid typed JSON, different canonical bytes.
    assert!(
        store
            .publish_optional_notification(&changed_body)
            .await
            .is_err()
    );
    assert!(body(&store, event).await.0.is_empty());
}

#[tokio::test]
async fn stale_claim_cannot_compact_optional_body_before_current_fenced_ack() {
    let store = fixture()
        .await
        .with_lifecycle_publication_catalog(catalog());
    let input = input();
    let event = publish(&store, &input).await;
    let old_worker = Uuid::now_v7();
    let first = store
        .claim_lifecycle_facts(old_worker, 10, Duration::seconds(30))
        .await
        .unwrap();
    let a = first.iter().find(|r| r.subscriber_id == "a").unwrap();
    let old_b = first.iter().find(|r| r.subscriber_id == "b").unwrap();
    ack(&store, a).await;
    sqlx::query("update lifecycle_outbox_deliveries set claimed_at=now()-interval '2 seconds',claim_expires_at=now()-interval '1 second' where event_id=$1 and subscriber_id='b'")
        .bind(event).execute(store.pool()).await.unwrap();
    let current = store
        .claim_lifecycle_facts(Uuid::now_v7(), 10, Duration::seconds(30))
        .await
        .unwrap();
    assert_eq!(current.len(), 1);
    assert!(
        store
            .mark_lifecycle_fact_delivered(event, "b", old_worker, old_b.claim_id.unwrap())
            .await
            .unwrap_err()
            .downcast_ref::<LifecycleClaimLost>()
            .is_some()
    );
    assert_eq!(body(&store, event).await.0, input.canonical_payload);
    ack(&store, &current[0]).await;
    assert!(body(&store, event).await.0.is_empty());
    assert!(
        store
            .mark_lifecycle_fact_delivered(event, "b", old_worker, old_b.claim_id.unwrap())
            .await
            .is_err()
    );
    assert!(body(&store, event).await.0.is_empty());
}

#[tokio::test]
async fn original_model_and_other_managed_fact_bodies_keep_existing_retention_and_cap() {
    let store = fixture().await;
    let notification = input();
    for contract in ["model_definition.committed", "managed.fixture.processed"] {
        let mut record = recorded_input(Uuid::now_v7(), &notification);
        record.contract_id = contract.into();
        record.canonical_payload = b"original business fact body".to_vec();
        store.record_lifecycle_fact(&record).await.unwrap();
        let claims = store
            .claim_lifecycle_facts(Uuid::now_v7(), 10, Duration::seconds(30))
            .await
            .unwrap();
        assert_eq!(claims.len(), 2);
        for claim in &claims {
            ack(&store, claim).await;
        }
        assert_eq!(
            body(&store, record.event_id).await,
            (record.canonical_payload.clone(), None)
        );
        record.event_id = Uuid::now_v7();
        record.canonical_payload = vec![b'x'; 1024 * 1024 + 1];
        assert!(
            store
                .record_lifecycle_fact(&record)
                .await
                .unwrap_err()
                .to_string()
                .contains("payload exceeds")
        );
    }
}

#[tokio::test]
async fn historical_optional_fact_without_digest_retains_full_body_after_ack() {
    let store = fixture()
        .await
        .with_lifecycle_publication_catalog(catalog());
    let input = input();
    let event = publish(&store, &input).await;
    // Simulate the precise pre-F1 full-body representation; no digest is guessed.
    sqlx::query("update lifecycle_outbox set notification_payload_digest=null where event_id=$1")
        .bind(event)
        .execute(store.pool())
        .await
        .unwrap();
    let claims = store
        .claim_lifecycle_facts(Uuid::now_v7(), 10, Duration::seconds(30))
        .await
        .unwrap();
    for claim in &claims {
        ack(&store, claim).await;
    }
    assert_eq!(
        body(&store, event).await,
        (input.canonical_payload.clone(), None)
    );
    assert_eq!(publish(&store, &input).await, event);
}

#[tokio::test]
async fn model_integration_stage_compacts_without_releasing_original_model_fact() {
    let base = input();
    let source_id = base.source_event_id;
    let model_id = Uuid::now_v7();
    let scope_id = base.workspace_id.unwrap();
    let timestamp = (base.occurred_at.unix_timestamp_nanos() / 1_000_000) as i64;
    let original = AfterCommitFact::new(
        LifecycleFactId::new(source_id.to_string()).unwrap(),
        LifecycleTransactionId::new(base.transaction_id.to_string()).unwrap(),
        timestamp,
        ModelDefinitionCommittedFact {
            model_definition_id: model_id,
            scope_kind: domain::DataModelScopeKind::Workspace,
            scope_id,
        },
    );
    let original_body = serde_json::to_vec(&original).unwrap();
    let notification = AfterCommitFact::new(
        original.fact_id().clone(),
        original.transaction_id().clone(),
        timestamp,
        ModelDefinitionIntegrationNotification {
            model_definition_id: model_id,
            scope_kind: domain::DataModelScopeKind::Workspace,
            scope_id,
        },
    );
    let input = PublishOptionalNotificationInput {
        contract: OptionalNotificationContract::ModelDefinition,
        canonical_payload: serde_json::to_vec(&notification).unwrap(),
        ..base
    };
    let targets = catalog()
        .plan_for(
            CreditNotificationFact::CONTRACT_ID,
            CreditNotificationFact::CONTRACT_VERSION,
        )
        .unwrap()
        .clone();
    let catalog = LifecyclePublicationCatalog::new([(
        (
            ModelDefinitionIntegrationNotification::CONTRACT_ID.into(),
            ModelDefinitionIntegrationNotification::CONTRACT_VERSION.into(),
        ),
        targets,
    )])
    .unwrap();
    let store = fixture().await.with_lifecycle_publication_catalog(catalog);
    sqlx::query("insert into lifecycle_outbox(event_id,transaction_id,contract_id,contract_version,canonical_payload,occurred_at,graph_fingerprint) values($1,$2,'model_definition.committed','v1',$3,$4,'original-model-graph')")
        .bind(source_id).bind(input.transaction_id).bind(&original_body).bind(input.occurred_at)
        .execute(store.pool()).await.unwrap();
    let event = publish(&store, &input).await;
    let claims = store
        .claim_lifecycle_facts(Uuid::now_v7(), 10, Duration::seconds(30))
        .await
        .unwrap();
    assert_eq!(claims.len(), 2);
    ack(&store, &claims[0]).await;
    assert_eq!(body(&store, event).await.0, input.canonical_payload);
    ack(&store, &claims[1]).await;
    assert_eq!(
        body(&store, event).await,
        (
            vec![],
            Some(Sha256::digest(&input.canonical_payload).to_vec())
        )
    );
    assert_eq!(body(&store, source_id).await, (original_body, None));
    assert_eq!(publish(&store, &input).await, event);
}
