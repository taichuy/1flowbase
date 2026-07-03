use control_plane::ports::{
    CreatePluginAssignmentInput, FrontendBlockCatalogRegistryInput, FrontendBlockCatalogRepository,
    PluginRepository, ReplaceInstallationFrontendBlocksInput, UpsertPluginInstallationInput,
};
use domain::{
    PluginArtifactStatus, PluginAvailabilityStatus, PluginDesiredState, PluginRuntimeStatus,
    PluginVerificationStatus,
};
use serde_json::json;
use sqlx::PgPool;
use storage_postgres::{connect, run_migrations, PgControlPlaneStore};
use uuid::Uuid;

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
            options: json!({}),
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

    (store, workspace, actor)
}

#[tokio::test]
async fn frontend_block_catalog_repository_lists_only_assigned_workspace_blocks() {
    let (store, workspace, actor) = seed_store().await;
    let installation = PluginRepository::upsert_installation(
        &store,
        &UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            provider_code: "fixture_frontend_blocks".into(),
            plugin_id: "fixture_frontend_blocks@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            contract_version: "1flowbase.capability/v1".into(),
            protocol: "stdio_json".into(),
            display_name: "Fixture Frontend Blocks".into(),
            source_kind: "uploaded".into(),
            trust_level: "checksum_only".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::ActiveRequested,
            artifact_status: PluginArtifactStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status: PluginAvailabilityStatus::Available,
            package_path: None,
            installed_path: "/tmp/plugins/fixture_frontend_blocks/0.1.0".into(),
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

    FrontendBlockCatalogRepository::replace_installation_frontend_blocks(
        &store,
        &ReplaceInstallationFrontendBlocksInput {
            installation_id: installation.id,
            provider_code: installation.provider_code.clone(),
            plugin_id: installation.plugin_id.clone(),
            plugin_version: installation.plugin_version.clone(),
            entries: vec![FrontendBlockCatalogRegistryInput {
                contribution_code: "hero_banner".into(),
                title: "Hero Banner".into(),
                runtime: "iframe".into(),
                entry: "blocks/hero/index.html".into(),
                context_contract: domain::FrontendBlockContextContract {
                    primitives: vec!["text".into(), "image".into()],
                    input_schema: json!({ "type": "object" }),
                },
                permissions: domain::FrontendBlockPermissions {
                    network: "none".into(),
                    storage: "none".into(),
                    secrets: "none".into(),
                },
                ui_capabilities: vec!["responsive".into()],
            }],
        },
    )
    .await
    .unwrap();

    assert!(
        FrontendBlockCatalogRepository::list_workspace_frontend_blocks(&store, workspace.id)
            .await
            .unwrap()
            .is_empty()
    );

    PluginRepository::create_assignment(
        &store,
        &CreatePluginAssignmentInput {
            installation_id: installation.id,
            workspace_id: workspace.id,
            provider_code: installation.provider_code.clone(),
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();

    let entries =
        FrontendBlockCatalogRepository::list_workspace_frontend_blocks(&store, workspace.id)
            .await
            .unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].contribution_code, "hero_banner");
    assert_eq!(
        entries[0].context_contract.primitives,
        vec!["text", "image"]
    );
}
