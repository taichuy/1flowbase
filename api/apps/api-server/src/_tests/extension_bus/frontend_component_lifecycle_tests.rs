//! Workspace service boundary with real installation/assignment repositories and the Boot graph.
use super::*;
use control_plane::{
    frontend_block_catalog::{FrontendModuleAssetService, GetFrontendModuleAssetQuery},
    ui_management::{ListUiComponentRecordsQuery, UiManagementService},
};
use control_plane_contracts::ports::*;
use storage_durable_postgres::{run_migrations, PgControlPlaneStore};

async fn store() -> (PgControlPlaneStore, Uuid) {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into());
    let schema = postgres_test_support::PostgresTestSchema::create(&url)
        .await
        .unwrap();
    let pool = schema.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "frontend-lifecycle")
        .await
        .unwrap();
    (store, workspace.id)
}

fn install(candidate: &FrontendContributionCandidate) -> CommitPluginInstallationInput {
    let installation = &candidate.installation;
    let block = &candidate.catalog_entry;
    let artifact = &candidate.artifact;
    CommitPluginInstallationInput {
        settings_templates: Vec::new(),
        installation: UpsertPluginInstallationInput {
            installation_id: installation.id,
            category: domain::ExtensionCategory::RuntimeExtensions,
            organization: installation.organization.clone(),
            provider_code: installation.provider_code.clone(),
            plugin_id: installation.plugin_id.clone(),
            plugin_version: installation.plugin_version.clone(),
            contract_version: "1flowbase.extension-bus/v1".into(),
            protocol: "stdio_json".into(),
            display_name: installation.display_name.clone(),
            source_kind: installation.source_kind.clone(),
            trust_level: installation.trust_level.clone(),
            verification_status: installation.verification_status,
            desired_state: installation.desired_state,
            expected_checksum: None,
            signature_status: installation.signature_status,
            signature_algorithm: None,
            signing_key_id: None,
            metadata_json: serde_json::json!({}),
            is_system_reserved: false,
            actor_user_id: installation.created_by,
        },
        artifact_instance: UpsertPluginArtifactInstanceInput {
            node_id: artifact.node_id.clone(),
            installation_id: installation.id,
            local_version: artifact.local_version.clone(),
            local_checksum: artifact.local_checksum.clone(),
            local_path: artifact.local_path.clone(),
            package_path: None,
            manifest_fingerprint: artifact.manifest_fingerprint.clone(),
            artifact_status: artifact.artifact_status,
            runtime_status: artifact.runtime_status,
            availability_status: artifact.availability_status,
            checked_at: artifact.checked_at,
            last_error: None,
            is_current: true,
        },
        package_catalog: None,
        node_contributions: ReplaceInstallationNodeContributionsInput {
            installation_id: installation.id,
            provider_code: installation.provider_code.clone(),
            plugin_id: installation.plugin_id.clone(),
            plugin_version: installation.plugin_version.clone(),
            entries: Vec::new(),
        },
        js_dependencies: ReplaceInstallationJsDependenciesInput {
            installation_id: installation.id,
            provider_code: installation.provider_code.clone(),
            plugin_id: installation.plugin_id.clone(),
            plugin_version: installation.plugin_version.clone(),
            entries: Vec::new(),
        },
        frontend_blocks: ReplaceInstallationFrontendBlocksInput {
            installation_id: installation.id,
            provider_code: installation.provider_code.clone(),
            plugin_id: installation.plugin_id.clone(),
            plugin_version: installation.plugin_version.clone(),
            entries: vec![FrontendBlockCatalogRegistryInput {
                contribution_code: block.contribution_code.clone(),
                title: block.title.clone(),
                runtime: block.runtime.clone(),
                entry: block.entry.clone(),
                code_template: Some("export default function Block() { return <Hero />; }".into()),
                code_template_version: Some("1.0.0".into()),
                code_template_language: Some("tsx".into()),
                code_modules: block.code_modules.clone(),
                context_contract: block.context_contract.clone(),
                permissions: block.permissions.clone(),
                ui_capabilities: block.ui_capabilities.clone(),
            }],
        },
        retained_frontend_module_assets: vec![RetainedFrontendModuleAssetInput {
            module_source: block.code_modules[0].source.clone(),
            sha256: block.code_modules[0].assets[0].sha256.clone(),
            media_type: block.code_modules[0].assets[0].media_type.clone(),
            bytes: b"export const block = true;\n".to_vec(),
        }],
    }
}

fn page() -> ListUiComponentRecordsQuery {
    ListUiComponentRecordsQuery {
        plugin_contribution_ids: None,
        query: None,
        offset: 0,
        limit: 20,
    }
}

#[tokio::test]
async fn workspace_assets_and_reusable_records_fail_closed_on_assignment_disable_grant_and_sha() {
    let (store, workspace) = store().await;
    let assembly = assembly();
    let graph = Arc::new(assembly.compile_graph().unwrap());
    let resolver = FrontendContributionResolver::compile(Arc::clone(&graph)).unwrap();
    let (candidate, root) = candidate(workspace);
    let input = install(&candidate);
    let installed = store.commit_plugin_installation(&input).await.unwrap();
    let digest = candidate.catalog_entry.code_modules[0].assets[0]
        .sha256
        .clone();
    let service = UiManagementService::new(store.clone(), "test-node");
    let assets =
        FrontendModuleAssetService::new(store.clone(), "test-node", Arc::clone(&graph)).unwrap();
    let asset_query = || GetFrontendModuleAssetQuery {
        workspace_id: workspace,
        sha256: digest.clone(),
    };
    let record = store.list_ui_component_records().await.unwrap().remove(0);
    // Installed and retained bytes are not themselves a workspace grant.
    assert!(assets
        .get_module_asset(asset_query())
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        service
            .list_workspace_component_records_page(page(), workspace, &resolver)
            .await
            .unwrap()
            .total,
        0
    );
    assert!(service
        .get_workspace_component_record(record.id, workspace, &resolver)
        .await
        .is_err());
    store
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: installed.id,
            workspace_id: workspace,
            provider_code: installed.provider_code.clone(),
            actor_user_id: installed.created_by,
        })
        .await
        .unwrap();
    assert_eq!(
        assets
            .get_module_asset(asset_query())
            .await
            .unwrap()
            .unwrap()
            .bytes,
        b"export const block = true;\n"
    );
    assert_eq!(
        service
            .list_workspace_component_records_page(page(), workspace, &resolver)
            .await
            .unwrap()
            .total,
        1
    );
    assert_eq!(
        service
            .get_workspace_component_record(record.id, workspace, &resolver)
            .await
            .unwrap()
            .id,
        record.id
    );
    assert!(assets
        .get_module_asset(GetFrontendModuleAssetQuery {
            workspace_id: Uuid::now_v7(),
            sha256: digest.clone()
        })
        .await
        .unwrap()
        .is_none());

    let mut modules = assembly.module_descriptors().to_vec();
    let point = modules
        .iter_mut()
        .flat_map(|module| module.extension_points.iter_mut())
        .find(|point| point.point_id.as_str() == FRONTEND_BLOCK_CONTRIBUTION_POINT_ID)
        .unwrap();
    point.allowed_permissions =
        BTreeSet::from([PermissionCode::new(FRONTEND_BLOCK_ISOLATED_UI_MOUNT_PERMISSION).unwrap()]);
    let denied_graph =
        Arc::new(plugin_framework::extension_bus::compile_extension_graph(modules).unwrap());
    let denied_resolver = FrontendContributionResolver::compile(Arc::clone(&denied_graph)).unwrap();
    let denied = FrontendModuleAssetService::new(store.clone(), "test-node", denied_graph).unwrap();
    assert!(denied
        .get_module_asset(asset_query())
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        service
            .list_workspace_component_records_page(page(), workspace, &denied_resolver)
            .await
            .unwrap()
            .total,
        0
    );

    store
        .update_desired_state(&UpdatePluginDesiredStateInput {
            installation_id: installed.id,
            actor_user_id: installed.created_by,
            desired_state: domain::PluginDesiredState::Disabled,
        })
        .await
        .unwrap();
    assert!(assets
        .get_module_asset(asset_query())
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        service
            .list_workspace_component_records_page(page(), workspace, &resolver)
            .await
            .unwrap()
            .total,
        0
    );
    assert!(service
        .get_workspace_component_record(record.id, workspace, &resolver)
        .await
        .is_err());
    store
        .update_desired_state(&UpdatePluginDesiredStateInput {
            installation_id: installed.id,
            actor_user_id: installed.created_by,
            desired_state: domain::PluginDesiredState::ActiveRequested,
        })
        .await
        .unwrap();
    sqlx::query("update retained_frontend_module_assets set bytes=$1 where installation_id=$2")
        .bind(b"tampered retained bytes".as_slice())
        .bind(installed.id)
        .execute(store.pool())
        .await
        .unwrap();
    assert!(assets.get_module_asset(asset_query()).await.is_err());
    std::fs::write(root.join("assets/block.js"), "tampered package bytes").unwrap();
    assert!(assets
        .get_module_asset(asset_query())
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        service
            .list_workspace_component_records_page(page(), workspace, &resolver)
            .await
            .unwrap()
            .total,
        0
    );
    // Restore verified bytes, then remove contributions. Retained assets cannot resurrect them.
    std::fs::write(
        root.join("assets/block.js"),
        b"export const block = true;\n",
    )
    .unwrap();
    sqlx::query("update retained_frontend_module_assets set bytes=$1 where installation_id=$2")
        .bind(b"export const block = true;\n".as_slice())
        .bind(installed.id)
        .execute(store.pool())
        .await
        .unwrap();
    let mut removed = input.frontend_blocks.clone();
    removed.installation_id = installed.id;
    removed.entries.clear();
    store
        .replace_installation_frontend_blocks(&removed)
        .await
        .unwrap();
    assert!(assets
        .get_module_asset(asset_query())
        .await
        .unwrap()
        .is_none());
    assert!(store.list_ui_component_records().await.unwrap().is_empty());
    // Reinstall keeps identity, then family uninstall revokes both catalog and reusable source.
    store.commit_plugin_installation(&input).await.unwrap();
    assert!(assets
        .get_module_asset(asset_query())
        .await
        .unwrap()
        .is_some());
    let mut artifact = input.artifact_instance.clone();
    artifact.installation_id = installed.id;
    artifact.artifact_status = domain::PluginArtifactInstanceStatus::Missing;
    artifact.availability_status = domain::PluginAvailabilityStatus::ArtifactMissing;
    artifact.local_path = None;
    store
        .commit_plugin_family_uninstall(&CommitPluginFamilyUninstallInput {
            artifact_instances: vec![artifact],
            artifact_cleanups: Vec::new(),
            audit_log: domain::AuditLogRecord {
                id: Uuid::now_v7(),
                workspace_id: None,
                actor_user_id: Some(installed.created_by),
                target_type: "plugin_family".into(),
                target_id: None,
                event_code: "plugin.family_uninstalled".into(),
                payload: serde_json::json!({}),
                created_at: OffsetDateTime::now_utc(),
            },
        })
        .await
        .unwrap();
    assert!(assets
        .get_module_asset(asset_query())
        .await
        .unwrap()
        .is_none());
    assert!(service
        .get_workspace_component_record(record.id, workspace, &resolver)
        .await
        .is_err());
    assert_eq!(
        service
            .list_workspace_component_records_page(page(), workspace, &resolver)
            .await
            .unwrap()
            .total,
        0
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_fetched_modules_are_verified_and_catalog_wire_preserves_settings_module_shape() {
    let graph = Arc::new(assembly().compile_graph().unwrap());
    let resolver = FrontendContributionResolver::compile(graph).unwrap();
    let (candidate, root) = candidate(Uuid::now_v7());
    let modules = candidate.catalog_entry.code_modules.clone();
    let binding = match resolver.resolve(candidate) {
        FrontendContributionResolution::Active(binding) => *binding,
        FrontendContributionResolution::Disabled(receipt) => {
            panic!("valid fixture disabled: {:?}", receipt.reason)
        }
    };
    assert_eq!(binding.assets.len(), 1);
    assert_eq!(binding.assets[0].integrity.as_str(), "verified_sha256");
    assert_eq!(
        binding.assets[0].url,
        format!(
            "/api/console/frontstage/component-module-assets/{}",
            modules[0].assets[0].sha256
        )
    );
    let response = crate::routes::frontend_block_catalog::to_response(binding).unwrap();
    let json = serde_json::to_value(response).unwrap();
    assert_eq!(
        json["code_modules"],
        serde_json::to_value(&modules).unwrap()
    );
    assert_eq!(json["code_modules"][0]["binding"], "fetched");
    assert!(json["code_modules"][0]["assets"][0].get("url").is_none());
    assert!(json["code_modules"][0]["assets"][0]
        .get("integrity")
        .is_none());
    std::fs::remove_dir_all(root).unwrap();
}
