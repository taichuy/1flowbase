use super::*;
use sqlx::migrate::Migrator;
use std::borrow::Cow;

const LEDGER_VERSION: i64 = 20261017230000;
const LEDGER_SQL: &str =
    include_str!("../../../migrations/20261017230000_legacy_event_recovery_ledger.sql");

async fn pre_ledger() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into());
    let pool = postgres_test_support::PostgresTestSchema::create(&url)
        .await
        .unwrap()
        .connect()
        .await
        .unwrap();
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|m| m.version < LEDGER_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
    .run(&pool)
    .await
    .unwrap();
    pool
}
async fn snapshot(pool: &PgPool, ledger: bool) -> Value {
    let sql = if ledger {
        // Project every original field out of the consolidated representation.
        "select jsonb_build_array((select jsonb_agg(jsonb_build_object('sequence',sequence,'topic',topic,'payload',payload,'credit_event_id',credit_event_id,'value_size_bytes',value_size_bytes) order by sequence) from legacy_event_recovery_ledger where disposition='pending'),(select jsonb_agg(jsonb_build_object('event_id',credit_event_id,'sequence',sequence,'cutover_id',cutover_id,'origin',origin,'handed_off_at',handed_off_at) order by sequence) from legacy_event_recovery_ledger where credit_event_id is not null),(select jsonb_build_object('singleton',true,'last_sequence',last_sequence,'cutover_id',cutover_id,'created_at',created_at) from legacy_event_recovery_ledger where sequence=0))"
    } else {
        "select jsonb_build_array((select jsonb_agg(to_jsonb(p) order by sequence) from legacy_event_recovery_pending p),(select jsonb_agg(to_jsonb(r) order by sequence) from legacy_credit_handoff_receipts r),(select to_jsonb(s) from legacy_event_recovery_state s))"
    };
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}
async fn seed(pool: &PgPool) {
    for sequence in [1i64, 7, 12] {
        let id = Uuid::now_v7();
        let payload = format!("{{\"retained\":{sequence}, \"text\":\"保留\\u0000\"}}");
        sqlx::query("insert into legacy_credit_handoff_receipts(event_id,sequence,cutover_id,origin) select $1,$2,cutover_id,'live' from legacy_event_recovery_state")
            .bind(id).bind(sequence).execute(pool).await.unwrap();
        sqlx::query("insert into legacy_event_recovery_pending(sequence,topic,credit_event_id,payload,value_size_bytes) values($1,'credit.events',$2,$3,$4)")
            .bind(sequence).bind(id).bind(&payload).bind(payload.len() as i64).execute(pool).await.unwrap();
    }
    sqlx::query("insert into legacy_credit_handoff_receipts(event_id,sequence,cutover_id,origin) select $1,19,cutover_id,'legacy_snapshot' from legacy_event_recovery_state")
        .bind(Uuid::now_v7()).execute(pool).await.unwrap();
    for (sequence, topic) in [(20i64, "unknown.future.topic"), (21, "unknown\0topic")] {
        let payload = "{\"escaped\":\"\\u0000\", \"number\": 1e+09}";
        sqlx::query("insert into legacy_event_recovery_pending(sequence,topic,payload,value_size_bytes) values($1,$2,$3,$4)")
            .bind(sequence).bind(topic.as_bytes()).bind(payload).bind(payload.len() as i64).execute(pool).await.unwrap();
    }
    sqlx::query("update legacy_event_recovery_state set last_sequence=9001")
        .execute(pool)
        .await
        .unwrap();
}
async fn assert_relations(pool: &PgPool, migrated: bool) {
    let (old,ledger):(i64,bool)=sqlx::query_as("select (select count(*) from pg_class where relnamespace=current_schema()::regnamespace and relkind='r' and relname in ('host_event_bus_pending','credit_event_handoffs','host_event_bus_counter','legacy_event_recovery_pending','legacy_credit_handoff_receipts','legacy_event_recovery_state')),to_regclass('legacy_event_recovery_ledger') is not null")
        .fetch_one(pool).await.unwrap();
    assert_eq!((old, ledger), if migrated { (0, true) } else { (3, false) });
}
#[tokio::test]
async fn ledger_migration_preserves_exact_obligations_receipt_only_and_highwater() {
    let pool = pre_ledger().await;
    seed(&pool).await;
    let before = snapshot(&pool, false).await;
    crate::run_migrations(&pool).await.unwrap();
    assert_relations(&pool, true).await;
    assert_eq!(snapshot(&pool, true).await, before);
    let (rows,indexes,bad_digests):(i64,i64,i64)=sqlx::query_as("select (select count(*) from legacy_event_recovery_ledger),(select count(*) from pg_index where indrelid='legacy_event_recovery_ledger'::regclass),(select count(*) from legacy_event_recovery_ledger where disposition='pending' and payload_digest is distinct from sha256(convert_to(payload,'UTF8')))")
        .fetch_one(&pool).await.unwrap();
    assert_eq!((rows, indexes, bad_digests), (7, 3, 0));
    let receipt:(String,Option<String>,Option<Vec<u8>>)=sqlx::query_as("select disposition,payload,payload_digest from legacy_event_recovery_ledger where sequence=19").fetch_one(&pool).await.unwrap();
    assert_eq!(receipt, ("taken".into(), None, None));
    let recovery = PgLegacyEventRecovery::new(pool.clone());
    assert_eq!(
        recovery.poll("unknown\0topic").await.unwrap(),
        Some(json!({"escaped":"\0","number":1e9}))
    );
    recovery.fixture_import("after", json!(true)).await.unwrap();
    let high: i64 = sqlx::query_scalar(
        "select last_sequence from legacy_event_recovery_ledger where sequence=0",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(high, 9002);
    let tombstone:(String,Option<String>,Vec<u8>)=sqlx::query_as("select disposition,payload,payload_digest from legacy_event_recovery_ledger where sequence=21").fetch_one(&pool).await.unwrap();
    assert_eq!(tombstone.0, "taken");
    assert!(tombstone.1.is_none());
    assert_eq!(tombstone.2.len(), 32);
    let after = snapshot(&pool, true).await;
    crate::run_migrations(&pool).await.unwrap();
    assert_eq!(snapshot(&pool, true).await, after);
}
#[tokio::test]
async fn ledger_migration_rollback_and_lock_timeout_leave_all_source_rows_intact() {
    let pool = pre_ledger().await;
    seed(&pool).await;
    let before = snapshot(&pool, false).await;
    let mut tx = pool.begin().await.unwrap();
    sqlx::raw_sql(LEDGER_SQL).execute(&mut *tx).await.unwrap();
    tx.rollback().await.unwrap();
    assert_relations(&pool, false).await;
    assert_eq!(snapshot(&pool, false).await, before);
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("lock table legacy_credit_handoff_receipts in access share mode")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("set local lock_timeout='50ms'")
        .execute(&mut *tx)
        .await
        .unwrap();
    let error = sqlx::raw_sql(LEDGER_SQL)
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("55P03")
    );
    tx.rollback().await.unwrap();
    blocker.rollback().await.unwrap();
    assert_relations(&pool, false).await;
    assert_eq!(snapshot(&pool, false).await, before);
    crate::run_migrations(&pool).await.unwrap();
    assert_eq!(snapshot(&pool, true).await, before);
}
#[tokio::test]
async fn inconsistent_legacy_receipt_sequence_aborts_consolidation_atomically() {
    let pool = pre_ledger().await;
    // The old schema permits a generic pending row to collide with a receipt-only sequence.
    sqlx::query("insert into legacy_credit_handoff_receipts(event_id,sequence,cutover_id,origin) select $1,3,cutover_id,'live' from legacy_event_recovery_state").bind(Uuid::now_v7()).execute(&pool).await.unwrap();
    sqlx::query("insert into legacy_event_recovery_pending(sequence,topic,payload,value_size_bytes) values(3,'unknown','{}',2)").execute(&pool).await.unwrap();
    let before = snapshot(&pool, false).await;
    assert!(crate::run_migrations(&pool).await.is_err());
    assert_relations(&pool, false).await;
    assert_eq!(snapshot(&pool, false).await, before);
}
