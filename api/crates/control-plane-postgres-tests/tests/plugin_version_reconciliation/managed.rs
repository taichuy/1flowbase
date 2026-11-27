use super::*;
use domain::{
    AuditLogRecord, ExtensionCategory, ExtensionSignatureStatus, PluginArtifactInstanceStatus,
    PluginVerificationStatus,
};
use serde_json::json;
use time::OffsetDateTime;

#[tokio::test]
async fn postgres_managed_switch_retires_only_after_the_last_workspace() {
    let f = Fixture::new().await;
    let workspace_b = f
        .store
        .upsert_workspace(f.workspace.tenant_id, "Workspace B")
        .await
        .unwrap()
        .id;
    let workspaces = [f.workspace.id, workspace_b];
    let mut ids = Vec::new();
    for (version, state) in [("1.0.0", ACTIVE), ("2.0.0", DISABLED)] {
        let install =
            seed_managed_version(&f, version, state, json!({"binding_targets":["workspace"]}))
                .await;
        ids.push(install);
    }
    let [old, new]: [domain::PluginInstallationRecord; 2] = ids.try_into().unwrap();
    for workspace in workspaces {
        f.store
            .create_assignment(&CreatePluginAssignmentInput {
                installation_id: old.id,
                workspace_id: workspace,
                provider_code: "workspace_worker".into(),
                actor_user_id: f.actor,
            })
            .await
            .unwrap();
        for install in [&old, &new] {
            f.store
                .grant_contribution_authorization(&GrantContributionAuthorizationInput {
                    candidate_node_id: Some(NODE.into()),
                    expected_installation_updated_at: install.updated_at,
                    installation_id: install.id,
                    workspace_id: workspace,
                    contribution_id: "acme.workspace_worker.fixture".into(),
                    point_id: "fixture.point".into(),
                    permission: "event.subscribe".into(),
                    resource_scope: domain::ContributionResourceScope::Workspace,
                    permission_contract_id: "fixture.contract".into(),
                    permission_contract_version: "1".into(),
                    actor_user_id: f.actor,
                    audit_log: AuditLogRecord {
                        id: Uuid::now_v7(),
                        workspace_id: Some(workspace),
                        actor_user_id: Some(f.actor),
                        target_type: "plugin_installation".into(),
                        target_id: Some(install.id),
                        event_code: "plugin.contribution_granted".into(),
                        payload: json!({}),
                        created_at: OffsetDateTime::now_utc(),
                    },
                })
                .await
                .unwrap();
        }
    }
    let (old, new) = (old.id, new.id);
    let switch = |workspace, current, target| {
        let f = &f;
        async move {
            f.store
                .lock_managed_installation_switch(&ManagedInstallationSwitch {
                    validate_manifest: validate_installed_managed_manifest,
                    workspace_id: workspace,
                    current_installation_id: current,
                    target_installation_id: target,
                    node_id: NODE.into(),
                    installation_ids: vec![current, target],
                })
                .await
                .unwrap()
                .commit_with_catalog(
                    AuditLogRecord {
                        id: Uuid::now_v7(),
                        workspace_id: Some(workspace),
                        actor_user_id: Some(f.actor),
                        target_type: "plugin_assignment".into(),
                        target_id: Some(target),
                        event_code: "plugin.version_switched".into(),
                        payload: json!({}),
                        created_at: OffsetDateTime::now_utc(),
                    },
                    None,
                    Vec::new(),
                    None,
                )
                .await
                .unwrap();
        }
    };
    let mut events = Vec::new();
    let digest = "0".repeat(64);
    for id in [old, new] {
        let event = Uuid::now_v7();
        let subscriber = format!("managed.{}.acme.workspace_worker.fixture", workspaces[0]);
        let handler_version = format!("{}:sha256:{digest}:sha256:{digest}:1:{id}", Uuid::nil());
        assert_eq!(managed_handler_installation(&handler_version), Some(id));
        sqlx::query("insert into lifecycle_outbox(event_id,transaction_id,contract_id,contract_version,canonical_payload,occurred_at) values($1,$1,'fixture.switch','v1',decode('ff','hex'),now())")
            .bind(event).execute(f.store.pool()).await.unwrap();
        sqlx::query("insert into lifecycle_outbox_deliveries(event_id,subscriber_id,handler_id,handler_version) values($1,$2,$2,$3)")
            .bind(event).bind(&subscriber).bind(handler_version).execute(f.store.pool()).await.unwrap();
        events.push(event);
    }
    let delivery = |event| {
        let f = &f;
        async move {
            sqlx::query_as::<_, (String, Option<String>)>(
                "select status,pause_reason from lifecycle_outbox_deliveries where event_id=$1",
            )
            .bind(event)
            .fetch_one(f.store.pool())
            .await
            .unwrap()
        }
    };
    let paused = (
        "paused".to_string(),
        Some(
            LifecycleDeliveryPauseReason::InstallationInactive
                .as_str()
                .to_string(),
        ),
    );
    switch(workspaces[0], old, new).await;
    f.assert_state(old, ACTIVE).await;
    let old_after_first = f
        .store
        .get_artifact_instance(NODE, old)
        .await
        .unwrap()
        .unwrap();
    assert!(!old_after_first.is_current);
    assert!(
        f.store
            .get_artifact_instance(NODE, new)
            .await
            .unwrap()
            .unwrap()
            .is_current
    );
    assert_eq!(delivery(events[0]).await, ("pending".into(), None));
    switch(workspaces[1], old, new).await;
    assert_eq!(delivery(events[0]).await, paused);
    assert_eq!(delivery(events[1]).await, ("pending".into(), None));
    assert_retired_then_unloaded(&f, old).await;
    assert!(
        !f.store
            .get_artifact_instance(NODE, old)
            .await
            .unwrap()
            .unwrap()
            .is_current
    );
    for workspace in workspaces {
        switch(workspace, new, old).await;
    }
    f.assert_state(old, ACTIVE).await;
    assert_retired_then_unloaded(&f, new).await;
    assert_eq!(delivery(events[1]).await, paused);
    assert_eq!(f.store.list_installations().await.unwrap().len(), 2);
}

pub(super) async fn seed_managed_version(
    f: &Fixture,
    version: &str,
    (desired, runtime, availability): State,
    metadata: serde_json::Value,
) -> domain::PluginInstallationRecord {
    let install = f
        .store
        .upsert_installation(&UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            category: ExtensionCategory::RuntimeExtensions,
            organization: "acme".into(),
            provider_code: "workspace_worker".into(),
            plugin_id: format!("acme.workspace_worker@{version}"),
            plugin_version: version.into(),
            contract_version: "1flowbase.extension-bus/v1".into(),
            protocol: "stdio_json".into(),
            display_name: "Workspace worker".into(),
            source_kind: "uploaded".into(),
            trust_level: "checksum_only".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: desired,
            expected_checksum: None,
            signature_status: ExtensionSignatureStatus::Missing,
            signature_algorithm: None,
            signing_key_id: None,
            is_system_reserved: false,
            metadata_json: metadata,
            actor_user_id: f.actor,
        })
        .await
        .unwrap();
    let root = f.root.join("managed").join(version);
    let raw =
        control_plane_test_support::managed_manifest_fixture("acme.workspace_worker", version);
    let fp = validate_installed_managed_manifest(&install, &raw).expect("valid managed fixture");
    tokio::fs::create_dir_all(&root).await.unwrap();
    tokio::fs::write(root.join("manifest.yaml"), raw)
        .await
        .unwrap();
    tokio::fs::write(
        root.join(".1flowbase-artifact.json"),
        serde_json::to_vec(&json!({
            "plugin_id": install.plugin_id,
            "version": version,
            "checksum": install.expected_checksum,
            "manifest_fingerprint": fp,
        }))
        .unwrap(),
    )
    .await
    .unwrap();
    f.store
        .upsert_artifact_instance(&UpsertPluginArtifactInstanceInput {
            node_id: NODE.into(),
            installation_id: install.id,
            local_version: Some(version.into()),
            local_path: Some(root.display().to_string()),
            local_checksum: None,
            manifest_fingerprint: Some(fp),
            package_path: None,
            artifact_status: PluginArtifactInstanceStatus::Ready,
            runtime_status: runtime,
            availability_status: availability,
            checked_at: OffsetDateTime::now_utc(),
            last_error: None,
            is_current: desired == PluginDesiredState::ActiveRequested,
        })
        .await
        .unwrap();
    install
}

#[tokio::test]
async fn postgres_mixed_system_switch_migrates_workspace_ui_assignments() {
    exercise_mixed_system_switch(true).await;
}

#[tokio::test]
async fn postgres_mixed_system_switch_rejects_removing_referenced_workspace_ui() {
    exercise_mixed_system_switch(false).await;
}

async fn exercise_mixed_system_switch(compatible: bool) {
    let f = Fixture::new().await;
    let metadata = |version: &str, with_ui| {
        let bindings = if with_ui {
            json!([{
                "contribution_id":"acme.workspace_worker.block", "execution_mode":"declarative_only",
                "runtime":{"protocol":"none","entry":"unused"}, "handler":"render",
                "payload":{"kind":"frontend_block", "contribution_code":"block"}
            }])
        } else {
            json!([])
        };
        let managed = json!({
            "module":{"bus_version":"v1","module_id":"workspace_worker","module_version":version,"module_kind":"runtime"},
            "execution_bindings": bindings
        });
        // The persisted projection deliberately consumes the same canonical metadata shape.
        let declared: plugin_framework::ManagedManifest = serde_json::from_value(managed).unwrap();
        json!({"managed_service":{"scope":"system"}, "managed":declared})
    };
    let old = seed_managed_version(&f, "1.0.0", ACTIVE, metadata("1.0.0", true)).await;
    let new = seed_managed_version(&f, "2.0.0", DISABLED, metadata("2.0.0", compatible)).await;
    let other = f
        .store
        .upsert_workspace(f.workspace.tenant_id, "System UI B")
        .await
        .unwrap();
    let workspaces = [f.workspace.id, other.id];
    for workspace in workspaces {
        f.store
            .create_assignment(&CreatePluginAssignmentInput {
                installation_id: old.id,
                workspace_id: workspace,
                provider_code: old.provider_code.clone(),
                actor_user_id: f.actor,
            })
            .await
            .unwrap();
    }
    f.store
        .lock_installation_contribution_authority(new.id, domain::SYSTEM_SCOPE_ID)
        .await
        .unwrap()
        .release()
        .await
        .unwrap();
    let result = f
        .store
        .lock_managed_installation_switch(&ManagedInstallationSwitch {
            validate_manifest: validate_installed_managed_manifest,
            workspace_id: domain::SYSTEM_SCOPE_ID,
            current_installation_id: old.id,
            target_installation_id: new.id,
            node_id: NODE.into(),
            installation_ids: vec![old.id, new.id],
        })
        .await
        .unwrap()
        .commit(AuditLogRecord {
            id: Uuid::now_v7(),
            workspace_id: None,
            actor_user_id: Some(f.actor),
            target_type: "plugin_installation".into(),
            target_id: Some(new.id),
            event_code: "plugin.version_switched".into(),
            payload: json!({}),
            created_at: OffsetDateTime::now_utc(),
        })
        .await;
    let selected = if compatible {
        result.unwrap();
        assert_retired_then_unloaded(&f, old.id).await;
        f.assert_state(new.id, ACTIVE).await;
        new.id
    } else {
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("managed_installation_in_use"));
        f.assert_state(old.id, ACTIVE).await;
        f.assert_state(new.id, DISABLED).await;
        old.id
    };
    for workspace in workspaces {
        assert_eq!(
            f.store.list_assignments(workspace).await.unwrap()[0].installation_id,
            selected
        );
    }
}

#[tokio::test]
async fn postgres_mixed_system_switch_rejects_stale_old_version_reassignment() {
    let f = Fixture::new().await;
    let metadata = |version: &str, with_ui| {
        let bindings = if with_ui {
            json!([{
                "contribution_id":"acme.workspace_worker.block", "execution_mode":"declarative_only",
                "runtime":{"protocol":"none","entry":"unused"}, "handler":"render",
                "payload":{"kind":"frontend_block", "contribution_code":"block"}
            }])
        } else {
            json!([])
        };
        let managed = json!({
            "module":{"bus_version":"v1","module_id":"workspace_worker","module_version":version,"module_kind":"runtime"},
            "execution_bindings": bindings
        });
        // The persisted projection deliberately consumes the same canonical metadata shape.
        let declared: plugin_framework::ManagedManifest = serde_json::from_value(managed).unwrap();
        json!({"managed_service":{"scope":"system"}, "managed":declared})
    };
    let old = seed_managed_version(&f, "1.0.0", ACTIVE, metadata("1.0.0", true)).await;
    let new = seed_managed_version(&f, "2.0.0", DISABLED, metadata("2.0.0", true)).await;
    let candidate = seed_managed_version(&f, "3.0.0", DISABLED, metadata("3.0.0", true)).await;
    let unassigned_ws = f
        .store
        .upsert_workspace(f.workspace.tenant_id, "System UI Unassigned")
        .await
        .unwrap()
        .id;

    f.store
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: old.id,
            workspace_id: f.workspace.id,
            provider_code: old.provider_code.clone(),
            actor_user_id: f.actor,
        })
        .await
        .unwrap();
    f.store
        .lock_installation_contribution_authority(new.id, domain::SYSTEM_SCOPE_ID)
        .await
        .unwrap()
        .release()
        .await
        .unwrap();
    f.store
        .lock_managed_installation_switch(&ManagedInstallationSwitch {
            validate_manifest: validate_installed_managed_manifest,
            workspace_id: domain::SYSTEM_SCOPE_ID,
            current_installation_id: old.id,
            target_installation_id: new.id,
            node_id: NODE.into(),
            installation_ids: vec![old.id, new.id],
        })
        .await
        .unwrap()
        .commit(AuditLogRecord {
            id: Uuid::now_v7(),
            workspace_id: None,
            actor_user_id: Some(f.actor),
            target_type: "plugin_installation".into(),
            target_id: Some(new.id),
            event_code: "plugin.version_switched".into(),
            payload: json!({}),
            created_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();

    assert_retired_then_unloaded(&f, old.id).await;
    f.assert_state(new.id, ACTIVE).await;

    // 1. Stale explicit reassignment to retired old version in migrated workspace must fail.
    let stale_migrated = f
        .store
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: old.id,
            workspace_id: f.workspace.id,
            provider_code: old.provider_code.clone(),
            actor_user_id: f.actor,
        })
        .await
        .unwrap_err();
    assert!(stale_migrated
        .to_string()
        .contains("plugin_installation_disabled"));
    assert_eq!(
        f.store.list_assignments(f.workspace.id).await.unwrap()[0].installation_id,
        new.id
    );

    // 2. Stale explicit assignment to retired old version in an unassigned workspace also fails.
    let stale_unassigned = f
        .store
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: old.id,
            workspace_id: unassigned_ws,
            provider_code: old.provider_code.clone(),
            actor_user_id: f.actor,
        })
        .await
        .unwrap_err();
    assert!(stale_unassigned
        .to_string()
        .contains("plugin_installation_disabled"));

    // 3. Installing a new disabled candidate (create_initial_assignment) succeeds cleanly as a no-op.
    assert!(f
        .store
        .create_initial_assignment(&CreatePluginAssignmentInput {
            installation_id: candidate.id,
            workspace_id: unassigned_ws,
            provider_code: candidate.provider_code.clone(),
            actor_user_id: f.actor,
        })
        .await
        .unwrap()
        .is_none());
    assert!(f
        .store
        .list_assignments(unassigned_ws)
        .await
        .unwrap()
        .is_empty());
}

async fn assert_retired_then_unloaded(f: &Fixture, id: Uuid) {
    f.assert_state(id, (DISABLED.0, PluginRuntimeStatus::Active, DISABLED.2))
        .await;
    f.store
        .record_artifact_runtime_status(&RecordPluginRuntimeStatusInput {
            node_id: NODE.into(),
            installation_id: id,
            runtime_status: PluginRuntimeStatus::Inactive,
            last_error: None,
            checked_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    f.assert_state(id, DISABLED).await;
}

#[tokio::test]
async fn postgres_managed_switch_commit_rechecks_disk_inside_the_repository_lease() {
    let f = Fixture::new().await;
    let old =
        seed_managed_version(&f, "1.0.0", ACTIVE, json!({"binding_targets":["system"]})).await;
    let new =
        seed_managed_version(&f, "2.0.0", DISABLED, json!({"binding_targets":["system"]})).await;
    f.store
        .lock_installation_contribution_authority(new.id, domain::SYSTEM_SCOPE_ID)
        .await
        .unwrap()
        .release()
        .await
        .unwrap();
    let lease = f
        .store
        .lock_managed_installation_switch(&ManagedInstallationSwitch {
            validate_manifest: validate_installed_managed_manifest,
            workspace_id: domain::SYSTEM_SCOPE_ID,
            current_installation_id: old.id,
            target_installation_id: new.id,
            node_id: NODE.into(),
            installation_ids: vec![old.id, new.id],
        })
        .await
        .unwrap();
    let path = lease
        .authority()
        .node_artifact(new.id)
        .unwrap()
        .local_path
        .as_deref()
        .unwrap();
    tokio::fs::remove_file(std::path::Path::new(path).join(".1flowbase-artifact.json"))
        .await
        .unwrap();
    let error = lease
        .commit(AuditLogRecord {
            id: Uuid::now_v7(),
            workspace_id: None,
            actor_user_id: Some(f.actor),
            target_type: "plugin_installation".into(),
            target_id: Some(new.id),
            event_code: "plugin.version_switched".into(),
            payload: json!({}),
            created_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap_err();
    assert!(
        matches!(
            error.downcast_ref(),
            Some(
                control_plane_contracts::ControlPlaneContractError::Conflict(
                    "managed_candidate_artifact_invalid"
                )
            )
        ),
        "{error:#}"
    );
    f.assert_state(old.id, ACTIVE).await;
    f.assert_state(new.id, DISABLED).await;
}
