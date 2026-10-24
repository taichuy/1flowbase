use super::*;
use crate::{errors::ControlPlaneError, host_extension::HOST_EXTENSION_CONTRACT_VERSION};

async fn seed_selection(
    repository: &MemoryPluginManagementRepository,
    version: &str,
    contract: &str,
    desired_state: PluginDesiredState,
) -> domain::PluginInstallationRecord {
    repository
        .upsert_installation(&UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            category: ExtensionCategory::RuntimeExtensions,
            organization: "1flowbase".into(),
            provider_code: "openai_compatible".into(),
            plugin_id: format!("openai_compatible-{contract}@{version}"),
            plugin_version: version.into(),
            contract_version: contract.into(),
            protocol: "openai_compatible".into(),
            display_name: "Selection fixture".into(),
            source_kind: "official_registry".into(),
            trust_level: "checksum_only".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state,
            expected_checksum: None,
            signature_status: ExtensionSignatureStatus::Missing,
            signature_algorithm: None,
            signing_key_id: None,
            metadata_json: serde_json::json!({ "managed_service": { "scope": "system" } }),
            is_system_reserved: false,
            actor_user_id: repository.actor.user_id,
        })
        .await
        .unwrap()
}

fn native_target(installation_id: Uuid) -> domain::NativePluginTarget {
    domain::NativePluginTarget {
        scope_id: domain::SYSTEM_SCOPE_ID,
        category: ExtensionCategory::RuntimeExtensions,
        organization: "1flowbase".into(),
        artifact_id: "openai_compatible".into(),
        installation_id,
        selection_revision: 1,
        enabled: true,
        application_generation: 1,
    }
}

fn selection_service(
    repository: &MemoryPluginManagementRepository,
) -> PluginManagementService<MemoryPluginManagementRepository, MemoryProviderRuntime> {
    PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        std::env::temp_dir().join(format!("family-selection-{}", Uuid::now_v7())),
    )
}

async fn upgrade_selection(
    repository: &MemoryPluginManagementRepository,
) -> Result<domain::PluginTaskRecord> {
    fn require_send<T: Send>(value: T) -> T {
        value
    }
    let service = selection_service(repository);
    require_send(service.upgrade_latest(UpgradeLatestPluginFamilyCommand {
        actor_user_id: repository.actor.user_id,
        provider_code: "openai_compatible".into(),
        compatibility_override: None,
        risk_override: None,
    }))
    .await
}

async fn assign_workspace(repository: &MemoryPluginManagementRepository) {
    let installation = seed_selection(
        repository,
        "0.0.8",
        CURRENT_PROVIDER_CONTRACT,
        PluginDesiredState::ActiveRequested,
    )
    .await;
    repository
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: installation.id,
            workspace_id: repository.actor.current_workspace_id,
            provider_code: installation.provider_code,
            actor_user_id: repository.actor.user_id,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn family_selection_native_requires_exactly_one_matching_target_before_other_scopes() {
    for target_count in [0, 1, 2] {
        let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
            Uuid::now_v7(),
            &["plugin_config.configure.all"],
        ));
        assign_workspace(&repository).await;
        seed_selection(
            &repository,
            "0.0.9",
            "1flowbase.extension-bus/v1",
            PluginDesiredState::ActiveRequested,
        )
        .await;
        let selected = seed_selection(
            &repository,
            "0.1.0",
            HOST_EXTENSION_CONTRACT_VERSION,
            PluginDesiredState::Disabled,
        )
        .await;
        seed_selection(
            &repository,
            "0.0.7",
            HOST_EXTENSION_CONTRACT_VERSION,
            PluginDesiredState::ActiveRequested,
        )
        .await;
        let mut targets = vec![native_target(selected.id); target_count];
        let mut unrelated = native_target(Uuid::now_v7());
        unrelated.artifact_id = "another-family".into();
        targets.insert(0, unrelated);
        repository.set_native_targets(targets).await;

        let error = upgrade_selection(&repository).await.unwrap_err();
        assert!(
            matches!(error.downcast_ref::<ControlPlaneError>(),
            Some(ControlPlaneError::Conflict(reason)) if *reason == if target_count == 1 {
                "plugin_version_already_current"
            } else {
                "native_plugin_selection_required"
            }),
            "{error:?}"
        );
        assert!(repository.list_tasks().await.unwrap().is_empty());
    }
}

#[tokio::test]
async fn family_selection_rejects_native_target_outside_native_family() {
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        Uuid::now_v7(),
        &["plugin_config.configure.all"],
    ));
    let workspace = seed_selection(
        &repository,
        "0.1.0",
        CURRENT_PROVIDER_CONTRACT,
        PluginDesiredState::ActiveRequested,
    )
    .await;
    seed_selection(
        &repository,
        "0.0.9",
        HOST_EXTENSION_CONTRACT_VERSION,
        PluginDesiredState::ActiveRequested,
    )
    .await;
    repository
        .set_native_targets(vec![native_target(workspace.id)])
        .await;
    let error = upgrade_selection(&repository).await.unwrap_err();
    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::NotFound("native_plugin_installation"))
    ));
}

#[tokio::test]
async fn family_selection_system_requires_one_active_without_workspace_fallback() {
    for active_count in [0, 1, 2] {
        let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
            Uuid::now_v7(),
            &["plugin_config.configure.all"],
        ));
        assign_workspace(&repository).await;
        let mut selected = None;
        for (index, version) in ["0.1.0", "0.0.9", "0.0.7"].into_iter().enumerate() {
            let installation = seed_selection(
                &repository,
                version,
                "1flowbase.extension-bus/v1",
                if index < active_count {
                    PluginDesiredState::ActiveRequested
                } else {
                    PluginDesiredState::Disabled
                },
            )
            .await;
            selected.get_or_insert(installation.id);
        }
        let error = selection_service(&repository)
            .switch_version(SwitchPluginVersionCommand {
                actor_user_id: repository.actor.user_id,
                provider_code: "openai_compatible".into(),
                target_installation_id: selected.unwrap(),
            })
            .await
            .unwrap_err();
        let expected = match active_count {
            0 => "managed_active_installation",
            1 => "plugin_version_already_current",
            _ => "managed_active_installation_ambiguous",
        };
        assert!(
            matches!(error.downcast_ref::<ControlPlaneError>(),
            Some(ControlPlaneError::NotFound(reason) | ControlPlaneError::Conflict(reason)) if *reason == expected),
            "{error:?}"
        );
        assert!(repository.list_tasks().await.unwrap().is_empty());
    }
}
