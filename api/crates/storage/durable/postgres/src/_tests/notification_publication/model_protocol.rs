use super::*;
use control_plane_contracts::ports::ModelDefinitionCommittedFact;
use serde_json::Value;

fn model_input(
    scope_kind: domain::DataModelScopeKind,
) -> (PublishOptionalNotificationInput, Vec<u8>) {
    let event_id = Uuid::now_v7();
    let transaction_id = Uuid::now_v7();
    let occurred_at = OffsetDateTime::now_utc();
    let model_id = Uuid::now_v7();
    let scope_id = Uuid::now_v7();
    let timestamp = (occurred_at.unix_timestamp_nanos() / 1_000_000) as i64;
    let original = AfterCommitFact::new(
        LifecycleFactId::new(event_id.to_string()).unwrap(),
        LifecycleTransactionId::new(transaction_id.to_string()).unwrap(),
        timestamp,
        ModelDefinitionCommittedFact {
            model_definition_id: model_id,
            scope_kind,
            scope_id,
        },
    );
    let notification = AfterCommitFact::new(
        LifecycleFactId::new(event_id.to_string()).unwrap(),
        LifecycleTransactionId::new(transaction_id.to_string()).unwrap(),
        timestamp,
        ModelDefinitionIntegrationNotification {
            model_definition_id: model_id,
            scope_kind,
            scope_id,
        },
    );
    (
        PublishOptionalNotificationInput {
            source_event_id: event_id,
            transaction_id,
            workspace_id: (scope_kind == domain::DataModelScopeKind::Workspace).then_some(scope_id),
            occurred_at,
            contract: OptionalNotificationContract::ModelDefinition,
            canonical_payload: serde_json::to_vec(&notification).unwrap(),
        },
        serde_json::to_vec(&original).unwrap(),
    )
}

async fn seed_source(
    store: &PgControlPlaneStore,
    input: &PublishOptionalNotificationInput,
    body: &[u8],
) {
    sqlx::query("insert into lifecycle_outbox(event_id,transaction_id,contract_id,contract_version,canonical_payload,occurred_at) values($1,$2,'model_definition.committed','v1',$3,$4)")
        .bind(input.source_event_id).bind(input.transaction_id).bind(body)
        .bind(input.occurred_at).execute(store.pool()).await.unwrap();
}

async fn transport_counts(store: &PgControlPlaneStore) -> (i64, i64) {
    sqlx::query_as("select (select count(*) from lifecycle_outbox where contract_id='model_definition.integration_notification'),(select count(*) from lifecycle_outbox_deliveries)")
        .fetch_one(store.pool()).await.unwrap()
}

#[tokio::test]
async fn model_optional_zero_retains_original_source_without_transport() {
    let store = fixture().await;
    for scope in [
        domain::DataModelScopeKind::Workspace,
        domain::DataModelScopeKind::System,
    ] {
        let (input, original) = model_input(scope);
        seed_source(&store, &input, &original).await;
        for _ in 0..2 {
            assert_eq!(
                store.publish_optional_notification(&input).await.unwrap(),
                OptionalNotificationPublicationOutcome::NoTargets
            );
        }
        let retained: Vec<u8> =
            sqlx::query_scalar("select canonical_payload from lifecycle_outbox where event_id=$1")
                .bind(input.source_event_id)
                .fetch_one(store.pool())
                .await
                .unwrap();
        assert_eq!(retained, original);
        assert_eq!(transport_counts(&store).await, (0, 0));
    }
}

#[tokio::test]
async fn model_missing_source_fails_closed_without_publication() {
    let store = fixture().await;
    let (input, _) = model_input(domain::DataModelScopeKind::Workspace);
    assert!(store.publish_optional_notification(&input).await.is_err());
    assert_eq!(transport_counts(&store).await, (0, 0));
}

#[tokio::test]
async fn model_source_mismatch_rejects_identity_transaction_model_scope_and_timestamp() {
    let store = fixture().await;
    for field in [
        "row_transaction_id",
        "transaction_id",
        "fact_id",
        "model_definition_id",
        "scope_id",
        "scope_kind",
        "occurred_at_unix_ms",
    ] {
        let (input, original) = model_input(domain::DataModelScopeKind::Workspace);
        let mut body: Value = serde_json::from_slice(&original).unwrap();
        match field {
            "row_transaction_id" => {}
            "transaction_id" | "fact_id" => body[field] = serde_json::json!(Uuid::now_v7()),
            "model_definition_id" | "scope_id" => {
                body["payload"][field] = serde_json::json!(Uuid::now_v7())
            }
            "scope_kind" => body["payload"][field] = serde_json::json!("system"),
            "occurred_at_unix_ms" => body[field] = serde_json::json!(0),
            _ => unreachable!(),
        }
        let changed = serde_json::to_vec(&body).unwrap();
        seed_source(&store, &input, &changed).await;
        if field == "row_transaction_id" {
            sqlx::query("update lifecycle_outbox set transaction_id=$2 where event_id=$1")
                .bind(input.source_event_id)
                .bind(Uuid::now_v7())
                .execute(store.pool())
                .await
                .unwrap();
        }
        let before: Value =
            sqlx::query_scalar("select to_jsonb(t) from lifecycle_outbox t where event_id=$1")
                .bind(input.source_event_id)
                .fetch_one(store.pool())
                .await
                .unwrap();
        assert!(
            store.publish_optional_notification(&input).await.is_err(),
            "accepted mismatched {field}"
        );
        let after: Value =
            sqlx::query_scalar("select to_jsonb(t) from lifecycle_outbox t where event_id=$1")
                .bind(input.source_event_id)
                .fetch_one(store.pool())
                .await
                .unwrap();
        assert_eq!(before, after);
        assert_eq!(transport_counts(&store).await, (0, 0));
    }
}

#[tokio::test]
async fn model_typed_publication_reuses_frozen_targets_after_current_plan_removed() {
    let credit_plan = catalog()
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
        credit_plan,
    )])
    .unwrap();
    let store = fixture().await.with_lifecycle_publication_catalog(catalog);
    let (input, original) = model_input(domain::DataModelScopeKind::Workspace);
    seed_source(&store, &input, &original).await;
    let outcome = store.publish_optional_notification(&input).await.unwrap();
    let OptionalNotificationPublicationOutcome::Enqueued {
        event_id,
        target_count,
    } = outcome
    else {
        panic!("expected typed delivery");
    };
    assert_eq!(target_count, 2);
    let rows: Vec<(String, String, String)> = sqlx::query_as("select subscriber_id,handler_id,handler_version from lifecycle_outbox_deliveries where event_id=$1 order by subscriber_id")
        .bind(event_id).fetch_all(store.pool()).await.unwrap();
    assert_eq!(
        rows,
        vec![
            ("a".into(), "fixture.a".into(), "v1".into()),
            ("b".into(), "fixture.b".into(), "v1".into())
        ]
    );
    let changed = store
        .clone()
        .with_lifecycle_publication_catalog(LifecyclePublicationCatalog::default());
    assert_eq!(
        changed.publish_optional_notification(&input).await.unwrap(),
        outcome
    );
    assert_eq!(transport_counts(&store).await, (1, 2));
    let derived: Vec<u8> =
        sqlx::query_scalar("select canonical_payload from lifecycle_outbox where event_id=$1")
            .bind(event_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(derived, input.canonical_payload);
    let retained: Vec<u8> =
        sqlx::query_scalar("select canonical_payload from lifecycle_outbox where event_id=$1")
            .bind(input.source_event_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(retained, original);
}
