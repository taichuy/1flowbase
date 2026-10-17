use super::*;
use control_plane_contracts::ports::{CreditEventHandoff, EphemeralValueRevealMode};
use serde_json::json;

mod credit;
mod queue;

async fn fixture() -> (PgEventBus, crate::PgControlPlaneStore, Uuid) {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into());
    let database = postgres_test_support::PostgresTestSchema::create(&url)
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    crate::run_migrations(&pool).await.unwrap();
    let store = crate::PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store.upsert_workspace(tenant.id, "EventBus").await.unwrap();
    (PgEventBus::new(pool), store, workspace.id)
}

async fn source(bus: &PgEventBus, workspace: Uuid, published: bool) -> (Uuid, Value) {
    let id = Uuid::now_v7();
    let occurred = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let body =
        json!({"reason":"permission_denied", "nested":[null, true, {"text":"换行\nquote\""}]});
    sqlx::query("insert into credit_event_outbox(event_id,workspace_id,event_type,payload,created_at,published_at) values ($1,$2,'CreditCommandRejected',$3,$4,case when $5 then $4 else null end)")
        .bind(id).bind(workspace).bind(&body).bind(occurred).bind(published).execute(&bus.pool).await.unwrap();
    (
        id,
        json!({"event_id":id,"event_type":"CreditCommandRejected","workspace_id":workspace,"account_id":null,"occurred_at":occurred,"payload":body}),
    )
}

async fn count(bus: &PgEventBus, table: &str) -> i64 {
    match table {
        "pending" => sqlx::query_scalar("select count(*) from host_event_bus_pending")
            .fetch_one(&bus.pool)
            .await
            .unwrap(),
        "receipts" => sqlx::query_scalar("select count(*) from credit_event_handoffs")
            .fetch_one(&bus.pool)
            .await
            .unwrap(),
        _ => panic!("unknown fixture counter"),
    }
}
