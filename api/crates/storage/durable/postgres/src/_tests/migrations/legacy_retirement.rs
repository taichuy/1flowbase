//! Forward-only retirement fixtures deliberately start with the complete old schema.
use std::borrow::Cow;

use control_plane_contracts::ports::{BillingRepository, CreditCommandInput, ReserveCreditInput};
use serde_json::{json, Value};
use sqlx::{migrate::Migrator, PgPool};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

const RETIREMENT: i64 = 20261018010000;

async fn old_database() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into());
    let pool = postgres_test_support::PostgresTestSchema::create(&url)
        .await
        .unwrap()
        .connect()
        .await
        .unwrap();
    let before = Migrator {
        migrations: Cow::Owned(
            sqlx::migrate!("./migrations")
                .iter()
                .filter(|migration| migration.version < RETIREMENT)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    };
    before.run(&pool).await.unwrap();
    pool
}

async fn old_transport(pool: &PgPool) -> Value {
    sqlx::query_scalar("select jsonb_build_object('ledger',(select jsonb_agg(to_jsonb(l) order by sequence) from legacy_event_recovery_ledger l),'sources',(select jsonb_agg(to_jsonb(f) order by event_id) from lifecycle_outbox f),'receipts',(select jsonb_agg(to_jsonb(d) order by event_id,subscriber_id) from lifecycle_outbox_deliveries d))")
        .fetch_one(pool).await.unwrap()
}

async fn retained_transport(pool: &PgPool) -> Value {
    sqlx::query_scalar("select jsonb_build_object('sources',(select jsonb_agg(to_jsonb(f)-'native_publication_version'-'legacy_handoff_sequence' order by event_id) from lifecycle_outbox f),'receipts',(select jsonb_agg(to_jsonb(d) order by event_id,subscriber_id) from lifecycle_outbox_deliveries d))")
        .fetch_one(pool).await.unwrap()
}

async fn money(pool: &PgPool) -> Value {
    sqlx::query_scalar("select jsonb_build_object('accounts',(select jsonb_agg(to_jsonb(a) order by id) from user_credit_accounts a),'ledger',(select jsonb_agg(to_jsonb(l) order by id) from runtime_credit_ledger l),'sessions',(select jsonb_agg(to_jsonb(s) order by id) from billing_sessions s),'credit_sources',(select jsonb_agg(to_jsonb(c) order by event_id) from credit_event_outbox c))")
        .fetch_one(pool).await.unwrap()
}

async fn checksums(pool: &PgPool) -> Vec<(i64, Vec<u8>)> {
    sqlx::query_as(
        "select version,checksum from _sqlx_migrations where version < $1 order by version",
    )
    .bind(RETIREMENT)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn seed_money(pool: &PgPool) {
    let store = crate::PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "Retirement money")
        .await
        .unwrap();
    let user = Uuid::now_v7();
    sqlx::query("insert into users(id,account,email,password_hash,name,nickname,status) values($1,$2,$3,'fixture','Retirement','Retirement','active')")
        .bind(user).bind(format!("retirement-{user}")).bind(format!("{user}@example.test")).execute(pool).await.unwrap();
    store
        .execute_credit_command(&CreditCommandInput {
            workspace_id: workspace.id,
            user_id: user,
            amount: "10.25".into(),
            credit_unit: "USD".into(),
            command: "grant".into(),
            reason: "retirement baseline".into(),
            source_type: Some("test".into()),
            source_id: Some("retirement".into()),
            idempotency_key: "retirement:grant".into(),
            actor_user_id: Some(user),
            actor_plugin_id: None,
            metadata: json!({}),
        })
        .await
        .unwrap();
    store
        .reserve_credit(&ReserveCreditInput {
            workspace_id: workspace.id,
            user_id: user,
            amount: "1".into(),
            flow_run_id: None,
            provider_invocation_id: Uuid::now_v7(),
            pricing_rule_id: Uuid::now_v7(),
            charge_enabled_default: true,
            reservation_expires_at: OffsetDateTime::now_utc() + Duration::minutes(15),
        })
        .await
        .unwrap();
}

async fn seed_model(pool: &PgPool, delivered: bool) -> Uuid {
    use control_plane_contracts::ports::ModelDefinitionCommittedFact;
    use extension_contracts::{AfterCommitFact, LifecycleFactId, LifecycleTransactionId};
    let event = Uuid::now_v7();
    let transaction = Uuid::now_v7();
    let now = OffsetDateTime::now_utc();
    let fact = AfterCommitFact::new(
        LifecycleFactId::new(event.to_string()).unwrap(),
        LifecycleTransactionId::new(transaction.to_string()).unwrap(),
        (now.unix_timestamp_nanos() / 1_000_000) as i64,
        ModelDefinitionCommittedFact {
            model_definition_id: Uuid::now_v7(),
            scope_kind: domain::DataModelScopeKind::Workspace,
            scope_id: Uuid::now_v7(),
        },
    );
    sqlx::query("insert into lifecycle_outbox(event_id,transaction_id,contract_id,contract_version,canonical_payload,occurred_at,status,delivered_at,native_publication_version,legacy_handoff_sequence) values($1,$2,'model_definition.committed','v1',$3,$4,case when $5 then 'delivered' else 'pending' end,case when $5 then $4 else null end,1,19)")
        .bind(event).bind(transaction).bind(serde_json::to_vec(&fact).unwrap()).bind(now).bind(delivered).execute(pool).await.unwrap();
    sqlx::query("insert into lifecycle_outbox_deliveries(event_id,subscriber_id,handler_id,handler_version,status,delivered_at) values($1,'original','original.handler','v1',case when $2 then 'delivered' else 'pending' end,case when $2 then now() else null end)")
        .bind(event).bind(delivered).execute(pool).await.unwrap();
    event
}

#[tokio::test]
async fn legacy_retirement_refuses_nonempty_ledger_then_preserves_thirteen_sources_receipts_and_money(
) {
    let pool = old_database().await;
    seed_money(&pool).await;
    for _ in 0..13 {
        seed_model(&pool, true).await;
    }
    let ids: Vec<Uuid> = (0..18).map(|_| Uuid::now_v7()).collect();
    for (index, id) in ids.iter().enumerate() {
        sqlx::query("insert into legacy_event_recovery_ledger(sequence,disposition,topic,payload,payload_digest,value_size_bytes,credit_event_id,cutover_id,origin,handed_off_at) select $1,'pending',convert_to('credit.events','UTF8'),'{}',sha256(convert_to('{}','UTF8')),2,$2,cutover_id,'legacy_snapshot',now() from legacy_event_recovery_ledger where sequence=0")
            .bind(index as i64 + 1).bind(id).execute(&pool).await.unwrap();
    }
    let before = old_transport(&pool).await;
    let retained = retained_transport(&pool).await;
    let monetary = money(&pool).await;
    for key in ["accounts", "ledger", "sessions"] {
        assert!(
            !monetary[key].as_array().unwrap().is_empty(),
            "empty baseline {key}"
        );
    }
    let history = checksums(&pool).await;
    let error = crate::run_migrations(&pool).await.unwrap_err();
    assert!(format!("{error:#}").contains("legacy recovery ledger is not empty"));
    assert_eq!(old_transport(&pool).await, before, "refusal must be atomic");
    assert_eq!(money(&pool).await, monetary);
    assert_eq!(checksums(&pool).await, history);
    let removed = sqlx::query(
        "delete from legacy_event_recovery_ledger where sequence>0 and credit_event_id=any($1)",
    )
    .bind(&ids)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(removed.rows_affected(), 18);
    crate::run_migrations(&pool).await.unwrap();
    assert_eq!(retained_transport(&pool).await, retained);
    assert_eq!(retained["sources"].as_array().unwrap().len(), 13);
    assert_eq!(retained["receipts"].as_array().unwrap().len(), 13);
    assert_eq!(money(&pool).await, monetary);
    assert_eq!(
        checksums(&pool).await,
        history,
        "historical migration hashes are immutable"
    );
    let removed_schema: (bool, i64) = sqlx::query_as("select to_regclass(current_schema()||'.legacy_event_recovery_ledger') is null,(select count(*) from information_schema.columns where table_schema=current_schema() and table_name='lifecycle_outbox' and column_name in ('native_publication_version','legacy_handoff_sequence'))")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(removed_schema, (true, 0));
}

#[tokio::test]
async fn legacy_retirement_refuses_pending_source_or_unfinished_sibling_receipt() {
    let pool = old_database().await;
    let event = seed_model(&pool, false).await;
    for source_delivered in [false, true] {
        if source_delivered {
            sqlx::query("update lifecycle_outbox set status='delivered',delivered_at=now() where event_id=$1")
                .bind(event).execute(&pool).await.unwrap();
        }
        let before = old_transport(&pool).await;
        let error = crate::run_migrations(&pool).await.unwrap_err();
        assert!(format!("{error:#}").contains("unfinished legacy model publication"));
        assert_eq!(old_transport(&pool).await, before);
    }
    sqlx::query("update lifecycle_outbox_deliveries set status='delivered',delivered_at=now() where event_id=$1")
        .bind(event).execute(&pool).await.unwrap();
    let before = retained_transport(&pool).await;
    crate::run_migrations(&pool).await.unwrap();
    assert_eq!(retained_transport(&pool).await, before);
}
