use control_plane::ports::{
    HostInfrastructureConfigRepository, PluginRepository,
    UpsertHostInfrastructureProviderConfigInput, UpsertPluginInstallationInput,
};
use domain::{
    HostInfrastructureConfigStatus, PluginArtifactStatus, PluginAvailabilityStatus,
    PluginDesiredState, PluginRuntimeStatus, PluginVerificationStatus,
};
use serde_json::json;
use storage_postgres::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database() -> postgres_test_support::PostgresTestSchema {
    postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap()
}

async fn seed_store() -> (PgControlPlaneStore, domain::UserRecord, Uuid) {
    let pool = isolated_database().await.connect().await.unwrap();
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
    let installation = PluginRepository::upsert_installation(
        &store,
        &UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            provider_code: "redis-infra-host".into(),
            plugin_id: "redis-infra-host@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            contract_version: "1flowbase.host_extension/v1".into(),
            protocol: "native_host".into(),
            display_name: "Redis Infra Host".into(),
            source_kind: "uploaded".into(),
            trust_level: "unverified".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::PendingRestart,
            artifact_status: PluginArtifactStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status: PluginAvailabilityStatus::PendingRestart,
            package_path: None,
            installed_path: "/tmp/plugin-installed/redis-infra-host/0.1.0".into(),
            checksum: None,
            manifest_fingerprint: None,
            signature_status: None,
            signature_algorithm: None,
            signing_key_id: None,
            last_load_error: None,
            metadata_json: json!({}),
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();

    (store, actor, installation.id)
}

#[tokio::test]
async fn host_infrastructure_config_repository_upserts_and_lists_provider_config() {
    let (store, actor, installation_id) = seed_store().await;

    let saved = HostInfrastructureConfigRepository::upsert_host_infrastructure_provider_config(
        &store,
        &UpsertHostInfrastructureProviderConfigInput {
            installation_id,
            extension_id: "redis-infra-host".into(),
            provider_code: "redis".into(),
            config_ref: "secret://system/redis-infra-host/config".into(),
            enabled_contracts: vec!["storage-ephemeral".into(), "cache-store".into()],
            config_json: json!({ "host": "localhost", "port": 6379 }),
            status: HostInfrastructureConfigStatus::PendingRestart,
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();

    assert_eq!(saved.installation_id, installation_id);
    assert_eq!(saved.provider_code, "redis");
    assert_eq!(saved.status, HostInfrastructureConfigStatus::PendingRestart);
    assert_eq!(
        saved.enabled_contracts,
        vec!["storage-ephemeral".to_string(), "cache-store".to_string()]
    );

    let listed =
        HostInfrastructureConfigRepository::list_host_infrastructure_provider_configs(&store)
            .await
            .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].provider_code, "redis");
    assert_eq!(
        listed[0].config_json,
        json!({ "host": "localhost", "port": 6379 })
    );
}
