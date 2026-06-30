use control_plane::ports::{
    ApplicationJsDependencySelectionRepository, CreateApplicationInput,
    CreatePluginAssignmentInput, JsDependencyRegistryInput, JsDependencyRepository,
    PluginRepository, ReplaceApplicationJsDependencySelectionInput,
    ReplaceInstallationJsDependenciesInput, UpsertPluginInstallationInput,
};
use domain::{
    ApplicationType, PluginArtifactStatus, PluginAvailabilityStatus, PluginDesiredState,
    PluginRuntimeStatus, PluginVerificationStatus,
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
            name: "password-local".into(),
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

    (store, workspace, actor)
}

async fn seed_js_dependency(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
    actor_id: Uuid,
    version: &str,
) -> domain::JsDependencyRegistryEntry {
    let installation = PluginRepository::upsert_installation(
        store,
        &UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            provider_code: format!("fixture_js_dependency_pack_{version}"),
            plugin_id: format!("fixture_js_dependency_pack@{version}"),
            plugin_version: version.into(),
            contract_version: "1flowbase.capability/v1".into(),
            protocol: "stdio_json".into(),
            display_name: "Fixture JS Dependency Pack".into(),
            source_kind: "uploaded".into(),
            trust_level: "checksum_only".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::ActiveRequested,
            artifact_status: PluginArtifactStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status: PluginAvailabilityStatus::Available,
            package_path: None,
            installed_path: format!("/tmp/plugins/fixture_js_dependency_pack/{version}"),
            checksum: None,
            manifest_fingerprint: None,
            signature_status: None,
            signature_algorithm: None,
            signing_key_id: None,
            last_load_error: None,
            metadata_json: json!({}),
            actor_user_id: actor_id,
        },
    )
    .await
    .unwrap();
    PluginRepository::create_assignment(
        store,
        &CreatePluginAssignmentInput {
            installation_id: installation.id,
            workspace_id,
            provider_code: installation.provider_code.clone(),
            actor_user_id: actor_id,
        },
    )
    .await
    .unwrap();
    JsDependencyRepository::replace_installation_js_dependencies(
        store,
        &ReplaceInstallationJsDependenciesInput {
            installation_id: installation.id,
            provider_code: installation.provider_code.clone(),
            plugin_id: installation.plugin_id.clone(),
            plugin_version: installation.plugin_version.clone(),
            entries: vec![JsDependencyRegistryInput {
                alias: "zod".into(),
                package: "zod".into(),
                version: version.into(),
                target: "backend_code".into(),
                artifact_path: format!("artifacts/zod-{version}.backend.mjs"),
                integrity: format!("sha256-zod-{version}"),
                permissions: domain::JsDependencyPermissions {
                    network: "outbound_only".into(),
                    filesystem: "deny".into(),
                    env: "deny".into(),
                },
            }],
        },
    )
    .await
    .unwrap();

    JsDependencyRepository::list_workspace_js_dependencies(store, workspace_id)
        .await
        .unwrap()
        .into_iter()
        .find(|entry| entry.installation_id == installation.id)
        .unwrap()
}

#[tokio::test]
async fn application_js_dependency_repository_replaces_alias_target_and_preserves_snapshot() {
    let (store, workspace, actor) = seed_store().await;
    let application = store
        .create_application(&CreateApplicationInput {
            actor_user_id: actor.id,
            workspace_id: workspace.id,
            application_type: ApplicationType::AgentFlow,
            name: "Agent Support".into(),
            description: String::new(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();
    let zod_v3 = seed_js_dependency(&store, workspace.id, actor.id, "3.24.0").await;
    let zod_v4 = seed_js_dependency(&store, workspace.id, actor.id, "4.0.0").await;

    ApplicationJsDependencySelectionRepository::replace_application_js_dependency_selection(
        &store,
        &ReplaceApplicationJsDependencySelectionInput::from_catalog_entry(
            actor.id,
            workspace.id,
            application.id,
            zod_v3,
        ),
    )
    .await
    .unwrap();
    ApplicationJsDependencySelectionRepository::replace_application_js_dependency_selection(
        &store,
        &ReplaceApplicationJsDependencySelectionInput::from_catalog_entry(
            actor.id,
            workspace.id,
            application.id,
            zod_v4,
        ),
    )
    .await
    .unwrap();

    let selections =
        ApplicationJsDependencySelectionRepository::list_application_js_dependency_selections(
            &store,
            workspace.id,
            application.id,
        )
        .await
        .unwrap();

    assert_eq!(selections.len(), 1);
    assert_eq!(selections[0].version, "4.0.0");
    assert_eq!(
        selections[0].artifact_path,
        "artifacts/zod-4.0.0.backend.mjs"
    );
    assert_eq!(selections[0].artifact_hash, "sha256-zod-4.0.0");
    assert_eq!(selections[0].permissions.env, "deny");
}
