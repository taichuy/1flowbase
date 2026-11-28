use super::*;
use control_plane::plugin_lifecycle::reconcile_installation_snapshot;
use domain::PluginArtifactInstanceStatus::Missing;
use std::{path::PathBuf, sync::Arc, time::Duration};
use storage_durable_postgres::PgControlPlaneStore;

async fn local(f: &Fixture, id: Uuid) -> domain::LocalPluginInstallationRecord {
    f.store
        .get_local_installation(NODE, id)
        .await
        .unwrap()
        .unwrap()
}

fn remove_artifact(local: &domain::LocalPluginInstallationRecord) {
    std::fs::remove_dir_all(PathBuf::from(local.artifact.local_path.as_ref().unwrap())).unwrap();
}

async fn invalidate(f: &Fixture, id: Uuid) {
    let snapshot = local(f, id).await;
    // Remove the installation marker so the scan records Missing instead of falling
    // back to an older sibling's directory and recording Outdated.
    let path = PathBuf::from(snapshot.artifact.local_path.as_ref().unwrap());
    std::fs::remove_file(path.join(".1flowbase-artifact.json")).unwrap();
    let artifact = refresh_current_node_plugin_artifact_instance(
        &f.store,
        NODE,
        &f.root.join("artifacts"),
        &snapshot.installation,
    )
    .await
    .unwrap();
    assert_eq!(artifact.artifact_status, Missing);
    assert_eq!(artifact.runtime_status, PluginRuntimeStatus::Inactive);
}

async fn wait_for_blocked_query(store: &PgControlPlaneStore, blocker: i32, query_match: &str) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "select exists(select 1 from pg_stat_activity where $1 = any(pg_blocking_pids(pid)) and query ilike $2)",
            )
            .bind(blocker)
            .bind(query_match)
            .fetch_one(store.pool())
            .await
            .unwrap();
            if waiting { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("expected transaction did not wait for the fixture lock");
}

#[tokio::test]
async fn postgres_reconcile_installation_snapshot_never_writes_stale_active_over_concurrent_disable(
) {
    let f = Fixture::new().await;
    let v1 = f.install("fixture_provider", "1.0.0").await;
    f.enable(v1).await;
    f.assert_state(v1, ACTIVE).await;

    let mut disable = f.store.pool().begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *disable)
        .await
        .unwrap();
    sqlx::query("select id from extension_installations where id = $1 for update")
        .bind(v1)
        .execute(&mut *disable)
        .await
        .unwrap();
    sqlx::query("select installation_id from extension_artifact_instances where node_id = $1 and installation_id = $2 for update")
        .bind(NODE).bind(v1).execute(&mut *disable).await.unwrap();
    remove_artifact(&local(&f, v1).await);

    let store = f.store.clone();
    let refresh =
        tokio::spawn(async move { reconcile_installation_snapshot(&store, NODE, v1).await });
    // The old upsert waits on the artifact row; the transactional refresh waits on the installation.
    wait_for_blocked_query(&f.store, pid, "%extension_%").await;
    sqlx::query("update extension_installations set desired_state = 'disabled' where id = $1")
        .bind(v1)
        .execute(&mut *disable)
        .await
        .unwrap();
    sqlx::query("update extension_artifact_instances set runtime_status = 'inactive', availability_status = 'disabled' where node_id = $1 and installation_id = $2")
        .bind(NODE).bind(v1).execute(&mut *disable).await.unwrap();
    disable.commit().await.unwrap();

    let snapshot = refresh.await.unwrap().unwrap();
    assert_eq!(snapshot.artifact.artifact_status, Missing);
    assert_eq!(
        snapshot.artifact.runtime_status,
        PluginRuntimeStatus::Inactive
    );
    assert_eq!(
        snapshot.artifact.availability_status,
        PluginAvailabilityStatus::Disabled
    );
    f.assert_state(v1, DISABLED).await;
}

#[tokio::test]
async fn postgres_concurrent_artifact_corruption_during_activation_aborts_active_commit_and_deactivates_runtime(
) {
    let f = Arc::new(Fixture::new().await);
    let v1 = f.install("fixture_provider", "1.0.0").await;
    let weak = Arc::downgrade(&f);
    *f.runtime.on_ensure_loaded.lock().unwrap() = Some((
        v1,
        Box::new(move |id| {
            Box::pin(async move { invalidate(weak.upgrade().unwrap().as_ref(), id).await })
        }),
    ));

    let error = f
        .service
        .enable_plugin(EnablePluginCommand {
            actor_user_id: f.actor,
            installation_id: v1,
        })
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref(),
        Some(control_plane_contracts::ControlPlaneContractError::PluginUnavailable)
    ));
    assert!(f.runtime.retired.lock().unwrap().contains(&v1));
    let artifact = local(&f, v1).await.artifact;
    assert_eq!(artifact.artifact_status, Missing);
    assert_eq!(artifact.runtime_status, PluginRuntimeStatus::Inactive);
    assert_eq!(
        artifact.availability_status,
        PluginAvailabilityStatus::ArtifactMissing
    );
}

#[tokio::test]
async fn postgres_concurrent_target_artifact_invalidation_aborts_provider_switch_without_partial_migration(
) {
    let f = Arc::new(Fixture::new().await);
    let v1 = f.install("fixture_provider", "1.0.0").await;
    let v2 = f.install("fixture_provider", "2.0.0").await;
    f.enable(v1).await;
    f.assert_state(v2, DISABLED).await;
    f.service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: f.actor,
            installation_id: v1,
        })
        .await
        .unwrap();
    let instance =
        seed_test_provider_instance(&f.store, f.workspace.id, f.actor, v1, "Stays on V1").await;
    let old = local(&f, v1).await;
    let mut hold = f.store.pool().begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *hold)
        .await
        .unwrap();
    let (loaded, loading) = tokio::sync::oneshot::channel();
    let (resume, resumed) = tokio::sync::oneshot::channel();
    *f.runtime.on_ensure_loaded.lock().unwrap() = Some((
        v2,
        Box::new(move |_| {
            Box::pin(async move {
                loaded.send(()).unwrap();
                resumed.await.unwrap();
            })
        }),
    ));

    let switcher = Arc::clone(&f);
    let switch = tokio::spawn(async move {
        switcher
            .service
            .switch_version(SwitchPluginVersionCommand {
                actor_user_id: switcher.actor,
                provider_code: "fixture_provider".into(),
                target_installation_id: v2,
            })
            .await
    });
    // Enabling also takes the family advisory lock; acquire ours only after its desired-state write.
    tokio::time::timeout(Duration::from_secs(10), loading)
        .await
        .unwrap()
        .unwrap();
    sqlx::query("select pg_advisory_xact_lock(hashtextextended('plugin-family:' || $1 || ':' || $2 || ':' || $3, 0))")
        .bind(old.category.as_str()).bind(&old.organization).bind(&old.provider_code)
        .execute(&mut *hold).await.unwrap();

    resume.send(()).unwrap();
    wait_for_blocked_query(&f.store, pid, "%pg_advisory_xact_lock%plugin-family:%").await;
    f.assert_state(v2, ACTIVE).await;
    invalidate(&f, v2).await;
    hold.commit().await.unwrap();

    let error = switch.await.unwrap().unwrap_err();
    assert!(matches!(
        error.downcast_ref(),
        Some(control_plane_contracts::ControlPlaneContractError::PluginUnavailable)
    ));
    let assignments: Vec<Uuid> = sqlx::query_scalar("select installation_id from plugin_assignments where workspace_id = $1 and provider_code = 'fixture_provider'")
        .bind(f.workspace.id).fetch_all(f.store.pool()).await.unwrap();
    assert_eq!(assignments, vec![v1]);
    let persisted = ModelProviderRepository::get_instance(&f.store, f.workspace.id, instance)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(persisted.installation_id, v1);
    f.assert_state(v1, ACTIVE).await;
    let target = local(&f, v2).await;
    assert_eq!(target.desired_state, PluginDesiredState::Disabled);
    assert_eq!(target.artifact.artifact_status, Missing);
    assert_eq!(
        target.artifact.runtime_status,
        PluginRuntimeStatus::Inactive
    );
    assert_ne!(
        target.artifact.availability_status,
        PluginAvailabilityStatus::Available
    );
    assert!(f.runtime.retired.lock().unwrap().contains(&v2));
}

#[tokio::test]
async fn postgres_js_dependency_selection_rejects_unassigned_version_kept_active_by_other_workspace(
) {
    let f = Fixture::new().await;
    let v1 = f.install("fixture_provider", "1.0.0").await;
    let v2 = f.install("fixture_provider", "2.0.0").await;
    f.enable(v1).await;
    let ws_b = f
        .store
        .upsert_workspace(f.workspace.tenant_id, "Workspace B")
        .await
        .unwrap();
    for workspace_id in [f.workspace.id, ws_b.id] {
        f.store
            .create_assignment(&CreatePluginAssignmentInput {
                workspace_id,
                installation_id: v1,
                provider_code: "fixture_provider".into(),
                actor_user_id: f.actor,
            })
            .await
            .unwrap();
    }
    f.store
        .replace_installation_js_dependencies(&ReplaceInstallationJsDependenciesInput {
            installation_id: v1,
            provider_code: "fixture_provider".into(),
            plugin_id: "fixture_provider@1.0.0".into(),
            plugin_version: "1.0.0".into(),
            entries: vec![JsDependencyRegistryInput {
                alias: "zod".into(),
                package: "zod".into(),
                version: "3.24.0".into(),
                target: "backend_code".into(),
                artifact_path: "artifacts/zod.backend.mjs".into(),
                integrity: "sha256-test".into(),
                permissions: domain::JsDependencyPermissions {
                    network: "deny".into(),
                    filesystem: "deny".into(),
                    env: "deny".into(),
                },
            }],
        })
        .await
        .unwrap();
    let application = f
        .store
        .create_application(&CreateApplicationInput {
            actor_user_id: f.actor,
            workspace_id: f.workspace.id,
            application_type: domain::ApplicationType::AgentFlow,
            name: "Stale JS catalog selection".into(),
            description: String::new(),
            icon: None,
            icon_type: None,
            icon_background: None,
            workflow_trigger_type: None,
            workflow_trigger_config: None,
        })
        .await
        .unwrap();
    let entry = f
        .store
        .list_workspace_js_dependencies(f.workspace.id)
        .await
        .unwrap()
        .into_iter()
        .find(|entry| entry.installation_id == v1)
        .expect("V1 catalog entry was visible to workspace A");
    f.switch(f.actor, v2).await;
    f.assert_state(v1, ACTIVE).await;
    assert!(!f
        .store
        .list_workspace_js_dependencies(f.workspace.id)
        .await
        .unwrap()
        .iter()
        .any(|entry| entry.installation_id == v1));
    assert!(f
        .store
        .list_workspace_js_dependencies(ws_b.id)
        .await
        .unwrap()
        .iter()
        .any(|entry| entry.installation_id == v1));

    let error = f
        .store
        .replace_application_js_dependency_selection(
            &ReplaceApplicationJsDependencySelectionInput::from_catalog_entry(
                f.actor,
                f.workspace.id,
                application.id,
                entry,
            ),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref(),
        Some(
            control_plane_contracts::ControlPlaneContractError::Conflict(
                "plugin_assignment_required"
            )
        )
    ));
    assert!(f
        .store
        .list_application_js_dependency_selections(f.workspace.id, application.id)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn postgres_failed_switch_assignment_conflict_retires_healthy_target_to_disabled_and_inactive(
) {
    assert_failed_switch_cleanup(false, false).await;
}

#[tokio::test]
async fn postgres_failed_switch_retires_preenabled_orphan() {
    assert_failed_switch_cleanup(true, false).await;
}

#[tokio::test]
async fn postgres_failed_switch_preserves_preenabled_target_referenced_by_another_workspace() {
    assert_failed_switch_cleanup(true, true).await;
}

async fn assert_failed_switch_cleanup(preenabled: bool, referenced: bool) {
    let f = Arc::new(Fixture::new().await);
    let v1 = f.install("fixture_provider", "1.0.0").await;
    let v2 = f.install("fixture_provider", "2.0.0").await;
    let v3 = f.install("fixture_provider", "3.0.0").await;
    f.enable(v1).await;
    f.enable(v3).await;
    if preenabled {
        f.enable(v2).await;
        f.assert_state(v2, ACTIVE).await;
    } else {
        f.assert_state(v2, DISABLED).await;
    }
    if referenced {
        let other = f
            .store
            .upsert_workspace(f.workspace.tenant_id, "Other")
            .await
            .unwrap();
        sqlx::query("insert into plugin_assignments (id,installation_id,workspace_id,provider_code,assigned_by) values ($1,$2,$3,'fixture_provider',$4)")
            .bind(Uuid::now_v7()).bind(v2).bind(other.id).bind(f.actor)
            .execute(f.store.pool()).await.unwrap();
    }
    f.service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: f.actor,
            installation_id: v1,
        })
        .await
        .unwrap();
    let (loaded, loading) = tokio::sync::oneshot::channel();
    let (resume, resumed) = tokio::sync::oneshot::channel();
    *f.runtime.on_ensure_loaded.lock().unwrap() = Some((
        v2,
        Box::new(move |_| {
            Box::pin(async move {
                loaded.send(()).unwrap();
                resumed.await.unwrap();
            })
        }),
    ));
    let switcher = Arc::clone(&f);
    let switch = tokio::spawn(async move {
        switcher
            .service
            .switch_version(SwitchPluginVersionCommand {
                actor_user_id: switcher.actor,
                provider_code: "fixture_provider".into(),
                target_installation_id: v2,
            })
            .await
    });
    // Enable's desired-state write also takes the family lock, so wait for it first.
    tokio::time::timeout(Duration::from_secs(10), loading)
        .await
        .unwrap()
        .unwrap();
    let mut reassign = f.store.pool().begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *reassign)
        .await
        .unwrap();
    let current = local(&f, v1).await;
    sqlx::query("select pg_advisory_xact_lock(hashtextextended('plugin-family:' || $1 || ':' || $2 || ':' || $3, 0))")
        .bind(current.category.as_str()).bind(&current.organization).bind(&current.provider_code)
        .execute(&mut *reassign).await.unwrap();
    resume.send(()).unwrap();
    wait_for_blocked_query(&f.store, pid, "%pg_advisory_xact_lock%plugin-family:%").await;
    f.assert_state(v2, ACTIVE).await;
    assert_eq!(
        local(&f, v2).await.artifact.artifact_status,
        domain::PluginArtifactInstanceStatus::Ready
    );
    // A different node is not reached by the service's current-node cleanup.
    sqlx::query(
        "insert into extension_artifact_instances (node_id, installation_id, artifact_status, runtime_status, availability_status, checked_at, is_current) values ('other-node', $1, 'ready', 'active', 'available', now(), true)",
    )
    .bind(v2)
    .execute(&mut *reassign)
    .await
    .unwrap();
    let changed = sqlx::query("update plugin_assignments set installation_id = $1 where workspace_id = $2 and installation_id = $3")
        .bind(v3).bind(f.workspace.id).bind(v1).execute(&mut *reassign).await.unwrap();
    assert_eq!(changed.rows_affected(), 1);
    reassign.commit().await.unwrap();

    let error = switch.await.unwrap().unwrap_err();
    assert!(
        matches!(
            error.downcast_ref(),
            Some(
                control_plane_contracts::ControlPlaneContractError::Conflict(
                    "plugin_assignment_changed"
                )
            )
        ),
        "{error:#}"
    );
    assert_eq!(f.runtime.retired.lock().unwrap().contains(&v2), !referenced);
    f.assert_state(v2, if referenced { ACTIVE } else { DISABLED })
        .await;
    assert_eq!(
        local(&f, v2).await.artifact.artifact_status,
        domain::PluginArtifactInstanceStatus::Ready
    );
    let other_node = f
        .store
        .get_artifact_instance("other-node", v2)
        .await
        .unwrap()
        .unwrap();
    let expected = if referenced { ACTIVE } else { DISABLED };
    assert_eq!(other_node.runtime_status, expected.1);
    assert_eq!(other_node.availability_status, expected.2);
}

#[tokio::test]
async fn postgres_record_runtime_status_preserves_non_ready_artifact_statuses() {
    use domain::PluginArtifactInstanceStatus::{
        Corrupted, LoadFailed, Mismatched, Outdated, Ready,
    };

    let f = Fixture::new().await;
    let v1 = f.install("fixture_provider", "1.0.0").await;
    f.enable(v1).await;
    let record_status = |runtime_status| {
        let store = &f.store;
        async move {
            store
                .record_artifact_runtime_status(&RecordPluginRuntimeStatusInput {
                    node_id: NODE.into(),
                    installation_id: v1,
                    runtime_status,
                    last_error: None,
                    checked_at: time::OffsetDateTime::now_utc(),
                })
                .await
                .unwrap()
        }
    };
    for contract in ["1flowbase.provider/v2", "1flowbase.extension-bus/v1"] {
        // Only the contract version discriminates this repository operation.
        sqlx::query("update extension_installations set contract_version = $2 where id = $1")
            .bind(v1)
            .bind(contract)
            .execute(f.store.pool())
            .await
            .unwrap();
        for (artifact, availability) in [
            (Missing, PluginAvailabilityStatus::ArtifactMissing),
            (Corrupted, PluginAvailabilityStatus::InstallIncomplete),
            (Outdated, PluginAvailabilityStatus::ArtifactMissing),
            (Mismatched, PluginAvailabilityStatus::InstallIncomplete),
            (LoadFailed, PluginAvailabilityStatus::LoadFailed),
        ] {
            sqlx::query("update extension_artifact_instances set artifact_status = $3 where node_id = $1 and installation_id = $2")
                .bind(NODE).bind(v1).bind(artifact.as_str()).execute(f.store.pool()).await.unwrap();
            let failed = record_status(PluginRuntimeStatus::LoadFailed).await;
            assert_eq!(failed.artifact_status, artifact, "{contract}");
            assert_eq!(
                failed.availability_status, availability,
                "{contract}: {artifact:?}"
            );
            let inactive = record_status(PluginRuntimeStatus::Inactive).await;
            assert_eq!(inactive.artifact_status, artifact, "{contract}");
            assert_eq!(inactive.runtime_status, PluginRuntimeStatus::Inactive);
        }
        sqlx::query("update extension_artifact_instances set artifact_status = 'ready' where node_id = $1 and installation_id = $2")
            .bind(NODE).bind(v1).execute(f.store.pool()).await.unwrap();
        let failed_status = if contract == "1flowbase.provider/v2" {
            LoadFailed
        } else {
            Ready
        };
        let failed = record_status(PluginRuntimeStatus::LoadFailed).await;
        assert_eq!(failed.artifact_status, failed_status);
        assert_eq!(
            failed.availability_status,
            PluginAvailabilityStatus::LoadFailed
        );
        assert_eq!(
            record_status(PluginRuntimeStatus::Inactive)
                .await
                .artifact_status,
            failed_status
        );

        for (runtime, expected) in [
            (PluginRuntimeStatus::Inactive, Missing),
            (
                PluginRuntimeStatus::LoadFailed,
                if failed_status == LoadFailed {
                    LoadFailed
                } else {
                    Missing
                },
            ),
        ] {
            sqlx::query("delete from extension_artifact_instances where node_id = $1 and installation_id = $2")
                .bind(NODE).bind(v1).execute(f.store.pool()).await.unwrap();
            assert_eq!(
                record_status(runtime).await.artifact_status,
                expected,
                "{contract}"
            );
            assert_eq!(local(&f, v1).await.artifact.artifact_status, expected);
        }
    }
}

#[tokio::test]
async fn postgres_preview_session_with_existing_instance_rejects_workspace_reassigned_away_from_version(
) {
    let f = Fixture::new().await;
    let v1 = f.install("fixture_provider", "1.0.0").await;
    let v2 = f.install("fixture_provider", "2.0.0").await;
    f.enable(v1).await;
    f.enable(v2).await;
    let workspace_b = f
        .store
        .upsert_workspace(f.workspace.tenant_id, "Workspace B")
        .await
        .unwrap();
    for workspace_id in [f.workspace.id, workspace_b.id] {
        f.store
            .create_assignment(&CreatePluginAssignmentInput {
                workspace_id,
                installation_id: v1,
                provider_code: "fixture_provider".into(),
                actor_user_id: f.actor,
            })
            .await
            .unwrap();
    }
    let instance_id =
        seed_test_provider_instance(&f.store, f.workspace.id, f.actor, v1, "Pinned to V1").await;
    f.service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: f.actor,
            installation_id: v2,
        })
        .await
        .unwrap();
    f.assert_state(v1, ACTIVE).await;
    let instance = ModelProviderRepository::get_instance(&f.store, f.workspace.id, instance_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(instance.installation_id, v1);
    let session_id = Uuid::now_v7();
    let error = ModelProviderRepository::create_preview_session(
        &f.store,
        &CreateModelProviderPreviewSessionInput {
            session_id,
            workspace_id: f.workspace.id,
            actor_user_id: f.actor,
            installation_id: Some(v1),
            instance_id: Some(instance_id),
            config_fingerprint: "stale-workspace-assignment".into(),
            models_json: serde_json::json!([]),
            expires_at: time::OffsetDateTime::now_utc() + time::Duration::hours(1),
        },
    )
    .await
    .unwrap_err();
    assert!(
        matches!(
            error.downcast_ref(),
            Some(
                control_plane_contracts::ControlPlaneContractError::Conflict(
                    "plugin_assignment_required"
                )
            )
        ),
        "{error:#}"
    );
    let persisted: bool = sqlx::query_scalar(
        "select exists(select 1 from model_provider_preview_sessions where id = $1)",
    )
    .bind(session_id)
    .fetch_one(f.store.pool())
    .await
    .unwrap();
    assert!(!persisted);
}

#[tokio::test]
async fn postgres_first_insert_refresh_never_overwrites_concurrent_insert_and_activation() {
    let f = Fixture::new().await;
    let v1 = f.install("fixture_provider", "1.0.0").await;
    f.enable(v1).await;
    let ready = local(&f, v1).await.artifact;
    let input = RefreshPluginArtifactInstanceInput {
        node_id: NODE.into(),
        installation_id: v1,
        observed_artifact: None,
        touch_checked_at: false,
        local_version: ready.local_version,
        local_checksum: ready.local_checksum,
        local_path: ready.local_path,
        manifest_fingerprint: ready.manifest_fingerprint,
        artifact_status: ready.artifact_status,
        last_error: None,
        reset_runtime_status: false,
        checked_at: time::OffsetDateTime::now_utc(),
    };
    for (take_slot_lock, record_runtime) in [(false, false), (true, false), (false, true)] {
        sqlx::query(
            "delete from extension_artifact_instances where node_id = $1 and installation_id = $2",
        )
        .bind(NODE)
        .bind(v1)
        .execute(f.store.pool())
        .await
        .unwrap();
        let mut writer = f.store.pool().begin().await.unwrap();
        let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
            .fetch_one(&mut *writer)
            .await
            .unwrap();
        if take_slot_lock {
            sqlx::query("select pg_advisory_xact_lock(hashtextextended('node-artifact:' || $1 || ':' || $2::text, 201406))")
                .bind(NODE).bind(v1.to_string()).execute(&mut *writer).await.unwrap();
        }
        // Without the advisory lock this row is invisible to the refresh's SELECT,
        // but its unique key blocks the INSERT until the writer commits the activation.
        sqlx::query("insert into extension_artifact_instances (node_id, installation_id, local_version, local_checksum, local_path, manifest_fingerprint, artifact_status, runtime_status, availability_status, is_current, checked_at) values ($1, $2, $3, $4, $5, $6, 'ready', 'inactive', 'install_incomplete', true, now())")
            .bind(NODE).bind(v1).bind(&input.local_version).bind(&input.local_checksum)
            .bind(&input.local_path).bind(&input.manifest_fingerprint).execute(&mut *writer).await.unwrap();
        let store = f.store.clone();
        let refresh_input = input.clone();
        let refresh = tokio::spawn(async move {
            if record_runtime {
                store
                    .record_artifact_runtime_status(&RecordPluginRuntimeStatusInput {
                        node_id: refresh_input.node_id,
                        installation_id: v1,
                        runtime_status: PluginRuntimeStatus::Inactive,
                        last_error: None,
                        checked_at: refresh_input.checked_at,
                    })
                    .await
                    .map(Some)
            } else {
                store.refresh_artifact_instance(&refresh_input).await
            }
        });
        let blocked_query = if take_slot_lock {
            "%pg_advisory_xact_lock%node-artifact:%"
        } else {
            "%insert into extension_artifact_instances%"
        };
        wait_for_blocked_query(&f.store, pid, blocked_query).await;
        sqlx::query("update extension_artifact_instances set runtime_status = 'active', availability_status = 'available' where node_id = $1 and installation_id = $2")
            .bind(NODE).bind(v1).execute(&mut *writer).await.unwrap();
        writer.commit().await.unwrap();
        let result = refresh.await.unwrap().unwrap();
        let refreshed = if record_runtime {
            result.unwrap()
        } else {
            assert!(
                result.is_none(),
                "a scan that observed no row must not refresh a newly inserted row"
            );
            let observed = local(&f, v1).await.artifact;
            let fresh_input = RefreshPluginArtifactInstanceInput {
                observed_artifact: Some(observed.clone()),
                ..input.clone()
            };
            let refreshed = f
                .store
                .refresh_artifact_instance(&fresh_input)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                refreshed, observed,
                "unchanged reconciliation must not write"
            );
            let touched = f
                .store
                .refresh_artifact_instance(&RefreshPluginArtifactInstanceInput {
                    touch_checked_at: true,
                    checked_at: observed.checked_at,
                    ..fresh_input
                })
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                touched.checked_at,
                observed.checked_at + time::Duration::microseconds(1)
            );
            touched
        };
        assert_eq!(refreshed.runtime_status, PluginRuntimeStatus::Active);
        assert_eq!(
            refreshed.availability_status,
            PluginAvailabilityStatus::Available
        );
        f.assert_state(v1, ACTIVE).await;
    }
}

#[tokio::test]
async fn postgres_family_uninstall_and_batch_authority_snapshot_lock_workspaces_in_same_order_without_deadlock(
) {
    let f = Fixture::new().await;
    let other = f
        .store
        .upsert_workspace(f.workspace.tenant_id, "Other")
        .await
        .unwrap();
    let (low, high) = (f.workspace.id.min(other.id), f.workspace.id.max(other.id));
    let mut ids = Vec::new();
    for version in ["1.0.0", "2.0.0"] {
        ids.push(
            super::managed::seed_managed_version(
                &f,
                version,
                DISABLED,
                serde_json::json!({"binding_targets": ["workspace"]}),
            )
            .await
            .id,
        );
    }
    ids.sort_unstable();
    let scopes = [(ids[0], high), (ids[1], low)];
    for (id, workspace_id) in scopes {
        f.store
            .create_assignment(&CreatePluginAssignmentInput {
                installation_id: id,
                workspace_id,
                provider_code: "workspace_worker".into(),
                actor_user_id: f.actor,
            })
            .await
            .unwrap();
    }
    let mut blocker = f.store.pool().begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let lock =
        "select pg_advisory_xact_lock(hashtextextended('managed-workspace:' || $1::uuid::text, 0))";
    sqlx::query(lock)
        .bind(low)
        .execute(&mut *blocker)
        .await
        .unwrap();
    let now = time::OffsetDateTime::now_utc();
    let input = CommitPluginFamilyUninstallInput {
        artifact_instances: ids
            .into_iter()
            .map(|installation_id| UpsertPluginArtifactInstanceInput {
                node_id: NODE.into(),
                installation_id,
                local_version: None,
                local_checksum: None,
                local_path: None,
                package_path: None,
                manifest_fingerprint: None,
                artifact_status: Missing,
                runtime_status: PluginRuntimeStatus::Inactive,
                availability_status: PluginAvailabilityStatus::Disabled,
                checked_at: now,
                last_error: None,
                is_current: false,
            })
            .collect(),
        artifact_cleanups: Vec::new(),
        audit_log: domain::AuditLogRecord {
            id: Uuid::now_v7(),
            workspace_id: None,
            actor_user_id: Some(f.actor),
            target_type: "plugin_installation".into(),
            target_id: Some(scopes[0].0),
            event_code: "plugin.family_uninstalled".into(),
            payload: serde_json::json!({}),
            created_at: now,
        },
    };
    let store = f.store.clone();
    let uninstall = tokio::spawn(async move { store.commit_plugin_family_uninstall(&input).await });
    wait_for_blocked_query(&f.store, pid, "%pg_advisory_xact_lock%managed-workspace:%").await;
    let mut probe = f.store.pool().begin().await.unwrap();
    let high_is_free: bool = sqlx::query_scalar(
        "select pg_try_advisory_xact_lock(hashtextextended('managed-workspace:' || $1::uuid::text, 0))",
    ).bind(high).fetch_one(&mut *probe).await.unwrap();
    probe.rollback().await.unwrap();

    let store = f.store.clone();
    let batch = tokio::spawn(async move {
        let lease = store
            .lock_contribution_authority_batch(&[], &scopes, false, None)
            .await?;
        drop(lease);
        anyhow::Ok(())
    });
    // Keep low locked until both transactions have queued, rather than depending on timing.
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let waiting: i64 = sqlx::query_scalar("select count(*) from pg_stat_activity where $1 = any(pg_blocking_pids(pid)) and query ilike '%pg_advisory_xact_lock%managed-workspace:%'")
                .bind(pid).fetch_one(f.store.pool()).await.unwrap();
            if waiting >= 2 { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    blocker.commit().await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        uninstall.await.unwrap().unwrap();
        batch.await.unwrap().unwrap();
    })
    .await
    .unwrap();
    assert!(
        high_is_free,
        "uninstall locked the higher workspace before the lower one"
    );
}

#[tokio::test]
async fn postgres_failed_switch_rolls_back_target_disabled_before_lock_even_without_serving_sibling(
) {
    use control_plane_contracts::ControlPlaneContractError as Error;

    for fail_activation in [false, true] {
        let f = Arc::new(Fixture::new().await);
        let v1 = f.install("fixture_provider", "1.0.0").await;
        let v2 = f.install("fixture_provider", "2.0.0").await;
        f.enable(v1).await;
        f.enable(v2).await;
        f.service
            .assign_plugin(AssignPluginCommand {
                actor_user_id: f.actor,
                installation_id: v1,
            })
            .await
            .unwrap();
        f.assert_state(v2, ACTIVE).await;
        let lock_owner = v1.min(v2);
        let (entered, entering) = tokio::sync::oneshot::channel();
        let (release, released) = tokio::sync::oneshot::channel();
        *f.runtime.on_ensure_loaded.lock().unwrap() = Some((
            lock_owner,
            Box::new(move |_| {
                Box::pin(async move {
                    entered.send(()).unwrap();
                    released.await.unwrap();
                })
            }),
        ));
        // The real enable path holds the lifecycle lock while its runtime is paused.
        let holder = Arc::clone(&f);
        let enable = tokio::spawn(async move {
            holder
                .service
                .enable_plugin(EnablePluginCommand {
                    actor_user_id: holder.actor,
                    installation_id: lock_owner,
                })
                .await
        });
        tokio::time::timeout(Duration::from_secs(10), entering)
            .await
            .unwrap()
            .unwrap();
        let switcher = Arc::clone(&f);
        let switch = tokio::spawn(async move {
            switcher
                .service
                .switch_version(SwitchPluginVersionCommand {
                    actor_user_id: switcher.actor,
                    provider_code: "fixture_provider".into(),
                    target_installation_id: v2,
                })
                .await
        });
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let running: bool = sqlx::query_scalar("select exists(select 1 from plugin_tasks where installation_id=$1 and task_kind='switch_version' and status='running')")
                    .bind(v2).fetch_one(f.store.pool()).await.unwrap();
                if running { break; }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.expect("switch must have read the active target before waiting for the lifecycle lock");
        assert!(!switch.is_finished());
        let mut change = f.store.pool().begin().await.unwrap();
        sqlx::query("update extension_installations set desired_state='disabled', updated_at=now() where id=any($1)")
            .bind(&[v1, v2][..]).execute(&mut *change).await.unwrap();
        sqlx::query("update extension_artifact_instances set runtime_status='inactive', availability_status='disabled', checked_at=checked_at + interval '1 microsecond' where installation_id=any($1)")
            .bind(&[v1, v2][..]).execute(&mut *change).await.unwrap();
        if !fail_activation {
            assert_eq!(
                sqlx::query(
                    "delete from plugin_assignments where workspace_id=$1 and installation_id=$2"
                )
                .bind(f.workspace.id)
                .bind(v1)
                .execute(&mut *change)
                .await
                .unwrap()
                .rows_affected(),
                1
            );
        }
        change.commit().await.unwrap();
        if fail_activation {
            let weak = Arc::downgrade(&f);
            *f.runtime.on_ensure_loaded.lock().unwrap() = Some((
                v2,
                Box::new(move |id| {
                    Box::pin(async move {
                        let f = weak.upgrade().unwrap();
                        assert_eq!(
                            local(&f, id).await.desired_state,
                            PluginDesiredState::ActiveRequested
                        );
                        invalidate(&f, id).await;
                    })
                }),
            ));
        }
        f.runtime.activated.lock().unwrap().clear();
        f.runtime.retired.lock().unwrap().clear();
        release.send(()).unwrap();
        let error = tokio::time::timeout(Duration::from_secs(10), async {
            assert!(
                enable.await.unwrap().is_err(),
                "concurrent disable must win over the paused enable"
            );
            switch.await.unwrap().unwrap_err()
        })
        .await
        .unwrap();
        assert!(
            if fail_activation {
                matches!(error.downcast_ref(), Some(Error::PluginUnavailable))
            } else {
                matches!(
                    error.downcast_ref(),
                    Some(Error::Conflict("plugin_assignment_changed"))
                )
            },
            "unexpected switch error: {error:#}"
        );
        assert!(f.runtime.activated.lock().unwrap().contains(&v2));
        assert!(f.runtime.retired.lock().unwrap().contains(&v2));
        f.assert_state(v1, DISABLED).await;
        f.assert_state(v2, DISABLED).await;
    }
}

#[tokio::test]
async fn postgres_stale_corrupted_scan_never_overwrites_concurrent_repair_and_activation() {
    let f = Fixture::new().await;
    let v1 = f.install("fixture_provider", "1.0.0").await;
    f.enable(v1).await;
    let before = local(&f, v1).await.artifact;
    let installed = PathBuf::from(before.local_path.as_ref().unwrap());
    let moved = f.root.join("parked-artifact");
    std::fs::rename(&installed, &moved).unwrap();

    let mut repair = f.store.pool().begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *repair)
        .await
        .unwrap();
    sqlx::query("select pg_advisory_xact_lock(hashtextextended('node-artifact:' || $1 || ':' || $2::text, 201406))")
        .bind(NODE).bind(v1.to_string()).execute(&mut *repair).await.unwrap();
    let store = f.store.clone();
    let scan = tokio::spawn(async move { reconcile_installation_snapshot(&store, NODE, v1).await });
    wait_for_blocked_query(&f.store, pid, "%pg_advisory_xact_lock%node-artifact:%").await;
    std::fs::rename(&moved, &installed).unwrap();
    // Repair returns to identical content and runtime state: only the row clock can distinguish it.
    let repaired_at: time::OffsetDateTime = sqlx::query_scalar("update extension_artifact_instances set artifact_status='ready', runtime_status='active', availability_status='available', checked_at=checked_at + interval '1 microsecond' where node_id=$1 and installation_id=$2 returning checked_at")
        .bind(NODE).bind(v1).fetch_one(&mut *repair).await.unwrap();
    repair.commit().await.unwrap();
    let snapshot = tokio::time::timeout(Duration::from_secs(10), scan)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let expected = domain::PluginArtifactInstanceRecord {
        checked_at: repaired_at,
        ..before
    };
    assert_eq!(snapshot.artifact, expected);
    assert_eq!(local(&f, v1).await.artifact, expected);
    f.assert_state(v1, ACTIVE).await;
}

#[tokio::test]
async fn postgres_runtime_refresh_demotes_active_plugin_when_manifest_fingerprint_changes_on_disk()
{
    let f = Fixture::new().await;
    let v1 = f.install("fixture_provider", "1.0.0").await;
    f.enable(v1).await;
    let initial = local(&f, v1).await;
    let path = PathBuf::from(initial.artifact.local_path.as_ref().unwrap());
    let manifest_path = path.join("manifest.yaml");
    let marker_path = path.join(".1flowbase-artifact.json");
    let original_manifest = std::fs::read_to_string(&manifest_path).unwrap();
    let original_marker = std::fs::read(&marker_path).unwrap();
    for update_marker in [false, true] {
        let active = local(&f, v1).await;
        assert_eq!(
            active.artifact.artifact_status,
            domain::PluginArtifactInstanceStatus::Ready
        );
        assert_eq!(active.artifact.runtime_status, PluginRuntimeStatus::Active);
        assert_eq!(
            active.artifact.availability_status,
            PluginAvailabilityStatus::Available
        );
        std::fs::write(
            &manifest_path,
            format!("{original_manifest}\n# manifest edited {update_marker}\n"),
        )
        .unwrap();
        let fingerprint = plugin_framework::compute_manifest_fingerprint(&manifest_path)
            .await
            .unwrap();
        assert_ne!(
            active.artifact.manifest_fingerprint.as_ref(),
            Some(&fingerprint)
        );
        if update_marker {
            let mut marker: serde_json::Value = serde_json::from_slice(&original_marker).unwrap();
            marker["manifest_fingerprint"] = serde_json::json!(&fingerprint);
            std::fs::write(&marker_path, serde_json::to_vec(&marker).unwrap()).unwrap();
        }
        let refreshed = refresh_current_node_plugin_artifact_instance(
            &f.store,
            NODE,
            &f.root.join("artifacts"),
            &active.installation,
        )
        .await
        .unwrap();
        assert_eq!(
            refreshed.artifact_status,
            domain::PluginArtifactInstanceStatus::Ready
        );
        assert_eq!(
            refreshed.last_error.as_deref(),
            (!update_marker).then_some("manifest_fingerprint_mismatch")
        );
        assert_eq!(refreshed.manifest_fingerprint.as_ref(), Some(&fingerprint));
        assert_eq!(refreshed.runtime_status, PluginRuntimeStatus::Inactive);
        assert_eq!(
            refreshed.availability_status,
            PluginAvailabilityStatus::InstallIncomplete
        );
        if !update_marker {
            std::fs::write(&manifest_path, &original_manifest).unwrap();
            f.enable(v1).await;
        }
    }
}

async fn install_artifact_with_warning(
    f: &Fixture,
    version: &str,
    warning: &str,
) -> (Uuid, domain::PluginArtifactInstanceRecord) {
    let target = f.install("fixture_provider", version).await;
    sqlx::query("update extension_installations set expected_checksum = 'expected' where id = $1")
        .bind(target)
        .execute(f.store.pool())
        .await
        .unwrap();
    let current = local(f, target).await;
    let marker =
        PathBuf::from(current.artifact.local_path.unwrap()).join(".1flowbase-artifact.json");
    let mut fields: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&marker).unwrap()).unwrap();
    match warning {
        "manifest_fingerprint_mismatch" => {
            fields["checksum"] = "expected".into();
            fields["manifest_fingerprint"] = "stale".into();
        }
        "checksum_mismatch" => fields["checksum"] = "wrong".into(),
        "checksum_missing" => {
            fields.as_object_mut().unwrap().remove("checksum");
        }
        _ => unreachable!(),
    }
    std::fs::write(marker, serde_json::to_vec(&fields).unwrap()).unwrap();
    let artifact = refresh_current_node_plugin_artifact_instance(
        &f.store,
        NODE,
        &f.root.join("artifacts"),
        &current.installation,
    )
    .await
    .unwrap();
    (target, artifact)
}

#[tokio::test]
async fn postgres_runtime_reconcile_preserves_unrepaired_checksum_warning() {
    let f = Fixture::new().await;
    for (version, warning) in [
        ("1.0.0", "checksum_missing"),
        ("2.0.0", "checksum_mismatch"),
    ] {
        let (id, before) = install_artifact_with_warning(&f, version, warning).await;
        assert!(before.artifact_status.is_ready());
        assert_eq!(before.last_error.as_deref(), Some(warning));
        let after = reconcile_installation_snapshot(&f.store, NODE, id)
            .await
            .unwrap()
            .artifact;
        assert_eq!(after.last_error.as_deref(), Some(warning));
        assert_eq!(after.runtime_status, PluginRuntimeStatus::Inactive);
    }
}

#[tokio::test]
async fn postgres_runtime_status_write_preserves_unrepaired_checksum_warning() {
    let f = Fixture::new().await;
    for (version, warning) in [
        ("1.0.0", "checksum_missing"),
        ("2.0.0", "checksum_mismatch"),
    ] {
        let (id, before) = install_artifact_with_warning(&f, version, warning).await;
        assert!(before.artifact_status.is_ready());
        assert_eq!(before.last_error.as_deref(), Some(warning));
        let after = f
            .store
            .record_artifact_runtime_status(&RecordPluginRuntimeStatusInput {
                node_id: NODE.into(),
                installation_id: id,
                runtime_status: PluginRuntimeStatus::Inactive,
                last_error: None,
                checked_at: time::OffsetDateTime::now_utc(),
            })
            .await
            .unwrap();
        assert_eq!(after.last_error.as_deref(), Some(warning));
        assert_eq!(after.runtime_status, PluginRuntimeStatus::Inactive);
    }
}

#[tokio::test]
async fn postgres_local_artifact_warnings_fence_active_commit_and_version_switch() {
    let f = Fixture::new().await;
    let old = f.install("fixture_provider", "1.0.0").await;
    f.enable(old).await;
    for (version, warning) in [
        ("2.0.0", "manifest_fingerprint_mismatch"),
        ("3.0.0", "checksum_mismatch"),
        ("4.0.0", "checksum_missing"),
    ] {
        let (target, refreshed) = install_artifact_with_warning(&f, version, warning).await;
        assert!(refreshed.artifact_status.is_ready());
        assert_eq!(refreshed.runtime_status, PluginRuntimeStatus::Inactive);
        assert_eq!(refreshed.last_error.as_deref(), Some(warning));
        assert!(f
            .service
            .enable_plugin(EnablePluginCommand {
                actor_user_id: f.actor,
                installation_id: target
            })
            .await
            .is_err());
        assert!(f
            .service
            .switch_version(SwitchPluginVersionCommand {
                actor_user_id: f.actor,
                provider_code: "fixture_provider".into(),
                target_installation_id: target,
            })
            .await
            .is_err());
        // Bypass the service's initial checks: even an already-requested load that presents
        // the exact recorded identity cannot publish an artifact with a local warning.
        sqlx::query(
            "update extension_installations set desired_state = 'active_requested' where id = $1",
        )
        .bind(target)
        .execute(f.store.pool())
        .await
        .unwrap();
        let outcome = f
            .store
            .try_mark_active_artifact_instance(&ActivatePluginArtifactInstanceInput {
                validate_manifest: validate_installed_artifact_manifest,
                node_id: NODE.into(),
                installation_id: target,
                expected_local_version: refreshed.local_version,
                expected_local_checksum: refreshed.local_checksum,
                expected_local_path: refreshed.local_path.unwrap(),
                expected_manifest_fingerprint: refreshed.manifest_fingerprint,
                checked_at: time::OffsetDateTime::now_utc(),
            })
            .await
            .unwrap();
        assert!(
            matches!(outcome, ActivatePluginArtifactOutcome::ArtifactInvalidated),
            "{warning}: {outcome:?}"
        );
        let artifact = local(&f, target).await.artifact;
        assert_eq!(artifact.runtime_status, PluginRuntimeStatus::Inactive);
        assert_eq!(artifact.last_error.as_deref(), Some(warning));
        f.assert_state(old, ACTIVE).await;
        sqlx::query("update extension_installations set desired_state = 'disabled' where id = $1")
            .bind(target)
            .execute(f.store.pool())
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn postgres_live_disk_change_fences_active_commit() {
    let f = Fixture::new().await;
    let target = f.install("fixture_provider", "1.0.0").await;
    let artifact = local(&f, target).await.artifact;
    sqlx::query(
        "update extension_installations set desired_state = 'active_requested' where id = $1",
    )
    .bind(target)
    .execute(f.store.pool())
    .await
    .unwrap();
    let path = artifact.local_path.unwrap();
    let manifest = PathBuf::from(&path).join("manifest.yaml");
    let raw = tokio::fs::read_to_string(&manifest).await.unwrap();
    tokio::fs::write(&manifest, format!("{raw}\n# changed after verification\n"))
        .await
        .unwrap();
    let outcome = f
        .store
        .try_mark_active_artifact_instance(&ActivatePluginArtifactInstanceInput {
            validate_manifest: validate_installed_artifact_manifest,
            node_id: NODE.into(),
            installation_id: target,
            expected_local_version: artifact.local_version,
            expected_local_checksum: artifact.local_checksum,
            expected_local_path: path,
            expected_manifest_fingerprint: artifact.manifest_fingerprint,
            checked_at: time::OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    assert!(
        matches!(outcome, ActivatePluginArtifactOutcome::ArtifactInvalidated),
        "{outcome:?}"
    );
    assert_eq!(
        local(&f, target).await.artifact.runtime_status,
        PluginRuntimeStatus::Inactive
    );
}

#[tokio::test]
async fn postgres_live_disk_change_fences_switch_and_preserves_assignment_and_instance() {
    let f = Fixture::new().await;
    let current = f.install("fixture_provider", "1.0.0").await;
    let target = f.install("fixture_provider", "2.0.0").await;
    f.enable(current).await;
    f.enable(target).await;
    f.service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: f.actor,
            installation_id: current,
        })
        .await
        .unwrap();
    let instance =
        seed_test_provider_instance(&f.store, f.workspace.id, f.actor, current, "Stays on V1")
            .await;
    let artifact = local(&f, target).await.artifact;
    let path = artifact.local_path.unwrap();
    let marker = PathBuf::from(&path).join(".1flowbase-artifact.json");
    let mut stored: serde_json::Value =
        serde_json::from_slice(&tokio::fs::read(&marker).await.unwrap()).unwrap();
    stored["checksum"] = "tampered".into();
    tokio::fs::write(marker, serde_json::to_vec(&stored).unwrap())
        .await
        .unwrap();
    let result = f
        .store
        .commit_provider_version_switch(&CommitProviderVersionSwitchInput {
            validate_manifest: validate_installed_artifact_manifest,
            node_id: NODE.into(),
            workspace_id: f.workspace.id,
            provider_code: "fixture_provider".into(),
            current_installation_id: current,
            target_installation_id: target,
            target_local_version: artifact.local_version,
            target_local_checksum: artifact.local_checksum,
            target_local_path: path,
            target_manifest_fingerprint: artifact.manifest_fingerprint,
            target_protocol: "openai_compatible".into(),
            model_discovery_mode: domain::ModelProviderDiscoveryMode::Hybrid,
            catalog_source: domain::ModelProviderCatalogSource::Hybrid,
            actor_user_id: f.actor,
        })
        .await;
    assert!(matches!(
        result.unwrap_err().downcast_ref(),
        Some(control_plane_contracts::ControlPlaneContractError::PluginUnavailable)
    ));
    assert_eq!(
        f.store.list_assignments(f.workspace.id).await.unwrap()[0].installation_id,
        current
    );
    let installed: Uuid =
        sqlx::query_scalar("select installation_id from model_provider_instances where id=$1")
            .bind(instance)
            .fetch_one(f.store.pool())
            .await
            .unwrap();
    assert_eq!(installed, current);
}

#[tokio::test]
async fn postgres_missing_manifest_is_corrupted_even_with_checksum_warning() {
    assert_corrupted_manifest("checksum_missing", "manifest_missing").await;
}

#[tokio::test]
async fn postgres_invalid_manifest_is_corrupted_even_with_fingerprint_warning() {
    assert_corrupted_manifest("manifest_fingerprint_mismatch", "manifest_invalid").await;
}

async fn assert_corrupted_manifest(warning: &str, error: &str) {
    let f = Fixture::new().await;
    let (id, artifact) = install_artifact_with_warning(&f, "1.0.0", warning).await;
    assert!(artifact.artifact_status.is_ready());
    assert_eq!(artifact.last_error.as_deref(), Some(warning));
    let path = PathBuf::from(artifact.local_path.unwrap()).join("manifest.yaml");
    if error == "manifest_missing" {
        tokio::fs::remove_file(path).await.unwrap();
    } else {
        tokio::fs::write(path, "broken: [").await.unwrap();
    }
    let refreshed = refresh_current_node_plugin_artifact_instance(
        &f.store,
        NODE,
        &f.root.join("artifacts"),
        &local(&f, id).await.installation,
    )
    .await
    .unwrap();
    assert_eq!(
        refreshed.artifact_status,
        domain::PluginArtifactInstanceStatus::Corrupted
    );
    assert_eq!(refreshed.last_error.as_deref(), Some(error));
}
