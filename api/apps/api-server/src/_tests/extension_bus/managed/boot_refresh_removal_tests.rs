use super::*;

#[tokio::test]
async fn boot_refresh_removes_and_retains_the_last_disabled_binding() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let actor_id = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let actor = AuthRepository::load_actor_context_for_user(&state.store, actor_id)
        .await
        .unwrap();
    let runtime = RuntimeFixture::new(&state);
    let management = management_for_snapshot(&state, &runtime);
    let installation_id =
        install_event_candidate(&runtime, &management, &actor, "a", "1.0.0").await;
    management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: actor_id,
            installation_id,
        })
        .await
        .unwrap();
    let workspace = actor.current_workspace_id;
    let before = runtime.composition.snapshot(workspace).await.unwrap();
    assert_eq!(before.bindings.len(), 1);
    let _reference = before.freeze_reference().unwrap();
    runtime
        .composition
        .attach_interface_module(
            plugin_framework::extension_bus::compile_managed_interface_module([
                "notifications.instances.list",
            ])
            .unwrap(),
        )
        .unwrap();
    // Commit directly through a separate checkout, without the in-process deactivation path.
    let mut writer = state.store.pool().acquire().await.unwrap();
    sqlx::query("update extension_installations set desired_state = 'disabled', updated_at = now() where id = $1")
        .bind(installation_id).execute(&mut *writer).await.unwrap();
    drop(writer);
    let pinned = Arc::new(std::sync::OnceLock::new());
    let pin = pinned.clone();
    let boot_state = state.clone();
    let visible = runtime
        .composition
        .refresh_boot_snapshots_with_event_catalog_and_pin(
            move |snapshots| {
                Box::pin(async move {
                    crate::event_center::persistence::snapshot(&boot_state, &snapshots).await
                })
            },
            move |published| {
                assert!(pin.set(published.clone()).is_ok());
                Ok(())
            },
        )
        .await
        .unwrap();
    assert!(
        visible.is_empty(),
        "refresh must remove, not publish, the empty workspace"
    );
    assert!(pinned.get().unwrap().is_empty());
    assert!(runtime.composition.current_snapshots().await.is_empty());
    let execution = runtime
        .composition
        .governance()
        .managed_execution_state(workspace, installation_id, None)
        .await
        .unwrap();
    assert_eq!(execution.executions.len(), 1);
    assert!(!execution.executions[0].current);
    runtime.composition.refresh_boot_snapshots().await.unwrap();
    runtime.host.stop().await.unwrap();
}

#[tokio::test]
async fn symmetric_f1_boot_refresh_removes_and_pins_after_last_assignment_disappears() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let actor_id = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let actor = AuthRepository::load_actor_context_for_user(&state.store, actor_id)
        .await
        .unwrap();
    let runtime = RuntimeFixture::new(&state);
    let management = management_for_snapshot(&state, &runtime);
    let installation_id =
        install_event_candidate(&runtime, &management, &actor, "a", "1.0.0").await;
    management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: actor_id,
            installation_id,
        })
        .await
        .unwrap();
    let workspace = actor.current_workspace_id;
    let before = runtime.composition.snapshot(workspace).await.unwrap();
    assert_eq!(before.bindings.len(), 1);
    let _reference = before.freeze_reference().unwrap();
    runtime
        .composition
        .attach_interface_module(
            plugin_framework::extension_bus::compile_managed_interface_module([
                "notifications.instances.list",
            ])
            .unwrap(),
        )
        .unwrap();
    // Commit directly through a separate checkout, without the in-process deactivation path.
    let mut writer = state.store.pool().begin().await.unwrap();
    sqlx::query("delete from plugin_assignments where workspace_id = $1 and installation_id = $2")
        .bind(workspace)
        .bind(installation_id)
        .execute(&mut *writer)
        .await
        .unwrap();
    sqlx::query("update extension_installations set desired_state = 'disabled', updated_at = now() where id = $1")
        .bind(installation_id).execute(&mut *writer).await.unwrap();
    writer.commit().await.unwrap();
    let pinned = Arc::new(std::sync::OnceLock::new());
    let pin = pinned.clone();
    let boot_state = state.clone();
    let visible = runtime
        .composition
        .refresh_boot_snapshots_with_event_catalog_and_pin(
            move |snapshots| {
                Box::pin(async move {
                    crate::event_center::persistence::snapshot(&boot_state, &snapshots).await
                })
            },
            move |published| {
                assert!(pin.set(published.clone()).is_ok());
                Ok(())
            },
        )
        .await
        .unwrap();
    assert!(
        visible.is_empty(),
        "refresh must remove, not publish, the empty workspace"
    );
    assert!(pinned.get().unwrap().is_empty());
    assert!(runtime.composition.current_snapshots().await.is_empty());
    let execution = runtime
        .composition
        .governance()
        .managed_execution_state(workspace, installation_id, None)
        .await
        .unwrap();
    assert_eq!(execution.executions.len(), 1);
    assert!(!execution.executions[0].current);
    runtime.composition.refresh_boot_snapshots().await.unwrap();
    runtime.host.stop().await.unwrap();
}

#[tokio::test]
async fn symmetric_f1_boot_refresh_replaces_snapshot_after_external_switch() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let actor_id = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let actor = AuthRepository::load_actor_context_for_user(&state.store, actor_id)
        .await
        .unwrap();
    let runtime = RuntimeFixture::new(&state);
    let management = management_for_snapshot(&state, &runtime);
    let first = install_event_candidate(&runtime, &management, &actor, "a", "1.0.0").await;
    management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: actor_id,
            installation_id: first,
        })
        .await
        .unwrap();
    let workspace = actor.current_workspace_id;
    let old = runtime.composition.snapshot(workspace).await.unwrap();
    let _reference = old.freeze_reference().unwrap();
    let second = install_event_candidate(&runtime, &management, &actor, "a", "2.0.0").await;

    // Another composition commits the real switch without replacing our local snapshot.
    let writer = RuntimeFixture::new(&state);
    for composition in [&runtime.composition, &writer.composition] {
        composition
            .attach_interface_module(
                plugin_framework::extension_bus::compile_managed_interface_module([
                    "notifications.instances.list",
                ])
                .unwrap(),
            )
            .unwrap();
    }
    writer
        .composition
        .rebuild_installation(first)
        .await
        .unwrap();
    management_for_snapshot(&state, &writer)
        .for_plugin_console_operation(
            domain::ConsolePolicyGroup::other("other.plugins").unwrap(),
            "plugins.families.switch",
        )
        .switch_version(SwitchPluginVersionCommand {
            actor_user_id: actor_id,
            provider_code: "acme.composition-a".into(),
            target_installation_id: second,
        })
        .await
        .unwrap();
    assert!(Arc::ptr_eq(
        &old,
        &runtime.composition.snapshot(workspace).await.unwrap()
    ));
    assert_eq!(
        state.store.list_assignments(workspace).await.unwrap()[0].installation_id,
        second
    );
    runtime.composition.refresh_boot_snapshots().await.unwrap();
    let current = runtime.composition.snapshot(workspace).await.unwrap();
    assert_eq!(current.bindings.len(), 1);
    assert!(current
        .bindings
        .values()
        .all(|binding| binding.installation.id == second));
    let retained = runtime
        .composition
        .governance()
        .managed_execution_state(workspace, first, None)
        .await
        .unwrap();
    assert_eq!(retained.executions.len(), 1);
    assert!(!retained.executions[0].current);
    writer.host.stop().await.unwrap();
    runtime.host.stop().await.unwrap();
}

#[tokio::test]
async fn symmetric_f2_managed_switch_rejects_marker_changed_after_preparation() {
    use runtime_core::runtime_backend::{RuntimeArtifactReference, RuntimeBackendError};
    use runtime_extension_host::RuntimeArtifactResolver;

    struct TamperingResolver(ApiRuntimeArtifactResolver, std::sync::OnceLock<Uuid>);
    #[async_trait::async_trait]
    impl RuntimeArtifactResolver for TamperingResolver {
        async fn resolve(
            &self,
            artifact: &RuntimeArtifactReference,
        ) -> Result<PathBuf, RuntimeBackendError> {
            let path = self.0.resolve(artifact).await?;
            if self
                .1
                .get()
                .is_some_and(|target| target.to_string() == artifact.as_str())
            {
                let marker = path.join(".1flowbase-artifact.json");
                let mut value: serde_json::Value =
                    serde_json::from_slice(&tokio::fs::read(&marker).await.unwrap()).unwrap();
                assert!(value["checksum"].is_string() || value["checksum"].is_null());
                value["checksum"] = "sha256:tampered".into();
                tokio::fs::write(marker, serde_json::to_vec(&value).unwrap())
                    .await
                    .unwrap();
            }
            Ok(path)
        }
    }

    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let actor_id = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let actor = AuthRepository::load_actor_context_for_user(&state.store, actor_id)
        .await
        .unwrap();
    let resolver = Arc::new(TamperingResolver(
        ApiRuntimeArtifactResolver::new(
            state.store.clone(),
            &state.api_node_id,
            &state.provider_install_root,
        ),
        std::sync::OnceLock::new(),
    ));
    let runtime = RuntimeFixture::with_artifact_resolver(&state, resolver.clone(), false);
    let management = management_for_snapshot(&state, &runtime);
    let first = install_event_candidate(&runtime, &management, &actor, "a", "1.0.0").await;
    management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: actor_id,
            installation_id: first,
        })
        .await
        .unwrap();
    let second = install_event_candidate(&runtime, &management, &actor, "a", "2.0.0").await;
    let target = state
        .store
        .get_local_installation(&state.api_node_id, second)
        .await
        .unwrap()
        .unwrap();
    assert!(target.expected_checksum.is_some());
    assert!(target.artifact.artifact_status.is_ready());
    resolver.1.set(second).unwrap();
    let workspace = actor.current_workspace_id;
    let old = runtime.composition.snapshot(workspace).await.unwrap();
    let result = management
        .for_plugin_console_operation(
            domain::ConsolePolicyGroup::other("other.plugins").unwrap(),
            "plugins.families.switch",
        )
        .switch_version(SwitchPluginVersionCommand {
            actor_user_id: actor_id,
            provider_code: "acme.composition-a".into(),
            target_installation_id: second,
        })
        .await;
    let error = result.expect_err("managed switch accepted a changed marker checksum");
    assert!(
        format!("{error:#}").contains("checksum_mismatch"),
        "{error:#}"
    );
    assert_eq!(
        state.store.list_assignments(workspace).await.unwrap()[0].installation_id,
        first
    );
    assert!(Arc::ptr_eq(
        &old,
        &runtime.composition.snapshot(workspace).await.unwrap()
    ));
    let new = state
        .store
        .get_artifact_instance(&state.api_node_id, second)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(new.runtime_status, domain::PluginRuntimeStatus::Inactive);
    assert!(new.artifact_status.is_ready());
    runtime.host.stop().await.unwrap();
}

#[tokio::test]
async fn managed_switch_keeps_runtime_active_until_frozen_reference_is_reclaimed() {
    assert_managed_switch_runtime_retirement(RetirementCase::Frozen).await;
}

#[tokio::test]
async fn managed_switch_unloads_unreferenced_runtime_before_returning() {
    assert_managed_switch_runtime_retirement(RetirementCase::Immediate).await;
}

#[tokio::test]
async fn managed_switch_retries_retirement_after_inactive_write_fails() {
    assert_managed_switch_runtime_retirement(RetirementCase::Retry).await;
}

#[derive(Clone, Copy)]
enum RetirementCase {
    Frozen,
    Immediate,
    Retry,
}

async fn assert_managed_switch_runtime_retirement(case: RetirementCase) {
    use domain::{PluginAvailabilityStatus, PluginDesiredState, PluginRuntimeStatus};

    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let actor_id = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let actor = AuthRepository::load_actor_context_for_user(&state.store, actor_id)
        .await
        .unwrap();
    let runtime = RuntimeFixture::new(&state);
    let management = management_for_snapshot(&state, &runtime);
    let first = install_event_candidate(&runtime, &management, &actor, "a", "1.0.0").await;
    management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: actor_id,
            installation_id: first,
        })
        .await
        .unwrap();
    let second = install_event_candidate(&runtime, &management, &actor, "a", "2.0.0").await;
    let old = runtime
        .composition
        .snapshot(actor.current_workspace_id)
        .await
        .unwrap();
    let reference = matches!(case, RetirementCase::Frozen).then(|| old.freeze_reference().unwrap());
    if matches!(case, RetirementCase::Retry) {
        // Fail only this installation's inactive write, after the real host has unmounted it.
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "alter table extension_artifact_instances add constraint fixture_reject_inactive \
             check (installation_id <> '{first}'::uuid or runtime_status <> 'inactive')"
        )))
        .execute(state.store.pool())
        .await
        .unwrap();
    }
    if matches!(case, RetirementCase::Frozen) {
        runtime.composition.start_reclamation();
    }
    runtime
        .composition
        .attach_interface_module(
            plugin_framework::extension_bus::compile_managed_interface_module([
                "notifications.instances.list",
            ])
            .unwrap(),
        )
        .unwrap();
    management
        .for_plugin_console_operation(
            domain::ConsolePolicyGroup::other("other.plugins").unwrap(),
            "plugins.families.switch",
        )
        .switch_version(SwitchPluginVersionCommand {
            actor_user_id: actor_id,
            provider_code: "acme.composition-a".into(),
            target_installation_id: second,
        })
        .await
        .unwrap();
    let read_old = || async {
        state
            .store
            .get_local_installation(&state.api_node_id, first)
            .await
            .unwrap()
            .unwrap()
    };
    let retired = read_old().await;
    assert_eq!(retired.desired_state, PluginDesiredState::Disabled);
    assert_eq!(
        retired.artifact.availability_status,
        PluginAvailabilityStatus::Disabled
    );
    assert!(!retired.artifact.is_current);
    match case {
        RetirementCase::Frozen => {
            assert_eq!(retired.artifact.runtime_status, PluginRuntimeStatus::Active);
            let refreshed =
                control_plane::plugin_management::refresh_current_node_plugin_artifact_instance(
                    &state.store,
                    &state.api_node_id,
                    std::path::Path::new(&state.provider_install_root),
                    &retired.installation,
                )
                .await
                .unwrap();
            assert_eq!(refreshed.runtime_status, PluginRuntimeStatus::Active);
            assert_eq!(
                refreshed.availability_status,
                PluginAvailabilityStatus::Disabled
            );
            assert_eq!(read_old().await.desired_state, PluginDesiredState::Disabled);
            drop(reference);
        }
        RetirementCase::Retry => {
            assert_eq!(retired.artifact.runtime_status, PluginRuntimeStatus::Active);
            assert!(
                old.freeze_reference().is_ok(),
                "failed retirement must not retire the lifetime"
            );
            sqlx::query(
                "alter table extension_artifact_instances drop constraint fixture_reject_inactive",
            )
            .execute(state.store.pool())
            .await
            .unwrap();
            runtime.composition.start_reclamation();
        }
        RetirementCase::Immediate => {
            assert_eq!(
                retired.artifact.runtime_status,
                PluginRuntimeStatus::Inactive
            );
        }
    }
    if !matches!(case, RetirementCase::Immediate) {
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            while read_old().await.artifact.runtime_status != PluginRuntimeStatus::Inactive
                || old.freeze_reference().is_ok()
            {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("background reclamation did not finish retiring the old runtime");
    }
    assert!(old.freeze_reference().is_err());
    let retired = read_old().await;
    assert_eq!(retired.desired_state, PluginDesiredState::Disabled);
    assert_eq!(
        retired.artifact.availability_status,
        PluginAvailabilityStatus::Disabled
    );
    runtime.host.stop().await.unwrap();
}
