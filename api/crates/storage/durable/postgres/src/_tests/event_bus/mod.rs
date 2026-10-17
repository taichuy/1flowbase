use super::*;
use control_plane_contracts::ports::EphemeralValueRevealMode;
use serde_json::json;

mod credit;
mod migration;
mod queue;

async fn fixture() -> (PgLegacyEventRecovery, crate::PgControlPlaneStore, Uuid) {
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
    (PgLegacyEventRecovery::new(pool), store, workspace.id)
}

async fn source(bus: &PgLegacyEventRecovery, workspace: Uuid, published: bool) -> (Uuid, Value) {
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

async fn count(bus: &PgLegacyEventRecovery, table: &str) -> i64 {
    match table {
        "pending" => sqlx::query_scalar(
            "select count(*) from legacy_event_recovery_ledger where disposition='pending'",
        )
        .fetch_one(&bus.pool)
        .await
        .unwrap(),
        "receipts" => sqlx::query_scalar(
            "select count(*) from legacy_event_recovery_ledger where credit_event_id is not null",
        )
        .fetch_one(&bus.pool)
        .await
        .unwrap(),
        _ => panic!("unknown fixture counter"),
    }
}

// Fixture-only historical writes. Production recovery exposes no generic publish
// or live handoff trait; these tests deliberately construct pre-cutover entries.
impl PgLegacyEventRecovery {
    async fn fixture_import(&self, topic: &str, value: Value) -> Result<()> {
        let payload = Self::serialize_payload(value)?;
        let mut tx = self.pool.begin().await?;
        Self::lock_queue(&mut tx).await?;
        let seq = Self::next_sequence(&mut tx).await?;
        sqlx::query("insert into legacy_event_recovery_ledger(sequence,disposition,topic,payload,payload_digest,value_size_bytes) values($1,'pending',$2,$3,sha256(convert_to($3,'UTF8')),$4)")
            .bind(seq).bind(topic.as_bytes()).bind(&payload).bind(payload.len() as i64).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
    async fn fixture_handoff(&self, limit: i64) -> Result<usize> {
        let mut tx = self.pool.begin().await?;
        Self::lock_queue(&mut tx).await?;
        let ids: Vec<Uuid> = sqlx::query_scalar("select event_id from credit_event_outbox where published_at is null and (locked_until is null or locked_until<now()) order by created_at,event_id limit $1 for update")
            .bind(limit).fetch_all(&mut *tx).await?;
        let mut inserted = 0;
        for id in ids {
            inserted += usize::from(Self::handoff_one(&mut tx, id, "live").await?);
        }
        tx.commit().await?;
        Ok(inserted)
    }
}
