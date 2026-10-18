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

    assert!(
        reservation.charge_skipped,
        "legacy in-flight free session fixture"
    );
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
    assert_eq!(finalized.credit.amount.parse::<f64>().unwrap(), 0.0);
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

async fn credit_domain_snapshot(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
) -> serde_json::Value {
    sqlx::query_scalar(
        "select jsonb_build_object('accounts',(select coalesce(jsonb_agg(to_jsonb(a) order by id),'[]'::jsonb) from user_credit_accounts a where workspace_id=$1),'sessions',(select coalesce(jsonb_agg(to_jsonb(s) order by id),'[]'::jsonb) from billing_sessions s where workspace_id=$1),'ledger',(select coalesce(jsonb_agg(to_jsonb(l) order by id),'[]'::jsonb) from runtime_credit_ledger l where workspace_id=$1),'outbox',(select coalesce(jsonb_agg(to_jsonb(o) order by event_id),'[]'::jsonb) from credit_event_outbox o where workspace_id=$1),'lifecycle_outbox',(select coalesce(jsonb_agg(to_jsonb(f) order by event_id),'[]'::jsonb) from lifecycle_outbox f),'lifecycle_deliveries',(select coalesce(jsonb_agg(to_jsonb(d) order by event_id,subscriber_id),'[]'::jsonb) from lifecycle_outbox_deliveries d))",
    ).bind(workspace_id).fetch_one(store.pool()).await.unwrap()
}

#[tokio::test]
async fn model_billing_observation_does_not_create_or_mutate_credit_domain() {
    use control_plane_contracts::ports::{
        ModelBillingAdmission, ModelBillingAdmissionInput, RecordModelObservationInput,
    };
    let database = isolated_database().await;
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    store.upsert_root_tenant().await.unwrap();
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let flow = seed_flow_run(&store, &seeded, &compiled, OffsetDateTime::now_utc()).await;
    let pricing_rule_id =
        sqlx::query_scalar("select id from model_pricing_rules order by created_at limit 1")
            .fetch_one(store.pool())
            .await
            .unwrap();
    // A model call must not recreate a missing default-free account.
    sqlx::query("delete from user_credit_accounts where workspace_id=$1 and user_id=$2")
        .bind(seeded.workspace_id)
        .bind(seeded.actor_user_id)
        .execute(store.pool())
        .await
        .unwrap();
    for existing in [false, true] {
        if existing {
            sqlx::query("insert into user_credit_accounts(id,workspace_id,user_id,charge_enabled,current_balance,reserved_amount,revision) values($1,$2,$3,false,12,3,7)")
                .bind(Uuid::now_v7()).bind(seeded.workspace_id).bind(seeded.actor_user_id).execute(store.pool()).await.unwrap();
        }
        let before = credit_domain_snapshot(&store, seeded.workspace_id).await;
        let input = ModelBillingAdmissionInput {
            workspace_id: seeded.workspace_id,
            started_at: OffsetDateTime::now_utc(),
            reservation: Some(ReserveCreditInput {
                workspace_id: seeded.workspace_id,
                user_id: seeded.actor_user_id,
                amount: "1".into(),
                flow_run_id: Some(flow.id),
                provider_invocation_id: Uuid::now_v7(),
                pricing_rule_id,
                charge_enabled_default: false,
                reservation_expires_at: OffsetDateTime::now_utc() + Duration::minutes(15),
            }),
        };
        for _ in 0..2 {
            assert!(matches!(
                store.model_billing_admit(&input).await.unwrap(),
                ModelBillingAdmission::ObserveOnly
            ));
            let observation = store
                .record_model_observation(&RecordModelObservationInput {
                    usage: usage_input(flow.id),
                    cost: cost_input(seeded.workspace_id),
                })
                .await
                .unwrap();
            assert_eq!(observation.usage.input_tokens, Some(1147));
            assert_eq!(observation.cost.usage_ledger_id, Some(observation.usage.id));
            assert_eq!(observation.cost.billing_session_id, None);
            assert_eq!(
                observation
                    .cost
                    .normalized_cost
                    .unwrap()
                    .parse::<f64>()
                    .unwrap(),
                0.002081
            );
        }
        // Failed observation must roll back the usage append as well.
        let usages_before: i64 =
            sqlx::query_scalar("select count(*) from runtime_usage_ledger where flow_run_id=$1")
                .bind(flow.id)
                .fetch_one(store.pool())
                .await
                .unwrap();
        let mut invalid_cost = cost_input(seeded.workspace_id);
        invalid_cost.normalized_cost = Some("invalid-number".into());
        assert!(store
            .record_model_observation(&RecordModelObservationInput {
                usage: usage_input(flow.id),
                cost: invalid_cost
            })
            .await
            .is_err());
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "select count(*) from runtime_usage_ledger where flow_run_id=$1"
            )
            .bind(flow.id)
            .fetch_one(store.pool())
            .await
            .unwrap(),
            usages_before
        );
        assert_eq!(
            credit_domain_snapshot(&store, seeded.workspace_id).await,
            before
        );
        assert_eq!(
            store
                .get_credit_account(seeded.workspace_id, seeded.actor_user_id)
                .await
                .unwrap()
                .is_some(),
            existing
        );
    }
    // Disabled billing needs neither actor nor pricing preparation.
    sqlx::query("update workspace_billing_settings set billing_enabled_at=now()+interval '1 day' where workspace_id=$1")
        .bind(seeded.workspace_id).execute(store.pool()).await.unwrap();
    let before = credit_domain_snapshot(&store, seeded.workspace_id).await;
    assert!(matches!(
        store
            .model_billing_admit(&ModelBillingAdmissionInput {
                workspace_id: seeded.workspace_id,
                started_at: OffsetDateTime::now_utc(),
                reservation: None
            })
            .await
            .unwrap(),
        ModelBillingAdmission::Disabled
    ));
    assert_eq!(
        credit_domain_snapshot(&store, seeded.workspace_id).await,
        before
    );
}

#[tokio::test]
async fn model_billing_charged_snapshot_survives_toggle_and_replays_atomically() {
    use control_plane_contracts::ports::{ModelBillingAdmission, ModelBillingAdmissionInput};
    let database = isolated_database().await;
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    store.upsert_root_tenant().await.unwrap();
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let flow = seed_flow_run(&store, &seeded, &compiled, OffsetDateTime::now_utc()).await;
    let pricing_rule_id =
        sqlx::query_scalar("select id from model_pricing_rules order by created_at limit 1")
            .fetch_one(store.pool())
            .await
            .unwrap();
    sqlx::query("insert into user_credit_accounts(id,workspace_id,user_id,charge_enabled,current_balance) values($1,$2,$3,true,10) on conflict(workspace_id,user_id,credit_unit) do update set charge_enabled=true,current_balance=10")
        .bind(Uuid::now_v7()).bind(seeded.workspace_id).bind(seeded.actor_user_id).execute(store.pool()).await.unwrap();
    let prepared = ReserveCreditInput {
        workspace_id: seeded.workspace_id,
        user_id: seeded.actor_user_id,
        amount: "1".into(),
        flow_run_id: Some(flow.id),
        provider_invocation_id: Uuid::now_v7(),
        pricing_rule_id,
        charge_enabled_default: true,
        reservation_expires_at: OffsetDateTime::now_utc() + Duration::minutes(15),
    };
    let admission_input = ModelBillingAdmissionInput {
        workspace_id: seeded.workspace_id,
        started_at: OffsetDateTime::now_utc(),
        reservation: Some(prepared.clone()),
    };
    // Prove admission waits on the administrator's account lock before taking
    // its mode snapshot. A committed false value cannot create a skipped session.
    let mut setting_tx = store.pool().begin().await.unwrap();
    sqlx::query(
        "select id from user_credit_accounts where workspace_id=$1 and user_id=$2 for update",
    )
    .bind(seeded.workspace_id)
    .bind(seeded.actor_user_id)
    .fetch_one(&mut *setting_tx)
    .await
    .unwrap();
    let admission_store = store.clone();
    let concurrent_input = admission_input.clone();
    let pending =
        tokio::spawn(async move { admission_store.model_billing_admit(&concurrent_input).await });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let waiting: bool = sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and wait_event_type='Lock' and query like '%from user_credit_accounts%for update%')")
                .fetch_one(store.pool()).await.unwrap();
            if waiting { break; }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.expect("admission must serialize on the existing account");
    sqlx::query(
        "update user_credit_accounts set charge_enabled=false where workspace_id=$1 and user_id=$2",
    )
    .bind(seeded.workspace_id)
    .bind(seeded.actor_user_id)
    .execute(&mut *setting_tx)
    .await
    .unwrap();
    setting_tx.commit().await.unwrap();
    assert!(matches!(
        pending.await.unwrap().unwrap(),
        ModelBillingAdmission::ObserveOnly
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from billing_sessions where workspace_id=$1")
            .bind(seeded.workspace_id)
            .fetch_one(store.pool())
            .await
            .unwrap(),
        0
    );
    sqlx::query(
        "update user_credit_accounts set charge_enabled=true where workspace_id=$1 and user_id=$2",
    )
    .bind(seeded.workspace_id)
    .bind(seeded.actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();
    let ModelBillingAdmission::Reserved(reservation) =
        store.model_billing_admit(&admission_input).await.unwrap()
    else {
        panic!("charged account must reserve")
    };
    assert!(!reservation.charge_skipped);
    sqlx::query(
        "update user_credit_accounts set charge_enabled=false where workspace_id=$1 and user_id=$2",
    )
    .bind(seeded.workspace_id)
    .bind(seeded.actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();
    // A retry retains its original session snapshot, while a new invocation observes.
    let ModelBillingAdmission::Reserved(retried) =
        store.model_billing_admit(&admission_input).await.unwrap()
    else {
        panic!("retry must retain reserved owner")
    };
    assert_eq!(retried.billing_session_id, reservation.billing_session_id);
    assert!(!retried.charge_skipped);
    let mut next = admission_input.clone();
    next.reservation.as_mut().unwrap().provider_invocation_id = Uuid::now_v7();
    assert!(matches!(
        store.model_billing_admit(&next).await.unwrap(),
        ModelBillingAdmission::ObserveOnly
    ));
    // Workspace off after admission cannot remove the outstanding finalizer.
    sqlx::query("update workspace_billing_settings set billing_enabled_at=now()+interval '1 day' where workspace_id=$1")
        .bind(seeded.workspace_id).execute(store.pool()).await.unwrap();
    let input = FinalizeModelBillingInput {
        usage: usage_input(flow.id),
        cost: cost_input(seeded.workspace_id),
        settlement: SettleCreditInput {
            billing_session_id: reservation.billing_session_id,
            actual_amount: "0.002081".into(),
            cost_ledger_id: None,
            usage_ledger_id: None,
            price_snapshot: json!({"fixture":"positive-charge"}),
            usage_snapshot: json!({"input_tokens":1147,"output_tokens":108}),
        },
    };
    let before = credit_domain_snapshot(&store, seeded.workspace_id).await;
    let mut invalid = input.clone();
    invalid.settlement.actual_amount = "-1".into();
    assert!(store.finalize_model_billing(&invalid).await.is_err());
    assert_eq!(
        credit_domain_snapshot(&store, seeded.workspace_id).await,
        before
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from runtime_usage_ledger where flow_run_id=$1"
        )
        .bind(flow.id)
        .fetch_one(store.pool())
        .await
        .unwrap(),
        0
    );
    let first = store.finalize_model_billing(&input).await.unwrap();
    let after = credit_domain_snapshot(&store, seeded.workspace_id).await;
    let replay = store.finalize_model_billing(&input).await.unwrap();
    assert_eq!(replay.usage.id, first.usage.id);
    assert_eq!(replay.cost.id, first.cost.id);
    assert_eq!(replay.credit.id, first.credit.id);
    assert_eq!(
        credit_domain_snapshot(&store, seeded.workspace_id).await,
        after
    );
    let account = store
        .get_credit_account(seeded.workspace_id, seeded.actor_user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(account.current_balance.parse::<f64>().unwrap(), 9.997919);
    assert_eq!(account.reserved_amount.parse::<f64>().unwrap(), 0.0);
    let mut changed_usage = input.clone();
    changed_usage.usage.raw_usage = json!({"input_tokens":999});
    assert!(store
        .finalize_model_billing(&changed_usage)
        .await
        .unwrap_err()
        .to_string()
        .contains("credit_idempotency_payload_mismatch"));
    let mut mismatch = input;
    mismatch.settlement.actual_amount = "2".into();
    assert!(store
        .finalize_model_billing(&mismatch)
        .await
        .unwrap_err()
        .to_string()
        .contains("credit_idempotency_payload_mismatch"));
    assert_eq!(
        credit_domain_snapshot(&store, seeded.workspace_id).await,
        after
    );
    // Cancellation still releases a charged reservation after both switches change.
    sqlx::query(
        "update user_credit_accounts set charge_enabled=true where workspace_id=$1 and user_id=$2",
    )
    .bind(seeded.workspace_id)
    .bind(seeded.actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();
    let cancel = store
        .reserve_credit(&ReserveCreditInput {
            provider_invocation_id: Uuid::now_v7(),
            ..prepared
        })
        .await
        .unwrap();
    let release = store
        .release_credit(cancel.billing_session_id, "cancelled")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(release.transaction_type, "release");
    let after_release = credit_domain_snapshot(&store, seeded.workspace_id).await;
    assert!(store
        .release_credit(cancel.billing_session_id, "cancelled")
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        credit_domain_snapshot(&store, seeded.workspace_id).await,
        after_release
    );
}
