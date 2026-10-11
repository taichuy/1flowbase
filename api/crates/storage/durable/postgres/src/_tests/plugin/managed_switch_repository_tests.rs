use super::*;

fn managed_input(actor: Uuid, version: &str) -> CommitPluginInstallationInput {
    let mut record = input(actor, version, "export default () => <p>managed</p>");
    record.installation.category = domain::ExtensionCategory::RuntimeExtensions;
    record.installation.contract_version = "1flowbase.extension-bus/v1".into();
    record.installation.protocol = "stdio_json_multiplex_v1".into();
    record.installation.metadata_json = json!({"managed_service":{"scope":"system"}});
    record.artifact_instance.is_current = false;
    record
}

fn permission_catalog() -> control_plane_contracts::CompiledConsolePolicyCatalog {
    control_plane_contracts::CompiledConsolePolicyCatalog {
        complete: true,
        groups: vec![control_plane_contracts::CompiledConsolePolicyGroup {
            group: domain::ConsolePolicyGroup::settings_feature("northwind.settings-page.settings")
                .unwrap(),
            full_operations: vec![domain::ConsoleOperationPolicy::simple(
                domain::ConsoleOperationId::try_from("northwind.settings-page.list").unwrap(),
                true,
            )],
        }],
    }
}

async fn catalog_published(store: &PgControlPlaneStore) -> bool {
    sqlx::query_scalar("select exists(select 1 from console_permission_catalog_operations where operation_id='northwind.settings-page.list')")
        .fetch_one(store.pool()).await.unwrap()
}

fn audit(actor: Uuid, target: Uuid) -> domain::AuditLogRecord {
    domain::AuditLogRecord {
        id: Uuid::now_v7(),
        workspace_id: None,
        actor_user_id: Some(actor),
        target_type: "plugin_installation".into(),
        target_id: Some(target),
        event_code: "plugin.version_switched".into(),
        payload: json!({}),
        created_at: time::OffsetDateTime::now_utc(),
    }
}

async fn selected(store: &PgControlPlaneStore) -> Vec<Uuid> {
    store
        .list_installations()
        .await
        .unwrap()
        .into_iter()
        .filter(|installation| installation.desired_state == PluginDesiredState::ActiveRequested)
        .map(|installation| installation.id)
        .collect()
}

#[tokio::test]
async fn system_managed_switch_is_atomic_without_workspace_assignment() {
    let store = store().await;
    let actor = actor(&store).await;
    let current = store
        .commit_plugin_installation(&managed_input(actor, "1.0.0"))
        .await
        .unwrap();
    let target = store
        .commit_plugin_installation(&managed_input(actor, "2.0.0"))
        .await
        .unwrap();
    desired(
        &store,
        current.id,
        actor,
        PluginDesiredState::ActiveRequested,
    )
    .await
    .unwrap();
    store
        .apply_managed_plugin_settings_templates(current.id)
        .await
        .unwrap();
    let mut command = ManagedInstallationSwitch {
        workspace_id: domain::SYSTEM_SCOPE_ID,
        current_installation_id: current.id,
        target_installation_id: target.id,
        node_id: "test-node".into(),
        installation_ids: vec![current.id, target.id],
    };
    let denied = store.lock_managed_installation_switch(&command).await;
    assert!(
        matches!(denied, Err(ref error) if error.to_string().contains("managed_candidate_authorization_required"))
    );
    assert_eq!(selected(&store).await, vec![current.id]);
    store
        .lock_installation_contribution_authority(target.id, domain::SYSTEM_SCOPE_ID)
        .await
        .unwrap()
        .release()
        .await
        .unwrap();
    command.workspace_id = Uuid::now_v7();
    let wrong_scope = store.lock_managed_installation_switch(&command).await;
    assert!(
        matches!(wrong_scope, Err(ref error) if error.to_string().contains("managed_candidate_scope_mismatch"))
    );
    command.workspace_id = domain::SYSTEM_SCOPE_ID;
    command.installation_ids.push(Uuid::now_v7());
    assert!(store
        .lock_managed_installation_switch(&command)
        .await
        .is_err());
    command.installation_ids.pop();
    // Candidate preparation and cancellation cannot alter the durable version selection.
    let prepared = store
        .lock_managed_installation_switch(&command)
        .await
        .unwrap();
    assert_eq!(selected(&store).await, vec![current.id]);
    prepared.release().await.unwrap();
    assert_eq!(selected(&store).await, vec![current.id]);
    let prepared = store
        .lock_managed_installation_switch(&command)
        .await
        .unwrap();
    drop(prepared);
    // Template ownership validation is part of the same selection transaction.
    sqlx::query("update plugin_settings_template_defaults set feature_id='northwind.settings-page.other' where installation_id=$1")
        .bind(target.id).execute(store.pool()).await.unwrap();
    let ownership_failure = store
        .lock_managed_installation_switch(&command)
        .await
        .unwrap()
        .commit(audit(actor, target.id))
        .await;
    assert!(ownership_failure
        .unwrap_err()
        .to_string()
        .contains("settings template feature ownership changed"));
    assert_eq!(selected(&store).await, vec![current.id]);
    sqlx::query("update plugin_settings_template_defaults set feature_id='northwind.settings-page.settings' where installation_id=$1")
        .bind(target.id).execute(store.pool()).await.unwrap();
    // A failed audit write rolls back the old-disable and new-enable writes together.
    let invalid = audit(actor, target.id);
    // Explicitly fail at the audit insert without relying on a varchar size constraint.
    sqlx::query("create function reject_switch_audit() returns trigger language plpgsql as $$ begin raise exception 'fixture audit rejected'; end $$")
        .execute(store.pool()).await.unwrap();
    sqlx::query("create trigger reject_switch_audit before insert on audit_logs for each row execute function reject_switch_audit()")
        .execute(store.pool()).await.unwrap();
    let failed = store
        .lock_managed_installation_switch(&command)
        .await
        .unwrap()
        .commit_with_catalog(invalid, Some(permission_catalog()))
        .await;
    assert!(failed.is_err());
    assert!(!catalog_published(&store).await);
    assert_eq!(selected(&store).await, vec![current.id]);
    assert_eq!(
        store.list_ui_code_templates().await.unwrap()[0]
            .applied_plugin_version
            .as_deref(),
        Some("1.0.0")
    );
    sqlx::query("drop trigger reject_switch_audit on audit_logs")
        .execute(store.pool())
        .await
        .unwrap();
    store
        .lock_managed_installation_switch(&command)
        .await
        .unwrap()
        .commit_with_catalog(audit(actor, target.id), Some(permission_catalog()))
        .await
        .unwrap();
    assert!(catalog_published(&store).await);
    assert_eq!(selected(&store).await, vec![target.id]);
    assert_eq!(
        store.list_ui_code_templates().await.unwrap()[0]
            .applied_plugin_version
            .as_deref(),
        Some("2.0.0")
    );
    assert_eq!(
        store
            .get_installation(current.id)
            .await
            .unwrap()
            .unwrap()
            .desired_state,
        PluginDesiredState::Disabled
    );
    assert!(store
        .list_assigned_installation_ids()
        .await
        .unwrap()
        .is_empty());
    let stale = store.lock_managed_installation_switch(&command).await;
    assert!(
        matches!(stale, Err(ref error) if error.to_string().contains("managed_system_selection_changed"))
    );
}

#[tokio::test]
async fn managed_template_commit_requires_locked_active_system_authority() {
    let store = store().await;
    let actor = actor(&store).await;
    let installation = store
        .commit_plugin_installation(&managed_input(actor, "1.0.0"))
        .await
        .unwrap();
    let denied = store
        .lock_installation_contribution_authority(installation.id, domain::SYSTEM_SCOPE_ID)
        .await
        .unwrap()
        .commit_managed_settings_templates(vec![installation.id], None)
        .await;
    assert!(denied
        .unwrap_err()
        .to_string()
        .contains("managed_settings_active_system_authority_required"));
    assert!(store.list_ui_code_templates().await.unwrap().is_empty());
    desired(
        &store,
        installation.id,
        actor,
        PluginDesiredState::ActiveRequested,
    )
    .await
    .unwrap();
    store
        .lock_installation_contribution_authority(installation.id, domain::SYSTEM_SCOPE_ID)
        .await
        .unwrap()
        .commit_managed_settings_templates(Vec::new(), None)
        .await
        .unwrap();
    assert!(store.list_ui_code_templates().await.unwrap().is_empty());
    let mut invalid_catalog = permission_catalog();
    invalid_catalog.complete = false;
    let rejected_catalog = store
        .lock_installation_contribution_authority(installation.id, domain::SYSTEM_SCOPE_ID)
        .await
        .unwrap()
        .commit_managed_settings_templates(vec![installation.id], Some(invalid_catalog))
        .await;
    assert!(rejected_catalog
        .unwrap_err()
        .to_string()
        .contains("complete catalog"));
    assert!(store.list_ui_code_templates().await.unwrap().is_empty());
    assert!(!catalog_published(&store).await);
    // The unknown ID sorts after the valid UUIDv7: even defaults already applied within
    // this transaction must roll back when a later requested installation is not locked.
    let rejected_batch = store
        .lock_installation_contribution_authority(installation.id, domain::SYSTEM_SCOPE_ID)
        .await
        .unwrap()
        .commit_managed_settings_templates(vec![installation.id, Uuid::from_u128(u128::MAX)], None)
        .await;
    assert!(rejected_batch
        .unwrap_err()
        .to_string()
        .contains("managed_settings_installation_not_locked"));
    assert!(store.list_ui_code_templates().await.unwrap().is_empty());
    let applications: i64 = sqlx::query_scalar(
        "select count(*) from plugin_settings_template_applications where installation_id=$1",
    )
    .bind(installation.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(applications, 0);
    store
        .lock_installation_contribution_authority(installation.id, domain::SYSTEM_SCOPE_ID)
        .await
        .unwrap()
        .commit_managed_settings_templates(
            vec![installation.id, installation.id],
            Some(permission_catalog()),
        )
        .await
        .unwrap();
    assert!(catalog_published(&store).await);
    let templates = store.list_ui_code_templates().await.unwrap();
    assert_eq!(templates.len(), 1);
    assert_eq!(
        templates[0].applied_plugin_version.as_deref(),
        Some("1.0.0")
    );
}

#[tokio::test]
async fn system_managed_rollback_reapplies_templates_with_monotonic_generations() {
    let store = store().await;
    let actor = actor(&store).await;
    let mut installations = Vec::new();
    for version in ["1.0.0", "2.0.0"] {
        let mut input = managed_input(actor, version);
        input.settings_templates[0].source = format!("export default () => <p>{version}</p>");
        let installation = store.commit_plugin_installation(&input).await.unwrap();
        store
            .lock_installation_contribution_authority(installation.id, domain::SYSTEM_SCOPE_ID)
            .await
            .unwrap()
            .release()
            .await
            .unwrap();
        installations.push(installation);
    }
    let mut previous = None;
    let mut expected_history = Vec::new();
    for (step, target_index) in [0, 1, 0, 1].into_iter().enumerate() {
        let target = &installations[target_index];
        if let Some(current) = previous {
            store
                .lock_managed_installation_switch(&ManagedInstallationSwitch {
                    workspace_id: domain::SYSTEM_SCOPE_ID,
                    current_installation_id: current,
                    target_installation_id: target.id,
                    node_id: "test-node".into(),
                    installation_ids: installations
                        .iter()
                        .map(|installation| installation.id)
                        .collect(),
                })
                .await
                .unwrap()
                .commit(audit(actor, target.id))
                .await
                .unwrap();
        } else {
            desired(
                &store,
                target.id,
                actor,
                PluginDesiredState::ActiveRequested,
            )
            .await
            .unwrap();
            store
                .apply_managed_plugin_settings_templates(target.id)
                .await
                .unwrap();
        }
        let generation = (step + 1) as i64;
        expected_history.push((generation, target.id));
        assert_eq!(selected(&store).await, vec![target.id]);
        for _ in 0..2 {
            // Repeated startup/reconciliation for the already selected version must not
            // overwrite user edits or create another revision/application generation.
            store
                .apply_managed_plugin_settings_templates(target.id)
                .await
                .unwrap();
            let history: Vec<(i64, Uuid)> = sqlx::query_as("select application_generation,installation_id from plugin_settings_template_applications where managed_service order by application_generation")
                .fetch_all(store.pool()).await.unwrap();
            assert_eq!(history, expected_history);
            let applied: (Uuid, i64, String, i32, String) = sqlx::query_as("select t.applied_installation_id,t.applied_application_generation,t.applied_plugin_version,r.revision,r.source from ui_code_templates t join ui_code_template_revisions r on r.template_id=t.id where r.is_latest")
                .fetch_one(store.pool()).await.unwrap();
            assert_eq!(applied.0, target.id);
            assert_eq!(applied.1, generation);
            assert_eq!(applied.2, target.plugin_version);
            assert_eq!(applied.3, generation as i32);
            assert_eq!(
                applied.4,
                format!("export default () => <p>{}</p>", target.plugin_version)
            );
        }
        assert_eq!(
            store
                .get_installation(installations[1 - target_index].id)
                .await
                .unwrap()
                .unwrap()
                .desired_state,
            PluginDesiredState::Disabled
        );
        previous = Some(target.id);
    }
}

#[tokio::test]
async fn rejected_managed_activation_compare_restore_preserves_newer_selection() {
    let store = store().await;
    let actor = actor(&store).await;
    let installation = store
        .commit_plugin_installation(&managed_input(actor, "1.0.0"))
        .await
        .unwrap();
    let requested = desired(
        &store,
        installation.id,
        actor,
        PluginDesiredState::ActiveRequested,
    )
    .await
    .unwrap();
    let restore = control_plane_contracts::ports::CompareRestorePluginDesiredStateInput {
        installation_id: installation.id,
        expected_updated_at: requested.updated_at,
        expected_desired_state: PluginDesiredState::ActiveRequested,
        restore_desired_state: PluginDesiredState::Disabled,
        actor_user_id: actor,
    };
    assert!(store.compare_restore_desired_state(&restore).await.unwrap());
    assert_eq!(
        store
            .get_installation(installation.id)
            .await
            .unwrap()
            .unwrap()
            .desired_state,
        PluginDesiredState::Disabled
    );
    assert!(!store.compare_restore_desired_state(&restore).await.unwrap());
    let requested = desired(
        &store,
        installation.id,
        actor,
        PluginDesiredState::ActiveRequested,
    )
    .await
    .unwrap();
    // Same desired state but a distinct selection revision must not be reverted. Fix the
    // revision delta explicitly so the negative is independent of database clock precision.
    sqlx::query("update extension_installations set updated_at=updated_at + interval '1 microsecond' where id=$1")
        .bind(installation.id).execute(store.pool()).await.unwrap();
    let newer = store
        .get_installation(installation.id)
        .await
        .unwrap()
        .unwrap();
    let stale = control_plane_contracts::ports::CompareRestorePluginDesiredStateInput {
        expected_updated_at: requested.updated_at,
        ..restore
    };
    assert!(!store.compare_restore_desired_state(&stale).await.unwrap());
    let retained = store
        .get_installation(installation.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.desired_state, PluginDesiredState::ActiveRequested);
    assert_eq!(retained.updated_at, newer.updated_at);
}
