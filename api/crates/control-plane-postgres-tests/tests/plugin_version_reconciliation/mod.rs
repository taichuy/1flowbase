#[path = "../support/mod.rs"]
mod support;

mod concurrency_fences;
mod fixture;
mod managed;
mod reference_fences;

use control_plane::{i18n::RequestedLocales, plugin_management::*};
use control_plane_contracts::ports::*;
use domain::{PluginAvailabilityStatus, PluginDesiredState, PluginRuntimeStatus};
use fixture::*;
use uuid::Uuid;

#[tokio::test]
async fn postgres_single_workspace_provider_switch_and_rollback_retires_both_versions() {
    let f = Fixture::new().await;
    let old = f.install("fixture_provider", "1.0.0").await;
    let new = f.install("fixture_provider", "2.0.0").await;
    f.enable(old).await;
    f.service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: f.actor,
            installation_id: old,
        })
        .await
        .unwrap();
    f.switch(f.actor, new).await;
    f.assert_state(old, DISABLED).await;
    f.assert_state(new, ACTIVE).await;
    f.switch(f.actor, old).await;
    f.assert_state(old, ACTIVE).await;
    f.assert_state(new, DISABLED).await;
    assert_eq!(f.store.list_installations().await.unwrap().len(), 2);
}

#[tokio::test]
async fn postgres_provider_switch_preserves_other_workspaces_and_rolls_back() {
    let f = Fixture::new().await;
    let old = f.install("fixture_provider", "1.0.0").await;
    let new = f.install("fixture_provider", "2.0.0").await;
    f.enable(old).await;
    let workspace_b = f
        .store
        .upsert_workspace(f.workspace.tenant_id, "Workspace B")
        .await
        .unwrap();
    control_plane_test_support::upsert_builtin_roles(&f.store, workspace_b.id)
        .await
        .unwrap();
    let actor_b = f
        .store
        .upsert_root_user(
            workspace_b.id,
            "root_b",
            "root-b@example.com",
            "$argon2id$v=19$m=19456,t=2,p=1$test$test",
            "Root B",
            "Root B",
        )
        .await
        .unwrap()
        .id;
    for actor in [f.actor, actor_b] {
        f.service
            .assign_plugin(AssignPluginCommand {
                actor_user_id: actor,
                installation_id: old,
            })
            .await
            .unwrap();
    }
    f.switch(f.actor, new).await;
    f.assert_state(old, ACTIVE).await;
    assert!(f.runtime.retired.lock().unwrap().is_empty());
    f.switch(actor_b, new).await;
    f.assert_state(old, DISABLED).await;
    f.assert_state(new, ACTIVE).await;
    let family = f
        .service
        .list_families(
            actor_b,
            PluginCatalogFilter::default(),
            RequestedLocales::new("en_US", "en_US"),
        )
        .await
        .unwrap();
    assert_eq!(
        family.entries[0]
            .installed_versions
            .iter()
            .map(|v| v.installation_id)
            .collect::<std::collections::HashSet<_>>(),
        [old, new].into()
    );
    f.switch(f.actor, old).await;
    f.assert_state(old, ACTIVE).await;
    f.assert_state(new, ACTIVE).await;
    f.switch(actor_b, old).await;
    f.assert_state(new, DISABLED).await;
    assert_eq!(*f.runtime.retired.lock().unwrap(), [old, new]);
}

#[tokio::test]
async fn postgres_boot_retires_orphans_but_keeps_first_install_and_builtin() {
    let f = Fixture::new().await;
    let ghost = f.install("fixture_provider", "0.9.0").await;
    let orphan = f.install("fixture_provider", "1.0.0").await;
    let selected = f.install("fixture_provider", "2.0.0").await;
    let fresh = f.install("fresh_provider", "1.0.0").await;
    for id in [orphan, selected, fresh] {
        f.enable(id).await;
    }
    f.service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: f.actor,
            installation_id: selected,
        })
        .await
        .unwrap();
    sqlx::query("update extension_artifact_instances set runtime_status='active',availability_status='available' where installation_id=$1 and node_id=$2")
        .bind(ghost).bind(NODE).execute(f.store.pool()).await.unwrap();
    let builtin = f
        .service
        .ensure_builtin_plugin(EnsureBuiltinPluginCommand {
            actor_user_id: f.actor,
            package_root: concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../plugins/capability-plugins/1flowbase"
            )
            .into(),
        })
        .await
        .unwrap()
        .id;
    let projection = f
        .store
        .get_plugin_package_catalog_projection(ghost)
        .await
        .unwrap()
        .unwrap();
    f.runtime.activated.lock().unwrap().clear();
    f.service.reconcile_all_installations().await.unwrap();
    for id in [ghost, orphan] {
        f.assert_state(id, DISABLED).await;
    }
    for id in [builtin, selected] {
        f.assert_state(id, ACTIVE).await;
    }
    f.assert_state(
        fresh,
        (
            PluginDesiredState::ActiveRequested,
            PluginRuntimeStatus::Inactive,
            PluginAvailabilityStatus::InstallIncomplete,
        ),
    )
    .await;
    assert_eq!(*f.runtime.activated.lock().unwrap(), [selected]);
    let checked = f
        .store
        .get_artifact_instance(NODE, ghost)
        .await
        .unwrap()
        .unwrap()
        .checked_at;
    f.service.reconcile_all_installations().await.unwrap();
    assert_eq!(
        f.store
            .get_artifact_instance(NODE, ghost)
            .await
            .unwrap()
            .unwrap()
            .checked_at,
        checked
    );
    assert_eq!(
        f.store
            .get_plugin_package_catalog_projection(ghost)
            .await
            .unwrap()
            .unwrap(),
        projection
    );
}

#[tokio::test]
async fn postgres_boot_reactivates_provider_referenced_only_by_instance() {
    let f = Fixture::new().await;
    let pinned = f.install("fixture_provider", "1.0.0").await;
    let selected = f.install("fixture_provider", "2.0.0").await;
    for id in [pinned, selected] {
        f.enable(id).await;
    }
    f.service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: f.actor,
            installation_id: pinned,
        })
        .await
        .unwrap();
    ModelProviderRepository::create_instance(
        &f.store,
        &CreateModelProviderInstanceInput {
            instance_id: Uuid::now_v7(),
            workspace_id: f.workspace.id,
            installation_id: pinned,
            provider_code: "fixture_provider".into(),
            protocol: "openai_compatible".into(),
            display_name: "Pinned previous provider".into(),
            status: domain::ModelProviderInstanceStatus::Ready,
            config_json: serde_json::json!({}),
            configured_models: Vec::new(),
            enabled_model_ids: Vec::new(),
            included_in_main: Some(false),
            created_by: f.actor,
        },
    )
    .await
    .unwrap();
    f.service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: f.actor,
            installation_id: selected,
        })
        .await
        .unwrap();
    f.runtime.activated.lock().unwrap().clear();
    f.service.reconcile_all_installations().await.unwrap();
    f.assert_state(pinned, ACTIVE).await;
    assert!(f.runtime.activated.lock().unwrap().contains(&pinned));
}

#[tokio::test]
async fn postgres_reference_inventory_excludes_expired_previews() {
    let f = Fixture::new().await;
    let live = f.install("fixture_provider", "1.0.0").await;
    let expired = f.install("fixture_provider", "2.0.0").await;
    let current = f.install("fixture_provider", "3.0.0").await;
    for id in [live, expired, current] {
        f.enable(id).await;
    }
    let now = time::OffsetDateTime::now_utc();
    for (id, expires_at) in [
        (live, now + time::Duration::hours(1)),
        (expired, now - time::Duration::hours(1)),
    ] {
        f.store
            .create_assignment(&CreatePluginAssignmentInput {
                installation_id: id,
                workspace_id: f.workspace.id,
                provider_code: "fixture_provider".into(),
                actor_user_id: f.actor,
            })
            .await
            .unwrap();
        ModelProviderRepository::create_preview_session(
            &f.store,
            &CreateModelProviderPreviewSessionInput {
                session_id: Uuid::now_v7(),
                workspace_id: f.workspace.id,
                actor_user_id: f.actor,
                installation_id: Some(id),
                instance_id: None,
                config_fingerprint: "fixture".into(),
                models_json: serde_json::json!([]),
                expires_at,
            },
        )
        .await
        .unwrap();
    }
    f.store
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: current,
            workspace_id: f.workspace.id,
            provider_code: "fixture_provider".into(),
            actor_user_id: f.actor,
        })
        .await
        .unwrap();
    let ids = f.store.list_referenced_installation_ids().await.unwrap();
    assert!(ids.contains(&live));
    assert!(!ids.contains(&expired));
}

#[tokio::test]
async fn postgres_retirement_waits_for_a_concurrent_assignment_and_rejects_later_assignment() {
    let f = Fixture::new().await;
    let referenced = f.install("fixture_provider", "1.0.0").await;
    let unreferenced = f.install("fixture_provider", "2.0.0").await;
    for id in [referenced, unreferenced] {
        f.enable(id).await;
    }
    let workspace = f
        .store
        .upsert_workspace(f.workspace.tenant_id, "Retirement race")
        .await
        .unwrap()
        .id;
    let retire = |installation_id| UpdatePluginDesiredStateInput {
        installation_id,
        desired_state: PluginDesiredState::Disabled,
        actor_user_id: f.actor,
    };

    let mut writer = f.store.pool().begin().await.unwrap();
    // The FK's KEY SHARE is held by connection A until commit; do not take the advisory lock
    // here, so this test specifically verifies the installation row fence.
    sqlx::query("insert into plugin_assignments (id,installation_id,workspace_id,provider_code,assigned_by) values ($1,$2,$3,'fixture_provider',$4)")
        .bind(Uuid::now_v7()).bind(referenced).bind(workspace).bind(f.actor).execute(&mut *writer).await.unwrap();
    let input = retire(referenced);
    let mut retirement = std::pin::pin!(f.store.retire_unreferenced_installation(&input));
    // Drive connection B until PostgreSQL confirms it is blocked by A, not just slow.
    let writer_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *writer)
        .await
        .unwrap();
    let blocked = async {
        loop {
            let waiting: bool = sqlx::query_scalar("select exists (select 1 from pg_stat_activity where $1 = any(pg_blocking_pids(pid)) and query like 'select desired_state, category,%')")
                .bind(writer_pid).fetch_one(f.store.pool()).await.unwrap();
            if waiting {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    };
    tokio::select! {
        result = &mut retirement => panic!("retirement escaped the row lock: {result:?}"),
        result = tokio::time::timeout(std::time::Duration::from_secs(5), blocked) => result.expect("retirement must block on connection A"),
    }
    writer.commit().await.unwrap();
    assert!(retirement.await.unwrap().is_none());
    f.assert_state(referenced, ACTIVE).await;

    assert!(f
        .store
        .retire_unreferenced_installation(&retire(unreferenced))
        .await
        .unwrap()
        .is_some());
    let error = f
        .store
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: unreferenced,
            workspace_id: workspace,
            provider_code: "fixture_provider".into(),
            actor_user_id: f.actor,
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("plugin_installation_disabled"));
    assert_eq!(
        f.store.list_assignments(workspace).await.unwrap()[0].installation_id,
        referenced
    );
}

#[tokio::test]
async fn postgres_boot_preserves_first_install_when_disabled_sibling_retains_assignment() {
    let f = Fixture::new().await;
    let old = f.install("fixture_provider", "1.0.0").await;
    let fresh = f.install("fixture_provider", "2.0.0").await;
    for id in [old, fresh] {
        f.enable(id).await;
    }
    f.service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: f.actor,
            installation_id: old,
        })
        .await
        .unwrap();
    f.service
        .disable_plugin(DisablePluginCommand {
            actor_user_id: f.actor,
            installation_id: old,
        })
        .await
        .unwrap();
    f.service.reconcile_all_installations().await.unwrap();
    f.assert_state(old, DISABLED).await;
    f.assert_state(
        fresh,
        (
            PluginDesiredState::ActiveRequested,
            PluginRuntimeStatus::Inactive,
            PluginAvailabilityStatus::InstallIncomplete,
        ),
    )
    .await;
    assert_eq!(
        f.store.list_assignments(f.workspace.id).await.unwrap()[0].installation_id,
        old
    );
}

#[tokio::test]
async fn postgres_retirement_rejects_stale_provider_instance_and_preview_writes() {
    use control_plane_contracts::ports::{
        CreateModelProviderInstanceInput, CreateModelProviderPreviewSessionInput,
        ModelProviderRepository,
    };
    let f = Fixture::new().await;
    let serving = f.install("fixture_provider", "1.0.0").await;
    let retiring = f.install("fixture_provider", "2.0.0").await;
    for id in [serving, retiring] {
        f.enable(id).await;
    }
    f.service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: f.actor,
            installation_id: serving,
        })
        .await
        .unwrap();
    assert!(f
        .store
        .retire_unreferenced_installation(&UpdatePluginDesiredStateInput {
            installation_id: retiring,
            desired_state: PluginDesiredState::Disabled,
            actor_user_id: f.actor,
        })
        .await
        .unwrap()
        .is_some());

    let inst_err = ModelProviderRepository::create_instance(
        &f.store,
        &CreateModelProviderInstanceInput {
            instance_id: Uuid::now_v7(),
            workspace_id: f.workspace.id,
            installation_id: retiring,
            provider_code: "fixture_provider".into(),
            protocol: "openai_compatible".into(),
            display_name: "Stale Instance".into(),
            status: domain::ModelProviderInstanceStatus::Draft,
            config_json: serde_json::json!({}),
            configured_models: Vec::new(),
            enabled_model_ids: Vec::new(),
            included_in_main: None,
            created_by: f.actor,
        },
    )
    .await
    .unwrap_err();
    assert!(inst_err
        .to_string()
        .contains("plugin_installation_disabled"));

    let prev_err = ModelProviderRepository::create_preview_session(
        &f.store,
        &CreateModelProviderPreviewSessionInput {
            session_id: Uuid::now_v7(),
            workspace_id: f.workspace.id,
            actor_user_id: f.actor,
            installation_id: Some(retiring),
            instance_id: None,
            config_fingerprint: "fp".into(),
            models_json: serde_json::json!([]),
            expires_at: time::OffsetDateTime::now_utc() + time::Duration::minutes(5),
        },
    )
    .await
    .unwrap_err();
    assert!(prev_err
        .to_string()
        .contains("plugin_installation_disabled"));
}

#[tokio::test]
async fn postgres_retirement_aborts_when_serving_sibling_is_concurrently_disabled() {
    let f = Fixture::new().await;
    let serving = f.install("fixture_provider", "1.0.0").await;
    let fresh = f.install("fixture_provider", "2.0.0").await;
    for id in [serving, fresh] {
        f.enable(id).await;
    }
    f.service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: f.actor,
            installation_id: serving,
        })
        .await
        .unwrap();

    let mut disabler = f.store.pool().begin().await.unwrap();
    sqlx::query("select pg_advisory_xact_lock(hashtextextended('plugin-family:runtime-extensions:1flowbase-tests:fixture_provider', 0))")
        .execute(&mut *disabler)
        .await
        .unwrap();
    sqlx::query(
        "update extension_installations set desired_state='disabled', updated_at=now() where id=$1",
    )
    .bind(serving)
    .execute(&mut *disabler)
    .await
    .unwrap();
    let disabler_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *disabler)
        .await
        .unwrap();

    let retire_input = UpdatePluginDesiredStateInput {
        installation_id: fresh,
        desired_state: PluginDesiredState::Disabled,
        actor_user_id: f.actor,
    };
    let mut retirement = std::pin::pin!(f.store.retire_unreferenced_installation(&retire_input));
    let blocked = async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "select exists (select 1 from pg_stat_activity where $1 = any(pg_blocking_pids(pid)))",
            )
            .bind(disabler_pid)
            .fetch_one(f.store.pool())
            .await
            .unwrap();
            if waiting {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    };
    tokio::select! {
        res = &mut retirement => panic!("retirement did not wait on family lock: {res:?}"),
        res = tokio::time::timeout(std::time::Duration::from_secs(5), blocked) => res.expect("retirement must wait on family lock"),
    }
    disabler.commit().await.unwrap();
    assert!(retirement.await.unwrap().is_none());
    assert_eq!(
        f.store
            .get_installation(fresh)
            .await
            .unwrap()
            .unwrap()
            .desired_state,
        PluginDesiredState::ActiveRequested
    );
}

async fn seed_test_provider_instance(
    store: &storage_durable_postgres::PgControlPlaneStore,
    workspace_id: Uuid,
    actor_id: Uuid,
    installation_id: Uuid,
    name: &str,
) -> Uuid {
    use control_plane_contracts::ports::{
        CreateModelProviderInstanceInput, ModelProviderRepository,
    };
    let id = Uuid::now_v7();
    ModelProviderRepository::create_instance(
        store,
        &CreateModelProviderInstanceInput {
            instance_id: id,
            workspace_id,
            installation_id,
            provider_code: "fixture_provider".into(),
            protocol: "openai_compatible".into(),
            display_name: name.into(),
            status: domain::ModelProviderInstanceStatus::Ready,
            config_json: serde_json::json!({ "base_url": "https://api.example.com/v1" }),
            configured_models: Vec::new(),
            enabled_model_ids: Vec::new(),
            included_in_main: None,
            created_by: actor_id,
        },
    )
    .await
    .unwrap();
    id
}

#[tokio::test]
async fn postgres_instance_preview_rejects_migrated_or_retired_installation() {
    use control_plane_contracts::ports::{
        CommitProviderVersionSwitchInput, CreateModelProviderPreviewSessionInput,
        ModelProviderRepository,
    };
    let f = Fixture::new().await;
    let v1 = f.install("fixture_provider", "1.0.0").await;
    let v2 = f.install("fixture_provider", "2.0.0").await;
    for id in [v1, v2] {
        f.enable(id).await;
    }
    let second_ws = f
        .store
        .upsert_workspace(f.workspace.tenant_id, "Second Preview Workspace")
        .await
        .unwrap()
        .id;
    f.service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: f.actor,
            installation_id: v1,
        })
        .await
        .unwrap();
    f.store
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: v1,
            workspace_id: second_ws,
            provider_code: "fixture_provider".into(),
            actor_user_id: f.actor,
        })
        .await
        .unwrap();
    let instance_id =
        seed_test_provider_instance(&f.store, f.workspace.id, f.actor, v1, "Primary").await;

    // Switch workspace 1 from v1 -> v2 while second_ws keeps v1 active_requested.
    f.switch(f.actor, v2).await;
    f.assert_state(v1, ACTIVE).await;

    let mismatch_err = ModelProviderRepository::create_preview_session(
        &f.store,
        &CreateModelProviderPreviewSessionInput {
            session_id: Uuid::now_v7(),
            workspace_id: f.workspace.id,
            actor_user_id: f.actor,
            installation_id: Some(v1),
            instance_id: Some(instance_id),
            config_fingerprint: "stale-instance-binding".into(),
            models_json: serde_json::json!([]),
            expires_at: time::OffsetDateTime::now_utc() + time::Duration::minutes(5),
        },
    )
    .await
    .unwrap_err();
    assert!(mismatch_err
        .to_string()
        .contains("model_provider_instance_installation_changed"));

    // Now switch second_ws from v1 -> v2 so v1 retires to Disabled; stale preview must also fail.
    let target_artifact = f
        .store
        .get_artifact_instance(NODE, v2)
        .await
        .unwrap()
        .unwrap();
    let switched = f
        .store
        .commit_provider_version_switch(&CommitProviderVersionSwitchInput {
            validate_manifest: validate_installed_artifact_manifest,
            node_id: fixture::NODE.into(),
            workspace_id: second_ws,
            provider_code: "fixture_provider".into(),
            current_installation_id: v1,
            target_installation_id: v2,
            target_local_version: target_artifact.local_version,
            target_local_checksum: target_artifact.local_checksum,
            target_local_path: target_artifact.local_path.unwrap(),
            target_manifest_fingerprint: target_artifact.manifest_fingerprint,
            target_protocol: "openai_compatible".into(),
            model_discovery_mode: domain::ModelProviderDiscoveryMode::Hybrid,
            catalog_source: domain::ModelProviderCatalogSource::Hybrid,
            actor_user_id: f.actor,
        })
        .await
        .unwrap();
    assert!(switched.retired_previous.is_some());

    let retired_err = ModelProviderRepository::create_preview_session(
        &f.store,
        &CreateModelProviderPreviewSessionInput {
            session_id: Uuid::now_v7(),
            workspace_id: f.workspace.id,
            actor_user_id: f.actor,
            installation_id: Some(v1),
            instance_id: Some(instance_id),
            config_fingerprint: "retired-v1".into(),
            models_json: serde_json::json!([]),
            expires_at: time::OffsetDateTime::now_utc() + time::Duration::minutes(5),
        },
    )
    .await
    .unwrap_err();
    assert!(retired_err
        .to_string()
        .contains("plugin_installation_disabled"));
    assert!(!f
        .store
        .list_referenced_installation_ids()
        .await
        .unwrap()
        .contains(&v1));
}

#[tokio::test]
async fn postgres_concurrent_provider_switches_never_partially_migrate_into_retired_target() {
    use control_plane_contracts::ports::{
        CommitProviderVersionSwitchInput, ModelProviderRepository,
    };
    let f = Fixture::new().await;
    let v1 = f.install("fixture_provider", "1.0.0").await;
    let v2 = f.install("fixture_provider", "2.0.0").await;
    let v3 = f.install("fixture_provider", "3.0.0").await;
    for id in [v1, v2, v3] {
        f.enable(id).await;
    }
    let ws_b = f
        .store
        .upsert_workspace(f.workspace.tenant_id, "Workspace B")
        .await
        .unwrap()
        .id;
    f.service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: f.actor,
            installation_id: v1,
        })
        .await
        .unwrap();
    f.store
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: v2,
            workspace_id: ws_b,
            provider_code: "fixture_provider".into(),
            actor_user_id: f.actor,
        })
        .await
        .unwrap();
    let inst_a =
        seed_test_provider_instance(&f.store, f.workspace.id, f.actor, v1, "WS-A Instance").await;
    let _inst_b = seed_test_provider_instance(&f.store, ws_b, f.actor, v2, "WS-B Instance").await;

    let mut switcher_b = f.store.pool().begin().await.unwrap();
    sqlx::query("select pg_advisory_xact_lock(hashtextextended('plugin-family:runtime-extensions:1flowbase-tests:fixture_provider', 0))")
        .execute(&mut *switcher_b)
        .await
        .unwrap();
    sqlx::query("update model_provider_instances set installation_id=$2, updated_at=now() where workspace_id=$1 and provider_code='fixture_provider'")
        .bind(ws_b)
        .bind(v3)
        .execute(&mut *switcher_b)
        .await
        .unwrap();
    sqlx::query("update plugin_assignments set installation_id=$2 where workspace_id=$1 and provider_code='fixture_provider'")
        .bind(ws_b)
        .bind(v3)
        .execute(&mut *switcher_b)
        .await
        .unwrap();
    sqlx::query(
        "update extension_installations set desired_state='disabled', updated_at=now() where id=$1",
    )
    .bind(v2)
    .execute(&mut *switcher_b)
    .await
    .unwrap();
    let b_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *switcher_b)
        .await
        .unwrap();

    let target_artifact = f
        .store
        .get_artifact_instance(NODE, v2)
        .await
        .unwrap()
        .unwrap();
    let input_a = CommitProviderVersionSwitchInput {
        validate_manifest: validate_installed_artifact_manifest,
        node_id: fixture::NODE.into(),
        workspace_id: f.workspace.id,
        provider_code: "fixture_provider".into(),
        current_installation_id: v1,
        target_installation_id: v2,
        target_local_version: target_artifact.local_version,
        target_local_checksum: target_artifact.local_checksum,
        target_local_path: target_artifact.local_path.unwrap(),
        target_manifest_fingerprint: target_artifact.manifest_fingerprint,
        target_protocol: "openai_compatible".into(),
        model_discovery_mode: domain::ModelProviderDiscoveryMode::Hybrid,
        catalog_source: domain::ModelProviderCatalogSource::Hybrid,
        actor_user_id: f.actor,
    };
    let mut switch_a = std::pin::pin!(f.store.commit_provider_version_switch(&input_a));
    let blocked = async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "select exists (select 1 from pg_stat_activity where $1 = any(pg_blocking_pids(pid)))",
            )
            .bind(b_pid)
            .fetch_one(f.store.pool())
            .await
            .unwrap();
            if waiting {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    };
    tokio::select! {
        res = &mut switch_a => panic!("switch A escaped family lock: {res:?}"),
        res = tokio::time::timeout(std::time::Duration::from_secs(5), blocked) => res.expect("switch A must wait on switcher B"),
    }
    switcher_b.commit().await.unwrap();
    let err_a = switch_a.await.unwrap_err();
    assert!(err_a.to_string().contains("plugin_installation_disabled"));
    assert_eq!(
        f.store.list_assignments(f.workspace.id).await.unwrap()[0].installation_id,
        v1
    );
    assert_eq!(
        ModelProviderRepository::get_instance(&f.store, f.workspace.id, inst_a)
            .await
            .unwrap()
            .unwrap()
            .installation_id,
        v1,
        "failed switch must never leave provider instances pointing to retired target"
    );
}

#[tokio::test]
async fn postgres_switched_workspace_rejects_stale_writes_even_when_other_workspace_keeps_old_active(
) {
    use control_plane_contracts::ports::{
        ApplicationJsDependencySelectionRepository, CreateDataSourceInstanceInput,
        CreateModelProviderInstanceInput, CreateModelProviderPreviewSessionInput,
        DataSourceRepository, ModelProviderRepository,
        ReplaceApplicationJsDependencySelectionInput,
    };
    let f = Fixture::new().await;
    let v1 = f.install("fixture_provider", "1.0.0").await;
    let v2 = f.install("fixture_provider", "2.0.0").await;
    let unreferenced_v3 = f.install("fixture_provider", "3.0.0").await;
    for id in [v1, v2, unreferenced_v3] {
        f.enable(id).await;
    }
    let ws_b = f
        .store
        .upsert_workspace(f.workspace.tenant_id, "Workspace B Keeps V1 Active")
        .await
        .unwrap()
        .id;
    f.service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: f.actor,
            installation_id: v1,
        })
        .await
        .unwrap();
    f.store
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: v1,
            workspace_id: ws_b,
            provider_code: "fixture_provider".into(),
            actor_user_id: f.actor,
        })
        .await
        .unwrap();

    // Workspace A switches v1 -> v2; Workspace B still holds v1 active_requested.
    f.switch(f.actor, v2).await;
    f.assert_state(v1, ACTIVE).await;

    // Stale Workspace A create_instance & preview(None) targeting v1 must fail with plugin_assignment_required!
    let stale_inst = ModelProviderRepository::create_instance(
        &f.store,
        &CreateModelProviderInstanceInput {
            instance_id: Uuid::now_v7(),
            workspace_id: f.workspace.id,
            installation_id: v1,
            provider_code: "fixture_provider".into(),
            protocol: "openai_compatible".into(),
            display_name: "Stale V1 in WS-A".into(),
            status: domain::ModelProviderInstanceStatus::Draft,
            config_json: serde_json::json!({}),
            configured_models: Vec::new(),
            enabled_model_ids: Vec::new(),
            included_in_main: None,
            created_by: f.actor,
        },
    )
    .await
    .unwrap_err();
    assert!(stale_inst
        .to_string()
        .contains("plugin_assignment_required"));

    let stale_prev = ModelProviderRepository::create_preview_session(
        &f.store,
        &CreateModelProviderPreviewSessionInput {
            session_id: Uuid::now_v7(),
            workspace_id: f.workspace.id,
            actor_user_id: f.actor,
            installation_id: Some(v1),
            instance_id: None,
            config_fingerprint: "stale-ws-a-v1".into(),
            models_json: serde_json::json!([]),
            expires_at: time::OffsetDateTime::now_utc() + time::Duration::minutes(5),
        },
    )
    .await
    .unwrap_err();
    assert!(stale_prev
        .to_string()
        .contains("plugin_assignment_required"));

    // Retire v3 and prove data_source, js_dependency_selection, and reassign_instances also reject disabled targets.
    assert!(f
        .store
        .retire_unreferenced_installation(&UpdatePluginDesiredStateInput {
            installation_id: unreferenced_v3,
            desired_state: PluginDesiredState::Disabled,
            actor_user_id: f.actor,
        })
        .await
        .unwrap()
        .is_some());

    let ds_err = DataSourceRepository::create_instance(
        &f.store,
        &CreateDataSourceInstanceInput {
            instance_id: Uuid::now_v7(),
            workspace_id: f.workspace.id,
            installation_id: unreferenced_v3,
            source_code: "fixture_provider".into(),
            display_name: "Retired DS".into(),
            status: domain::DataSourceInstanceStatus::Draft,
            config_json: serde_json::json!({}),
            metadata_json: serde_json::json!({}),
            defaults: domain::DataSourceDefaults {
                data_model_status: domain::DataModelStatus::Draft,
            },
            created_by: f.actor,
        },
    )
    .await
    .unwrap_err();
    assert!(ds_err.to_string().contains("plugin_installation_disabled"));

    let js_err =
        ApplicationJsDependencySelectionRepository::replace_application_js_dependency_selection(
            &f.store,
            &ReplaceApplicationJsDependencySelectionInput {
                workspace_id: f.workspace.id,
                application_id: Uuid::now_v7(),
                installation_id: unreferenced_v3,
                provider_code: "fixture_provider".into(),
                plugin_id: "fixture_provider@3.0.0".into(),
                plugin_version: "3.0.0".into(),
                alias: "lodash".into(),
                package: "lodash".into(),
                version: "4.17.21".into(),
                target: "browser".into(),
                artifact_path: "dist/index.js".into(),
                artifact_hash: "00".repeat(32),
                integrity: "sha256-test".into(),
                permissions: domain::JsDependencyPermissions {
                    network: "deny".into(),
                    filesystem: "deny".into(),
                    env: "deny".into(),
                },
                actor_user_id: f.actor,
            },
        )
        .await
        .unwrap_err();
    assert!(js_err.to_string().contains("plugin_installation_disabled"));

    let target_artifact = f
        .store
        .get_artifact_instance(NODE, unreferenced_v3)
        .await
        .unwrap()
        .unwrap();
    let switch_disabled_err = f
        .store
        .commit_provider_version_switch(
            &control_plane_contracts::ports::CommitProviderVersionSwitchInput {
                validate_manifest: validate_installed_artifact_manifest,
                node_id: fixture::NODE.into(),
                workspace_id: f.workspace.id,
                provider_code: "fixture_provider".into(),
                current_installation_id: v2,
                target_installation_id: unreferenced_v3,
                target_local_version: target_artifact.local_version,
                target_local_checksum: target_artifact.local_checksum,
                target_local_path: target_artifact.local_path.unwrap(),
                target_manifest_fingerprint: target_artifact.manifest_fingerprint,
                target_protocol: "openai_compatible".into(),
                model_discovery_mode: domain::ModelProviderDiscoveryMode::Hybrid,
                catalog_source: domain::ModelProviderCatalogSource::Hybrid,
                actor_user_id: f.actor,
            },
        )
        .await
        .unwrap_err();
    assert!(switch_disabled_err
        .to_string()
        .contains("plugin_installation_disabled"));
}

#[tokio::test]
async fn postgres_delayed_post_switch_cleanup_never_clobbers_rolled_back_version() {
    use control_plane_contracts::ports::CommitProviderVersionSwitchInput;
    let f = Fixture::new().await;
    let v1 = f.install("fixture_provider", "1.0.0").await;
    let v2 = f.install("fixture_provider", "2.0.0").await;
    for id in [v1, v2] {
        f.enable(id).await;
    }
    f.service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: f.actor,
            installation_id: v1,
        })
        .await
        .unwrap();

    let target_artifact = f
        .store
        .get_artifact_instance(NODE, v2)
        .await
        .unwrap()
        .unwrap();
    // Simulate Switch 1 (v1 -> v2) committing DB state and pausing before its runtime cleanup runs.
    let outcome = f
        .store
        .commit_provider_version_switch(&CommitProviderVersionSwitchInput {
            validate_manifest: validate_installed_artifact_manifest,
            node_id: fixture::NODE.into(),
            workspace_id: f.workspace.id,
            provider_code: "fixture_provider".into(),
            current_installation_id: v1,
            target_installation_id: v2,
            target_local_version: target_artifact.local_version,
            target_local_checksum: target_artifact.local_checksum,
            target_local_path: target_artifact.local_path.unwrap(),
            target_manifest_fingerprint: target_artifact.manifest_fingerprint,
            target_protocol: "openai_compatible".into(),
            model_discovery_mode: domain::ModelProviderDiscoveryMode::Hybrid,
            catalog_source: domain::ModelProviderCatalogSource::Hybrid,
            actor_user_id: f.actor,
        })
        .await
        .unwrap();
    assert!(outcome.retired_previous.is_some());
    f.assert_state(v1, DISABLED).await;

    // Concurrent Switch 2 (v2 -> v1 rollback) runs to completion while Switch 1 is still paused.
    f.switch(f.actor, v1).await;
    f.assert_state(v1, ACTIVE).await;
    f.runtime.retired.lock().unwrap().clear();

    // Now Switch 1 wakes up and runs its delayed post-commit cleanup for v1: must be a no-op!
    let deactivated = f
        .service
        .deactivate_retired_installation_if_still_disabled(v1)
        .await
        .unwrap();
    assert!(!deactivated);
    assert!(!f.runtime.retired.lock().unwrap().contains(&v1));
    f.assert_state(v1, ACTIVE).await;
    let local_v1 = f
        .store
        .get_local_installation(fixture::NODE, v1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        local_v1.artifact.runtime_status,
        domain::PluginRuntimeStatus::Active
    );
}

#[tokio::test]
async fn postgres_switch_and_concurrent_instance_create_follow_lock_order_without_deadlock() {
    use control_plane_contracts::ports::{
        CommitProviderVersionSwitchInput, ModelProviderRepository,
    };
    let f = Fixture::new().await;
    let v1 = f.install("fixture_provider", "1.0.0").await;
    let v2 = f.install("fixture_provider", "2.0.0").await;
    for id in [v1, v2] {
        f.enable(id).await;
    }
    f.service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: f.actor,
            installation_id: v1,
        })
        .await
        .unwrap();

    // Connection A takes step 1 of create_instance (FOR SHARE on extension_installations(v1))
    // BEFORE locking plugin_assignments.
    let mut writer = f.store.pool().begin().await.unwrap();
    let _: (String, String) = sqlx::query_as(
        "select desired_state, source_kind from extension_installations where id=$1 for share",
    )
    .bind(v1)
    .fetch_one(&mut *writer)
    .await
    .unwrap();

    // Concurrent switch v1 -> v2 starts: because switch locks extension_installations BEFORE
    // plugin_assignments, it blocks on v1's row lock WITHOUT holding plugin_assignments!
    let store_for_switch = f.store.clone();
    let workspace_id = f.workspace.id;
    let actor = f.actor;
    let target_artifact = f
        .store
        .get_artifact_instance(NODE, v2)
        .await
        .unwrap()
        .unwrap();
    let switch_task = tokio::spawn(async move {
        store_for_switch
            .commit_provider_version_switch(&CommitProviderVersionSwitchInput {
                validate_manifest: validate_installed_artifact_manifest,
                node_id: fixture::NODE.into(),
                workspace_id,
                provider_code: "fixture_provider".into(),
                current_installation_id: v1,
                target_installation_id: v2,
                target_local_version: target_artifact.local_version,
                target_local_checksum: target_artifact.local_checksum,
                target_local_path: target_artifact.local_path.unwrap(),
                target_manifest_fingerprint: target_artifact.manifest_fingerprint,
                target_protocol: "openai_compatible".into(),
                model_discovery_mode: domain::ModelProviderDiscoveryMode::Hybrid,
                catalog_source: domain::ModelProviderCatalogSource::Hybrid,
                actor_user_id: actor,
            })
            .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(60)).await;
    assert!(!switch_task.is_finished());

    // Connection A now acquires FOR SHARE on plugin_assignments with zero deadlock, inserts its
    // instance, and commits; switch then wakes up, migrates the new instance to v2, and retires v1.
    let assigned: bool = sqlx::query_scalar(
        "select exists(select 1 from plugin_assignments where workspace_id=$1 and installation_id=$2 for share)",
    )
    .bind(f.workspace.id)
    .bind(v1)
    .fetch_one(&mut *writer)
    .await
    .unwrap();
    assert!(assigned);
    let inst_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into model_provider_instances (
            id, workspace_id, installation_id, provider_code, protocol,
            display_name, status, config_json, configured_models_json, enabled_model_ids,
            included_in_main, created_by, updated_by
        ) values ($1, $2, $3, 'fixture_provider', 'openai_compatible', 'Concurrent V1 Inst', 'draft', '{}'::jsonb, '[]'::jsonb, '{}'::text[], false, $4, $4)
        "#,
    )
    .bind(inst_id)
    .bind(f.workspace.id)
    .bind(v1)
    .bind(f.actor)
    .execute(&mut *writer)
    .await
    .unwrap();
    writer.commit().await.unwrap();

    let outcome = switch_task.await.unwrap().unwrap();
    assert_eq!(outcome.migrated_instance_count, 1);
    assert!(outcome.retired_previous.is_some());
    f.assert_state(v1, DISABLED).await;
    let migrated = ModelProviderRepository::get_instance(&f.store, f.workspace.id, inst_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(migrated.installation_id, v2);
}

#[tokio::test]
async fn postgres_concurrent_switch_during_enable_never_writes_ghost_active() {
    use control_plane_contracts::ports::CommitProviderVersionSwitchInput;
    let f = Fixture::new().await;
    let v1 = f.install("fixture_provider", "1.0.0").await;
    let v2 = f.install("fixture_provider", "2.0.0").await;
    f.enable(v2).await;

    // Seed v1 as Disabled in DB while assigned to workspace (simulating cross-node mid-enable window).
    f.enable(v1).await;
    f.service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: f.actor,
            installation_id: v1,
        })
        .await
        .unwrap();
    f.store
        .update_desired_state(&UpdatePluginDesiredStateInput {
            installation_id: v1,
            desired_state: PluginDesiredState::Disabled,
            actor_user_id: f.actor,
        })
        .await
        .unwrap();

    // Right inside Runtime::ensure_loaded(v1) (after enable_plugin set v1 ActiveRequested, before
    // enable_plugin writes runtime_status = Active), another node commits switch v1 -> v2!
    let store_other_node = f.store.clone();
    let workspace_id = f.workspace.id;
    let actor = f.actor;
    let target_artifact = f
        .store
        .get_artifact_instance(NODE, v2)
        .await
        .unwrap()
        .unwrap();
    *f.runtime.on_ensure_loaded.lock().unwrap() = Some((
        v1,
        Box::new(move |loaded_id| {
            assert_eq!(loaded_id, v1);
            Box::pin(async move {
                let outcome = store_other_node
                    .commit_provider_version_switch(&CommitProviderVersionSwitchInput {
                        validate_manifest: validate_installed_artifact_manifest,
                        node_id: fixture::NODE.into(),
                        workspace_id,
                        provider_code: "fixture_provider".into(),
                        current_installation_id: v1,
                        target_installation_id: v2,
                        target_local_version: target_artifact.local_version,
                        target_local_checksum: target_artifact.local_checksum,
                        target_local_path: target_artifact.local_path.unwrap(),
                        target_manifest_fingerprint: target_artifact.manifest_fingerprint,
                        target_protocol: "openai_compatible".into(),
                        model_discovery_mode: domain::ModelProviderDiscoveryMode::Hybrid,
                        catalog_source: domain::ModelProviderCatalogSource::Hybrid,
                        actor_user_id: actor,
                    })
                    .await
                    .unwrap();
                assert!(outcome.retired_previous.is_some());
            })
        }),
    ));
    f.runtime.retired.lock().unwrap().clear();

    let err = f
        .service
        .enable_plugin(EnablePluginCommand {
            actor_user_id: f.actor,
            installation_id: v1,
        })
        .await
        .unwrap_err();
    assert!(err.to_string().contains("plugin_installation_disabled"));
    assert!(f.runtime.retired.lock().unwrap().contains(&v1));
    f.assert_state(v1, DISABLED).await;
}

#[tokio::test]
async fn postgres_boot_activation_concurrent_retirement_never_writes_ghost_active() {
    let f = Fixture::new().await;
    let v1 = f.install("fixture_provider", "1.0.0").await;
    let v2 = f.install("fixture_provider", "2.0.0").await;
    for id in [v1, v2] {
        f.enable(id).await;
    }
    let ws_b = f
        .store
        .upsert_workspace(f.workspace.tenant_id, "Boot Retirement Race WS")
        .await
        .unwrap()
        .id;
    f.service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: f.actor,
            installation_id: v1,
        })
        .await
        .unwrap();
    f.store
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: v2,
            workspace_id: ws_b,
            provider_code: "fixture_provider".into(),
            actor_user_id: f.actor,
        })
        .await
        .unwrap();

    // Simulate another node switching Workspace A from v1 -> v2 (retiring v1) right while this
    // booting node is mid-way through `reconcile_all_installations` activating v1!
    let store_other = f.store.clone();
    let workspace_a = f.workspace.id;
    let actor = f.actor;
    let target_artifact = f
        .store
        .get_artifact_instance(NODE, v2)
        .await
        .unwrap()
        .unwrap();
    f.store
        .upsert_artifact_instance(&UpsertPluginArtifactInstanceInput {
            node_id: "other-node".into(),
            installation_id: v2,
            local_version: target_artifact.local_version.clone(),
            local_checksum: target_artifact.local_checksum.clone(),
            local_path: target_artifact.local_path.clone(),
            package_path: target_artifact.package_path.clone(),
            manifest_fingerprint: target_artifact.manifest_fingerprint.clone(),
            artifact_status: target_artifact.artifact_status,
            runtime_status: target_artifact.runtime_status,
            availability_status: target_artifact.availability_status,
            checked_at: target_artifact.checked_at,
            last_error: target_artifact.last_error.clone(),
            is_current: target_artifact.is_current,
        })
        .await
        .unwrap();
    *f.runtime.on_ensure_loaded.lock().unwrap() = Some((
        v1,
        Box::new(move |loaded_id| {
            if loaded_id == v1 {
                Box::pin(async move {
                    let outcome = store_other
                        .commit_provider_version_switch(
                            &control_plane_contracts::ports::CommitProviderVersionSwitchInput {
                                validate_manifest: validate_installed_artifact_manifest,
                                node_id: "other-node".into(),
                                workspace_id: workspace_a,
                                provider_code: "fixture_provider".into(),
                                current_installation_id: v1,
                                target_installation_id: v2,
                                target_local_version: target_artifact.local_version,
                                target_local_checksum: target_artifact.local_checksum,
                                target_local_path: target_artifact.local_path.unwrap(),
                                target_manifest_fingerprint: target_artifact.manifest_fingerprint,
                                target_protocol: "openai_compatible".into(),
                                model_discovery_mode: domain::ModelProviderDiscoveryMode::Hybrid,
                                catalog_source: domain::ModelProviderCatalogSource::Hybrid,
                                actor_user_id: actor,
                            },
                        )
                        .await
                        .unwrap();
                    assert!(outcome.retired_previous.is_some());
                })
            } else {
                Box::pin(async {})
            }
        }),
    ));
    f.runtime.retired.lock().unwrap().clear();

    f.service.reconcile_all_installations().await.unwrap();
    assert!(f.runtime.retired.lock().unwrap().contains(&v1));
    f.assert_state(v1, DISABLED).await;
    f.assert_state(v2, ACTIVE).await;
}

#[tokio::test]
async fn postgres_stale_artifact_refresh_preserves_live_enable_and_disable_transitions() {
    use control_plane::plugin_management::{
        refresh_current_node_plugin_artifact_instance, DisablePluginCommand,
    };
    let f = Fixture::new().await;
    let v1 = f.install("fixture_provider", "1.0.0").await;

    // Counterexample 1: caller holds stale Disabled snapshot of v1, then enable_plugin completes;
    // running refresh with the stale Disabled record must preserve Active, NOT clobber to Inactive!
    let stale_disabled = f.store.get_installation(v1).await.unwrap().unwrap();
    assert_eq!(stale_disabled.desired_state, PluginDesiredState::Disabled);
    f.enable(v1).await;
    f.assert_state(v1, ACTIVE).await;

    let refreshed_after_enable = refresh_current_node_plugin_artifact_instance(
        &f.store,
        fixture::NODE,
        &f.root.join("artifacts"),
        &stale_disabled,
    )
    .await
    .unwrap();
    assert_eq!(
        refreshed_after_enable.runtime_status,
        PluginRuntimeStatus::Active
    );
    assert_eq!(
        refreshed_after_enable.availability_status,
        PluginAvailabilityStatus::Available
    );
    f.assert_state(v1, ACTIVE).await;

    // Counterexample 2: caller holds stale ActiveRequested snapshot of v1, then disable_plugin
    // completes; running refresh with the stale ActiveRequested record must preserve Inactive!
    let stale_active = f.store.get_installation(v1).await.unwrap().unwrap();
    assert_eq!(
        stale_active.desired_state,
        PluginDesiredState::ActiveRequested
    );
    f.service
        .disable_plugin(DisablePluginCommand {
            actor_user_id: f.actor,
            installation_id: v1,
        })
        .await
        .unwrap();
    f.assert_state(v1, DISABLED).await;

    let refreshed_after_disable = refresh_current_node_plugin_artifact_instance(
        &f.store,
        fixture::NODE,
        &f.root.join("artifacts"),
        &stale_active,
    )
    .await
    .unwrap();
    assert_eq!(
        refreshed_after_disable.runtime_status,
        PluginRuntimeStatus::Inactive
    );
    assert_eq!(
        refreshed_after_disable.availability_status,
        PluginAvailabilityStatus::Disabled
    );
    f.assert_state(v1, DISABLED).await;
}
