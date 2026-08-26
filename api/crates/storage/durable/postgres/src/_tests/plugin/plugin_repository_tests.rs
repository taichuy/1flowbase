use control_plane::ports::{
    CreateNetworkEgressProviderInput, CreatePluginAssignmentInput, CreatePluginTaskInput,
    JsDependencyRegistryInput, JsDependencyRepository, NetworkEgressRepository,
    NetworkEgressRuntimePort, NetworkEgressSecretMaterial, NetworkEgressSecretResolver,
    PluginRepository, ReplaceInstallationJsDependenciesInput, UpdatePluginTaskStatusInput,
    UpsertPluginInstallationInput, UpsertPluginPackageCatalogProjectionInput,
};
use domain::{
    PluginDesiredState, PluginPackageCatalogProjectionStatus, PluginRuntimeStatus, PluginTaskKind,
    PluginTaskStatus, PluginVerificationStatus,
};
use serde_json::json;
use sqlx::Row;
use storage_durable_postgres::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

const REPAIR_NETWORK_EGRESS_CURRENT_ARTIFACTS_SQL: &str =
    include_str!("../../../migrations/20260823180000_repair_network_egress_current_artifacts.sql");

struct RejectNetworkEgressPreflight;

#[async_trait::async_trait]
impl NetworkEgressRuntimePort for RejectNetworkEgressPreflight {
    async fn unload_network_egress_provider(&self, _provider_id: Uuid) -> anyhow::Result<()> {
        Ok(())
    }

    async fn preflight_network_egresses(
        &self,
        _provider_id: Uuid,
        _installation: &domain::LocalPluginInstallationRecord,
        _secret: NetworkEgressSecretMaterial,
    ) -> anyhow::Result<()> {
        anyhow::bail!("controlled preflight rejection")
    }

    async fn sync_network_egresses(
        &self,
        _provider_id: Uuid,
        _installation: &domain::LocalPluginInstallationRecord,
        _secret: NetworkEgressSecretMaterial,
    ) -> anyhow::Result<Vec<plugin_framework::EgressDescriptor>> {
        unreachable!("activation preflight must not synchronize the active runtime")
    }
}

struct StaticNetworkEgressSecret;

#[async_trait::async_trait]
impl NetworkEgressSecretResolver for StaticNetworkEgressSecret {
    async fn resolve_for_runner(
        &self,
        provider: &domain::NetworkEgressProviderRecord,
    ) -> anyhow::Result<Option<NetworkEgressSecretMaterial>> {
        Ok(Some(NetworkEgressSecretMaterial {
            secret_ref: provider.secret_ref.clone(),
            secret_json: json!({"subscription_url": "https://fixture.invalid/subscription"}),
        }))
    }
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
            expected_checksum: None,
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
            desired_state: PluginDesiredState::PendingRestart,
            expected_checksum: Some("abc123".into()),
            signature_status: domain::ExtensionSignatureStatus::Missing,
            signature_algorithm: None,
            signing_key_id: None,
            metadata_json: json!({ "help_url": "https://example.com/help" }),
            is_system_reserved: false,
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
    assert_eq!(installation.expected_checksum.as_deref(), Some("abc123"));

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
async fn publisher_cutover_repository_projects_legacy_manifest_compatibility_receipt() {
    let (store, _workspace, actor) = seed_store().await;
    let installation_id = Uuid::now_v7();
    PluginRepository::upsert_installation(
        &store,
        &UpsertPluginInstallationInput {
            installation_id,
            category: domain::ExtensionCategory::RuntimeExtensions,
            organization: "1flowbase".into(),
            provider_code: "publisher_cutover".into(),
            plugin_id: "publisher_cutover@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            contract_version: "1flowbase.provider/v1".into(),
            protocol: "stdio_json".into(),
            display_name: "Publisher Cutover".into(),
            source_kind: "official_registry".into(),
            trust_level: "verified_official".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::ActiveRequested,
            expected_checksum: None,
            signature_status: domain::ExtensionSignatureStatus::Verified,
            signature_algorithm: None,
            signing_key_id: None,
            metadata_json: json!({}),
            is_system_reserved: false,
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();
    sqlx::query(
        "update extension_installations set receipt = jsonb_build_object('legacy_manifest_compatibility', 'missing_publisher_namespace_v1') where id = $1",
    )
    .bind(installation_id)
    .execute(store.pool())
    .await
    .unwrap();

    let installation = PluginRepository::get_installation(&store, installation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        installation.legacy_manifest_compatibility.as_deref(),
        Some("missing_publisher_namespace_v1")
    );
}

#[tokio::test]
async fn js_dependency_repository_replaces_entries_and_lists_assigned_workspace_catalog() {
    let (store, workspace, actor) = seed_store().await;
    let installation = PluginRepository::upsert_installation(
        &store,
        &UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            category: domain::ExtensionCategory::RuntimeExtensions,
            organization: "test".to_string(),
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
            expected_checksum: None,
            signature_status: domain::ExtensionSignatureStatus::Verified,
            signature_algorithm: None,
            signing_key_id: None,
            metadata_json: json!({}),
            is_system_reserved: false,
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
            category: domain::ExtensionCategory::RuntimeExtensions,
            organization: "test".to_string(),
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
            expected_checksum: None,
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
    let installation_v2 = PluginRepository::upsert_installation(
        &store,
        &UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            category: domain::ExtensionCategory::RuntimeExtensions,
            organization: "test".to_string(),
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
            expected_checksum: None,
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
            category: domain::ExtensionCategory::RuntimeExtensions,
            organization: "test".to_string(),
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
            expected_checksum: Some("sha256:abc123".into()),
            signature_status: domain::ExtensionSignatureStatus::Verified,
            signature_algorithm: Some("ed25519".into()),
            signing_key_id: Some("official-key-2026-04".into()),
            metadata_json: json!({}),
            is_system_reserved: false,
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();

    assert_eq!(installation.trust_level, "verified_official");
    assert_eq!(
        installation.signature_status,
        domain::ExtensionSignatureStatus::Verified
    );
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
            category: domain::ExtensionCategory::HostExtensions,
            organization: "test".to_string(),
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
            expected_checksum: None,
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
    PluginRepository::upsert_installation(
        &store,
        &UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            category: domain::ExtensionCategory::RuntimeExtensions,
            organization: "test".to_string(),
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
            expected_checksum: None,
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
) -> control_plane::ports::CommitPluginInstallationInput {
    use control_plane::ports::{
        CommitPluginInstallationInput, FrontendBlockCatalogRegistryInput,
        ReplaceInstallationFrontendBlocksInput, ReplaceInstallationJsDependenciesInput,
        ReplaceInstallationNodeContributionsInput, UpsertPluginArtifactInstanceInput,
    };

    CommitPluginInstallationInput {
        installation: UpsertPluginInstallationInput {
            installation_id,
            category: domain::ExtensionCategory::CapabilityPlugins,
            organization: "test".to_string(),
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
            expected_checksum: None,
            signature_status: domain::ExtensionSignatureStatus::Missing,
            signature_algorithm: None,
            signing_key_id: None,
            metadata_json: json!({"block_contributions": ["hero_banner"]}),
            is_system_reserved: false,
            actor_user_id,
        },
        artifact_instance: UpsertPluginArtifactInstanceInput {
            node_id: "test-node".into(),
            installation_id,
            local_version: Some("0.1.0".into()),
            local_checksum: None,
            local_path: Some("/tmp/fixture_frontend_blocks/0.1.0".into()),
            package_path: None,
            manifest_fingerprint: None,
            artifact_status: domain::PluginArtifactInstanceStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status: domain::PluginAvailabilityStatus::Disabled,
            checked_at: time::OffsetDateTime::now_utc(),
            last_error: None,
            is_current: true,
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
        retained_frontend_module_assets: Vec::new(),
    }
}

fn network_egress_commit_input(
    installation_id: Uuid,
    actor_user_id: Uuid,
    version: &str,
    runtime: &str,
) -> control_plane::ports::CommitPluginInstallationInput {
    let mut input =
        installation_commit_input(installation_id, actor_user_id, "Clash Proxy", runtime);
    let plugin_id = format!("clash-proxy@{version}");
    input.installation.category = domain::ExtensionCategory::RuntimeExtensions;
    input.installation.organization = "taichuy".into();
    input.installation.provider_code = "clash-proxy".into();
    input.installation.plugin_id = plugin_id.clone();
    input.installation.plugin_version = version.into();
    input.installation.contract_version = plugin_framework::NETWORK_EGRESS_PROVIDER_CONTRACT.into();
    input.installation.metadata_json = json!({ "plugin_type": "network_egress_provider" });
    input.artifact_instance.local_version = Some(version.into());
    input.artifact_instance.local_path = Some(format!("/tmp/clash-proxy/{version}"));
    input.artifact_instance.is_current = true;
    input.node_contributions.provider_code = "clash-proxy".into();
    input.node_contributions.plugin_id = plugin_id.clone();
    input.node_contributions.plugin_version = version.into();
    input.js_dependencies.provider_code = "clash-proxy".into();
    input.js_dependencies.plugin_id = plugin_id.clone();
    input.js_dependencies.plugin_version = version.into();
    input.frontend_blocks.provider_code = "clash-proxy".into();
    input.frontend_blocks.plugin_id = plugin_id;
    input.frontend_blocks.plugin_version = version.into();
    input
}

fn network_egress_commit_input_for_organization(
    installation_id: Uuid,
    actor_user_id: Uuid,
    organization: &str,
    version: &str,
) -> control_plane::ports::CommitPluginInstallationInput {
    let mut input =
        network_egress_commit_input(installation_id, actor_user_id, version, "native_react");
    let plugin_id = format!("{organization}-clash-proxy@{version}");
    input.installation.organization = organization.to_string();
    input.installation.plugin_id = plugin_id.clone();
    input.node_contributions.plugin_id = plugin_id.clone();
    input.js_dependencies.plugin_id = plugin_id.clone();
    input.frontend_blocks.plugin_id = plugin_id;
    input
}

#[tokio::test]
async fn network_egress_installation_commit_keeps_one_current_artifact_and_rolls_back_failed_update(
) {
    let (store, _workspace, actor) = seed_store().await;
    let retained_id = Uuid::now_v7();
    let current_id = Uuid::now_v7();
    PluginRepository::commit_plugin_installation(
        &store,
        &network_egress_commit_input(retained_id, actor.id, "0.2.2", "native_react"),
    )
    .await
    .unwrap();

    PluginRepository::commit_plugin_installation(
        &store,
        &network_egress_commit_input(current_id, actor.id, "0.2.3", "invalid"),
    )
    .await
    .unwrap_err();
    assert!(
        PluginRepository::get_artifact_instance(&store, "test-node", retained_id)
            .await
            .unwrap()
            .expect("retained artifact must remain")
            .is_current
    );
    assert!(PluginRepository::get_installation(&store, current_id)
        .await
        .unwrap()
        .is_none());

    PluginRepository::commit_plugin_installation(
        &store,
        &network_egress_commit_input(current_id, actor.id, "0.2.3", "native_react"),
    )
    .await
    .unwrap();

    assert!(
        !PluginRepository::get_artifact_instance(&store, "test-node", retained_id)
            .await
            .unwrap()
            .expect("retained artifact must remain")
            .is_current
    );
    assert!(
        PluginRepository::get_artifact_instance(&store, "test-node", current_id)
            .await
            .unwrap()
            .expect("new artifact must be current")
            .is_current
    );
}

#[tokio::test]
async fn network_egress_current_selection_switches_one_provider_family_atomically() {
    let (store, _workspace, actor) = seed_store().await;
    let retained_id = Uuid::now_v7();
    let current_id = Uuid::now_v7();
    let other_publisher_id = Uuid::now_v7();
    PluginRepository::commit_plugin_installation(
        &store,
        &network_egress_commit_input(retained_id, actor.id, "0.2.2", "native_react"),
    )
    .await
    .unwrap();
    PluginRepository::commit_plugin_installation(
        &store,
        &network_egress_commit_input(current_id, actor.id, "0.2.3", "native_react"),
    )
    .await
    .unwrap();
    PluginRepository::commit_plugin_installation(
        &store,
        &network_egress_commit_input_for_organization(
            other_publisher_id,
            actor.id,
            "other-publisher",
            "0.3.0",
        ),
    )
    .await
    .unwrap();

    // AC-NCP03: selecting a retained version changes only the family current projection.
    let selected =
        PluginRepository::select_network_egress_current(&store, "test-node", retained_id)
            .await
            .unwrap()
            .expect("ready retained version must be selectable");

    assert_eq!(selected.installation_id, retained_id);
    assert!(selected.is_current);
    assert!(
        PluginRepository::get_artifact_instance(&store, "test-node", retained_id)
            .await
            .unwrap()
            .expect("selected artifact must remain")
            .is_current
    );
    assert!(
        !PluginRepository::get_artifact_instance(&store, "test-node", current_id)
            .await
            .unwrap()
            .expect("previous current artifact must remain")
            .is_current
    );
    assert!(
        PluginRepository::get_artifact_instance(&store, "test-node", other_publisher_id)
            .await
            .unwrap()
            .expect("another publisher's same-named artifact must remain")
            .is_current,
        "current selection must be scoped by category, organization, and artifact_id"
    );
}

#[tokio::test]
async fn ac_001_network_egress_provider_persists_stable_family_identity_across_version_switches() {
    let (store, _workspace, actor) = seed_store().await;
    let old_installation_id = Uuid::now_v7();
    let current_installation_id = Uuid::now_v7();
    PluginRepository::commit_plugin_installation(
        &store,
        &network_egress_commit_input(old_installation_id, actor.id, "0.2.5", "native_react"),
    )
    .await
    .unwrap();

    let provider_id = Uuid::now_v7();
    NetworkEgressRepository::create_network_egress_provider(
        &store,
        &CreateNetworkEgressProviderInput {
            provider_id,
            extension_family: domain::ExtensionCatalogIdentity::new(
                domain::ExtensionCategory::RuntimeExtensions,
                "taichuy",
                "clash-proxy",
            ),
            provider_code: "clash-proxy".into(),
            display_name: "Clash subscription".into(),
            description: String::new(),
            secret_ref: format!("secret://system/network-egress/{provider_id}"),
            lifecycle: domain::NetworkEgressProviderLifecycle::Active,
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();

    PluginRepository::commit_plugin_installation(
        &store,
        &network_egress_commit_input(current_installation_id, actor.id, "0.2.8", "native_react"),
    )
    .await
    .unwrap();

    let row = sqlx::query(
        "select extension_category, extension_organization, extension_artifact_id \
         from network_egress_providers where id = $1",
    )
    .bind(provider_id)
    .fetch_one(store.pool())
    .await
    .expect("provider must persist a version-independent extension family");

    assert_eq!(
        row.get::<String, _>("extension_category"),
        "runtime-extensions"
    );
    assert_eq!(row.get::<String, _>("extension_organization"), "taichuy");
    assert_eq!(row.get::<String, _>("extension_artifact_id"), "clash-proxy");

    let provider = NetworkEgressRepository::get_network_egress_provider(&store, provider_id)
        .await
        .unwrap()
        .unwrap();
    let resolved = PluginRepository::get_current_local_installation(
        &store,
        "test-node",
        provider.extension_family.as_ref().unwrap(),
    )
    .await
    .unwrap()
    .expect("provider family must resolve the current artifact on the executing node");
    assert_eq!(resolved.id, current_installation_id);
}

#[tokio::test]
async fn ac_005_failed_provider_preflight_preserves_the_previous_current_artifact() {
    let (store, _workspace, actor) = seed_store().await;
    let old_installation_id = Uuid::now_v7();
    let target_installation_id = Uuid::now_v7();
    PluginRepository::commit_plugin_installation(
        &store,
        &network_egress_commit_input(old_installation_id, actor.id, "0.2.5", "native_react"),
    )
    .await
    .unwrap();
    PluginRepository::commit_plugin_installation(
        &store,
        &network_egress_commit_input(target_installation_id, actor.id, "0.2.8", "native_react"),
    )
    .await
    .unwrap();
    PluginRepository::select_network_egress_current(&store, "test-node", old_installation_id)
        .await
        .unwrap()
        .unwrap();
    let provider_id = Uuid::now_v7();
    NetworkEgressRepository::create_network_egress_provider(
        &store,
        &CreateNetworkEgressProviderInput {
            provider_id,
            extension_family: domain::ExtensionCatalogIdentity::new(
                domain::ExtensionCategory::RuntimeExtensions,
                "taichuy",
                "clash-proxy",
            ),
            provider_code: "clash-proxy".into(),
            display_name: "Clash subscription".into(),
            description: String::new(),
            secret_ref: format!("secret://system/network-egress/{provider_id}"),
            lifecycle: domain::NetworkEgressProviderLifecycle::Active,
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();

    let service = control_plane::network_egress::NetworkEgressProviderService::new(
        store.clone(),
        RejectNetworkEgressPreflight,
        StaticNetworkEgressSecret,
        "unused-test-key".into(),
        "test-node".into(),
    );
    let error = service
        .activate_version(target_installation_id)
        .await
        .expect_err("a rejected provider config must abort activation");
    assert!(error
        .to_string()
        .contains("network_egress_plugin_version_preflight_failed"));
    assert!(
        PluginRepository::get_artifact_instance(&store, "test-node", old_installation_id)
            .await
            .unwrap()
            .unwrap()
            .is_current
    );
    assert!(
        !PluginRepository::get_artifact_instance(&store, "test-node", target_installation_id)
            .await
            .unwrap()
            .unwrap()
            .is_current
    );
}

#[tokio::test]
async fn network_egress_current_repair_selects_latest_ready_version_from_legacy_artifacts() {
    let (store, _workspace, actor) = seed_store().await;
    let retained_id = Uuid::now_v7();
    let current_id = Uuid::now_v7();
    PluginRepository::commit_plugin_installation(
        &store,
        &network_egress_commit_input(retained_id, actor.id, "0.2.2", "native_react"),
    )
    .await
    .unwrap();
    PluginRepository::commit_plugin_installation(
        &store,
        &network_egress_commit_input(current_id, actor.id, "0.2.3", "native_react"),
    )
    .await
    .unwrap();
    sqlx::query(
        "update extension_artifact_instances set is_current = false where installation_id in ($1, $2)",
    )
    .bind(retained_id)
    .bind(current_id)
    .execute(store.pool())
    .await
    .unwrap();

    sqlx::raw_sql(REPAIR_NETWORK_EGRESS_CURRENT_ARTIFACTS_SQL)
        .execute(store.pool())
        .await
        .unwrap();

    assert!(
        !PluginRepository::get_artifact_instance(&store, "test-node", retained_id)
            .await
            .unwrap()
            .expect("retained artifact must remain")
            .is_current
    );
    assert!(
        PluginRepository::get_artifact_instance(&store, "test-node", current_id)
            .await
            .unwrap()
            .expect("latest artifact must become current")
            .is_current
    );
}

#[tokio::test]
async fn plugin_installation_commit_rolls_back_new_installation_when_frontend_catalog_violates_constraint(
) {
    let (store, _workspace, actor) = seed_store().await;
    let installation_id = Uuid::now_v7();
    let error = PluginRepository::commit_plugin_installation(
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
    PluginRepository::commit_plugin_installation(
        &store,
        &installation_commit_input(installation_id, actor.id, "Original", "native_react"),
    )
    .await
    .unwrap();

    PluginRepository::commit_plugin_installation(
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
