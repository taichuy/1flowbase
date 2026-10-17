use super::*;
use sqlx::migrate::Migrator;
use std::borrow::Cow;

const RETIREMENT_VERSION: i64 = 20261017210000;
const RETIREMENT_SQL: &str =
    include_str!("../../../migrations/20261017210000_legacy_event_recovery_ownership.sql");

async fn pre_retirement() -> PgPool {
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
        .filter(|migration| migration.version < RETIREMENT_VERSION)
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

// Compare every stored column, including timestamps, exact payload text/bytea,
// receipt origin/cutover identity and counter high-water mark.
async fn snapshot(pool: &PgPool, renamed: bool) -> Value {
    let sql = if renamed {
        "select jsonb_build_array((select jsonb_agg(to_jsonb(p) order by sequence) from legacy_event_recovery_pending p),(select jsonb_agg(to_jsonb(r) order by sequence) from legacy_credit_handoff_receipts r),(select to_jsonb(s) from legacy_event_recovery_state s),(select jsonb_agg(to_jsonb(c) order by event_id) from credit_event_outbox c))"
    } else {
        "select jsonb_build_array((select jsonb_agg(to_jsonb(p) order by sequence) from host_event_bus_pending p),(select jsonb_agg(to_jsonb(r) order by sequence) from credit_event_handoffs r),(select to_jsonb(s) from host_event_bus_counter s),(select jsonb_agg(to_jsonb(c) order by event_id) from credit_event_outbox c))"
    };
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

async fn seed(pool: &PgPool) {
    let store = crate::PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "Recovery migration")
        .await
        .unwrap();
    for sequence in 1..=18_i64 {
        let id = Uuid::now_v7();
        let body = json!({"event_id": id, "retained": sequence, "text": "保留 snapshot"});
        let payload = serde_json::to_string(&body).unwrap();
        sqlx::query("insert into credit_event_outbox(event_id,workspace_id,event_type,payload,published_at) values($1,$2,'CreditCommandRejected',$3,now())")
            .bind(id).bind(workspace.id).bind(&body).execute(pool).await.unwrap();
        sqlx::query("insert into credit_event_handoffs(event_id,sequence,cutover_id,origin) select $1,$2,cutover_id,'live' from host_event_bus_counter")
            .bind(id).bind(sequence).execute(pool).await.unwrap();
        sqlx::query("insert into host_event_bus_pending(sequence,topic,credit_event_id,payload,value_size_bytes) values($1,$2,$3,$4,$5)")
            .bind(sequence).bind(b"credit.events".as_slice()).bind(id).bind(&payload).bind(payload.len() as i64).execute(pool).await.unwrap();
    }
    // A consumed snapshot's permanent receipt must survive with no source/pending row.
    sqlx::query("insert into credit_event_handoffs(event_id,sequence,cutover_id,origin) select $1,19,cutover_id,'legacy_snapshot' from host_event_bus_counter")
        .bind(Uuid::now_v7()).execute(pool).await.unwrap();
    for (sequence, topic) in [(20_i64, "unknown.future.topic"), (21, "unknown\0topic")] {
        let payload = "{\"escaped\":\"\\u0000\", \"number\": 1e+09}";
        sqlx::query("insert into host_event_bus_pending(sequence,topic,payload,value_size_bytes) values($1,$2,$3,$4)")
            .bind(sequence).bind(topic.as_bytes()).bind(payload).bind(payload.len() as i64).execute(pool).await.unwrap();
    }
    sqlx::query("update host_event_bus_counter set last_sequence=9001")
        .execute(pool)
        .await
        .unwrap();
}

async fn assert_old_names(pool: &PgPool, present: bool) {
    let existence: (bool, bool, bool, bool, bool, bool) = sqlx::query_as(
        "select to_regclass('host_event_bus_pending') is not null,to_regclass('credit_event_handoffs') is not null,to_regclass('host_event_bus_counter') is not null,to_regclass('legacy_event_recovery_pending') is not null,to_regclass('legacy_credit_handoff_receipts') is not null,to_regclass('legacy_event_recovery_state') is not null",
    ).fetch_one(pool).await.unwrap();
    assert_eq!(
        existence,
        (present, present, present, !present, !present, !present)
    );
}

#[tokio::test]
async fn retirement_migration_preserves_exact_obligations_and_foreign_key() {
    let pool = pre_retirement().await;
    seed(&pool).await;
    let before = snapshot(&pool, false).await;
    let oids: (i64, i64, i64) = sqlx::query_as("select 'host_event_bus_pending'::regclass::oid::bigint,'credit_event_handoffs'::regclass::oid::bigint,'host_event_bus_counter'::regclass::oid::bigint")
        .fetch_one(&pool).await.unwrap();
    crate::run_migrations(&pool).await.unwrap();
    assert_old_names(&pool, false).await;
    let generic_names: i64 = sqlx::query_scalar("select count(*) from pg_class where relnamespace=current_schema()::regnamespace and (relname like 'host_event_bus_%' or relname like 'credit_event_handoffs%')")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(generic_names, 0);
    assert_eq!(snapshot(&pool, true).await, before);
    let renamed_oids: (i64, i64, i64) = sqlx::query_as("select 'legacy_event_recovery_pending'::regclass::oid::bigint,'legacy_credit_handoff_receipts'::regclass::oid::bigint,'legacy_event_recovery_state'::regclass::oid::bigint")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(renamed_oids, oids);
    let fk: (bool, bool, String) = sqlx::query_as("select condeferrable,condeferred,confdeltype::text from pg_constraint where conrelid='legacy_event_recovery_pending'::regclass and confrelid='legacy_credit_handoff_receipts'::regclass and contype='f'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(fk, (true, true, "r".into()));
    let error = sqlx::query("delete from legacy_credit_handoff_receipts where sequence=1")
        .execute(&pool)
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("23503")
    );
    let error = sqlx::query("insert into legacy_event_recovery_pending(sequence,topic,credit_event_id,payload,value_size_bytes) values(9002,'credit.events',$1,'{}',2)")
        .bind(Uuid::now_v7()).execute(&pool).await.unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("23503")
    );

    let recovery = PgLegacyEventRecovery::new(pool.clone());
    assert_eq!(
        recovery.poll("unknown\0topic").await.unwrap(),
        Some(json!({"escaped":"\0", "number":1e9}))
    );
    recovery
        .publish("after-retirement", json!(true))
        .await
        .unwrap();
    let high_water: i64 =
        sqlx::query_scalar("select last_sequence from legacy_event_recovery_state")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(high_water, 9002);
    assert_eq!(count(&recovery, "receipts").await, 19);
    // The formal migration ledger makes a repeat run a no-op.
    let after = snapshot(&pool, true).await;
    crate::run_migrations(&pool).await.unwrap();
    assert_eq!(snapshot(&pool, true).await, after);
}

#[tokio::test]
async fn retirement_migration_rollback_and_lock_timeout_leave_all_old_objects_intact() {
    let pool = pre_retirement().await;
    seed(&pool).await;
    let before = snapshot(&pool, false).await;
    let mut tx = pool.begin().await.unwrap();
    sqlx::raw_sql(RETIREMENT_SQL)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    assert_old_names(&pool, true).await;
    assert_eq!(snapshot(&pool, false).await, before);

    // Block the second rename: the first ALTER succeeds before the lock timeout,
    // so rollback must also undo that already-applied rename.
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("lock table credit_event_handoffs in access share mode")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("set local lock_timeout='50ms'")
        .execute(&mut *tx)
        .await
        .unwrap();
    let error = sqlx::raw_sql(RETIREMENT_SQL)
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("55P03")
    );
    tx.rollback().await.unwrap();
    blocker.rollback().await.unwrap();
    assert_old_names(&pool, true).await;
    assert_eq!(snapshot(&pool, false).await, before);
    crate::run_migrations(&pool).await.unwrap();
    assert_eq!(snapshot(&pool, true).await, before);
}
