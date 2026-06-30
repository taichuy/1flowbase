use control_plane::ports::{
    CreateModelCatalogSyncRunInput, CreateModelFailoverQueueItemInput,
    CreateModelFailoverQueueSnapshotInput, CreateModelFailoverQueueTemplateInput,
    CreateModelProviderCatalogSourceInput, CreateModelProviderInstanceInput,
    ModelProviderRepository, ReassignModelProviderInstancesInput, UpdateModelProviderInstanceInput,
    UpsertModelProviderCatalogCacheInput, UpsertModelProviderCatalogEntryInput,
    UpsertModelProviderMainInstanceInput, UpsertModelProviderSecretInput,
    UpsertPluginInstallationInput,
};
use domain::{
    ModelProviderCatalogRefreshStatus, ModelProviderCatalogSource, ModelProviderDiscoveryMode,
    ModelProviderInstanceStatus, PluginArtifactStatus, PluginAvailabilityStatus,
    PluginDesiredState, PluginRuntimeStatus, PluginVerificationStatus,
};
use serde_json::{json, Value};
use sqlx::PgPool;
use storage_postgres::{connect, run_migrations, PgControlPlaneStore};
use uuid::Uuid;

const PRE_MAIN_INSTANCE_AGGREGATION_MIGRATIONS: &[&str] = &[
    include_str!("../../../migrations/20260412183000_create_auth_team_acl_tables.sql"),
    include_str!("../../../migrations/20260412230000_create_model_definition_tables.sql"),
    include_str!("../../../migrations/20260413103000_align_model_definition_physical_schema.sql"),
    include_str!("../../../migrations/20260413220000_add_tenant_workspace_scope.sql"),
    include_str!("../../../migrations/20260413223000_add_runtime_metadata_health.sql"),
    include_str!("../../../migrations/20260414203000_add_role_policy_flags.sql"),
    include_str!("../../../migrations/20260415093000_create_application_tables.sql"),
    include_str!("../../../migrations/20260415113000_create_flow_tables.sql"),
    include_str!("../../../migrations/20260416174500_add_application_tags.sql"),
    include_str!("../../../migrations/20260417173000_create_orchestration_runtime_tables.sql"),
    include_str!("../../../migrations/20260417210000_add_flow_run_resume_and_callback_tasks.sql"),
    include_str!("../../../migrations/20260418120000_create_provider_kernel_tables.sql"),
    include_str!("../../../migrations/20260418123000_create_model_provider_instance_tables.sql"),
    include_str!("../../../migrations/20260419143000_add_plugin_version_pointer.sql"),
    include_str!("../../../migrations/20260419183000_add_plugin_install_trust_fields.sql"),
    include_str!("../../../migrations/20260420120000_add_user_preferred_locale.sql"),
    include_str!("../../../migrations/20260519120000_add_user_meta.sql"),
    include_str!("../../../migrations/20260420203000_add_plugin_lifecycle_snapshots.sql"),
    include_str!("../../../migrations/20260421113000_create_node_contribution_registry_tables.sql"),
    include_str!("../../../migrations/20260421123000_create_plugin_worker_lease_tables.sql"),
    include_str!(
        "../../../migrations/20260422121000_add_model_provider_validation_and_preview_sessions.sql"
    ),
    include_str!(
        "../../../migrations/20260422180000_replace_validation_model_with_enabled_models.sql"
    ),
    include_str!("../../../migrations/20260422193000_add_model_provider_configured_models.sql"),
    include_str!("../../../migrations/20260422223000_create_model_provider_routings.sql"),
];

const MAIN_INSTANCE_AGGREGATION_MIGRATION_SQL: &str = include_str!(
    "../../../migrations/20260423093000_replace_manual_primary_with_main_instance_aggregation.sql"
);

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database_url() -> String {
    let admin_pool = PgPool::connect(&base_database_url()).await.unwrap();
    let schema = format!("test_{}", Uuid::now_v7().to_string().replace('-', ""));
    sqlx::query(&format!("create schema if not exists {schema}"))
        .execute(&admin_pool)
        .await
        .unwrap();

    format!("{}?options=-csearch_path%3D{schema}", base_database_url())
}

async fn seed_store() -> (
    PgControlPlaneStore,
    domain::WorkspaceRecord,
    domain::UserRecord,
    Uuid,
) {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);

    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "1flowbase")
        .await
        .unwrap();
    store
        .upsert_permission_catalog(&access_control::permission_catalog())
        .await
        .unwrap();
    store.upsert_builtin_roles(workspace.id).await.unwrap();
    store
        .upsert_authenticator(&domain::AuthenticatorRecord {
            id: domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
            auth_type: "password-local".into(),
            title: "Password".into(),
            enabled: true,
            is_builtin: true,
            sort_order: 0,
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
    control_plane::ports::PluginRepository::upsert_installation(
        &store,
        &UpsertPluginInstallationInput {
            installation_id,
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
            artifact_status: PluginArtifactStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status: PluginAvailabilityStatus::InstallIncomplete,
            package_path: None,
            installed_path: "/tmp/plugin-installed/fixture_provider/0.1.0".into(),
            checksum: Some("abc123".into()),
            manifest_fingerprint: None,
            signature_status: Some("unsigned".into()),
            signature_algorithm: None,
            signing_key_id: None,
            last_load_error: None,
            metadata_json: json!({}),
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();

    (store, workspace, actor, installation_id)
}

async fn seed_store_before_main_instance_aggregation() -> (
    PgControlPlaneStore,
    domain::WorkspaceRecord,
    domain::UserRecord,
    Uuid,
) {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    for migration_sql in PRE_MAIN_INSTANCE_AGGREGATION_MIGRATIONS {
        sqlx::raw_sql(migration_sql).execute(&pool).await.unwrap();
    }
    sqlx::raw_sql(
        r#"
        alter table permission_definitions
            add column if not exists scope_id uuid not null
            default '00000000-0000-0000-0000-000000000000'::uuid;
        alter table roles add column if not exists scope_id uuid;
        alter table role_permissions add column if not exists scope_id uuid;
        alter table user_role_bindings add column if not exists scope_id uuid;
        alter table plugin_installations
            add column if not exists scope_id uuid not null
            default '00000000-0000-0000-0000-000000000000'::uuid;
        alter table plugin_installations add column if not exists updated_by uuid;
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let store = PgControlPlaneStore::new(pool);
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "1flowbase")
        .await
        .unwrap();
    store
        .upsert_permission_catalog(&access_control::permission_catalog())
        .await
        .unwrap();
    store.upsert_builtin_roles(workspace.id).await.unwrap();
    store
        .upsert_authenticator(&domain::AuthenticatorRecord {
            id: domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
            auth_type: "password-local".into(),
            title: "Password".into(),
            enabled: true,
            is_builtin: true,
            sort_order: 0,
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
    control_plane::ports::PluginRepository::upsert_installation(
        &store,
        &UpsertPluginInstallationInput {
            installation_id,
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
            artifact_status: PluginArtifactStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status: PluginAvailabilityStatus::InstallIncomplete,
            package_path: None,
            installed_path: "/tmp/plugin-installed/fixture_provider/0.1.0".into(),
            checksum: Some("abc123".into()),
            manifest_fingerprint: None,
            signature_status: Some("unsigned".into()),
            signature_algorithm: None,
            signing_key_id: None,
            last_load_error: None,
            metadata_json: json!({}),
            actor_user_id: actor.id,
        },
    )
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
