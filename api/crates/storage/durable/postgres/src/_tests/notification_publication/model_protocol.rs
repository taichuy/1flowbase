use super::*;
use control_plane_contracts::ports::{EventBus, ModelDefinitionCommittedFact};
use serde_json::Value;

const ORIGINAL_TOPIC: &str = "official.plugin-host.lifecycle.model-definition-committed";

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
    version: i16,
) {
    // No model/workspace row is needed: the committed fact owns its delivery snapshot.
    sqlx::query("insert into lifecycle_outbox(event_id,transaction_id,contract_id,contract_version,canonical_payload,occurred_at,native_publication_version) values($1,$2,'model_definition.committed','v1',$3,$4,$5)")
        .bind(input.source_event_id).bind(input.transaction_id).bind(body)
        .bind(input.occurred_at).bind(version).execute(store.pool()).await.unwrap();
}

async fn transport_counts(store: &PgControlPlaneStore) -> (i64, i64, i64) {
    sqlx::query_as("select (select count(*) from lifecycle_outbox),(select count(*) from lifecycle_outbox_deliveries),(select count(*) from host_event_bus_pending)")
        .fetch_one(store.pool()).await.unwrap()
}

#[tokio::test]
async fn model_protocol_one_preserves_exact_original_topic_and_fact_body() {
    let store = fixture().await;
    let (input, original) = model_input(domain::DataModelScopeKind::Workspace);
    seed_source(&store, &input, &original, 1).await;
    let before: Value = sqlx::query_scalar(
        "select to_jsonb(t)-'legacy_handoff_sequence' from lifecycle_outbox t where event_id=$1",
    )
    .bind(input.source_event_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    let outcome = store.publish_optional_notification(&input).await.unwrap();
    let OptionalNotificationPublicationOutcome::LegacyRecoveryHandoff { sequence } = outcome else {
        panic!("historical source must retain original raw handoff responsibility");
    };
    assert!(sequence > 0);
    let pending: (i64, Vec<u8>, String, i64) = sqlx::query_as(
        "select sequence,topic,payload,value_size_bytes from host_event_bus_pending",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    let original_value: Value = serde_json::from_slice(&original).unwrap();
    let original_payload = serde_json::to_string(&original_value).unwrap();
    assert_eq!(
        pending,
        (
            sequence,
            ORIGINAL_TOPIC.as_bytes().to_vec(),
            original_payload.clone(),
            original_payload.len() as i64
        )
    );
    assert_eq!(
        serde_json::from_str::<Value>(&original_payload).unwrap()["contract"]["contract_id"],
        "model_definition.committed"
    );
    let receipt: Option<i64> = sqlx::query_scalar(
        "select legacy_handoff_sequence from lifecycle_outbox where event_id=$1",
    )
    .bind(input.source_event_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(receipt, Some(sequence));
    let after: Value = sqlx::query_scalar(
        "select to_jsonb(t)-'legacy_handoff_sequence' from lifecycle_outbox t where event_id=$1",
    )
    .bind(input.source_event_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        after, before,
        "handoff must not rewrite the committed source fact"
    );
    assert_eq!(transport_counts(&store).await, (1, 0, 1));
}

#[tokio::test]
async fn model_protocol_one_lost_ack_reuses_sequence_and_polled_receipt_never_resurrects() {
    let store = fixture().await;
    let (input, original) = model_input(domain::DataModelScopeKind::Workspace);
    seed_source(&store, &input, &original, 1).await;
    let first = store.publish_optional_notification(&input).await.unwrap();
    let OptionalNotificationPublicationOutcome::LegacyRecoveryHandoff { sequence } = first else {
        panic!("original source requires a durable handoff receipt");
    };
    // The caller loses its lifecycle ACK after the handoff transaction committed.
    assert_eq!(
        store.publish_optional_notification(&input).await.unwrap(),
        first
    );
    assert_eq!(transport_counts(&store).await, (1, 0, 1));
    let queue = crate::PgEventBus::new(store.pool().clone());
    assert_eq!(
        queue.poll(ORIGINAL_TOPIC).await.unwrap(),
        Some(serde_json::from_slice::<Value>(&original).unwrap())
    );
    assert!(queue.poll(ORIGINAL_TOPIC).await.unwrap().is_none());
    assert_eq!(
        store.publish_optional_notification(&input).await.unwrap(),
        first
    );
    assert_eq!(transport_counts(&store).await, (1, 0, 0));
    assert!(queue.poll(ORIGINAL_TOPIC).await.unwrap().is_none());
    let receipt: Option<i64> = sqlx::query_scalar(
        "select legacy_handoff_sequence from lifecycle_outbox where event_id=$1",
    )
    .bind(input.source_event_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(receipt, Some(sequence));
    let counter: i64 =
        sqlx::query_scalar("select last_sequence from host_event_bus_counter where singleton")
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(
        counter, sequence,
        "retry must not allocate a new transport identity"
    );
}

#[tokio::test]
async fn model_protocol_two_optional_zero_creates_no_pending_or_derived_transport() {
    let store = fixture().await;
    for (index, scope) in [
        domain::DataModelScopeKind::Workspace,
        domain::DataModelScopeKind::System,
    ]
    .into_iter()
    .enumerate()
    {
        let (input, original) = model_input(scope);
        seed_source(&store, &input, &original, 2).await;
        for _ in 0..2 {
            assert_eq!(
                store.publish_optional_notification(&input).await.unwrap(),
                OptionalNotificationPublicationOutcome::NoTargets
            );
        }
        assert_eq!(transport_counts(&store).await, (index as i64 + 1, 0, 0));
        let retained: (i16, Option<i64>, Vec<u8>) = sqlx::query_as("select native_publication_version,legacy_handoff_sequence,canonical_payload from lifecycle_outbox where event_id=$1")
            .bind(input.source_event_id).fetch_one(store.pool()).await.unwrap();
        assert_eq!(retained, (2, None, original));
    }
    let counter: i64 =
        sqlx::query_scalar("select last_sequence from host_event_bus_counter where singleton")
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(counter, 0);
}

#[tokio::test]
async fn model_protocol_missing_source_fails_closed_without_publication() {
    let store = fixture().await;
    let (input, _) = model_input(domain::DataModelScopeKind::Workspace);
    assert!(store.publish_optional_notification(&input).await.is_err());
    assert_eq!(transport_counts(&store).await, (0, 0, 0));
}

#[tokio::test]
async fn model_protocol_body_mismatch_fails_closed_for_both_versions() {
    let store = fixture().await;
    for version in [1, 2] {
        let (mut input, original) = model_input(domain::DataModelScopeKind::Workspace);
        seed_source(&store, &input, &original, version).await;
        let before: Value =
            sqlx::query_scalar("select to_jsonb(t) from lifecycle_outbox t where event_id=$1")
                .bind(input.source_event_id)
                .fetch_one(store.pool())
                .await
                .unwrap();
        let mut body: Value = serde_json::from_slice(&input.canonical_payload).unwrap();
        body["payload"]["model_definition_id"] = serde_json::json!(Uuid::now_v7());
        input.canonical_payload = serde_json::to_vec(&body).unwrap();
        let error = store
            .publish_optional_notification(&input)
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("model source body mismatch"),
            "{error:#}"
        );
        let after: Value =
            sqlx::query_scalar("select to_jsonb(t) from lifecycle_outbox t where event_id=$1")
                .bind(input.source_event_id)
                .fetch_one(store.pool())
                .await
                .unwrap();
        assert_eq!(after, before);
        assert_eq!(transport_counts(&store).await, (version as i64, 0, 0));
    }
}
