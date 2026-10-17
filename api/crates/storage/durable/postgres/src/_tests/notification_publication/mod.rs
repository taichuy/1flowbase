mod compaction;
mod model_protocol;

use super::*;
use extension_contracts::{LifecycleFactId, LifecycleTransactionId};
use time::OffsetDateTime;

async fn fixture() -> PgControlPlaneStore {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into());
    let pool = postgres_test_support::PostgresTestSchema::create(&url)
        .await
        .unwrap()
        .connect()
        .await
        .unwrap();
    crate::run_migrations(&pool).await.unwrap();
    PgControlPlaneStore::new(pool)
}
fn input() -> PublishOptionalNotificationInput {
    let event = Uuid::now_v7();
    let tx = Uuid::now_v7();
    let now = OffsetDateTime::now_utc();
    let workspace = Uuid::now_v7();
    let fact = AfterCommitFact::new(
        LifecycleFactId::new(event.to_string()).unwrap(),
        LifecycleTransactionId::new(tx.to_string()).unwrap(),
        (now.unix_timestamp_nanos() / 1_000_000) as i64,
        CreditNotificationFact {
            workspace_id: workspace,
            account_id: None,
            event_type: CreditNotificationEventType::CreditSettled,
            payload: serde_json::json!({"charged_amount":"1.25", "nested":[null, true, "原文\u{0}body"]}),
        },
    );
    PublishOptionalNotificationInput {
        source_event_id: event,
        transaction_id: tx,
        workspace_id: Some(workspace),
        occurred_at: now,
        contract: OptionalNotificationContract::Credit,
        canonical_payload: serde_json::to_vec(&fact).unwrap(),
    }
}
fn catalog() -> LifecyclePublicationCatalog {
    LifecyclePublicationCatalog::new([(
        (
            CreditNotificationFact::CONTRACT_ID.into(),
            CreditNotificationFact::CONTRACT_VERSION.into(),
        ),
        LifecyclePublicationPlan {
            graph_fingerprint: "fixture-frozen-v1".into(),
            subscribers: vec![
                LifecycleSubscriberTarget {
                    subscriber_id: "a".into(),
                    handler_id: "fixture.a".into(),
                    handler_version: "v1".into(),
                },
                LifecycleSubscriberTarget {
                    subscriber_id: "b".into(),
                    handler_id: "fixture.b".into(),
                    handler_version: "v1".into(),
                },
            ],
        },
    )])
    .unwrap()
}

#[tokio::test]
async fn optional_zero_creates_no_transport_body_and_rejects_malformed_identity() {
    let store = fixture().await;
    let input = input();
    assert_eq!(
        store.publish_optional_notification(&input).await.unwrap(),
        OptionalNotificationPublicationOutcome::NoTargets
    );
    let counts:(i64,i64,i64,i64)=sqlx::query_as("select (select count(*) from lifecycle_outbox),(select count(*) from lifecycle_outbox_deliveries),(select count(*) from legacy_event_recovery_ledger where disposition='pending'),(select count(*) from credit_event_outbox)").fetch_one(store.pool()).await.unwrap();
    assert_eq!(counts, (0, 0, 0, 0));
    let mut malformed = input.clone();
    malformed.source_event_id = Uuid::now_v7();
    assert!(
        store
            .publish_optional_notification(&malformed)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn optional_publication_freezes_targets_and_body_across_upgrade_and_zero_current_plan() {
    let store = fixture()
        .await
        .with_lifecycle_publication_catalog(catalog());
    let input = input();
    let first = store.publish_optional_notification(&input).await.unwrap();
    let OptionalNotificationPublicationOutcome::Enqueued {
        event_id,
        target_count,
    } = first
    else {
        panic!("declared targets require durable responsibility")
    };
    assert_eq!(target_count, 2);
    let body: Vec<u8> =
        sqlx::query_scalar("select canonical_payload from lifecycle_outbox where event_id=$1")
            .bind(event_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(body, input.canonical_payload);
    // Removing current targets does not adjudicate previously frozen responsibility away.
    let changed = store
        .clone()
        .with_lifecycle_publication_catalog(LifecyclePublicationCatalog::default());
    assert_eq!(
        changed.publish_optional_notification(&input).await.unwrap(),
        first
    );
    let rows:Vec<(String,String)>=sqlx::query_as("select subscriber_id,handler_version from lifecycle_outbox_deliveries where event_id=$1 order by subscriber_id").bind(event_id).fetch_all(store.pool()).await.unwrap();
    assert_eq!(
        rows,
        vec![("a".into(), "v1".into()), ("b".into(), "v1".into())]
    );
    let mut conflict = input.clone();
    let mut value: serde_json::Value = serde_json::from_slice(&conflict.canonical_payload).unwrap();
    value["payload"]["payload"]["charged_amount"] = serde_json::json!("9.99");
    conflict.canonical_payload = serde_json::to_vec(&value).unwrap();
    assert!(
        changed
            .publish_optional_notification(&conflict)
            .await
            .is_err()
    );
    let duplicates: i64 = sqlx::query_scalar(
        "select count(*) from legacy_event_recovery_ledger where disposition='pending'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(duplicates, 0);
}

struct Unavailable;
#[async_trait]
impl WorkspaceLifecyclePublicationSource for Unavailable {
    async fn plan_for_workspace(
        &self,
        _: Uuid,
        _: &str,
        _: &str,
    ) -> Result<Option<FrozenLifecyclePublication>> {
        anyhow::bail!("fixture source unavailable")
    }
}
#[tokio::test]
async fn unavailable_publication_source_is_an_error_not_optional_zero() {
    let catalog = LifecyclePublicationCatalog::default();
    catalog
        .attach_workspace_source(std::sync::Arc::new(Unavailable))
        .unwrap();
    let store = fixture().await.with_lifecycle_publication_catalog(catalog);
    assert!(
        store
            .publish_optional_notification(&input())
            .await
            .unwrap_err()
            .to_string()
            .contains("source unavailable")
    );
}
