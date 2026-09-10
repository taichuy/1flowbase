use sqlx::migrate::Migrator;
use std::borrow::Cow;
use storage_durable_postgres::PgControlPlaneStore;
use uuid::Uuid;

const CLEANUP_MIGRATION_VERSION: i64 = 20260817140000;
const LEGACY_MIGRATION_SQL: &str =
    include_str!("../../migrations/20260817130000_backfill_legacy_model_pricing_rules.sql");
const CLEANUP_MIGRATION_SQL: &str =
    include_str!("../../migrations/20260817140000_replace_generated_zero_pricing_rules.sql");

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

fn before_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version < CLEANUP_MIGRATION_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

#[tokio::test]
async fn pricing_cleanup_replaces_generated_model_rules_with_one_global_fallback() {
    let database = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    before_migrator().run(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "Pricing migration")
        .await
        .unwrap();
    let user_id = Uuid::now_v7();
    let account = format!("pricing-migration-{}", user_id.simple());
    sqlx::query(
        r#"insert into users
        (id,account,email,password_hash,name,nickname,introduction,email_login_enabled,phone_login_enabled,status,session_version)
        values ($1,$2,$3,'hash',$2,$2,'',true,false,'active',1)"#,
    )
    .bind(user_id)
    .bind(&account)
    .bind(format!("{account}@example.test"))
    .execute(store.pool())
    .await
    .unwrap();
    let installation_id = Uuid::now_v7();
    sqlx::query(
        r#"insert into extension_installations
        (id,category,organization,artifact_id,artifact_version,plugin_id,contract_version,
         protocol,display_name,source_kind,trust_level,verification_status,desired_state,
         expected_checksum,signature_status,created_by,updated_by)
        values ($1,'runtime-extensions','taichuy','legacy-provider','1.0.0',$2,
                '1flowbase.provider/v1','openai','Legacy provider','uploaded','checksum_only',
                'valid','active_requested','sha256:fixture','missing',$3,$3)"#,
    )
    .bind(installation_id)
    .bind(format!("legacy-provider-{}", installation_id.simple()))
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"insert into model_provider_instances
        (id,workspace_id,installation_id,provider_code,protocol,display_name,status,config_json,
         configured_models_json,enabled_model_ids,included_in_main,created_by,updated_by)
        values ($1,$2,$3,'legacy-provider','openai','Legacy instance','ready','{}',
                '[{"model_id":"legacy-free","enabled":true},{"model_id":"legacy-priced","enabled":false}]',
                array['legacy-free'],$4,$5,$5)"#,
    )
    .bind(Uuid::now_v7())
    .bind(workspace.id)
    .bind(installation_id)
    .bind(true)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"insert into model_pricing_rules
        (id,provider_code,upstream_model_id,input_token_unit_size,input_token_unit_price,
         output_token_unit_size,output_token_unit_price,cache_hit_token_unit_size,
         cache_hit_token_unit_price,currency_code,effective_from,timezone,weekday_mask,
         priority,enabled,source_kind,extensions,created_by)
        values ($1,'legacy-provider','legacy-priced',1000000,2,1000000,2,1000000,2,
                'USD',now(),'UTC',127,0,true,'manual','{}',$2)"#,
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();

    sqlx::raw_sql(LEGACY_MIGRATION_SQL)
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::raw_sql(LEGACY_MIGRATION_SQL)
        .execute(store.pool())
        .await
        .unwrap();

    sqlx::query(
        r#"insert into model_pricing_rules
        (id,provider_code,upstream_model_id,input_token_unit_size,input_token_unit_price,
         output_token_unit_size,output_token_unit_price,cache_hit_token_unit_size,
         cache_hit_token_unit_price,currency_code,effective_from,timezone,weekday_mask,
         priority,enabled,source_kind,source_catalog_id,source_version,source_checksum,
         extensions,created_by)
        values ($1,'plugin-provider','plugin-model',1000000,0,1000000,0,1000000,0,
                'USD',now(),'UTC',127,10,true,'official',$2,'2026-08-17.2',$3,
                '{"pricing_policy":"official_zero_default","reason":"upgrade_compatibility"}',$4)"#,
    )
    .bind(Uuid::now_v7())
    .bind("old-plugin-model-rule")
    .bind(format!("sha256:{}", "0".repeat(64)))
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();

    sqlx::raw_sql(CLEANUP_MIGRATION_SQL)
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::raw_sql(CLEANUP_MIGRATION_SQL)
        .execute(store.pool())
        .await
        .unwrap();

    let free_rules: i64 = sqlx::query_scalar(
        "select count(*) from model_pricing_rules where provider_code='legacy-provider' and upstream_model_id='legacy-free' and input_token_unit_price=0 and output_token_unit_price=0 and cache_hit_token_unit_price=0 and extensions->>'origin'='upgrade_compat'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(free_rules, 0);
    let old_official_rules: i64 = sqlx::query_scalar(
        "select count(*) from model_pricing_rules where extensions->>'pricing_policy'='official_zero_default'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(old_official_rules, 0);
    let global_fallbacks: i64 = sqlx::query_scalar(
        "select count(*) from model_pricing_rules where provider_code='zero' and upstream_model_id='any' and input_token_unit_price=0 and output_token_unit_price=0 and cache_hit_token_unit_price=0 and extensions->>'pricing_policy'='global_zero_fallback'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(global_fallbacks, 1);
    let existing_price: String = sqlx::query_scalar(
        "select input_token_unit_price::text from model_pricing_rules where provider_code='legacy-provider' and upstream_model_id='legacy-priced'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(existing_price, "2.000000000000000000");
}

// AC3: migrate real stored v1/v2 defaults and sparse rules without changing identities/schedules.
#[tokio::test]
async fn pricing_format_v2_migrates_fixed_tier_and_ttl_records_in_place() {
    let database = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    let version = 20260910120000;
    let before = Migrator {
        migrations: Cow::Owned(
            sqlx::migrate!("./migrations")
                .iter()
                .filter(|m| m.version < version)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    };
    before.run(&pool).await.unwrap();
    let fixed = Uuid::now_v7();
    let tier = Uuid::now_v7();
    let ttl = Uuid::now_v7();
    for (id, enabled, policy) in [
        (fixed, false, serde_json::json!({})),
        (
            tier,
            true,
            serde_json::json!({"schema_version":"1flowbase.model-rating-policy/v1","type":"input_token_tiers","tiers":[{"when":{"operator":"gte","value":200000},"rates":{"input":{"unit_size":1000,"unit_price":"2"},"output":{"unit_size":1000,"unit_price":"4"},"cache_hit":{"unit_size":1000,"unit_price":"0.2"}}}]}),
        ),
        (
            ttl,
            true,
            serde_json::json!({"schema_version":"1flowbase.model-rating-policy/v2","type":"token_pricing","unit_size":1000000,"rates":{"input":"10","output":"50","cache_hit":"0.25","cache_write":{"by_ttl_seconds":{"300":"12.5","3600":"20"}}}}),
        ),
    ] {
        sqlx::query("insert into model_pricing_rules (id,provider_code,upstream_model_id,input_token_unit_size,input_token_unit_price,output_token_unit_size,output_token_unit_price,cache_hit_token_unit_size,cache_hit_token_unit_price,currency_code,effective_from,timezone,weekday_mask,priority,enabled,source_kind,extensions,rating_policy_enabled,rating_policy) values ($1,'migration-v2',$2,1000,1,1000,3,1000,0.1,'USD','2026-01-01T00:00:00Z','UTC',127,0,true,'manual','{}',$3,$4)")
            .bind(id).bind(id.to_string()).bind(enabled).bind(policy).execute(&pool).await.unwrap();
    }
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    use control_plane_contracts::ports::BillingRepository;
    let fixed_row = store.get_pricing_rule(fixed).await.unwrap().unwrap();
    assert_eq!(
        fixed_row.input_token_unit_price,
        fixed_row.cache_write_token_unit_price
    );
    assert_eq!(fixed_row.cache_write_token_unit_size, 1000);
    assert_eq!(fixed_row.rules, serde_json::json!([]));
    let tier_row = store.get_pricing_rule(tier).await.unwrap().unwrap();
    assert_eq!(tier_row.rules[0]["when"]["input_tokens"]["operator"], "gte");
    assert_eq!(
        tier_row.rules[0]["overrides"]["cache_write_token_unit_price"],
        "2"
    );
    assert_eq!(
        tier_row.rules[0]["overrides"]["input_token_unit_size"],
        1000
    );
    let ttl_row = store.get_pricing_rule(ttl).await.unwrap().unwrap();
    assert_eq!(
        ttl_row.input_token_unit_price,
        rust_decimal::Decimal::from(10)
    );
    assert_eq!(
        ttl_row.cache_write_token_unit_price,
        rust_decimal::Decimal::new(125, 1)
    );
    assert_eq!(
        ttl_row.rules,
        serde_json::json!([{"when":{"cache_write_ttl_seconds":3600},"overrides":{"cache_write_token_unit_price":"20"}}])
    );
    for row in [&fixed_row, &tier_row, &ttl_row] {
        row.validate().unwrap();
        assert_eq!(row.source_kind, "manual");
        assert_eq!(row.timezone, "UTC");
        assert!(row.enabled);
    }
    sqlx::migrate!("./migrations")
        .run(store.pool())
        .await
        .unwrap();
    assert_eq!(
        store.get_pricing_rule(ttl).await.unwrap().unwrap().rules,
        ttl_row.rules
    );
}
