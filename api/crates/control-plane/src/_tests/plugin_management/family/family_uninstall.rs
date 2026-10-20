use super::*;

#[tokio::test]
async fn plugin_management_service_uninstalls_family_without_deleting_durable_records() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let install_root =
        std::env::temp_dir().join(format!("plugin-delete-family-{}", Uuid::now_v7()));
    let runtime = MemoryProviderRuntime::default();
    let service = PluginManagementService::new(
        repository.clone(),
        runtime.clone(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );

    let current_installation = seed_test_installation(
        &repository,
        &install_root,
        "fixture_provider",
        "0.1.0",
        PluginDesiredState::ActiveRequested,
    )
    .await;
    let old_installation = seed_test_installation(
        &repository,
        &install_root,
        "fixture_provider",
        "0.0.9",
        PluginDesiredState::Disabled,
    )
    .await;
    repository
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: current_installation,
            workspace_id,
            provider_code: "fixture_provider".into(),
            actor_user_id: repository.actor.user_id,
        })
        .await
        .unwrap();
    repository
        .seed_instance_with_ready_cache(
            current_installation,
            "fixture_provider",
            "Fixture Provider Prod",
        )
        .await;
    repository
        .seed_instance_with_ready_cache(
            old_installation,
            "fixture_provider",
            "Fixture Provider Staging",
        )
        .await;

    let current_path = PathBuf::from(
        repository
            .get_artifact_instance(
                &format!("local:{}", install_root.display()),
                current_installation,
            )
            .await
            .unwrap()
            .unwrap()
            .local_path
            .unwrap(),
    );
    let old_path = PathBuf::from(
        repository
            .get_artifact_instance(
                &format!("local:{}", install_root.display()),
                old_installation,
            )
            .await
            .unwrap()
            .unwrap()
            .local_path
            .unwrap(),
    );

    let task = service
        .delete_family(DeletePluginFamilyCommand {
            actor_user_id: repository.actor.user_id,
            provider_code: "fixture_provider".into(),
        })
        .await
        .unwrap();
    assert_eq!(task.status, PluginTaskStatus::Succeeded);
    assert_eq!(task.status_message.as_deref(), Some("uninstalled"));
    assert_eq!(task.detail_json["retained_instance_count"], 2);
    assert_eq!(task.detail_json["retained_installation_count"], 2);

    assert_eq!(repository.list_installations().await.unwrap().len(), 2);
    assert_eq!(
        repository
            .list_assignments(workspace_id)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        repository.list_instances(workspace_id).await.unwrap().len(),
        2
    );
    assert!(!current_path.exists());
    assert!(!old_path.exists());
    for installation_id in [current_installation, old_installation] {
        let artifact = repository
            .get_artifact_instance(
                &format!("local:{}", install_root.display()),
                installation_id,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            artifact.artifact_status,
            PluginArtifactInstanceStatus::Missing
        );
        assert_eq!(artifact.runtime_status, PluginRuntimeStatus::Inactive);
        assert_eq!(
            artifact.availability_status,
            PluginAvailabilityStatus::ArtifactMissing
        );
        assert!(artifact.local_path.is_none());
        assert!(artifact.package_path.is_none());
        assert!(!artifact.is_current);
    }
    let mut unloaded = runtime.unloaded_installations().await;
    unloaded.sort();
    let mut expected_unloaded = vec![current_installation, old_installation];
    expected_unloaded.sort();
    assert_eq!(unloaded, expected_unloaded);
    assert_eq!(
        repository.audit_events().await,
        vec!["plugin.family_uninstalled"]
    );
    assert_eq!(repository.pending_artifact_cleanup_count().await, 0);

    let repeated_task = service
        .delete_family(DeletePluginFamilyCommand {
            actor_user_id: repository.actor.user_id,
            provider_code: "fixture_provider".into(),
        })
        .await
        .unwrap();
    assert_eq!(repeated_task.status, PluginTaskStatus::Succeeded);
    assert_eq!(repository.list_installations().await.unwrap().len(), 2);
    assert_eq!(
        repository.list_instances(workspace_id).await.unwrap().len(),
        2
    );

    let reinstall_source =
        std::env::temp_dir().join(format!("plugin-reinstall-source-{}", Uuid::now_v7()));
    create_provider_fixture(&reinstall_source);
    let reinstalled = service
        .install_plugin(InstallPluginCommand {
            actor_user_id: repository.actor.user_id,
            package_root: reinstall_source.display().to_string(),
        })
        .await
        .unwrap();

    assert_eq!(reinstalled.installation.id, current_installation);
    assert_eq!(
        reinstalled.installation.desired_state,
        PluginDesiredState::ActiveRequested
    );
    assert_eq!(
        reinstalled.local_artifact.artifact_status,
        PluginArtifactInstanceStatus::Ready
    );
    assert_eq!(
        reinstalled.local_artifact.runtime_status,
        PluginRuntimeStatus::Active
    );
    assert_eq!(
        reinstalled.local_artifact.availability_status,
        PluginAvailabilityStatus::Available
    );
    assert_eq!(
        repository
            .list_instances(workspace_id)
            .await
            .unwrap()
            .into_iter()
            .filter(|instance| instance.installation_id == current_installation)
            .count(),
        1
    );
    assert_eq!(
        repository
            .assignment_installation_id("fixture_provider")
            .await,
        current_installation
    );
    assert!(runtime
        .loaded_installations()
        .await
        .contains(&current_installation));
    let _ = fs::remove_dir_all(reinstall_source);
}

#[tokio::test]
async fn plugin_management_service_restores_family_artifacts_when_uninstall_commit_is_rejected() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let install_root =
        std::env::temp_dir().join(format!("plugin-delete-rollback-{}", Uuid::now_v7()));
    let runtime = MemoryProviderRuntime::default();
    let service = PluginManagementService::new(
        repository.clone(),
        runtime.clone(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );
    let current_installation = seed_test_installation(
        &repository,
        &install_root,
        "fixture_provider",
        "0.1.0",
        PluginDesiredState::ActiveRequested,
    )
    .await;
    let prior_installation = seed_test_installation(
        &repository,
        &install_root,
        "fixture_provider",
        "0.0.9",
        PluginDesiredState::Disabled,
    )
    .await;
    let current_artifact = repository
        .get_artifact_instance(
            &format!("local:{}", install_root.display()),
            current_installation,
        )
        .await
        .unwrap()
        .unwrap();
    repository
        .upsert_artifact_instance(&UpsertPluginArtifactInstanceInput {
            node_id: current_artifact.node_id.clone(),
            installation_id: current_artifact.installation_id,
            local_version: current_artifact.local_version.clone(),
            local_checksum: current_artifact.local_checksum.clone(),
            local_path: current_artifact.local_path.clone(),
            package_path: current_artifact.package_path.clone(),
            manifest_fingerprint: current_artifact.manifest_fingerprint.clone(),
            artifact_status: current_artifact.artifact_status,
            runtime_status: PluginRuntimeStatus::Active,
            availability_status: PluginAvailabilityStatus::Available,
            checked_at: current_artifact.checked_at,
            last_error: current_artifact.last_error.clone(),
            is_current: current_artifact.is_current,
        })
        .await
        .unwrap();
    let current_artifact = repository
        .get_artifact_instance(
            &format!("local:{}", install_root.display()),
            current_installation,
        )
        .await
        .unwrap()
        .unwrap();
    let prior_artifact = repository
        .get_artifact_instance(
            &format!("local:{}", install_root.display()),
            prior_installation,
        )
        .await
        .unwrap()
        .unwrap();
    let current_path = PathBuf::from(current_artifact.local_path.clone().unwrap());
    let prior_path = PathBuf::from(prior_artifact.local_path.clone().unwrap());
    let artifact_paths = [current_path, prior_path];
    let original_artifacts = [
        (current_installation, current_artifact),
        (prior_installation, prior_artifact),
    ];

    repository
        .fail_next_family_uninstall_commit_after_first_artifact()
        .await;
    let error = service
        .delete_family(DeletePluginFamilyCommand {
            actor_user_id: repository.actor.user_id,
            provider_code: "fixture_provider".into(),
        })
        .await
        .expect_err("database rejection must not delete any family artifact");
    assert!(error
        .to_string()
        .contains("injected plugin family uninstall commit failure"));
    assert!(artifact_paths.iter().all(|path| path.exists()));
    for (installation_id, expected) in original_artifacts {
        let artifact = repository
            .get_artifact_instance(
                &format!("local:{}", install_root.display()),
                installation_id,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(artifact, expected);
    }
    assert!(runtime.unloaded_installations().await.len() == 2);
    assert_eq!(
        runtime.loaded_installations().await,
        vec![current_installation],
        "only the formerly active runtime scope is restored after a rejected durable commit"
    );
    assert!(repository.audit_events().await.is_empty());
    let _ = fs::remove_dir_all(install_root);
}

#[tokio::test]
async fn plugin_management_service_keeps_committed_family_uninstall_succeeded_when_cleanup_retries()
{
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let install_root =
        std::env::temp_dir().join(format!("plugin-delete-cleanup-retry-{}", Uuid::now_v7()));
    let runtime = MemoryProviderRuntime::default();
    let service = PluginManagementService::new(
        repository.clone(),
        runtime,
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );
    let installation_id = seed_test_installation(
        &repository,
        &install_root,
        "fixture_provider",
        "0.1.0",
        PluginDesiredState::ActiveRequested,
    )
    .await;
    let artifact = repository
        .get_artifact_instance(
            &format!("local:{}", install_root.display()),
            installation_id,
        )
        .await
        .unwrap()
        .unwrap();
    let original_path = PathBuf::from(artifact.local_path.clone().unwrap());
    crate::plugin_management::fail_staged_artifact_removal_for(&original_path);

    let task = service
        .delete_family(DeletePluginFamilyCommand {
            actor_user_id: repository.actor.user_id,
            provider_code: "fixture_provider".into(),
        })
        .await
        .expect(
            "post-commit artifact cleanup failure must not fail an already committed uninstall",
        );

    assert_eq!(task.status, PluginTaskStatus::Succeeded);
    assert_eq!(
        repository
            .get_artifact_instance(
                &format!("local:{}", install_root.display()),
                installation_id,
            )
            .await
            .unwrap()
            .unwrap()
            .artifact_status,
        PluginArtifactInstanceStatus::Missing
    );
    assert_eq!(
        repository.audit_events().await,
        vec!["plugin.family_uninstalled"]
    );
    assert_eq!(repository.pending_artifact_cleanup_count().await, 1);
    assert!(
        fs::read_dir(original_path.parent().unwrap())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .any(|entry| entry
                .file_name()
                .to_string_lossy()
                .contains(".uninstalling-")),
        "the failed physical cleanup remains discoverable until reconciliation succeeds"
    );

    service
        .reconcile_pending_artifact_removals()
        .await
        .expect("the persisted cleanup record must make a retry possible after restart");
    assert_eq!(repository.pending_artifact_cleanup_count().await, 0);
    assert!(fs::read_dir(original_path.parent().unwrap())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .all(|entry| !entry
            .file_name()
            .to_string_lossy()
            .contains(".uninstalling-")));
    let _ = fs::remove_dir_all(install_root);
}

#[tokio::test]
async fn plugin_management_service_uninstalls_builtin_family_without_a_local_artifact() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let install_root =
        std::env::temp_dir().join(format!("plugin-delete-builtin-{}", Uuid::now_v7()));
    let runtime = MemoryProviderRuntime::default();
    let service = PluginManagementService::new(
        repository.clone(),
        runtime.clone(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );
    let installation_id = seed_test_installation(
        &repository,
        &install_root,
        "builtin_frontstage",
        "1.0.0",
        PluginDesiredState::ActiveRequested,
    )
    .await;
    repository.mark_installation_builtin(installation_id).await;

    let artifact = repository
        .get_artifact_instance(
            &format!("local:{}", install_root.display()),
            installation_id,
        )
        .await
        .unwrap()
        .unwrap();
    fs::remove_dir_all(artifact.local_path.as_deref().unwrap()).unwrap();
    repository
        .upsert_artifact_instance(&UpsertPluginArtifactInstanceInput {
            node_id: artifact.node_id,
            installation_id: artifact.installation_id,
            local_version: artifact.local_version,
            local_checksum: artifact.local_checksum,
            local_path: None,
            package_path: None,
            manifest_fingerprint: artifact.manifest_fingerprint,
            artifact_status: PluginArtifactInstanceStatus::Missing,
            runtime_status: PluginRuntimeStatus::Active,
            availability_status: PluginAvailabilityStatus::ArtifactMissing,
            is_current: artifact.is_current,
            last_error: Some("artifact_missing".into()),
            checked_at: time::OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();

    let task = service
        .delete_family(DeletePluginFamilyCommand {
            actor_user_id: repository.actor.user_id,
            provider_code: "builtin_frontstage".into(),
        })
        .await
        .unwrap();

    assert_eq!(task.status, PluginTaskStatus::Succeeded);
    assert!(repository
        .get_installation(installation_id)
        .await
        .unwrap()
        .is_some());
    assert_eq!(
        runtime.unloaded_installations().await,
        vec![installation_id]
    );
}
