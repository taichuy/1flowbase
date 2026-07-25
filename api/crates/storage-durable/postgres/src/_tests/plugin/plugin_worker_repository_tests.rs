use control_plane::ports::{
    CreatePluginWorkerLeaseInput, PluginRepository, PluginWorkerRepository,
    UpsertPluginInstallationInput,
};
use domain::{
    PluginArtifactStatus, PluginAvailabilityStatus, PluginDesiredState, PluginRuntimeStatus,
    PluginVerificationStatus, PluginWorkerStatus,
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

async fn seed_store() -> (PgControlPlaneStore, Uuid) {
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

    (store, actor.id)
}

#[tokio::test]
async fn plugin_worker_repository_tracks_process_per_call_worker_lifecycle() {
    let (store, _actor_id) = seed_store().await;
    let installation_id = Uuid::now_v7();
    let _installation = PluginRepository::upsert_installation(
        &store,
        &UpsertPluginInstallationInput {
            installation_id,
            provider_code: "capability_provider".into(),
            plugin_id: "capability_provider@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            contract_version: "1flowbase.capability/v1".into(),
            protocol: "stdio_json".into(),
            display_name: "Capability Provider".into(),
            source_kind: "uploaded".into(),
            trust_level: "unverified".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::ActiveRequested,
            artifact_status: PluginArtifactStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status: PluginAvailabilityStatus::InstallIncomplete,
            package_path: None,
            installed_path: "/tmp/plugin-installed/capability_provider/0.1.0".into(),
            checksum: None,
            manifest_fingerprint: None,
            signature_status: None,
            signature_algorithm: None,
            signing_key_id: None,
            last_load_error: None,
            metadata_json: json!({}),
            actor_user_id: _actor_id,
        },
    )
    .await
    .unwrap();

    let lease = PluginWorkerRepository::create_worker_lease(
        &store,
        &CreatePluginWorkerLeaseInput {
            installation_id,
            worker_key: "capability:openai_prompt".into(),
            status: PluginWorkerStatus::Starting,
        },
    )
    .await
    .unwrap();

    assert_eq!(lease.installation_id, installation_id);
    assert_eq!(lease.worker_key, "capability:openai_prompt");
    assert_eq!(lease.status, PluginWorkerStatus::Starting);
    assert_eq!(lease.runtime_scope, json!({}));
    let lease_scope: Uuid =
        sqlx::query_scalar("select scope_id from plugin_worker_leases where id = $1")
            .bind(lease.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(lease_scope, domain::SYSTEM_SCOPE_ID);
}
