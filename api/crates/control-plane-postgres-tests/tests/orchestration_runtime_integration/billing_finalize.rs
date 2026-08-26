use super::*;
use control_plane_contracts::ports::{
    AppendCostLedgerInput, BillingRepository, FinalizeModelBillingInput, ReserveCreditInput,
    SettleCreditInput,
};

fn usage_input(flow_run_id: Uuid) -> AppendUsageLedgerInput {
    AppendUsageLedgerInput {
        flow_run_id,
        node_run_id: None,
        span_id: None,
        failover_attempt_id: None,
        provider_instance_id: None,
        gateway_route_id: None,
        model_id: Some("glm-5.2".into()),
        upstream_model_id: Some("glm-5.2".into()),
        upstream_request_id: Some("upstream-request".into()),
        input_tokens: Some(1_147),
        cached_input_tokens: None,
        output_tokens: Some(108),
        reasoning_output_tokens: None,
        total_tokens: Some(1_255),
        input_cache_hit_tokens: None,
        input_cache_miss_tokens: Some(1_147),
        cache_read_tokens: None,
        cache_write_tokens: None,
        price_snapshot: Some(json!({"pricing_rule_id": "fixture"})),
        cost_snapshot: Some(json!({"total_cost": "0.002081", "currency_code": "USD"})),
        usage_status: domain::UsageLedgerStatus::Recorded,
        raw_usage: json!({"input_tokens": 1147, "output_tokens": 108}),
        normalized_usage: json!({"ordinary_input_tokens": 1147, "output_tokens": 108}),
    }
}

fn cost_input(workspace_id: Uuid) -> AppendCostLedgerInput {
    AppendCostLedgerInput {
        flow_run_id: None,
        span_id: None,
        usage_ledger_id: None,
        billing_session_id: None,
        workspace_id,
        provider_instance_id: None,
        provider_account_id: None,
        gateway_route_id: None,
        model_id: Some("glm-5.2".into()),
        upstream_model_id: Some("glm-5.2".into()),
        price_snapshot: json!({"pricing_rule_id": "fixture"}),
        raw_cost: Some("0.002081".into()),
        normalized_cost: Some("0.002081".into()),
        settlement_currency: Some("USD".into()),
        cost_source: "local_token_pricing".into(),
        cost_status: "rated".into(),
    }
}

#[tokio::test]
async fn model_billing_finalize_commits_usage_cost_and_credit_atomically() {
    let database = isolated_database().await;
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    store.upsert_root_tenant().await.unwrap();
    let seeded = seed_runtime_base(&store).await;
    sqlx::query(
        "update user_credit_accounts set charge_enabled = false where workspace_id = $1 and user_id = $2",
    )
    .bind(seeded.workspace_id)
    .bind(seeded.actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let flow_run = seed_flow_run(&store, &seeded, &compiled, OffsetDateTime::now_utc()).await;
    let pricing_rule_id: Uuid =
        sqlx::query_scalar("select id from model_pricing_rules order by created_at limit 1")
            .fetch_one(store.pool())
            .await
            .unwrap();
    let reservation = store
        .reserve_credit(&ReserveCreditInput {
            workspace_id: seeded.workspace_id,
            user_id: seeded.actor_user_id,
            amount: "0".into(),
            flow_run_id: Some(flow_run.id),
            provider_invocation_id: Uuid::now_v7(),
            pricing_rule_id,
            charge_enabled_default: false,
            reservation_expires_at: OffsetDateTime::now_utc() + Duration::minutes(15),
        })
        .await
        .unwrap();

    let finalized = store
        .finalize_model_billing(&FinalizeModelBillingInput {
            usage: usage_input(flow_run.id),
            cost: cost_input(seeded.workspace_id),
            settlement: SettleCreditInput {
                billing_session_id: reservation.billing_session_id,
                actual_amount: "0.002081".into(),
                cost_ledger_id: None,
                usage_ledger_id: None,
                price_snapshot: json!({"pricing_rule_id": "fixture"}),
                usage_snapshot: json!({"ordinary_input_tokens": 1147, "output_tokens": 108}),
            },
        })
        .await
        .unwrap();
    assert_eq!(finalized.cost.usage_ledger_id, Some(finalized.usage.id));
    assert_eq!(
        finalized.cost.billing_session_id,
        Some(reservation.billing_session_id)
    );

    let second_reservation = store
        .reserve_credit(&ReserveCreditInput {
            workspace_id: seeded.workspace_id,
            user_id: seeded.actor_user_id,
            amount: "0".into(),
            flow_run_id: Some(flow_run.id),
            provider_invocation_id: Uuid::now_v7(),
            pricing_rule_id,
            charge_enabled_default: false,
            reservation_expires_at: OffsetDateTime::now_utc() + Duration::minutes(15),
        })
        .await
        .unwrap();
    store
        .settle_credit(&SettleCreditInput {
            billing_session_id: second_reservation.billing_session_id,
            actual_amount: "0".into(),
            cost_ledger_id: None,
            usage_ledger_id: None,
            price_snapshot: json!({"fixture": "already-settled"}),
            usage_snapshot: json!({}),
        })
        .await
        .unwrap();
    let usage_before: i64 =
        sqlx::query_scalar("select count(*) from runtime_usage_ledger where flow_run_id = $1")
            .bind(flow_run.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let cost_before: i64 =
        sqlx::query_scalar("select count(*) from runtime_cost_ledger where flow_run_id = $1")
            .bind(flow_run.id)
            .fetch_one(store.pool())
            .await
            .unwrap();

    let error = store
        .finalize_model_billing(&FinalizeModelBillingInput {
            usage: usage_input(flow_run.id),
            cost: AppendCostLedgerInput {
                flow_run_id: Some(flow_run.id),
                ..cost_input(seeded.workspace_id)
            },
            settlement: SettleCreditInput {
                billing_session_id: second_reservation.billing_session_id,
                actual_amount: "0.002081".into(),
                cost_ledger_id: None,
                usage_ledger_id: None,
                price_snapshot: json!({"pricing_rule_id": "fixture"}),
                usage_snapshot: json!({"ordinary_input_tokens": 1147}),
            },
        })
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("credit_idempotency_payload_mismatch"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from runtime_usage_ledger where flow_run_id = $1"
        )
        .bind(flow_run.id)
        .fetch_one(store.pool())
        .await
        .unwrap(),
        usage_before
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from runtime_cost_ledger where flow_run_id = $1"
        )
        .bind(flow_run.id)
        .fetch_one(store.pool())
        .await
        .unwrap(),
        cost_before
    );
}
