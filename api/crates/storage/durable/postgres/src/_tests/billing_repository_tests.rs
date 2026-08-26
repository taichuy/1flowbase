use std::sync::Arc;

use control_plane::ports::{
    BillingRepository, CreditCommandInput, ReserveCreditInput, UpsertPricingRuleInput,
};
use control_plane_contracts::billing::PricingRule;
use rust_decimal::Decimal;
use serde_json::json;
use storage_durable_postgres::{run_migrations, PgControlPlaneStore};
use time::{Duration, OffsetDateTime};
use tokio::sync::Barrier;
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn seeded_store() -> (PgControlPlaneStore, Uuid, Uuid) {
    let database = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store.upsert_workspace(tenant.id, "Billing").await.unwrap();
    let user_id = Uuid::now_v7();
    sqlx::query(
        r#"insert into users
        (id,account,email,password_hash,name,nickname,introduction,email_login_enabled,phone_login_enabled,status,session_version)
        values ($1,$2,$3,'hash','Billing User','Billing','',true,false,'active',1)"#,
    )
    .bind(user_id)
    .bind(format!("billing-{}", user_id.simple()))
    .bind(format!("{}@example.test", user_id.simple()))
    .execute(store.pool())
    .await
    .unwrap();
    (store, workspace.id, user_id)
}

#[tokio::test]
async fn credit_command_is_idempotent_and_updates_projection_with_ledger() {
    let (store, workspace_id, user_id) = seeded_store().await;
    let input = CreditCommandInput {
        workspace_id,
        user_id,
        amount: "10.25".into(),
        credit_unit: "USD".into(),
        command: "grant".into(),
        reason: "test_grant".into(),
        source_type: Some("test".into()),
        source_id: Some("one".into()),
        idempotency_key: "grant:test:one".into(),
        actor_user_id: Some(user_id),
        actor_plugin_id: None,
        metadata: json!({}),
    };
    let first = store.execute_credit_command(&input).await.unwrap();
    let repeated = store.execute_credit_command(&input).await.unwrap();
    assert_eq!(first.id, repeated.id);
    let account = store
        .get_credit_account(workspace_id, user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(account.current_balance, "10.250000000000000000");
    assert_eq!(account.available_balance, "10.250000000000000000");
    assert!(!account.credit_insufficient);
    let outbox_count: i64 =
        sqlx::query_scalar("select count(*) from credit_event_outbox where account_id=$1")
            .bind(account.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(outbox_count, 1);
}

#[tokio::test]
async fn zero_balance_allows_only_one_concurrent_boundary_reservation() {
    let (store, workspace_id, user_id) = seeded_store().await;
    let store = Arc::new(store);
    let barrier = Arc::new(Barrier::new(2));
    let mut tasks = Vec::new();
    for _ in 0..2 {
        let store = store.clone();
        let barrier = barrier.clone();
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            store
                .reserve_credit(&ReserveCreditInput {
                    workspace_id,
                    user_id,
                    amount: "1".into(),
                    flow_run_id: None,
                    provider_invocation_id: Uuid::now_v7(),
                    pricing_rule_id: Uuid::now_v7(),
                    charge_enabled_default: true,
                    reservation_expires_at: OffsetDateTime::now_utc() + Duration::minutes(15),
                })
                .await
        }));
    }
    let mut accepted = 0;
    let mut rejected = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(_) => accepted += 1,
            Err(error) if error.to_string().contains("credit_insufficient") => rejected += 1,
            Err(error) => panic!("unexpected reservation error: {error}"),
        }
    }
    assert_eq!((accepted, rejected), (1, 1));
}

#[tokio::test]
async fn expired_reservation_with_rated_cost_is_settled_instead_of_released() {
    let (store, workspace_id, user_id) = seeded_store().await;
    store
        .execute_credit_command(&CreditCommandInput {
            workspace_id,
            user_id,
            amount: "10".into(),
            credit_unit: "USD".into(),
            command: "grant".into(),
            reason: "recovery_fixture".into(),
            source_type: Some("test".into()),
            source_id: Some("recovery".into()),
            idempotency_key: "grant:recovery".into(),
            actor_user_id: Some(user_id),
            actor_plugin_id: None,
            metadata: json!({}),
        })
        .await
        .unwrap();
    let reservation = store
        .reserve_credit(&ReserveCreditInput {
            workspace_id,
            user_id,
            amount: "3".into(),
            flow_run_id: None,
            provider_invocation_id: Uuid::now_v7(),
            pricing_rule_id: Uuid::now_v7(),
            charge_enabled_default: true,
            reservation_expires_at: OffsetDateTime::now_utc() - Duration::minutes(1),
        })
        .await
        .unwrap();
    sqlx::query(
        r#"insert into runtime_cost_ledger
           (id,billing_session_id,workspace_id,price_snapshot,raw_cost,normalized_cost,
            settlement_currency,cost_source,cost_status)
           values ($1,$2,$3,$4,2,2,'USD','local_token_pricing','rated')"#,
    )
    .bind(Uuid::now_v7())
    .bind(reservation.billing_session_id)
    .bind(workspace_id)
    .bind(json!({"pricing_rule_id":"fixture"}))
    .execute(store.pool())
    .await
    .unwrap();

    assert_eq!(
        store
            .recover_expired_credit_reservations(OffsetDateTime::now_utc(), 10)
            .await
            .unwrap(),
        1
    );
    let account = store
        .get_credit_account(workspace_id, user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(account.current_balance, "8.000000000000000000");
    assert_eq!(account.reserved_amount, "0.000000000000000000");
    let status: String = sqlx::query_scalar("select status from billing_sessions where id=$1")
        .bind(reservation.billing_session_id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(status, "settled");
}

#[tokio::test]
async fn expired_reservation_without_cost_is_released() {
    let (store, workspace_id, user_id) = seeded_store().await;
    let reservation = store
        .reserve_credit(&ReserveCreditInput {
            workspace_id,
            user_id,
            amount: "1".into(),
            flow_run_id: None,
            provider_invocation_id: Uuid::now_v7(),
            pricing_rule_id: Uuid::now_v7(),
            charge_enabled_default: true,
            reservation_expires_at: OffsetDateTime::now_utc() - Duration::minutes(1),
        })
        .await
        .unwrap();
    assert_eq!(
        store
            .recover_expired_credit_reservations(OffsetDateTime::now_utc(), 10)
            .await
            .unwrap(),
        1
    );
    let status: String = sqlx::query_scalar("select status from billing_sessions where id=$1")
        .bind(reservation.billing_session_id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(status, "released");
}

#[tokio::test]
async fn plugin_credit_command_round_trips_actor_and_enforces_idempotency() {
    let (store, workspace_id, user_id) = seeded_store().await;
    let command = CreditCommandInput {
        workspace_id,
        user_id,
        amount: "2.50".into(),
        credit_unit: "USD".into(),
        command: "grant".into(),
        reason: "daily_checkin".into(),
        source_type: Some("checkin".into()),
        source_id: Some("2026-08-17".into()),
        idempotency_key: "checkin:user:2026-08-17".into(),
        actor_user_id: None,
        actor_plugin_id: Some("checkin-plugin".into()),
        metadata: json!({}),
    };
    let first = store.execute_credit_command(&command).await.unwrap();
    let repeated = store.execute_credit_command(&command).await.unwrap();
    assert_eq!(first.id, repeated.id);
    assert_eq!(first.actor_plugin_id.as_deref(), Some("checkin-plugin"));

    let conflicting = CreditCommandInput {
        amount: "9.00".into(),
        idempotency_key: "checkin:user:2026-08-17".into(),
        ..CreditCommandInput {
            workspace_id,
            user_id,
            amount: "2.50".into(),
            credit_unit: "USD".into(),
            command: "grant".into(),
            reason: "daily_checkin".into(),
            source_type: Some("checkin".into()),
            source_id: Some("2026-08-17".into()),
            idempotency_key: String::new(),
            actor_user_id: None,
            actor_plugin_id: Some("checkin-plugin".into()),
            metadata: json!({}),
        }
    };
    let conflict = store
        .execute_credit_command(&conflicting)
        .await
        .unwrap_err();
    assert!(conflict
        .to_string()
        .contains("credit_idempotency_payload_mismatch"));

    let account = store
        .get_credit_account(workspace_id, user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(account.current_balance, "2.500000000000000000");
    let ledger_count: i64 = sqlx::query_scalar(
        r#"select count(*) from runtime_credit_ledger
           where workspace_id=$1 and user_id=$2 and idempotency_key=$3"#,
    )
    .bind(workspace_id)
    .bind(user_id)
    .bind("checkin:user:2026-08-17")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(ledger_count, 1);
}

#[tokio::test]
async fn pricing_candidates_include_exact_rule_and_global_fallback() {
    let (store, _workspace_id, user_id) = seeded_store().await;
    let exact_id = Uuid::now_v7();
    sqlx::query(
        r#"insert into model_pricing_rules
        (id,provider_code,upstream_model_id,input_token_unit_size,input_token_unit_price,
         output_token_unit_size,output_token_unit_price,cache_hit_token_unit_size,
         cache_hit_token_unit_price,currency_code,effective_from,timezone,weekday_mask,
         priority,enabled,source_kind,extensions,created_by)
        values ($1,'fixture-provider','fixture-model',1000000,1,1000000,2,1000000,0.5,
                'USD',now()-interval '1 hour','UTC',127,0,true,'manual','{}',$2)"#,
    )
    .bind(exact_id)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();

    let candidates = store
        .match_pricing_rules(
            "fixture-provider",
            "fixture-model",
            OffsetDateTime::now_utc(),
        )
        .await
        .unwrap();
    assert!(candidates.iter().any(|rule| rule.id == exact_id));
    assert!(candidates
        .iter()
        .any(|rule| { rule.provider_code == "zero" && rule.upstream_model_id == "any" }));
}

#[tokio::test]
async fn concurrent_pricing_rule_writes_cannot_create_an_overlapping_schedule() {
    let (store, _workspace_id, user_id) = seeded_store().await;
    let store = Arc::new(store);
    let barrier = Arc::new(Barrier::new(2));
    let effective_from = OffsetDateTime::now_utc() - Duration::hours(1);
    let mut tasks = Vec::new();
    for _ in 0..2 {
        let store = store.clone();
        let barrier = barrier.clone();
        let rule = PricingRule {
            id: Uuid::now_v7(),
            provider_code: "concurrent-provider".into(),
            upstream_model_id: "concurrent-model".into(),
            input_token_unit_size: 1_000_000,
            input_token_unit_price: Decimal::ONE,
            output_token_unit_size: 1_000_000,
            output_token_unit_price: Decimal::ONE,
            cache_hit_token_unit_size: 1_000_000,
            cache_hit_token_unit_price: Decimal::ZERO,
            currency_code: "USD".into(),
            effective_from,
            effective_to: None,
            timezone: "UTC".into(),
            weekday_mask: 127,
            local_time_start: None,
            local_time_end: None,
            priority: 0,
            enabled: true,
            rating_policy_enabled: false,
            rating_policy: json!({}),
            source_kind: "manual".into(),
            source_catalog_id: None,
            source_version: None,
            source_checksum: None,
            extensions: json!({}),
            created_by: Some(user_id),
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        };
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            store
                .upsert_pricing_rule(&UpsertPricingRuleInput { rule })
                .await
        }));
    }

    let mut accepted = 0;
    let mut conflicts = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(_) => accepted += 1,
            Err(error) if error.to_string().contains("pricing_rule_conflict") => conflicts += 1,
            Err(error) => panic!("unexpected pricing rule error: {error}"),
        }
    }
    assert_eq!((accepted, conflicts), (1, 1));
}
