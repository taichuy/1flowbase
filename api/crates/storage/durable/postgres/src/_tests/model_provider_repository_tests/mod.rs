use control_plane_contracts::ports::{
    CreateModelCatalogSyncRunInput, CreateModelFailoverQueueItemInput,
    CreateModelFailoverQueueSnapshotInput, CreateModelFailoverQueueTemplateInput,
    CreateModelProviderCatalogSourceInput, CreateModelProviderInstanceInput,
    ModelProviderRepository, PatchModelProviderSecretInput, ReassignModelProviderInstancesInput,
    UpdateModelProviderInstanceInput, UpsertModelProviderCatalogCacheInput,
    UpsertModelProviderCatalogEntryInput, UpsertModelProviderMainInstanceInput,
    UpsertModelProviderSecretInput, UpsertPluginInstallationInput,
};
use domain::{
    ModelProviderCatalogRefreshStatus, ModelProviderCatalogSource, ModelProviderDiscoveryMode,
    ModelProviderInstanceStatus, PluginDesiredState, PluginVerificationStatus,
};
use serde_json::{json, Value};
use storage_durable_postgres::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

const LEGACY_PROVIDER_SCHEMA_VERSION: i64 = 20260422223000;
const MAIN_INSTANCE_AGGREGATION_VERSION: i64 = 20260423093000;
const CONTEXT_WINDOW_OVERRIDE_VERSION: i64 = 20260423235000;

async fn migrate_provider_history(pool: &sqlx::PgPool, through: i64) {
    sqlx::migrate::Migrator::with_migrations(
        sqlx::migrate!("./migrations")
            .iter()
            .filter(|migration| migration.version <= through)
            .cloned()
            .collect(),
    )
    .run(pool)
    .await
    .unwrap();
}

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database() -> postgres_test_support::PostgresTestSchema {
    postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap()
}

async fn seed_store() -> (
    PgControlPlaneStore,
    domain::WorkspaceRecord,
    domain::UserRecord,
    Uuid,
) {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);

    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "1flowbase")
        .await
        .unwrap();
    control_plane_test_support::upsert_permission_catalog(&store)
        .await
        .unwrap();
    control_plane_test_support::upsert_builtin_roles(&store, workspace.id)
        .await
        .unwrap();
    store
        .upsert_login_entry(&domain::LoginEntryRecord {
            id: domain::BUILTIN_PASSWORD_LOGIN_ENTRY_ID,
            connection_id: domain::PASSWORD_LOCAL_CONNECTION_ID,
            auth_type: "password-local".into(),
            title: "Password".into(),
            enabled: true,
            is_builtin: true,
            sort_order: 0,
            public_ui_block: String::new(),
            options: serde_json::json!({}),
        })
        .await
        .unwrap();
    let actor = store
        .upsert_root_user(
            workspace.id,
            "root",
            "root@example.com",
            "$argon2id$v=19$m=19456,t=2,p=1$test$test",
            "Root",
            "Root",
        )
        .await
        .unwrap();

    let installation_id = Uuid::now_v7();
    control_plane_contracts::ports::PluginRepository::upsert_installation(
        &store,
        &UpsertPluginInstallationInput {
            installation_id,
            category: domain::ExtensionCategory::RuntimeExtensions,
            organization: "test".to_string(),
            provider_code: "fixture_provider".into(),
            plugin_id: "fixture_provider@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            contract_version: "1flowbase.provider/v1".into(),
            protocol: "openai_compatible".into(),
            display_name: "Fixture Provider".into(),
            source_kind: "uploaded".into(),
            trust_level: "unverified".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::ActiveRequested,
            expected_checksum: Some("abc123".into()),
            signature_status: domain::ExtensionSignatureStatus::Missing,
            signature_algorithm: None,
            signing_key_id: None,
            metadata_json: json!({}),
            is_system_reserved: false,
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();

    (store, workspace, actor, installation_id)
}

async fn seed_store_before_main_instance_aggregation(
) -> (PgControlPlaneStore, domain::WorkspaceRecord, Uuid, Uuid) {
    let pool = isolated_database().await.connect().await.unwrap();
    migrate_provider_history(&pool, LEGACY_PROVIDER_SCHEMA_VERSION).await;

    // AC009 fixture authenticity: do not accidentally apply the migration under test.
    let aggregation_absent: bool = sqlx::query_scalar(
        "select to_regclass(current_schema() || '.model_provider_main_instances') is null",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(aggregation_absent);

    let store = PgControlPlaneStore::new(pool);
    let tenant_id = postgres_test_support::insert_historical_root_tenant(store.pool())
        .await
        .unwrap();
    let workspace = domain::WorkspaceRecord {
        id: domain::DEFAULT_SCOPE_ID,
        tenant_id,
        name: "1flowbase".into(),
        logo_url: None,
        introduction: String::new(),
    };
    postgres_test_support::insert_historical_workspace(
        store.pool(),
        workspace.id,
        tenant_id,
        &workspace.name,
    )
    .await
    .unwrap();
    let actor = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into users (id, account, email, password_hash, name, nickname, status)
        values ($1, $2, $3, 'fixture', 'Upgrade fixture', 'Upgrade fixture', 'active')
        "#,
    )
    .bind(actor)
    .bind(format!("upgrade-fixture-{}", actor.simple()))
    .bind(format!("upgrade-fixture-{}@example.com", actor.simple()))
    .execute(store.pool())
    .await
    .unwrap();
    let installation_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into plugin_installations (
            id,
            provider_code,
            plugin_id,
            plugin_version,
            contract_version,
            protocol,
            display_name,
            source_kind,
            trust_level,
            verification_status,
            desired_state,
            installed_path,
            checksum,
            signature_status,
            metadata_json,
            created_by
        ) values (
            $1, $2, $3, $4, $5, $6, $7, $8,
            $9, $10, $11, $12, $13, $14, $15, $16
        )
        "#,
    )
    .bind(installation_id)
    .bind("fixture_provider")
    .bind("fixture_provider@0.1.0")
    .bind("0.1.0")
    .bind("1flowbase.provider/v1")
    .bind("openai_compatible")
    .bind("Fixture Provider")
    .bind("uploaded")
    .bind("unverified")
    .bind("valid")
    .bind("active_requested")
    .bind("/tmp/fixture-provider")
    .bind("abc123")
    .bind("missing")
    .bind(json!({"vendor": "test"}))
    .bind(actor)
    .execute(store.pool())
    .await
    .unwrap();

    (store, workspace, actor, installation_id)
}

async fn insert_legacy_instance(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
    installation_id: Uuid,
    actor_id: Uuid,
    display_name: &str,
    enabled_model_ids: Vec<String>,
) -> Uuid {
    let instance_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into model_provider_instances (
            id,
            workspace_id,
            installation_id,
            provider_code,
            protocol,
            display_name,
            status,
            config_json,
            configured_models_json,
            enabled_model_ids,
            created_by,
            updated_by
        ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $11)
        "#,
    )
    .bind(instance_id)
    .bind(workspace_id)
    .bind(installation_id)
    .bind("fixture_provider")
    .bind("openai_compatible")
    .bind(display_name)
    .bind(ModelProviderInstanceStatus::Ready.as_str())
    .bind(json!({ "base_url": format!("https://{}.example.com/v1", display_name.to_lowercase()) }))
    .bind(Value::Array(
        enabled_model_ids
            .iter()
            .map(|model_id| {
                json!({
                    "model_id": model_id,
                    "enabled": true
                })
            })
            .collect(),
    ))
    .bind(enabled_model_ids)
    .bind(actor_id)
    .execute(store.pool())
    .await
    .unwrap();

    instance_id
}

async fn create_ready_instance(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
    actor_id: Uuid,
    installation_id: Uuid,
    display_name: &str,
    enabled_model_ids: Vec<String>,
) -> domain::ModelProviderInstanceRecord {
    ModelProviderRepository::create_instance(
        store,
        &CreateModelProviderInstanceInput {
            instance_id: Uuid::now_v7(),
            workspace_id,
            installation_id,
            provider_code: "fixture_provider".into(),
            protocol: "openai_compatible".into(),
            display_name: display_name.into(),
            status: ModelProviderInstanceStatus::Ready,
            config_json: json!({ "base_url": "https://api.example.com/v1" }),
            configured_models: enabled_model_ids
                .iter()
                .map(|model_id| domain::ModelProviderConfiguredModel {
                    model_id: model_id.clone(),
                    enabled: true,
                    context_window_override_tokens: None,
                    supports_multimodal: None,
                    pricing_provider_code: domain::DEFAULT_MODEL_PRICING_PROVIDER_CODE.to_string(),
                    pricing_model_id: domain::DEFAULT_MODEL_PRICING_MODEL_ID.to_string(),
                })
                .collect(),
            enabled_model_ids,
            included_in_main: Some(true),
            created_by: actor_id,
        },
    )
    .await
    .unwrap()
}

mod catalogs_and_failover;
mod instances;
mod session_affinity;
