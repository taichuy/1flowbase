use super::*;

#[tokio::test]
async fn plugin_management_service_syncs_manifest_node_contributions_on_install() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let runtime = MemoryProviderRuntime::default();
    let nonce = Uuid::now_v7().to_string();
    let package_root =
        std::env::temp_dir().join(format!("plugin-node-contribution-source-{nonce}"));
    let install_root =
        std::env::temp_dir().join(format!("plugin-node-contribution-installed-{nonce}"));
    create_provider_fixture_with_node_contribution(&package_root);

    let service = PluginManagementService::new(
        repository.clone(),
        runtime,
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );

    let installation = service
        .install_plugin(InstallPluginCommand {
            actor_user_id: repository.actor.user_id,
            package_root: package_root.display().to_string(),
        })
        .await
        .unwrap()
        .installation;
    let entries = NodeContributionRepository::list_node_contributions(&repository, workspace_id)
        .await
        .unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].installation_id, installation.id);
    assert_eq!(entries[0].contribution_code, "openai_prompt");
    assert_eq!(
        entries[0].dependency_status,
        NodeContributionDependencyStatus::MissingPlugin
    );
}

#[tokio::test]
async fn plugin_management_service_installs_capability_plugin_node_contributions() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let runtime = MemoryProviderRuntime::default();
    let nonce = Uuid::now_v7().to_string();
    let package_root = std::env::temp_dir().join(format!("capability-plugin-source-{nonce}"));
    let install_root = std::env::temp_dir().join(format!("capability-plugin-installed-{nonce}"));
    create_capability_plugin_fixture(&package_root);

    let service = PluginManagementService::new(
        repository.clone(),
        runtime,
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );

    let installation = service
        .install_plugin(InstallPluginCommand {
            actor_user_id: repository.actor.user_id,
            package_root: package_root.display().to_string(),
        })
        .await
        .unwrap()
        .installation;
    let entries = NodeContributionRepository::list_node_contributions(&repository, workspace_id)
        .await
        .unwrap();

    assert_eq!(installation.contract_version, "1flowbase.capability/v1");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].installation_id, installation.id);
    assert_eq!(entries[0].plugin_unique_identifier, "fixture_capability");
    assert_eq!(entries[0].package_id, "fixture_capability@0.1.0");
    assert_eq!(entries[0].schema_version, "1flowbase.node-contribution/v2");
    assert!(entries[0].contribution_checksum.starts_with("sha256:"));
    assert!(entries[0].compiled_contribution_hash.starts_with("sha256:"));
    assert_eq!(
        entries[0].output_schema_snapshot["outputs"][0]["key"],
        "answer"
    );
    assert_eq!(
        entries[0].dependency_status,
        NodeContributionDependencyStatus::MissingPlugin
    );
}

#[tokio::test]
async fn plugin_management_service_syncs_js_dependency_pack_and_catalog_requires_assignment() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let runtime = MemoryProviderRuntime::default();
    let nonce = Uuid::now_v7().to_string();
    let package_root = std::env::temp_dir().join(format!("js-dependency-pack-source-{nonce}"));
    let install_root = std::env::temp_dir().join(format!("js-dependency-pack-installed-{nonce}"));
    create_js_dependency_pack_fixture(&package_root, "zod", "zod");

    let service = PluginManagementService::new(
        repository.clone(),
        runtime,
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );

    let installation = service
        .install_plugin(InstallPluginCommand {
            actor_user_id: repository.actor.user_id,
            package_root: package_root.display().to_string(),
        })
        .await
        .unwrap()
        .installation;

    let hidden_entries =
        JsDependencyRepository::list_workspace_js_dependencies(&repository, workspace_id)
            .await
            .unwrap();
    assert!(hidden_entries.is_empty());

    service
        .enable_plugin(EnablePluginCommand {
            actor_user_id: repository.actor.user_id,
            installation_id: installation.id,
        })
        .await
        .unwrap();
    service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: repository.actor.user_id,
            installation_id: installation.id,
        })
        .await
        .unwrap();

    let entries = JsDependencyRepository::list_workspace_js_dependencies(&repository, workspace_id)
        .await
        .unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].installation_id, installation.id);
    assert_eq!(entries[0].provider_code, "fixture_js_dependency_pack");
    assert_eq!(entries[0].alias, "zod");
    assert_eq!(entries[0].package, "zod");
    assert_eq!(entries[0].version, "1.2.3");
    assert_eq!(entries[0].target, "backend_code");
    assert_eq!(entries[0].artifact_path, "artifacts/zod.backend.mjs");
    assert_eq!(entries[0].integrity, "sha256-zod");
    assert_eq!(entries[0].permissions.network, "outbound_only");
    assert_eq!(entries[0].permissions.filesystem, "deny");
    assert_eq!(entries[0].permissions.env, "deny");
}

#[tokio::test]
async fn plugin_management_service_syncs_frontend_block_catalog_and_requires_assignment() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let nonce = Uuid::now_v7().to_string();
    let package_root = std::env::temp_dir().join(format!("frontend-block-source-{nonce}"));
    let install_root = std::env::temp_dir().join(format!("frontend-block-installed-{nonce}"));
    create_frontend_block_fixture(&package_root);

    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );

    let installation = service
        .install_plugin(InstallPluginCommand {
            actor_user_id: repository.actor.user_id,
            package_root: package_root.display().to_string(),
        })
        .await
        .unwrap()
        .installation;

    let hidden_entries = FrontendBlockCatalogRepository::list_workspace_frontend_blocks(
        &repository,
        &format!("local:{}", install_root.display()),
        workspace_id,
    )
    .await
    .unwrap();
    assert!(hidden_entries.is_empty());

    service
        .enable_plugin(EnablePluginCommand {
            actor_user_id: repository.actor.user_id,
            installation_id: installation.id,
        })
        .await
        .unwrap();
    service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: repository.actor.user_id,
            installation_id: installation.id,
        })
        .await
        .unwrap();

    let entries = FrontendBlockCatalogRepository::list_workspace_frontend_blocks(
        &repository,
        &format!("local:{}", install_root.display()),
        workspace_id,
    )
    .await
    .unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].installation_id, installation.id);
    assert_eq!(entries[0].provider_code, "fixture_frontend_blocks");
    assert_eq!(entries[0].contribution_code, "hero_banner");
    assert_eq!(entries[0].runtime, "native_react");
    assert_eq!(entries[0].entry, "blocks/hero/index.html");
    assert_eq!(entries[0].code_template_version.as_deref(), Some("1.0.0"));
    assert!(entries[0]
        .code_template
        .as_deref()
        .is_some_and(|template| template.contains("export default function Hero")));
    assert_eq!(entries[0].code_modules.len(), 1);
    assert_eq!(entries[0].code_modules[0].source, "@1flowbase/block-sdk");
    assert_eq!(entries[0].code_modules[0].version, "1.0.0");
    assert_eq!(entries[0].code_modules[0].exports, vec!["blockSdkVersion"]);
    assert_eq!(entries[0].code_modules[0].assets.len(), 1);
    assert_eq!(
        entries[0].context_contract.primitives,
        vec!["text", "image"]
    );
    assert_eq!(entries[0].permissions.storage, "none");
    assert_eq!(
        entries[0].ui_capabilities,
        vec!["responsive", "configurable"]
    );
}

#[tokio::test]
async fn plugin_install_catalog_failure_rolls_back_new_installation_and_artifact() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let nonce = Uuid::now_v7().to_string();
    let package_root = std::env::temp_dir().join(format!("frontend-block-failure-{nonce}"));
    let install_root = std::env::temp_dir().join(format!("frontend-block-failure-install-{nonce}"));
    create_frontend_block_fixture(&package_root);
    repository.fail_next_installation_catalog_commit().await;
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );

    let error = service
        .install_plugin(InstallPluginCommand {
            actor_user_id: repository.actor.user_id,
            package_root: package_root.display().to_string(),
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("constraint violation"));
    assert!(PluginRepository::list_installations(&repository)
        .await
        .unwrap()
        .is_empty());
    assert!(!install_root
        .join("installed/fixture_frontend_blocks/0.1.0")
        .exists());
    assert!(PluginRepository::list_tasks(&repository)
        .await
        .unwrap()
        .iter()
        .any(|task| task.status == PluginTaskStatus::Failed));
}

#[tokio::test]
async fn plugin_reinstall_catalog_failure_preserves_previous_installation_catalog_and_artifact() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let nonce = Uuid::now_v7().to_string();
    let package_root = std::env::temp_dir().join(format!("frontend-block-reinstall-{nonce}"));
    let install_root =
        std::env::temp_dir().join(format!("frontend-block-reinstall-install-{nonce}"));
    create_frontend_block_fixture(&package_root);
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );
    let original = service
        .install_plugin(InstallPluginCommand {
            actor_user_id: repository.actor.user_id,
            package_root: package_root.display().to_string(),
        })
        .await
        .unwrap()
        .installation;
    let original_artifact = repository
        .get_artifact_instance(&format!("local:{}", install_root.display()), original.id)
        .await
        .unwrap()
        .unwrap();
    let installed_entry = PathBuf::from(original_artifact.local_path.as_deref().unwrap())
        .join("blocks/hero/index.html");
    let original_artifact_contents = fs::read_to_string(&installed_entry).unwrap();

    let manifest_path = package_root.join("manifest.yaml");
    let manifest = fs::read_to_string(&manifest_path).unwrap();
    fs::write(
        &manifest_path,
        manifest
            .replacen("version: 0.1.0", "version: 0.2.0", 1)
            .replace("title: Hero Banner", "title: Changed Banner"),
    )
    .unwrap();
    fs::write(
        package_root.join("blocks/hero/index.html"),
        "<div>changed</div>",
    )
    .unwrap();
    repository.fail_next_installation_catalog_commit().await;

    service
        .install_plugin(InstallPluginCommand {
            actor_user_id: repository.actor.user_id,
            package_root: package_root.display().to_string(),
        })
        .await
        .unwrap_err();

    let preserved = PluginRepository::get_installation(&repository, original.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(preserved.plugin_version, original.plugin_version);
    assert_eq!(preserved.desired_state, original.desired_state);
    assert_eq!(
        fs::read_to_string(installed_entry).unwrap(),
        original_artifact_contents
    );
    assert!(!install_root
        .join("installed/fixture_frontend_blocks/0.2.0")
        .exists());
    assert!(PluginRepository::list_tasks(&repository)
        .await
        .unwrap()
        .iter()
        .any(|task| task.status == PluginTaskStatus::Failed));
    service
        .enable_plugin(EnablePluginCommand {
            actor_user_id: repository.actor.user_id,
            installation_id: original.id,
        })
        .await
        .unwrap();
    service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: repository.actor.user_id,
            installation_id: original.id,
        })
        .await
        .unwrap();
    let entries = FrontendBlockCatalogRepository::list_workspace_frontend_blocks(
        &repository,
        &format!("local:{}", install_root.display()),
        workspace_id,
    )
    .await
    .unwrap();
    assert_eq!(entries[0].title, "Hero Banner");
}

#[tokio::test]
async fn plugin_management_service_replaces_js_dependency_registry_on_reinstall() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let runtime = MemoryProviderRuntime::default();
    let nonce = Uuid::now_v7().to_string();
    let package_root =
        std::env::temp_dir().join(format!("js-dependency-pack-replace-source-{nonce}"));
    let install_root =
        std::env::temp_dir().join(format!("js-dependency-pack-replace-installed-{nonce}"));
    create_js_dependency_pack_fixture(&package_root, "zod", "zod");

    let service = PluginManagementService::new(
        repository.clone(),
        runtime,
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );

    let installation = service
        .install_plugin(InstallPluginCommand {
            actor_user_id: repository.actor.user_id,
            package_root: package_root.display().to_string(),
        })
        .await
        .unwrap()
        .installation;
    service
        .enable_plugin(EnablePluginCommand {
            actor_user_id: repository.actor.user_id,
            installation_id: installation.id,
        })
        .await
        .unwrap();
    service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: repository.actor.user_id,
            installation_id: installation.id,
        })
        .await
        .unwrap();

    fs::remove_dir_all(&package_root).unwrap();
    create_js_dependency_pack_fixture(&package_root, "valibot", "valibot");

    let replaced_installation = service
        .install_plugin(InstallPluginCommand {
            actor_user_id: repository.actor.user_id,
            package_root: package_root.display().to_string(),
        })
        .await
        .unwrap()
        .installation;
    let entries = JsDependencyRepository::list_workspace_js_dependencies(&repository, workspace_id)
        .await
        .unwrap();

    assert_eq!(replaced_installation.id, installation.id);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].alias, "valibot");
    assert_eq!(entries[0].package, "valibot");
    assert_eq!(entries[0].artifact_path, "artifacts/valibot.backend.mjs");
}
