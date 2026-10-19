use super::*;

#[tokio::test]
async fn plugin_management_service_lists_official_catalog_and_installs_latest_release_asset() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let runtime = MemoryProviderRuntime::default();
    let service = PluginManagementService::new(
        repository.clone(),
        runtime,
        Arc::new(MemoryOfficialPluginSource::default()),
        std::env::temp_dir().join(format!("plugin-installed-{}", Uuid::now_v7())),
    );

    let catalog = service
        .list_official_catalog(
            repository.actor.user_id,
            OfficialPluginCatalogFilter::default(),
            requested_locales(),
        )
        .await
        .unwrap();
    assert_eq!(catalog.source_kind, "official_registry");
    assert_eq!(catalog.source_label, "官方源");
    assert_eq!(catalog.entries.len(), 1);
    assert_eq!(catalog.entries[0].plugin_id, "1flowbase.openai_compatible");

    let expected_package_bytes = build_openai_compatible_package_bytes("0.1.0", false);

    let install = service
        .install_official_plugin(InstallOfficialPluginCommand {
            actor_user_id: repository.actor.user_id,
            plugin_id: "1flowbase.openai_compatible".to_string(),
            compatibility_override: None,
            risk_override: Some(PluginRiskOverride {
                reason: "test acknowledges unsigned fixture".into(),
                acknowledged_warnings: vec![PLUGIN_RISK_SIGNATURE_MISSING.into()],
            }),
        })
        .await
        .unwrap();

    assert_eq!(install.installation.provider_code, "openai_compatible");
    assert_eq!(install.installation.source_kind, "official_registry");
    assert_eq!(
        install.installation.expected_checksum.as_deref(),
        Some(
            format!(
                "sha256:{:x}",
                base16ct::HexDisplay(&Sha256::digest(&expected_package_bytes))
            )
            .as_str()
        )
    );
    assert_eq!(
        install.installation.signature_status,
        ExtensionSignatureStatus::Missing
    );
    assert_eq!(install.installation.trust_level, "unverified");
    assert_eq!(install.task.status, PluginTaskStatus::Succeeded);
}

#[tokio::test]
async fn plugin_management_service_marks_official_catalog_entry_below_minimum_host_version() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::with_minimum_host_version(
            "999.0.0",
        )),
        std::env::temp_dir().join(format!("plugin-installed-{}", Uuid::now_v7())),
    );

    let catalog = service
        .list_official_catalog(
            repository.actor.user_id,
            OfficialPluginCatalogFilter::default(),
            requested_locales(),
        )
        .await
        .unwrap();

    let entry = &catalog.entries[0];
    assert_eq!(entry.minimum_host_version, "999.0.0");
    assert_eq!(entry.current_host_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(entry.compatibility_status, "below_minimum_host_version");
    assert_eq!(
        entry.compatibility_warning_reason.as_deref(),
        Some("below_minimum_host_version")
    );
}

#[tokio::test]
async fn plugin_management_service_requires_explicit_override_for_below_minimum_official_install() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::with_minimum_host_version(
            "999.0.0",
        )),
        std::env::temp_dir().join(format!("plugin-installed-{}", Uuid::now_v7())),
    );

    let blocked = service
        .install_official_plugin(InstallOfficialPluginCommand {
            actor_user_id: repository.actor.user_id,
            plugin_id: "1flowbase.openai_compatible".to_string(),
            compatibility_override: None,
            risk_override: Some(PluginRiskOverride {
                reason: "test acknowledges unsigned fixture".into(),
                acknowledged_warnings: vec![PLUGIN_RISK_SIGNATURE_MISSING.into()],
            }),
        })
        .await
        .unwrap_err();

    assert!(matches!(
        blocked.downcast_ref::<crate::errors::ControlPlaneError>(),
        Some(crate::errors::ControlPlaneError::Conflict(
            "plugin_host_version_below_minimum"
        ))
    ));

    let install = service
        .install_official_plugin(InstallOfficialPluginCommand {
            actor_user_id: repository.actor.user_id,
            plugin_id: "1flowbase.openai_compatible".to_string(),
            compatibility_override: Some(PluginCompatibilityOverride {
                reason: "below_minimum_host_version".to_string(),
                acknowledged_current_host_version: env!("CARGO_PKG_VERSION").to_string(),
                acknowledged_minimum_host_version: "999.0.0".to_string(),
            }),
            risk_override: Some(PluginRiskOverride {
                reason: "test acknowledges unsigned fixture".into(),
                acknowledged_warnings: vec![PLUGIN_RISK_SIGNATURE_MISSING.into()],
            }),
        })
        .await
        .unwrap();

    assert_eq!(
        install.installation.metadata_json["compatibility_override"]["reason"],
        "below_minimum_host_version"
    );
    assert_eq!(
        install.installation.metadata_json["compatibility_override"]
            ["acknowledged_minimum_host_version"],
        "999.0.0"
    );
    let install_task = repository
        .list_tasks()
        .await
        .unwrap()
        .into_iter()
        .find(|task| task.task_kind == domain::PluginTaskKind::Install)
        .expect("install task should be recorded");
    assert_eq!(
        install_task.detail_json["compatibility_override"]["reason"],
        "below_minimum_host_version"
    );
}

#[tokio::test]
async fn ac_be_4_plugin_management_service_requires_confirmation_for_unsigned_official_package() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::unsigned_required()),
        std::env::temp_dir().join(format!("plugin-installed-{}", Uuid::now_v7())),
    );

    let error = service
        .install_official_plugin(InstallOfficialPluginCommand {
            actor_user_id: repository.actor.user_id,
            plugin_id: "1flowbase.openai_compatible".into(),
            compatibility_override: None,
            risk_override: None,
        })
        .await
        .expect_err("unsigned official package must require confirmation");

    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::Conflict(
            "plugin_risk_confirmation_required"
        ))
    ));

    let result = service
        .install_official_plugin(InstallOfficialPluginCommand {
            actor_user_id: repository.actor.user_id,
            plugin_id: "1flowbase.openai_compatible".into(),
            compatibility_override: None,
            risk_override: Some(PluginRiskOverride {
                reason: "operator accepted unsigned package".into(),
                acknowledged_warnings: vec![PLUGIN_RISK_SIGNATURE_MISSING.into()],
            }),
        })
        .await
        .expect("confirmed warning should allow installation");
    assert_eq!(
        result.installation.metadata_json["risk_warnings"][0],
        PLUGIN_RISK_SIGNATURE_MISSING
    );
}
