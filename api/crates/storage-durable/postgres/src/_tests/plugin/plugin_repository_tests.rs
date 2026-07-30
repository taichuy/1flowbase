use control_plane::ports::{
    CreatePluginAssignmentInput, CreatePluginTaskInput, JsDependencyRegistryInput,
    JsDependencyRepository, PluginRepository, ReplaceInstallationJsDependenciesInput,
    UpdatePluginTaskStatusInput, UpsertPluginInstallationInput,
    UpsertPluginPackageCatalogProjectionInput,
};
use domain::{
    PluginArtifactStatus, PluginAvailabilityStatus, PluginDesiredState,
    PluginPackageCatalogProjectionStatus, PluginRuntimeStatus, PluginTaskKind, PluginTaskStatus,
    PluginVerificationStatus,
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

async fn seed_store() -> (
    PgControlPlaneStore,
    domain::WorkspaceRecord,
    domain::UserRecord,
) {
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

    (store, workspace, actor)
}

#[tokio::test]
async fn plugin_repository_persists_package_catalog_projection() {
    let (store, _workspace, actor) = seed_store().await;
    let installation_id = Uuid::now_v7();
    PluginRepository::upsert_installation(
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
            runtime_status: PluginRuntimeStatus::Active,
            availability_status: PluginAvailabilityStatus::Available,
            package_path: None,
            installed_path: "/tmp/plugin-installed/fixture_provider/0.1.0".into(),
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

    let projection = PluginRepository::upsert_plugin_package_catalog_projection(
        &store,
        &UpsertPluginPackageCatalogProjectionInput {
            installation_id,
            package_code: "fixture_provider".into(),
            package_version: "0.1.0".into(),
            catalog_snapshot_json: json!({
                "provider": {
                    "model_discovery_mode": "hybrid"
                }
            }),
            projection_status: PluginPackageCatalogProjectionStatus::Ok,
            last_error_message: None,
            refreshed_at: Some(time::OffsetDateTime::now_utc()),
        },
    )
    .await
    .unwrap();

    assert_eq!(projection.installation_id, installation_id);
    assert_eq!(
        projection.projection_status,
        PluginPackageCatalogProjectionStatus::Ok
    );
    assert_eq!(
        projection.catalog_snapshot_json["provider"]["model_discovery_mode"],
        "hybrid"
    );

    let failed = PluginRepository::upsert_plugin_package_catalog_projection(
        &store,
        &UpsertPluginPackageCatalogProjectionInput {
            installation_id,
            package_code: "fixture_provider".into(),
            package_version: "0.1.0".into(),
            catalog_snapshot_json: projection.catalog_snapshot_json.clone(),
            projection_status: PluginPackageCatalogProjectionStatus::Failed,
            last_error_message: Some("package parse failed".into()),
            refreshed_at: projection.refreshed_at,
        },
    )
    .await
    .unwrap();
    let fetched = PluginRepository::get_plugin_package_catalog_projection(&store, installation_id)
        .await
        .unwrap()
        .expect("projection should be stored");
    let listed = PluginRepository::list_plugin_package_catalog_projections(&store)
        .await
        .unwrap();

    assert_eq!(
        failed.projection_status,
        PluginPackageCatalogProjectionStatus::Failed
    );
    assert_eq!(
        fetched.last_error_message.as_deref(),
        Some("package parse failed")
    );
    assert_eq!(listed.len(), 1);
}

#[tokio::test]
async fn plugin_repository_persists_installations_assignments_and_tasks() {
    let (store, workspace, actor) = seed_store().await;
    let installation_id = Uuid::now_v7();
    let task_id = Uuid::now_v7();

    let installation = PluginRepository::upsert_installation(
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
            desired_state: PluginDesiredState::PendingRestart,
            artifact_status: PluginArtifactStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status: PluginAvailabilityStatus::PendingRestart,
            package_path: Some("/tmp/plugin-packages/fixture_provider/0.1.0.1flowbasepkg".into()),
            installed_path: "/tmp/plugin-installed/fixture_provider/0.1.0".into(),
            checksum: Some("abc123".into()),
            manifest_fingerprint: Some("sha256:manifest".into()),
            signature_status: Some("unsigned".into()),
            signature_algorithm: None,
            signing_key_id: None,
            last_load_error: None,
            metadata_json: json!({ "help_url": "https://example.com/help" }),
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();

    assert_eq!(installation.id, installation_id);
    assert_eq!(
        installation.desired_state,
        PluginDesiredState::PendingRestart
    );
    assert_eq!(installation.artifact_status, PluginArtifactStatus::Ready);
    assert_eq!(installation.runtime_status, PluginRuntimeStatus::Inactive);
    assert_eq!(
        installation.availability_status,
        PluginAvailabilityStatus::PendingRestart
    );
    assert_eq!(
        installation.package_path.as_deref(),
        Some("/tmp/plugin-packages/fixture_provider/0.1.0.1flowbasepkg")
    );
    assert_eq!(
        installation.installed_path,
        "/tmp/plugin-installed/fixture_provider/0.1.0"
    );
    assert_eq!(
        installation.manifest_fingerprint.as_deref(),
        Some("sha256:manifest")
    );

    let assignment = PluginRepository::create_assignment(
        &store,
        &CreatePluginAssignmentInput {
            installation_id,
            workspace_id: workspace.id,
            provider_code: "fixture_provider".into(),
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();
    assert_eq!(assignment.installation_id, installation_id);

    let task = PluginRepository::create_task(
        &store,
        &CreatePluginTaskInput {
            task_id,
            installation_id: Some(installation_id),
            workspace_id: None,
            provider_code: "fixture_provider".into(),
            task_kind: PluginTaskKind::Install,
            status: PluginTaskStatus::Queued,
            status_message: Some("waiting".into()),
            detail_json: json!({ "step": "download" }),
            actor_user_id: Some(actor.id),
        },
    )
    .await
    .unwrap();
    assert_eq!(task.status, PluginTaskStatus::Queued);
    let install_scope: (String, Uuid) =
        sqlx::query_as("select scope_kind, scope_id from plugin_tasks where id = $1")
            .bind(task.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(
        install_scope,
        ("system".to_string(), domain::SYSTEM_SCOPE_ID)
    );

    let completed_task = PluginRepository::update_task_status(
        &store,
        &UpdatePluginTaskStatusInput {
            task_id,
            status: PluginTaskStatus::Succeeded,
            status_message: Some("done".into()),
            detail_json: json!({ "step": "enabled" }),
        },
    )
    .await
    .unwrap();

    assert_eq!(completed_task.status, PluginTaskStatus::Succeeded);
    assert!(completed_task.finished_at.is_some());

    let assign_task_id = Uuid::now_v7();
    let assign_task = PluginRepository::create_task(
        &store,
        &CreatePluginTaskInput {
            task_id: assign_task_id,
            installation_id: Some(installation_id),
            workspace_id: Some(workspace.id),
            provider_code: "fixture_provider".into(),
            task_kind: PluginTaskKind::Assign,
            status: PluginTaskStatus::Queued,
            status_message: None,
            detail_json: json!({ "workspace_id": workspace.id }),
            actor_user_id: Some(actor.id),
        },
    )
    .await
    .unwrap();
    let assign_scope: (String, Uuid) =
        sqlx::query_as("select scope_kind, scope_id from plugin_tasks where id = $1")
            .bind(assign_task.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(assign_scope, ("workspace".to_string(), workspace.id));

    let installations = PluginRepository::list_installations(&store).await.unwrap();
    assert_eq!(installations.len(), 1);
    let assignments = PluginRepository::list_assignments(&store, workspace.id)
        .await
        .unwrap();
    assert_eq!(assignments.len(), 1);
}

#[tokio::test]
async fn js_dependency_repository_replaces_entries_and_lists_assigned_workspace_catalog() {
    let (store, workspace, actor) = seed_store().await;
    let installation = PluginRepository::upsert_installation(
        &store,
        &UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            provider_code: "fixture_js_dependency_pack".into(),
            plugin_id: "fixture_js_dependency_pack@0.1.0".into(),
            plugin_version: "0.1.0".into(),
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
            installed_path: "/tmp/plugins/fixture_js_dependency_pack/0.1.0".into(),
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

    JsDependencyRepository::replace_installation_js_dependencies(
        &store,
        &ReplaceInstallationJsDependenciesInput {
            installation_id: installation.id,
            provider_code: installation.provider_code.clone(),
            plugin_id: installation.plugin_id.clone(),
            plugin_version: installation.plugin_version.clone(),
            entries: vec![JsDependencyRegistryInput {
                alias: "zod".into(),
                package: "zod".into(),
                version: "3.24.0".into(),
                target: "backend_code".into(),
                artifact_path: "artifacts/zod.backend.mjs".into(),
                integrity: "sha256-zod".into(),
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

    let hidden = JsDependencyRepository::list_workspace_js_dependencies(&store, workspace.id)
        .await
        .unwrap();
    assert!(hidden.is_empty());

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

    JsDependencyRepository::replace_installation_js_dependencies(
        &store,
        &ReplaceInstallationJsDependenciesInput {
            installation_id: installation.id,
            provider_code: installation.provider_code.clone(),
            plugin_id: installation.plugin_id.clone(),
            plugin_version: installation.plugin_version.clone(),
            entries: vec![JsDependencyRegistryInput {
                alias: "valibot".into(),
                package: "valibot".into(),
                version: "1.2.3".into(),
                target: "backend_code".into(),
                artifact_path: "artifacts/valibot.backend.mjs".into(),
                integrity: "sha256-valibot".into(),
                permissions: domain::JsDependencyPermissions {
                    network: "none".into(),
                    filesystem: "deny".into(),
                    env: "deny".into(),
                },
            }],
        },
    )
    .await
    .unwrap();

    let entries = JsDependencyRepository::list_workspace_js_dependencies(&store, workspace.id)
        .await
        .unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].alias, "valibot");
    assert_eq!(entries[0].package, "valibot");
    assert_eq!(entries[0].artifact_path, "artifacts/valibot.backend.mjs");
    assert_eq!(entries[0].permissions.network, "none");
}

#[tokio::test]
async fn plugin_repository_repoints_assignment_by_workspace_and_provider_code() {
    let (store, workspace, actor) = seed_store().await;
    let installation_v1 = PluginRepository::upsert_installation(
        &store,
        &UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            provider_code: "fixture_provider".into(),
            plugin_id: "fixture_provider@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            contract_version: "1flowbase.provider/v1".into(),
            protocol: "openai_compatible".into(),
            display_name: "Fixture Provider".into(),
            source_kind: "official_registry".into(),
            trust_level: "checksum_only".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::ActiveRequested,
            artifact_status: PluginArtifactStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status: PluginAvailabilityStatus::InstallIncomplete,
            package_path: None,
            installed_path: "/tmp/plugin-installed/fixture_provider/0.1.0".into(),
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
    let installation_v2 = PluginRepository::upsert_installation(
        &store,
        &UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            provider_code: "fixture_provider".into(),
            plugin_id: "fixture_provider@0.2.0".into(),
            plugin_version: "0.2.0".into(),
            contract_version: "1flowbase.provider/v1".into(),
            protocol: "openai_compatible".into(),
            display_name: "Fixture Provider".into(),
            source_kind: "official_registry".into(),
            trust_level: "checksum_only".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::ActiveRequested,
            artifact_status: PluginArtifactStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status: PluginAvailabilityStatus::InstallIncomplete,
            package_path: None,
            installed_path: "/tmp/plugin-installed/fixture_provider/0.2.0".into(),
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

    PluginRepository::create_assignment(
        &store,
        &CreatePluginAssignmentInput {
            installation_id: installation_v1.id,
            workspace_id: workspace.id,
            provider_code: "fixture_provider".into(),
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();
    PluginRepository::create_assignment(
        &store,
        &CreatePluginAssignmentInput {
            installation_id: installation_v2.id,
            workspace_id: workspace.id,
            provider_code: "fixture_provider".into(),
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();

    let assignments = PluginRepository::list_assignments(&store, workspace.id)
        .await
        .unwrap();
    assert_eq!(assignments.len(), 1);
    assert_eq!(assignments[0].provider_code, "fixture_provider");
    assert_eq!(assignments[0].installation_id, installation_v2.id);
}

#[tokio::test]
async fn plugin_repository_persists_trust_level_and_signature_metadata() {
    let (store, _workspace, actor) = seed_store().await;
    let installation = PluginRepository::upsert_installation(
        &store,
        &UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            provider_code: "openai_compatible".into(),
            plugin_id: "1flowbase.openai_compatible@0.2.0".into(),
            plugin_version: "0.2.0".into(),
            contract_version: "1flowbase.provider/v1".into(),
            protocol: "openai_compatible".into(),
            display_name: "OpenAI Compatible".into(),
            source_kind: "mirror_registry".into(),
            trust_level: "verified_official".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::ActiveRequested,
            artifact_status: PluginArtifactStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status: PluginAvailabilityStatus::InstallIncomplete,
            package_path: None,
            installed_path: "/tmp/plugin-installed/openai_compatible/0.2.0".into(),
            checksum: Some("sha256:abc123".into()),
            manifest_fingerprint: None,
            signature_status: Some("verified".into()),
            signature_algorithm: Some("ed25519".into()),
            signing_key_id: Some("official-key-2026-04".into()),
            last_load_error: None,
            metadata_json: json!({}),
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();

    assert_eq!(installation.trust_level, "verified_official");
    assert_eq!(installation.signature_status.as_deref(), Some("verified"));
    assert_eq!(installation.signature_algorithm.as_deref(), Some("ed25519"));
    assert_eq!(
        installation.signing_key_id.as_deref(),
        Some("official-key-2026-04")
    );
}

#[tokio::test]
async fn plugin_repository_maps_succeeded_task_status() {
    let (store, _, actor) = seed_store().await;

    let task = PluginRepository::create_task(
        &store,
        &CreatePluginTaskInput {
            task_id: Uuid::now_v7(),
            installation_id: None,
            workspace_id: None,
            provider_code: "fixture_provider".into(),
            task_kind: PluginTaskKind::Install,
            status: PluginTaskStatus::Succeeded,
            status_message: Some("installed".into()),
            detail_json: json!({}),
            actor_user_id: Some(actor.id),
        },
    )
    .await
    .unwrap();

    assert_eq!(task.status, PluginTaskStatus::Succeeded);
}

#[tokio::test]
async fn plugin_repository_lists_only_pending_restart_host_extensions() {
    let (store, _workspace, actor) = seed_store().await;

    PluginRepository::upsert_installation(
        &store,
        &UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            provider_code: "fixture_host_extension".into(),
            plugin_id: "fixture_host_extension@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            contract_version: "1flowbase.host_extension/v1".into(),
            protocol: "native_host".into(),
            display_name: "Fixture Host Extension".into(),
            source_kind: "uploaded".into(),
            trust_level: "checksum_only".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::PendingRestart,
            artifact_status: PluginArtifactStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status: PluginAvailabilityStatus::PendingRestart,
            package_path: None,
            installed_path: "/tmp/plugin-installed/fixture_host_extension/0.1.0".into(),
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
    PluginRepository::upsert_installation(
        &store,
        &UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            provider_code: "fixture_provider".into(),
            plugin_id: "fixture_provider@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            contract_version: "1flowbase.provider/v1".into(),
            protocol: "openai_compatible".into(),
            display_name: "Fixture Provider".into(),
            source_kind: "uploaded".into(),
            trust_level: "checksum_only".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::PendingRestart,
            artifact_status: PluginArtifactStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status: PluginAvailabilityStatus::PendingRestart,
            package_path: None,
            installed_path: "/tmp/plugin-installed/fixture_provider/0.1.0".into(),
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

    let pending = PluginRepository::list_pending_restart_host_extensions(&store)
        .await
        .unwrap();

    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].contract_version, "1flowbase.host_extension/v1");
}

fn installation_commit_input(
    installation_id: Uuid,
    actor_user_id: Uuid,
    title: &str,
    runtime: &str,
) -> control_plane::ports::CommitPluginInstallationProjectionInput {
    use control_plane::ports::{
        CommitPluginInstallationProjectionInput, FrontendBlockCatalogRegistryInput,
        ReplaceInstallationFrontendBlocksInput, ReplaceInstallationJsDependenciesInput,
        ReplaceInstallationNodeContributionsInput, UpsertPluginArtifactInstanceInput,
    };

    CommitPluginInstallationProjectionInput {
        installation: UpsertPluginInstallationInput {
            installation_id,
            provider_code: "fixture_frontend_blocks".into(),
            plugin_id: "fixture_frontend_blocks@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            contract_version: "1flowbase.capability/v1".into(),
            protocol: "stdio_json".into(),
            display_name: title.into(),
            source_kind: "uploaded".into(),
            trust_level: "checksum_only".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::Disabled,
            artifact_status: PluginArtifactStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status: PluginAvailabilityStatus::Disabled,
            package_path: None,
            installed_path: "/tmp/fixture_frontend_blocks/0.1.0".into(),
            checksum: None,
            manifest_fingerprint: Some("sha256:fixture".into()),
            signature_status: None,
            signature_algorithm: None,
            signing_key_id: None,
            last_load_error: None,
            metadata_json: json!({"block_contributions": ["hero_banner"]}),
            actor_user_id,
        },
        artifact_instance: UpsertPluginArtifactInstanceInput {
            node_id: "test-node".into(),
            installation_id,
            local_version: Some("0.1.0".into()),
            local_checksum: None,
            installed_path: Some("/tmp/fixture_frontend_blocks/0.1.0".into()),
            artifact_status: domain::PluginArtifactInstanceStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            checked_at: time::OffsetDateTime::now_utc(),
            last_error: None,
        },
        package_catalog: None,
        node_contributions: ReplaceInstallationNodeContributionsInput {
            installation_id,
            provider_code: "fixture_frontend_blocks".into(),
            plugin_id: "fixture_frontend_blocks@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            entries: Vec::new(),
        },
        js_dependencies: ReplaceInstallationJsDependenciesInput {
            installation_id,
            provider_code: "fixture_frontend_blocks".into(),
            plugin_id: "fixture_frontend_blocks@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            entries: Vec::new(),
        },
        frontend_blocks: ReplaceInstallationFrontendBlocksInput {
            installation_id,
            provider_code: "fixture_frontend_blocks".into(),
            plugin_id: "fixture_frontend_blocks@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            entries: vec![FrontendBlockCatalogRegistryInput {
                contribution_code: "hero_banner".into(),
                title: title.into(),
                runtime: runtime.into(),
                entry: "blocks/hero/index.html".into(),
                code_template: None,
                code_template_version: None,
                code_template_language: None,
                code_modules: Vec::new(),
                context_contract: domain::FrontendBlockContextContract {
                    primitives: vec!["text".into()],
                    input_schema: json!({"type": "object"}),
                },
                permissions: domain::FrontendBlockPermissions {
                    network: "none".into(),
                    storage: "none".into(),
                    secrets: "none".into(),
                },
                ui_capabilities: Vec::new(),
            }],
        },
    }
}

#[tokio::test]
async fn plugin_installation_commit_rolls_back_new_installation_when_frontend_catalog_violates_constraint(
) {
    let (store, _workspace, actor) = seed_store().await;
    let installation_id = Uuid::now_v7();
    let error = PluginRepository::commit_plugin_installation_projection(
        &store,
        &installation_commit_input(installation_id, actor.id, "Invalid", "invalid"),
    )
    .await
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("frontend_block_catalog_runtime_check"));
    assert!(PluginRepository::get_installation(&store, installation_id)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn plugin_installation_commit_preserves_previous_installation_and_catalog_on_reinstall_failure(
) {
    let (store, _workspace, actor) = seed_store().await;
    let installation_id = Uuid::now_v7();
    PluginRepository::commit_plugin_installation_projection(
        &store,
        &installation_commit_input(installation_id, actor.id, "Original", "native_react"),
    )
    .await
    .unwrap();

    PluginRepository::commit_plugin_installation_projection(
        &store,
        &installation_commit_input(installation_id, actor.id, "Changed", "invalid"),
    )
    .await
    .unwrap_err();

    let installation = PluginRepository::get_installation(&store, installation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(installation.display_name, "Original");
    let catalog: (String, String) = sqlx::query_as(
        "select title, runtime from frontend_block_catalog where installation_id = $1",
    )
    .bind(installation_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(catalog, ("Original".into(), "native_react".into()));
}
