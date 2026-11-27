use super::*;
use serde_json::json;
use std::time::Duration;
use storage_durable_postgres::PgControlPlaneStore;

async fn await_lock_wait(store: &PgControlPlaneStore, holder: i32, query: &str) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let waiter: Option<i32> = sqlx::query_scalar(
                "select l.pid from pg_locks l join pg_stat_activity a on a.pid=l.pid \
                 where not l.granted and $1=any(pg_blocking_pids(a.pid)) and a.query ilike $2 limit 1",
            )
            .bind(holder)
            .bind(query)
            .fetch_optional(store.pool())
            .await
            .unwrap();
            if let Some(waiter) = waiter {
                assert_ne!(waiter, holder);
                eprintln!("pg_locks: waiter PID {waiter} blocked by PID {holder} ({query})");
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("expected the repository write to wait on the uncommitted change");
}

async fn install_pair(f: &Fixture) -> (Uuid, Uuid) {
    let old = f.install("fixture_provider", "1.0.0").await;
    let new = f.install("fixture_provider", "2.0.0").await;
    f.enable(old).await;
    f.store
        .create_assignment(&CreatePluginAssignmentInput {
            workspace_id: f.workspace.id,
            installation_id: old,
            provider_code: "fixture_provider".into(),
            actor_user_id: f.actor,
        })
        .await
        .unwrap();
    (old, new)
}

#[tokio::test]
async fn postgres_data_source_create_instance_waits_on_in_flight_disable_or_switch_and_rejects_retired_version(
) {
    let f = Fixture::new().await;
    let (old, _) = install_pair(&f).await;
    let mut writer = f.store.pool().begin().await.unwrap();
    let holder: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *writer)
        .await
        .unwrap();
    sqlx::query("update extension_installations set desired_state='disabled' where id=$1")
        .bind(old)
        .execute(&mut *writer)
        .await
        .unwrap();
    let store = f.store.clone();
    let input = CreateDataSourceInstanceInput {
        instance_id: Uuid::now_v7(),
        workspace_id: f.workspace.id,
        installation_id: old,
        source_code: "fixture_source".into(),
        display_name: "Lost disable race".into(),
        status: domain::DataSourceInstanceStatus::Draft,
        config_json: json!({}),
        metadata_json: json!({}),
        defaults: domain::DataSourceDefaults::default(),
        created_by: f.actor,
    };
    let create =
        tokio::spawn(async move { DataSourceRepository::create_instance(&store, &input).await });
    await_lock_wait(&f.store, holder, "%extension_installations%for share%").await;
    writer.commit().await.unwrap();
    assert_conflict(
        create.await.unwrap().unwrap_err(),
        "plugin_installation_disabled",
    );
    let refs: i64 =
        sqlx::query_scalar("select count(*) from data_source_instances where installation_id=$1")
            .bind(old)
            .fetch_one(f.store.pool())
            .await
            .unwrap();
    assert_eq!(refs, 0);
}

#[tokio::test]
async fn postgres_js_dependency_selection_waits_on_in_flight_assignment_switch_and_rejects_unassigned_version(
) {
    let f = Fixture::new().await;
    let (old, new) = install_pair(&f).await;
    f.enable(new).await;
    let other = f
        .store
        .upsert_workspace(f.workspace.tenant_id, "Other JS workspace")
        .await
        .unwrap();
    f.store
        .create_assignment(&CreatePluginAssignmentInput {
            workspace_id: other.id,
            installation_id: old,
            provider_code: "fixture_provider".into(),
            actor_user_id: f.actor,
        })
        .await
        .unwrap();
    f.store
        .replace_installation_js_dependencies(&ReplaceInstallationJsDependenciesInput {
            installation_id: old,
            provider_code: "fixture_provider".into(),
            plugin_id: "fixture_provider@1.0.0".into(),
            plugin_version: "1.0.0".into(),
            entries: vec![JsDependencyRegistryInput {
                alias: "fixture-package".into(),
                package: "fixture-package".into(),
                version: "1.0.0".into(),
                target: "backend_code".into(),
                artifact_path: "artifacts/fixture.mjs".into(),
                integrity: "sha256-fixture".into(),
                permissions: domain::JsDependencyPermissions {
                    network: "deny".into(),
                    filesystem: "deny".into(),
                    env: "deny".into(),
                },
            }],
        })
        .await
        .unwrap();
    let entry = f
        .store
        .list_workspace_js_dependencies(f.workspace.id)
        .await
        .unwrap()
        .into_iter()
        .find(|entry| entry.installation_id == old)
        .unwrap();
    let app = f
        .store
        .create_application(&CreateApplicationInput {
            actor_user_id: f.actor,
            workspace_id: f.workspace.id,
            application_type: domain::ApplicationType::AgentFlow,
            name: "Lost assignment race".into(),
            description: String::new(),
            icon: None,
            icon_type: None,
            icon_background: None,
            workflow_trigger_type: None,
            workflow_trigger_config: None,
        })
        .await
        .unwrap();
    let mut writer = f.store.pool().begin().await.unwrap();
    let holder: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *writer)
        .await
        .unwrap();
    assert_eq!(sqlx::query("update plugin_assignments set installation_id=$1 where workspace_id=$2 and installation_id=$3")
        .bind(new).bind(f.workspace.id).bind(old).execute(&mut *writer).await.unwrap().rows_affected(), 1);
    let input = ReplaceApplicationJsDependencySelectionInput::from_catalog_entry(
        f.actor,
        f.workspace.id,
        app.id,
        entry,
    );
    let store = f.store.clone();
    let selection = tokio::spawn(async move {
        store
            .replace_application_js_dependency_selection(&input)
            .await
    });
    await_lock_wait(&f.store, holder, "%plugin_assignments%for share%").await;
    writer.commit().await.unwrap();
    assert_conflict(
        selection.await.unwrap().unwrap_err(),
        "plugin_assignment_required",
    );
    assert!(f
        .store
        .list_application_js_dependency_selections(f.workspace.id, app.id)
        .await
        .unwrap()
        .is_empty());
    f.assert_state(old, ACTIVE).await;
}

#[tokio::test]
async fn postgres_failed_switch_rolls_back_preenabled_orphan_when_local_repair_is_unavailable() {
    let f = Fixture::new().await;
    let (old, target) = install_pair(&f).await;
    f.enable(target).await;
    f.assert_state(target, ACTIVE).await;
    let artifact = f
        .store
        .get_artifact_instance(NODE, target)
        .await
        .unwrap()
        .unwrap();
    tokio::fs::remove_file(
        std::path::Path::new(artifact.local_path.as_ref().unwrap()).join("manifest.yaml"),
    )
    .await
    .unwrap();
    if let Some(package) = artifact.package_path {
        tokio::fs::remove_file(package).await.unwrap();
    }
    let error = try_switch(&f, target).await.unwrap_err();
    assert!(
        format!("{error:#}").contains("offline fixture"),
        "{error:#}"
    );
    f.assert_state(target, DISABLED).await;
    assert_eq!(
        f.store.list_assignments(f.workspace.id).await.unwrap()[0].installation_id,
        old
    );
    assert_eq!(*f.runtime.retired.lock().unwrap(), [target]);
}

#[tokio::test]
async fn postgres_failed_switch_does_not_unload_target_already_active_for_other_workspace() {
    let f = Fixture::new().await;
    let (old, target) = install_pair(&f).await;
    f.enable(target).await;
    let other = f
        .store
        .upsert_workspace(f.workspace.tenant_id, "Target owner")
        .await
        .unwrap();
    f.store
        .create_assignment(&CreatePluginAssignmentInput {
            workspace_id: other.id,
            installation_id: target,
            provider_code: "fixture_provider".into(),
            actor_user_id: f.actor,
        })
        .await
        .unwrap();
    f.assert_state(target, ACTIVE).await;
    let artifact = f
        .store
        .get_artifact_instance(NODE, target)
        .await
        .unwrap()
        .unwrap();
    let marker = std::path::Path::new(artifact.local_path.as_ref().unwrap())
        .join(".1flowbase-artifact.json");
    let original = tokio::fs::read(&marker).await.unwrap();
    let changed = marker.clone();
    *f.runtime.on_ensure_loaded.lock().unwrap() = Some((
        target,
        Box::new(move |_| {
            Box::pin(async move {
                tokio::fs::write(changed, "invalid marker").await.unwrap();
            })
        }),
    ));
    let result = try_switch(&f, target).await;
    tokio::fs::write(marker, original).await.unwrap();
    let error = result.expect_err("the mid-load artifact verification must reject the target");
    assert!(
        matches!(
            error.downcast_ref(),
            Some(control_plane::errors::ControlPlaneError::PluginUnavailable)
        ),
        "{error:#}"
    );
    assert!(!f.runtime.retired.lock().unwrap().contains(&target));
    f.assert_state(target, ACTIVE).await;
    assert_eq!(
        f.store.list_assignments(f.workspace.id).await.unwrap()[0].installation_id,
        old
    );
    assert_eq!(
        f.store.list_assignments(other.id).await.unwrap()[0].installation_id,
        target
    );
}

#[tokio::test]
async fn postgres_failed_switch_keeps_referenced_worker_when_node_projection_is_inactive() {
    assert_rejected_activation_keeps_referenced_worker(ActivationPath::Switch).await;
}

#[tokio::test]
async fn postgres_failed_enable_keeps_referenced_worker_when_node_projection_is_inactive() {
    assert_rejected_activation_keeps_referenced_worker(ActivationPath::Enable).await;
}

#[tokio::test]
async fn postgres_failed_reinstall_keeps_referenced_worker_when_node_projection_is_inactive() {
    assert_rejected_activation_keeps_referenced_worker(ActivationPath::Reinstall).await;
}

enum ActivationPath {
    Switch,
    Enable,
    Reinstall,
}

async fn assert_rejected_activation_keeps_referenced_worker(path: ActivationPath) {
    let f = Fixture::new().await;
    let (old, target) = install_pair(&f).await;
    f.enable(target).await;
    let other = f
        .store
        .upsert_workspace(f.workspace.tenant_id, "Live target owner")
        .await
        .unwrap();
    f.store
        .create_assignment(&CreatePluginAssignmentInput {
            workspace_id: other.id,
            installation_id: target,
            provider_code: "fixture_provider".into(),
            actor_user_id: f.actor,
        })
        .await
        .unwrap();
    let installation = f.store.get_installation(target).await.unwrap().unwrap();
    let artifact = f
        .store
        .get_artifact_instance(NODE, target)
        .await
        .unwrap()
        .unwrap();
    let marker = std::path::Path::new(artifact.local_path.as_ref().unwrap())
        .join(".1flowbase-artifact.json");
    let original = tokio::fs::read(&marker).await.unwrap();
    tokio::fs::remove_file(&marker).await.unwrap();
    let scanned = refresh_current_node_plugin_artifact_instance(
        &f.store,
        NODE,
        &f.root.join("artifacts"),
        &installation,
    )
    .await
    .unwrap();
    assert_eq!(scanned.runtime_status, PluginRuntimeStatus::Inactive);
    assert!(f.runtime.retired.lock().unwrap().is_empty());
    tokio::fs::write(&marker, original).await.unwrap();
    *f.runtime.on_ensure_loaded.lock().unwrap() = Some((
        target,
        Box::new(move |_| {
            Box::pin(async move {
                tokio::fs::write(marker, "invalid marker").await.unwrap();
            })
        }),
    ));
    let error = match path {
        ActivationPath::Switch => try_switch(&f, target).await.unwrap_err(),
        ActivationPath::Enable => f
            .service
            .enable_plugin(EnablePluginCommand {
                actor_user_id: f.actor,
                installation_id: target,
            })
            .await
            .unwrap_err(),
        ActivationPath::Reinstall => f
            .service
            .install_plugin(InstallPluginCommand {
                actor_user_id: f.actor,
                package_root: f
                    .root
                    .join("packages/fixture_provider/2.0.0")
                    .display()
                    .to_string(),
            })
            .await
            .unwrap_err(),
    };
    assert!(
        matches!(
            error.downcast_ref(),
            Some(control_plane::errors::ControlPlaneError::PluginUnavailable)
        ),
        "{error:#}"
    );
    assert!(
        f.runtime.on_ensure_loaded.lock().unwrap().is_none(),
        "activation hook must run"
    );
    assert!(
        f.runtime.retired.lock().unwrap().is_empty(),
        "the shared worker must not be unloaded"
    );
    assert_eq!(
        f.store.list_assignments(f.workspace.id).await.unwrap()[0].installation_id,
        old
    );
    assert_eq!(
        f.store.list_assignments(other.id).await.unwrap()[0].installation_id,
        target
    );
    assert_eq!(
        f.store
            .get_installation(target)
            .await
            .unwrap()
            .unwrap()
            .desired_state,
        PluginDesiredState::ActiveRequested
    );
}

#[tokio::test]
async fn postgres_managed_batch_waits_for_node_slot_then_rejects_missing_marker() {
    let f = Fixture::new().await;
    let install = super::managed::seed_managed_version(
        &f,
        "1.0.0",
        ACTIVE,
        json!({"binding_targets":["workspace"]}),
    )
    .await;
    f.store
        .create_assignment(&CreatePluginAssignmentInput {
            workspace_id: f.workspace.id,
            installation_id: install.id,
            provider_code: install.provider_code,
            actor_user_id: f.actor,
        })
        .await
        .unwrap();
    let artifact = f
        .store
        .get_artifact_instance(NODE, install.id)
        .await
        .unwrap()
        .unwrap();
    let mut writer = f.store.pool().begin().await.unwrap();
    let holder: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *writer)
        .await
        .unwrap();
    sqlx::query("select pg_advisory_xact_lock(hashtextextended('node-artifact:' || $1 || ':' || $2::text, 201406))")
        .bind(NODE).bind(install.id.to_string()).execute(&mut *writer).await.unwrap();
    let store = f.store.clone();
    let workspace = f.workspace.id;
    let batch = tokio::spawn(async move {
        store
            .lock_contribution_authority_batch(
                &[workspace],
                &[],
                false,
                Some(ManagedArtifactVerification {
                    node_id: NODE,
                    validate_manifest: validate_installed_managed_manifest,
                }),
            )
            .await
    });
    await_lock_wait(
        &f.store,
        holder,
        "%pg_advisory_xact_lock_shared%node-artifact:%",
    )
    .await;
    tokio::fs::remove_file(
        std::path::Path::new(artifact.local_path.as_ref().unwrap())
            .join(".1flowbase-artifact.json"),
    )
    .await
    .unwrap();
    writer.commit().await.unwrap();
    let error = batch
        .await
        .unwrap()
        .err()
        .expect("shared authority accepted missing marker");
    assert_conflict(error, "managed_candidate_artifact_invalid");
}

async fn try_switch(f: &Fixture, target: Uuid) -> anyhow::Result<domain::PluginTaskRecord> {
    f.service
        .switch_version(SwitchPluginVersionCommand {
            actor_user_id: f.actor,
            provider_code: "fixture_provider".into(),
            target_installation_id: target,
        })
        .await
}

fn assert_conflict(error: anyhow::Error, reason: &str) {
    assert!(
        matches!(error.downcast_ref(), Some(control_plane_contracts::ControlPlaneContractError::Conflict(actual)) if *actual == reason),
        "{error:#}"
    );
}

#[tokio::test]
async fn postgres_notification_create_instance_waits_on_in_flight_switch_and_rejects_retired_version(
) {
    let f = Fixture::new().await;
    let workspace_id = f.workspace.id;
    let old = managed::seed_managed_version(
        &f,
        "1.0.0",
        ACTIVE,
        json!({"binding_targets":["workspace"]}),
    )
    .await;
    let new = managed::seed_managed_version(
        &f,
        "2.0.0",
        DISABLED,
        json!({"binding_targets":["workspace"]}),
    )
    .await;
    f.store
        .create_assignment(&CreatePluginAssignmentInput {
            workspace_id,
            installation_id: old.id,
            provider_code: old.provider_code.clone(),
            actor_user_id: f.actor,
        })
        .await
        .unwrap();
    // Provision the candidate's workspace authority revision without selecting it yet.
    sqlx::query("insert into plugin_contribution_authorization_revisions(installation_id, workspace_id) values ($1, $2)")
        .bind(new.id).bind(workspace_id).execute(f.store.pool()).await.unwrap();
    let lease = f
        .store
        .lock_managed_installation_switch(&ManagedInstallationSwitch {
            validate_manifest: control_plane_test_support::validate_installed_managed_manifest,
            workspace_id,
            current_installation_id: old.id,
            target_installation_id: new.id,
            node_id: NODE.into(),
            installation_ids: vec![old.id, new.id],
        })
        .await
        .unwrap();
    // The lease owns the connection; find its PID from the granted workspace lock.
    let holder: i32 = sqlx::query_scalar(
        "select pid from pg_locks where locktype='advisory' and granted and objsubid=1 \
         and ((classid::bigint << 32) | objid::bigint) = hashtextextended('managed-workspace:' || $1::text, 0)",
    ).bind(workspace_id.to_string()).fetch_one(f.store.pool()).await.unwrap();
    let now = time::OffsetDateTime::now_utc();
    let store = f.store.clone();
    let create = tokio::spawn(async move {
        NotificationRepository::save_notification_instance(
            &store,
            SaveNotificationInstanceInput {
                record: domain::NotificationInstanceRecord {
                    id: Uuid::now_v7(),
                    workspace_id,
                    type_code: "fixture/channel".into(),
                    installation_id: Some(old.id),
                    display_name: "Stale channel".into(),
                    status: domain::NotificationInstanceStatus::Enabled,
                    config_json: json!({}),
                    configured_secret_fields: Vec::new(),
                    created_at: now,
                    updated_at: now,
                },
                secret_config_json: json!({}),
                master_key: "fixture".into(),
                create: true,
                initial_authorization: None,
            },
        )
        .await
    });
    await_lock_wait(
        &f.store,
        holder,
        "%pg_advisory_xact_lock%managed-workspace%",
    )
    .await;
    lease
        .commit_with_catalog(
            domain::AuditLogRecord {
                id: Uuid::now_v7(),
                workspace_id: Some(workspace_id),
                actor_user_id: Some(f.actor),
                target_type: "plugin_assignment".into(),
                target_id: Some(new.id),
                event_code: "plugin.version_switched".into(),
                payload: json!({}),
                created_at: now,
            },
            None,
            vec!["fixture/channel".into()],
            None,
        )
        .await
        .unwrap();
    assert_conflict(
        tokio::time::timeout(Duration::from_secs(5), create)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err(),
        "type_unavailable",
    );
    let persisted: bool = sqlx::query_scalar(
        "select exists(select 1 from notification_instances where workspace_id=$1 and installation_id=$2)",
    ).bind(workspace_id).bind(old.id).fetch_one(f.store.pool()).await.unwrap();
    assert!(!persisted);
    assert_eq!(
        f.store.list_assignments(workspace_id).await.unwrap()[0].installation_id,
        new.id
    );
}

#[tokio::test]
async fn postgres_bound_preview_session_waits_on_in_flight_provider_switch_and_rejects_migrated_instance(
) {
    let f = Fixture::new().await;
    let (old, new) = install_pair(&f).await;
    f.enable(new).await;
    let workspace = f.workspace.id;
    let other = f
        .store
        .upsert_workspace(f.workspace.tenant_id, "Old provider owner")
        .await
        .unwrap();
    // Keep the old installation enabled so the waiter must compare the bound instance.
    f.store
        .create_assignment(&CreatePluginAssignmentInput {
            workspace_id: other.id,
            installation_id: old,
            provider_code: "fixture_provider".into(),
            actor_user_id: f.actor,
        })
        .await
        .unwrap();
    let instance =
        seed_test_provider_instance(&f.store, workspace, f.actor, old, "Migrating provider").await;
    let mut writer = f.store.pool().begin().await.unwrap();
    let holder: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *writer)
        .await
        .unwrap();
    // Reproduce the switch transaction's order: family, global/workspace fences,
    // sorted installations, workspace assignment, then the instance mutation.
    sqlx::query("select pg_advisory_xact_lock(hashtextextended('plugin-family:' || category || ':' || organization || ':' || artifact_id, 0)) from extension_installations where id=$1")
        .bind(old).execute(&mut *writer).await.unwrap();
    for scope in [domain::SYSTEM_SCOPE_ID, workspace] {
        sqlx::query(
            "select pg_advisory_xact_lock(hashtextextended('managed-workspace:' || $1::text, 0))",
        )
        .bind(scope.to_string())
        .execute(&mut *writer)
        .await
        .unwrap();
    }
    sqlx::query("select id from extension_installations where id=any($1) order by id for update")
        .bind([old, new].as_slice())
        .fetch_all(&mut *writer)
        .await
        .unwrap();
    sqlx::query(
        "select id from plugin_assignments where workspace_id=$1 and installation_id=$2 for update",
    )
    .bind(workspace)
    .bind(old)
    .fetch_one(&mut *writer)
    .await
    .unwrap();
    sqlx::query("update model_provider_instances set installation_id=$2 where id=$1")
        .bind(instance)
        .bind(new)
        .execute(&mut *writer)
        .await
        .unwrap();
    sqlx::query("update plugin_assignments set installation_id=$3 where workspace_id=$1 and installation_id=$2")
        .bind(workspace).bind(old).bind(new).execute(&mut *writer).await.unwrap();

    let store = f.store.clone();
    let actor = f.actor;
    let session_id = Uuid::now_v7();
    let create = tokio::spawn(async move {
        ModelProviderRepository::create_preview_session(
            &store,
            &CreateModelProviderPreviewSessionInput {
                session_id,
                workspace_id: workspace,
                actor_user_id: actor,
                installation_id: Some(old),
                instance_id: Some(instance),
                config_fingerprint: "stale-instance-preview".into(),
                models_json: json!([]),
                expires_at: time::OffsetDateTime::now_utc() + time::Duration::hours(1),
            },
        )
        .await
    });
    await_lock_wait(&f.store, holder, "%extension_installations%for share%").await;
    writer.commit().await.unwrap();
    assert_conflict(
        tokio::time::timeout(Duration::from_secs(5), create)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err(),
        "model_provider_instance_installation_changed",
    );
    let persisted: bool = sqlx::query_scalar(
        "select exists(select 1 from model_provider_preview_sessions where id=$1 or (instance_id=$2 and installation_id=$3))",
    ).bind(session_id).bind(instance).bind(old).fetch_one(f.store.pool()).await.unwrap();
    assert!(!persisted);
}
