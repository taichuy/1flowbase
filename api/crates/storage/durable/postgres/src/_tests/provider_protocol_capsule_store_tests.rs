use control_plane_contracts::ports::{
    ProviderContinuation, ProviderContinuationSlotId, ProviderProtocolCapsuleStore,
    ProviderProtocolContextSlotId, ProviderProtocolContextValue, ProviderTransportAffinity,
};
use serde_json::json;
use storage_durable_postgres::{
    run_migrations, PgControlPlaneStore, PgProviderProtocolCapsuleStore,
};
use time::Duration;
use uuid::Uuid;

const MASTER_KEY: &str = "issue-2039-capsule-test-master-key";

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

pub(crate) async fn seeded_flow_run() -> (sqlx::PgPool, Uuid) {
    let schema = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = schema.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let tenant_id: Uuid = sqlx::query_scalar("select id from tenants where code = 'root-tenant'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    let workspace_id = Uuid::now_v7();
    sqlx::query("insert into workspaces (id, tenant_id, name) values ($1, $2, 'Capsule fixture')")
        .bind(workspace_id)
        .bind(tenant_id)
        .execute(store.pool())
        .await
        .unwrap();
    let user_id = Uuid::now_v7();
    let account = format!("capsule-{}", user_id.simple());
    sqlx::query(
        r#"
        insert into users (
            id, account, email, password_hash, name, nickname, introduction,
            default_display_role, email_login_enabled, phone_login_enabled, status,
            session_version
        ) values ($1, $2, $3, 'hash', $2, $2, '', 'member', true, false, 'active', 1)
        "#,
    )
    .bind(user_id)
    .bind(&account)
    .bind(format!("{account}@example.com"))
    .execute(store.pool())
    .await
    .unwrap();
    let application_id = Uuid::now_v7();
    sqlx::query(
        "insert into applications (id, workspace_id, application_type, name, description, created_by, updated_by) values ($1, $2, 'agent_flow', 'Capsule fixture', '', $3, $3)",
    )
    .bind(application_id)
    .bind(workspace_id)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    let flow_id = Uuid::now_v7();
    sqlx::query(
        "insert into flows (id, application_id, scope_id, created_by, updated_by) values ($1, $2, (select scope_id from applications where id = $2), $3, $3)",
    )
    .bind(flow_id)
    .bind(application_id)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    let draft_id = Uuid::now_v7();
    sqlx::query(
        "insert into flow_drafts (id, flow_id, scope_id, schema_version, document, created_by, updated_by) values ($1, $2, (select scope_id from flows where id = $2), $3, '{}'::jsonb, $4, $4)",
    )
    .bind(draft_id)
    .bind(flow_id)
    .bind(domain::FLOW_SCHEMA_VERSION)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    let plan_id = Uuid::now_v7();
    sqlx::query(
        "insert into flow_compiled_plans (id, flow_id, flow_draft_id, schema_version, document_updated_at, plan, scope_id, created_by, updated_by) values ($1, $2, $3, $4, now(), '{}'::jsonb, (select scope_id from flows where id = $2), $5, $5)",
    )
    .bind(plan_id)
    .bind(flow_id)
    .bind(draft_id)
    .bind(domain::FLOW_SCHEMA_VERSION)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    let flow_run_id = Uuid::now_v7();
    sqlx::query(
        "insert into flow_runs (id, application_id, flow_id, flow_draft_id, compiled_plan_id, run_mode, status, created_by) values ($1, $2, $3, $4, $5, 'published_api_run', 'running', $6)",
    )
    .bind(flow_run_id)
    .bind(application_id)
    .bind(flow_id)
    .bind(draft_id)
    .bind(plan_id)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    (pool, flow_run_id)
}

fn capsule_store(pool: sqlx::PgPool) -> PgProviderProtocolCapsuleStore {
    PgProviderProtocolCapsuleStore::new(pool, MASTER_KEY, Duration::days(7), 2 * 1024 * 1024)
        .unwrap()
}

#[tokio::test]
async fn issue_2048_bulk_deletes_all_response_round_continuations_for_one_flow() {
    let (pool, flow_run_id) = seeded_flow_run().await;
    let store = capsule_store(pool);
    let affinity = ProviderTransportAffinity::new(
        Uuid::now_v7().to_string(),
        "openai",
        "openai_responses",
        "model",
    );
    let current = ProviderContinuationSlotId::for_flow_run(flow_run_id);
    let round = ProviderContinuationSlotId::for_response_round(flow_run_id, Uuid::now_v7());
    for (slot, response_id) in [(current, "current"), (round, "round")] {
        store
            .put_continuation(
                slot,
                ProviderContinuation::new(response_id, affinity.clone()).unwrap(),
            )
            .await
            .unwrap();
    }

    assert_eq!(
        store
            .delete_flow_run_continuations(flow_run_id)
            .await
            .unwrap(),
        2
    );
    assert!(store.get_continuation(current).await.unwrap().is_none());
    assert!(store.get_continuation(round).await.unwrap().is_none());
}

#[tokio::test]
async fn capsules_survive_adapter_restart_without_the_old_fifteen_minute_ttl() {
    let (pool, flow_run_id) = seeded_flow_run().await;
    let slot = ProviderProtocolContextSlotId::for_original_flow_run(flow_run_id);
    let context = ProviderProtocolContextValue::new(json!({
        "source_protocol": "openai_responses",
        "secret_marker": "capsule-plaintext-must-not-leak",
        "request": {"tools": [{"type": "function", "name": "lookup"}]}
    }))
    .unwrap();
    let locator = context.original_locator();
    capsule_store(pool.clone())
        .put_protocol_context(slot, context)
        .await
        .unwrap();

    sqlx::query(
        "update provider_protocol_capsules set created_at = now() - interval '16 minutes', updated_at = now() - interval '16 minutes' where flow_run_id = $1",
    )
    .bind(flow_run_id)
    .execute(&pool)
    .await
    .unwrap();
    let encrypted: serde_json::Value = sqlx::query_scalar(
        "select encrypted_payload from provider_protocol_capsules where flow_run_id = $1",
    )
    .bind(flow_run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(encrypted["algorithm"], "aead_xchacha20poly1305_v1");
    assert!(!encrypted
        .to_string()
        .contains("capsule-plaintext-must-not-leak"));

    let restarted_adapter = capsule_store(pool.clone());
    let restored = restarted_adapter
        .get_protocol_context(slot)
        .await
        .unwrap()
        .expect("an active capsule must survive a new adapter and the old TTL boundary");
    assert!(restored.matches_locator(&locator));

    let derived_locator = restored.derived_locator();
    let derived_slot = ProviderProtocolContextSlotId::for_locator(flow_run_id, &derived_locator);
    restarted_adapter
        .put_protocol_context(
            derived_slot,
            ProviderProtocolContextValue::new(json!({
                "source_protocol": "openai_responses",
                "secret_marker": "different-capsule"
            }))
            .unwrap(),
        )
        .await
        .unwrap();
    sqlx::query(
        r#"
        update provider_protocol_capsules as target
        set encrypted_payload = source.encrypted_payload
        from provider_protocol_capsules as source
        where target.flow_run_id = $1 and target.capsule_kind = 'protocol_context'
          and target.slot_key = $2 and source.flow_run_id = $1
          and source.capsule_kind = 'protocol_context' and source.slot_key = 'original'
        "#,
    )
    .bind(flow_run_id)
    .bind(derived_slot.storage_key())
    .execute(&pool)
    .await
    .unwrap();
    let relocation_error = restarted_adapter
        .get_protocol_context(derived_slot)
        .await
        .expect_err("ciphertext copied to another capsule identity must fail AEAD authentication");
    assert!(relocation_error.to_string().contains("secret ciphertext"));

    sqlx::query(
        "update provider_protocol_capsules set plaintext_digest = 'sha256:tampered' where flow_run_id = $1 and capsule_kind = 'protocol_context'",
    )
    .bind(flow_run_id)
    .execute(&pool)
    .await
    .unwrap();
    let error = restarted_adapter
        .get_protocol_context(slot)
        .await
        .expect_err("tampered capsule metadata must be rejected");
    assert!(error
        .to_string()
        .contains("provider_protocol_capsule_integrity_mismatch"));
}

#[tokio::test]
async fn continuation_claim_is_atomic_and_expiry_and_terminal_cleanup_are_bounded() {
    let (pool, flow_run_id) = seeded_flow_run().await;
    let store = capsule_store(pool.clone());
    let round_slot = ProviderContinuationSlotId::for_response_round(flow_run_id, Uuid::now_v7());
    let affinity = ProviderTransportAffinity::new(
        Uuid::now_v7().to_string(),
        "openai",
        "openai_responses",
        "gpt-fixture",
    );
    let continuation = ProviderContinuation::new("provider-response-id", affinity.clone()).unwrap();
    store
        .put_continuation(round_slot, continuation)
        .await
        .unwrap();

    let first = capsule_store(pool.clone());
    let second = capsule_store(pool.clone());
    let (left, right) = tokio::join!(
        first.consume_continuation(round_slot),
        second.consume_continuation(round_slot)
    );
    let claims = [left, right];
    assert_eq!(claims.iter().filter(|claim| claim.is_ok()).count(), 1);
    assert_eq!(claims.iter().filter(|claim| claim.is_err()).count(), 1);
    let claimed = claims
        .into_iter()
        .find_map(Result::ok)
        .expect("one claimant owns the continuation");
    assert_eq!(claimed.response_id(), "provider-response-id");
    assert!(claimed.matches_affinity(&affinity));

    let original = ProviderProtocolContextSlotId::for_original_flow_run(flow_run_id);
    let context =
        ProviderProtocolContextValue::new(json!({"source_protocol":"openai_responses"})).unwrap();
    store
        .put_protocol_context(original, context.clone())
        .await
        .unwrap();
    let derived_locator = context.derived_locator();
    store
        .put_protocol_context(
            ProviderProtocolContextSlotId::for_locator(flow_run_id, &derived_locator),
            context,
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .delete_flow_run_protocol_contexts(flow_run_id)
            .await
            .unwrap(),
        2
    );

    let expiring = ProviderContinuationSlotId::for_flow_run(flow_run_id);
    store
        .put_continuation(
            expiring,
            ProviderContinuation::new("expired", affinity).unwrap(),
        )
        .await
        .unwrap();
    sqlx::query(
        "update provider_protocol_capsules set hard_expires_at = now() - interval '1 second' where flow_run_id = $1",
    )
    .bind(flow_run_id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(store.clear_expired().await.unwrap(), 1);
    assert!(store.get_continuation(expiring).await.unwrap().is_none());
}

#[tokio::test]
async fn trusted_response_history_survives_capsule_restart_and_single_atomic_claim() {
    let (pool, flow_run_id) = seeded_flow_run().await;
    let slot = ProviderContinuationSlotId::for_response_round(flow_run_id, Uuid::now_v7());
    let history = json!({"version":1,"item_count":2,"digest":"ordered-proof"});
    let continuation = ProviderContinuation::new(
        "resp_prewarm",
        ProviderTransportAffinity::new("instance", "openai", "openai_responses", "model"),
    )
    .unwrap()
    .with_native_history(Some(history.clone()));
    capsule_store(pool.clone())
        .put_continuation(slot, continuation)
        .await
        .unwrap();
    let restarted = capsule_store(pool);
    assert_eq!(
        restarted
            .get_continuation(slot)
            .await
            .unwrap()
            .unwrap()
            .native_history(),
        Some(&history)
    );
    let (a, b) = tokio::join!(
        restarted.consume_continuation(slot),
        restarted.consume_continuation(slot)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert_eq!(a.or(b).unwrap().native_history(), Some(&history));
    assert!(restarted.get_continuation(slot).await.unwrap().is_none());
}
