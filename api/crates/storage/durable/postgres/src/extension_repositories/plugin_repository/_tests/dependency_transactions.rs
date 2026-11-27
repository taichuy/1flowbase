//! Real adapter/transaction regressions; execute in Root's single centralized PostgreSQL batch.
use super::dependency_tests::{consumer, provider};
use super::*;
use std::time::Duration;

async fn store() -> (PgControlPlaneStore, Uuid) {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into());
    let database = postgres_test_support::PostgresTestSchema::create(&url)
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    crate::run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "dependency-fixture")
        .await
        .unwrap();
    let actor = store
        .upsert_root_user(
            workspace.id,
            "root",
            "root@example.com",
            "fixture",
            "Root",
            "Root",
        )
        .await
        .unwrap()
        .id;
    (store, actor)
}
fn artifact(i: &domain::PluginInstallationRecord) -> UpsertPluginArtifactInstanceInput {
    UpsertPluginArtifactInstanceInput {
        installation_id: i.id,
        node_id: "node-a".into(),
        local_version: Some(i.plugin_version.clone()),
        local_checksum: None,
        local_path: Some("/fixture/installed".into()),
        package_path: None,
        manifest_fingerprint: None,
        artifact_status: domain::PluginArtifactInstanceStatus::Ready,
        runtime_status: domain::PluginRuntimeStatus::Inactive,
        availability_status: domain::PluginAvailabilityStatus::Disabled,
        checked_at: time::OffsetDateTime::now_utc(),
        last_error: None,
        is_current: true,
    }
}
fn installation_input(
    i: &domain::PluginInstallationRecord,
    actor: Uuid,
) -> UpsertPluginInstallationInput {
    UpsertPluginInstallationInput {
        installation_id: i.id,
        category: i.category,
        organization: i.organization.clone(),
        provider_code: i.provider_code.clone(),
        plugin_id: format!("{}@{}", i.provider_code, i.plugin_version),
        plugin_version: i.plugin_version.clone(),
        contract_version: i.contract_version.clone(),
        protocol: i.protocol.clone(),
        display_name: i.display_name.clone(),
        source_kind: i.source_kind.clone(),
        trust_level: i.trust_level.clone(),
        verification_status: i.verification_status,
        desired_state: i.desired_state,
        expected_checksum: None,
        signature_status: i.signature_status,
        signature_algorithm: None,
        signing_key_id: None,
        metadata_json: i.metadata_json.clone(),
        is_system_reserved: false,
        actor_user_id: actor,
    }
}
async fn seed(store: &PgControlPlaneStore, i: &domain::PluginInstallationRecord, actor: Uuid) {
    // Match real upload -> ready artifact -> explicit enable. Direct active upsert must not
    // manufacture readiness or bypass prerequisite admission.
    let mut input = installation_input(i, actor);
    input.desired_state = domain::PluginDesiredState::Disabled;
    store.upsert_installation(&input).await.unwrap();
    store.upsert_artifact_instance(&artifact(i)).await.unwrap();
    if i.desired_state == domain::PluginDesiredState::ActiveRequested {
        desired(store, i.id, actor, i.desired_state).await.unwrap();
    }
}
async fn desired(
    store: &PgControlPlaneStore,
    id: Uuid,
    actor: Uuid,
    state: domain::PluginDesiredState,
) -> Result<domain::PluginInstallationRecord> {
    store
        .update_desired_state(&UpdatePluginDesiredStateInput {
            installation_id: id,
            actor_user_id: actor,
            desired_state: state,
        })
        .await
}
#[tokio::test]
async fn activation_waits_for_provider_lifecycle_fence_and_rechecks_current_state() {
    let (store, actor) = store().await;
    let p = provider();
    let mut c = consumer();
    c.desired_state = domain::PluginDesiredState::Disabled;
    seed(&store, &p, actor).await;
    seed(&store, &c, actor).await;
    let mut transaction = store.pool().begin().await.unwrap();
    lock_managed_dependency_scope(&mut transaction)
        .await
        .unwrap();
    guard_managed_desired_state(&mut transaction, p.id, domain::PluginDesiredState::Disabled)
        .await
        .unwrap();
    sqlx::query("update extension_installations set desired_state='disabled' where id=$1")
        .bind(p.id)
        .execute(&mut *transaction)
        .await
        .unwrap();
    let worker = store.clone();
    let c_id = c.id;
    let (started, entered) = tokio::sync::oneshot::channel();
    let mut pending = tokio::spawn(async move {
        started.send(()).unwrap();
        desired(
            &worker,
            c_id,
            actor,
            domain::PluginDesiredState::ActiveRequested,
        )
        .await
    });
    entered.await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut pending)
            .await
            .is_err(),
        "activation must wait for the same transaction fence"
    );
    transaction.commit().await.unwrap();
    let error = tokio::time::timeout(Duration::from_secs(5), pending)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PluginDependency { .. })
    ));
    assert_eq!(
        store
            .get_installation(c.id)
            .await
            .unwrap()
            .unwrap()
            .desired_state,
        domain::PluginDesiredState::Disabled
    );
}
#[tokio::test]
async fn provider_disable_waits_for_enable_then_preserves_state_and_uninstall_artifacts() {
    let (store, actor) = store().await;
    let p = provider();
    let mut c = consumer();
    c.desired_state = domain::PluginDesiredState::Disabled;
    seed(&store, &p, actor).await;
    seed(&store, &c, actor).await;
    let mut transaction = store.pool().begin().await.unwrap();
    lock_managed_dependency_scope(&mut transaction)
        .await
        .unwrap();
    guard_managed_desired_state(
        &mut transaction,
        c.id,
        domain::PluginDesiredState::ActiveRequested,
    )
    .await
    .unwrap();
    sqlx::query("update extension_installations set desired_state='active_requested' where id=$1")
        .bind(c.id)
        .execute(&mut *transaction)
        .await
        .unwrap();
    let worker = store.clone();
    let p_id = p.id;
    let (started, entered) = tokio::sync::oneshot::channel();
    let mut pending = tokio::spawn(async move {
        started.send(()).unwrap();
        desired(&worker, p_id, actor, domain::PluginDesiredState::Disabled).await
    });
    entered.await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut pending)
            .await
            .is_err()
    );
    transaction.commit().await.unwrap();
    assert!(tokio::time::timeout(Duration::from_secs(5), pending)
        .await
        .unwrap()
        .unwrap()
        .is_err());
    assert_eq!(
        store
            .get_installation(p.id)
            .await
            .unwrap()
            .unwrap()
            .desired_state,
        domain::PluginDesiredState::ActiveRequested
    );
    let current = store.get_installation(p.id).await.unwrap().unwrap();
    let rollback = control_plane_contracts::ports::CompareRestorePluginDesiredStateInput {
        installation_id: p.id,
        expected_updated_at: current.updated_at,
        expected_desired_state: domain::PluginDesiredState::ActiveRequested,
        restore_desired_state: domain::PluginDesiredState::Disabled,
        actor_user_id: actor,
    };
    assert!(
        store
            .compare_restore_desired_state(&rollback)
            .await
            .is_err(),
        "CAS rollback must not deactivate a newly depended-on provider"
    );
    let stale = control_plane_contracts::ports::CompareRestorePluginDesiredStateInput {
        expected_updated_at: time::OffsetDateTime::UNIX_EPOCH,
        ..rollback
    };
    assert!(
        !store.compare_restore_desired_state(&stale).await.unwrap(),
        "stale CAS remains an unaffected no-op"
    );
    let mut removed = artifact(&p);
    removed.artifact_status = domain::PluginArtifactInstanceStatus::Missing;
    removed.local_path = None;
    removed.is_current = false;
    let error = store
        .commit_plugin_family_uninstall(&CommitPluginFamilyUninstallInput {
            artifact_instances: vec![removed],
            artifact_cleanups: Vec::new(),
            audit_log: domain::AuditLogRecord {
                id: Uuid::now_v7(),
                workspace_id: None,
                actor_user_id: Some(actor),
                target_type: "plugin_installation".into(),
                target_id: Some(p.id),
                event_code: "plugin.family_uninstalled".into(),
                payload: serde_json::json!({}),
                created_at: time::OffsetDateTime::now_utc(),
            },
        })
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PluginDependency { .. })
    ));
    assert!(store
        .get_artifact_instance("node-a", p.id)
        .await
        .unwrap()
        .unwrap()
        .artifact_status
        .is_ready());
    desired(&store, c.id, actor, domain::PluginDesiredState::Disabled)
        .await
        .unwrap();
    desired(&store, p.id, actor, domain::PluginDesiredState::Disabled)
        .await
        .unwrap();
}
#[tokio::test]
async fn locked_switch_guard_checks_candidate_exports_and_target_prerequisites() {
    let (store, actor) = store().await;
    let p = provider();
    let c = consumer();
    let mut next = p.clone();
    next.id = Uuid::now_v7();
    next.plugin_version = "2.0.0".into();
    next.desired_state = domain::PluginDesiredState::Disabled;
    seed(&store, &p, actor).await;
    seed(&store, &c, actor).await;
    seed(&store, &next, actor).await;
    let mut transaction = store.pool().begin().await.unwrap();
    lock_managed_dependency_scope(&mut transaction)
        .await
        .unwrap();
    validate_managed_dependency_switch(
        &mut transaction,
        p.id,
        next.id,
        "node-a",
        domain::SYSTEM_SCOPE_ID,
    )
    .await
    .unwrap();
    sqlx::query("update extension_installations set metadata_json=jsonb_set(metadata_json,'{resource_exports}','[]'::jsonb) where id=$1").bind(next.id).execute(&mut *transaction).await.unwrap();
    assert!(validate_managed_dependency_switch(
        &mut transaction,
        p.id,
        next.id,
        "node-a",
        domain::SYSTEM_SCOPE_ID
    )
    .await
    .is_err());
    let mut missing_prerequisite = next.metadata_json.clone();
    missing_prerequisite["plugin_dependencies"] =
        serde_json::json!([{"publisher_namespace":"fixture.publisher","plugin_code":"missing"}]);
    sqlx::query("update extension_installations set metadata_json=$2 where id=$1")
        .bind(next.id)
        .bind(missing_prerequisite)
        .execute(&mut *transaction)
        .await
        .unwrap();
    let error = validate_managed_dependency_switch(
        &mut transaction,
        p.id,
        next.id,
        "node-a",
        domain::SYSTEM_SCOPE_ID,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(error.downcast_ref::<ControlPlaneError>(),Some(ControlPlaneError::PluginDependency {provider_plugin_code,..}) if provider_plugin_code == "missing")
    );
    transaction.rollback().await.unwrap();
}

#[tokio::test]
async fn low_level_provider_writers_fence_before_row_locks_and_roll_back_incompatible_changes() {
    let (store, actor) = store().await;
    let p = provider();
    let c = consumer();
    seed(&store, &p, actor).await;
    seed(&store, &c, actor).await;
    for writer in 0..3 {
        let mut fence = store.pool().begin().await.unwrap();
        lock_managed_dependency_scope(&mut fence).await.unwrap();
        let worker = store.clone();
        let provider = p.clone();
        let (started, entered) = tokio::sync::oneshot::channel();
        let mut pending = tokio::spawn(async move {
            started.send(()).unwrap();
            match writer {
                0 => worker.delete_installation(provider.id).await,
                1 => {
                    let mut input = installation_input(&provider, actor);
                    // Resolve the persisted conflict id rather than this proposed id. Keep the
                    // immutable publisher identity valid so SQL reaches the dependency guard.
                    input.installation_id = Uuid::now_v7();
                    input.metadata_json["resource_exports"] = serde_json::json!([]);
                    worker.upsert_installation(&input).await.map(|_| ())
                }
                _ => {
                    let mut input = artifact(&provider);
                    input.artifact_status = domain::PluginArtifactInstanceStatus::Missing;
                    input.local_path = None;
                    worker.upsert_artifact_instance(&input).await.map(|_| ())
                }
            }
        });
        entered.await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(100), &mut pending)
                .await
                .is_err(),
            "writer {writer} must wait for the reader/lifecycle system fence"
        );
        // If a writer takes the installation row before the fence this NOWAIT fails.
        sqlx::query("select id from extension_installations where id=$1 for update nowait")
            .bind(p.id)
            .fetch_one(&mut *fence)
            .await
            .unwrap();
        fence.commit().await.unwrap();
        let error = tokio::time::timeout(Duration::from_secs(5), pending)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        assert!(
            matches!(error.downcast_ref::<ControlPlaneError>(),
                Some(ControlPlaneError::PluginDependency { code, dependents, .. })
                    if code == "plugin_dependency_in_use" && dependents == &["fixture.publisher/consumer@1.0.0"]),
            "writer {writer} must reject with the dependency owner error, got {error:#}"
        );
        let current = store.get_installation(p.id).await.unwrap().unwrap();
        assert_eq!(current.organization, p.organization);
        assert_eq!(current.metadata_json, p.metadata_json);
        assert_eq!(
            current.desired_state,
            domain::PluginDesiredState::ActiveRequested
        );
        assert!(store
            .get_artifact_instance("node-a", p.id)
            .await
            .unwrap()
            .unwrap()
            .artifact_status
            .is_ready());
    }
    desired(&store, c.id, actor, domain::PluginDesiredState::Disabled)
        .await
        .unwrap();
    let mut input = artifact(&p);
    input.artifact_status = domain::PluginArtifactInstanceStatus::Missing;
    store.upsert_artifact_instance(&input).await.unwrap();
    store.delete_installation(c.id).await.unwrap();
    store.delete_installation(p.id).await.unwrap();
}

#[tokio::test]
async fn installation_upsert_rolls_back_missing_exports_and_record_binding_rewrites() {
    let (store, actor) = store().await;
    let p = provider();
    let c = consumer();
    seed(&store, &p, actor).await;
    seed(&store, &c, actor).await;
    for binding_rewrite in [false, true] {
        let mut input = installation_input(&p, actor);
        if binding_rewrite {
            input.metadata_json["resource_exports"][0]["record_resources"][0]["id_field"] =
                serde_json::json!("different_id");
        } else {
            input.metadata_json["resource_exports"] = serde_json::json!([]);
        }
        let error = store.upsert_installation(&input).await.unwrap_err();
        assert!(matches!(error.downcast_ref::<ControlPlaneError>(),
            Some(ControlPlaneError::PluginDependency { code, resource_code: Some(resource), .. })
                if code == "plugin_dependency_in_use" && resource == "records"));
        assert_eq!(
            store
                .get_installation(p.id)
                .await
                .unwrap()
                .unwrap()
                .metadata_json,
            p.metadata_json
        );
    }
    store
        .upsert_installation(&installation_input(&p, actor))
        .await
        .unwrap();
    let grants: i64 = sqlx::query_scalar(
        "select count(*) from plugin_contribution_authorizations where installation_id=any($1)",
    )
    .bind(&[p.id, c.id][..])
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        grants, 0,
        "installation/readiness writes do not implicitly grant authority"
    );
}

#[tokio::test]
async fn artifact_upsert_cannot_publish_an_ambiguous_enabled_provider_version() {
    let (store, actor) = store().await;
    let p = provider();
    let c = consumer();
    seed(&store, &p, actor).await;
    seed(&store, &c, actor).await;
    let mut next = p.clone();
    next.id = Uuid::now_v7();
    next.plugin_version = "2.0.0".into();
    // No visible artifact yet, so this provider does not compete with the selected version.
    store
        .upsert_installation(&installation_input(&next, actor))
        .await
        .unwrap();
    let error = store
        .upsert_artifact_instance(&artifact(&next))
        .await
        .unwrap_err();
    assert!(matches!(error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PluginDependency { code, .. }) if code == "plugin_dependency_in_use"));
    assert!(
        store
            .get_artifact_instance("node-a", next.id)
            .await
            .unwrap()
            .is_none(),
        "the candidate artifact insert must roll back, not become visible in reader inventory"
    );
    assert!(store
        .get_artifact_instance("node-a", p.id)
        .await
        .unwrap()
        .unwrap()
        .artifact_status
        .is_ready());
}

fn managed_commit_input(
    i: &domain::PluginInstallationRecord,
    actor: Uuid,
) -> CommitPluginInstallationInput {
    use control_plane_contracts::ports::{
        ReplaceInstallationFrontendBlocksInput, ReplaceInstallationJsDependenciesInput,
        ReplaceInstallationNodeContributionsInput,
    };
    let mut installation = installation_input(i, actor);
    // Admission identity is persisted even though these adapter fixtures need no real archive.
    installation.metadata_json["managed"] = serde_json::json!({});
    installation.expected_checksum = Some("a".repeat(64));
    let mut artifact_instance = artifact(i);
    artifact_instance.local_checksum = installation.expected_checksum.clone();
    CommitPluginInstallationInput {
        settings_templates: Vec::new(),
        node_contributions: ReplaceInstallationNodeContributionsInput {
            installation_id: i.id,
            provider_code: i.provider_code.clone(),
            plugin_id: installation.plugin_id.clone(),
            plugin_version: i.plugin_version.clone(),
            entries: Vec::new(),
        },
        js_dependencies: ReplaceInstallationJsDependenciesInput {
            installation_id: i.id,
            provider_code: i.provider_code.clone(),
            plugin_id: installation.plugin_id.clone(),
            plugin_version: i.plugin_version.clone(),
            entries: Vec::new(),
        },
        frontend_blocks: ReplaceInstallationFrontendBlocksInput {
            installation_id: i.id,
            provider_code: i.provider_code.clone(),
            plugin_id: installation.plugin_id.clone(),
            plugin_version: i.plugin_version.clone(),
            entries: Vec::new(),
        },
        installation,
        artifact_instance,
        package_catalog: None,
        retained_frontend_module_assets: Vec::new(),
    }
}

#[tokio::test]
async fn installation_lease_commit_fences_before_rows_rolls_back_and_reuses_admission_for_reupload()
{
    let (store, actor) = store().await;
    let mut p = provider();
    p.desired_state = domain::PluginDesiredState::Disabled;
    let installed = store
        .commit_plugin_installation(&managed_commit_input(&p, actor))
        .await
        .unwrap();
    desired(
        &store,
        installed.id,
        actor,
        domain::PluginDesiredState::ActiveRequested,
    )
    .await
    .unwrap();
    let c = consumer();
    seed(&store, &c, actor).await;
    let mut bad = managed_commit_input(&p, actor);
    // The conflict key refers to installed.id, not this fresh proposed upload id.
    bad.installation.installation_id = Uuid::now_v7();
    bad.artifact_instance.artifact_status = domain::PluginArtifactInstanceStatus::Missing;
    let admission =
        crate::plugin_installation_commit_repository::admission_for_input(&bad.installation)
            .unwrap();
    let mut lease = store.begin_plugin_installation(&admission).await.unwrap();
    let mut fence = store.pool().begin().await.unwrap();
    lock_managed_dependency_scope(&mut fence).await.unwrap();
    let (started, entered) = tokio::sync::oneshot::channel();
    let mut pending = tokio::spawn(async move {
        started.send(()).unwrap();
        let result = lease.commit(&bad).await;
        (lease, result)
    });
    entered.await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut pending)
            .await
            .is_err(),
        "the install-commit writer must wait for reader/lifecycle inventory's system fence"
    );
    sqlx::query("select id from extension_installations where id=$1 for update nowait")
        .bind(installed.id)
        .fetch_one(&mut *fence)
        .await
        .unwrap();
    fence.commit().await.unwrap();
    let (mut lease, rejected) = tokio::time::timeout(Duration::from_secs(5), pending)
        .await
        .unwrap()
        .unwrap();
    let error = rejected.unwrap_err();
    assert!(matches!(error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PluginDependency { code, .. }) if code == "plugin_dependency_in_use"));
    assert!(store
        .get_artifact_instance("node-a", installed.id)
        .await
        .unwrap()
        .unwrap()
        .artifact_status
        .is_ready());
    assert_eq!(
        store
            .get_installation(installed.id)
            .await
            .unwrap()
            .unwrap()
            .desired_state,
        domain::PluginDesiredState::ActiveRequested
    );
    // Failed commit did not lose admission or poison the connection: retry the same lease.
    let mut retry = managed_commit_input(&p, actor);
    retry.installation.installation_id = Uuid::now_v7();
    let reuploaded = lease.commit(&retry).await.unwrap();
    assert_eq!(reuploaded.id, installed.id);
    assert_eq!(
        reuploaded.desired_state,
        domain::PluginDesiredState::ActiveRequested,
        "a no-op archive reupload must preserve enabled state despite disabled upload intent"
    );
    assert_eq!(reuploaded.metadata_json, installed.metadata_json);
    lease.release().await.unwrap();
    // Successful release unlocks admission so the public convenience path can reenter it.
    assert_eq!(
        store
            .commit_plugin_installation(&managed_commit_input(&p, actor))
            .await
            .unwrap()
            .id,
        installed.id
    );
}

#[tokio::test]
async fn installation_commit_rolls_back_new_ready_provider_that_would_ambiguate_active_consumers() {
    let (store, actor) = store().await;
    let p = provider();
    let c = consumer();
    seed(&store, &p, actor).await;
    seed(&store, &c, actor).await;
    let mut next = p.clone();
    next.id = Uuid::now_v7();
    next.plugin_version = "2.0.0".into();
    let input = managed_commit_input(&next, actor);
    let error = store.commit_plugin_installation(&input).await.unwrap_err();
    assert!(matches!(error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PluginDependency { code, .. }) if code == "plugin_dependency_in_use"));
    assert!(
        store.get_installation(next.id).await.unwrap().is_none(),
        "installation insert rolls back with readiness publication"
    );
    assert!(store
        .get_artifact_instance("node-a", next.id)
        .await
        .unwrap()
        .is_none());
    next.desired_state = domain::PluginDesiredState::Disabled;
    let uploaded = store
        .commit_plugin_installation(&managed_commit_input(&next, actor))
        .await
        .unwrap();
    assert_eq!(uploaded.desired_state, domain::PluginDesiredState::Disabled);
    assert!(store
        .get_artifact_instance("node-a", next.id)
        .await
        .unwrap()
        .unwrap()
        .artifact_status
        .is_ready());
}

#[tokio::test]
async fn artifact_refresh_guards_readiness_flips_while_runtime_writes_skip_dependency_facts() {
    let (store, actor) = store().await;
    let p = provider();
    let c = consumer();
    seed(&store, &p, actor).await;
    seed(&store, &c, actor).await;
    let refresh = |artifact_status, observed_artifact| RefreshPluginArtifactInstanceInput {
        node_id: "node-a".into(),
        installation_id: p.id,
        observed_artifact,
        touch_checked_at: true,
        local_version: Some(p.plugin_version.clone()),
        local_checksum: None,
        local_path: Some("/fixture/installed".into()),
        manifest_fingerprint: None,
        artifact_status,
        last_error: None,
        reset_runtime_status: false,
        checked_at: time::OffsetDateTime::now_utc(),
    };
    let observed = store.get_artifact_instance("node-a", p.id).await.unwrap();
    // Readiness unchanged: the dependency facts cannot change, the write commits.
    let ready = store
        .refresh_artifact_instance(&refresh(
            domain::PluginArtifactInstanceStatus::Ready,
            observed,
        ))
        .await
        .unwrap()
        .expect("observed row is current");
    // Readiness flip: the guard still sees the active consumer and rolls back.
    let error = store
        .refresh_artifact_instance(&refresh(
            domain::PluginArtifactInstanceStatus::Missing,
            Some(ready),
        ))
        .await
        .unwrap_err();
    assert!(matches!(error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PluginDependency { code, .. }) if code == "plugin_dependency_in_use"));
    // Runtime-only writes keep artifact status and version, so they never touch the guard.
    let failed = store
        .record_artifact_runtime_status(&RecordPluginRuntimeStatusInput {
            node_id: "node-a".into(),
            installation_id: p.id,
            runtime_status: domain::PluginRuntimeStatus::LoadFailed,
            last_error: Some("fixture load failure".into()),
            checked_at: time::OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    assert!(failed.artifact_status.is_ready());
    assert_eq!(
        failed.runtime_status,
        domain::PluginRuntimeStatus::LoadFailed
    );
}

#[tokio::test]
async fn runtime_activation_rechecks_target_dependencies_before_publishing_active() {
    let (store, actor) = store().await;
    let p = provider();
    let c = consumer();
    seed(&store, &p, actor).await;
    seed(&store, &c, actor).await;

    let dir = std::env::temp_dir().join(format!("artifact-activation-{}", c.id));
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("manifest.yaml"), "fixture").unwrap();
    std::fs::write(
        dir.join(".1flowbase-artifact.json"),
        serde_json::json!({
            "plugin_id": c.plugin_id, "version": c.plugin_version, "manifest_fingerprint": "fixture"
        })
        .to_string(),
    )
    .unwrap();
    let mut ready = artifact(&c);
    ready.local_path = Some(dir.display().to_string());
    ready.manifest_fingerprint = Some("fixture".into());
    let before = store.upsert_artifact_instance(&ready).await.unwrap();
    let activate = ActivatePluginArtifactInstanceInput {
        node_id: ready.node_id,
        installation_id: c.id,
        expected_local_version: ready.local_version,
        expected_local_checksum: ready.local_checksum,
        expected_local_path: ready.local_path.unwrap(),
        expected_manifest_fingerprint: ready.manifest_fingerprint,
        validate_manifest: |_, raw| (raw == "fixture").then(|| raw.to_owned()),
        checked_at: time::OffsetDateTime::now_utc(),
    };
    // Simulate provider drift after enable without changing the consumer or its local files.
    sqlx::query("update extension_artifact_instances set artifact_status='missing' where node_id='node-a' and installation_id=$1")
        .bind(p.id).execute(store.pool()).await.unwrap();
    let refused = store.try_mark_active_artifact_instance(&activate).await;
    let persisted = store.get_artifact_instance("node-a", c.id).await.unwrap();
    // Positive control: the exact same observed artifact can activate after provider repair.
    store.upsert_artifact_instance(&artifact(&p)).await.unwrap();
    let repaired = store.try_mark_active_artifact_instance(&activate).await;
    std::fs::remove_dir_all(dir).unwrap();
    let error = refused.unwrap_err();
    assert!(matches!(error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PluginDependency { code, provider_plugin_code, .. })
            if code == "plugin_dependency_not_ready" && provider_plugin_code == "provider"));
    assert_eq!(
        persisted,
        Some(before),
        "refusal must not change the row clock or status"
    );
    assert!(
        matches!(repaired.unwrap(), ActivatePluginArtifactOutcome::Activated(row)
        if row.runtime_status == PluginRuntimeStatus::Active)
    );
}

#[tokio::test]
async fn refresh_uses_the_locked_version_when_readiness_would_ambiguate_a_provider() {
    let (store, actor) = store().await;
    let p = provider();
    let c = consumer();
    seed(&store, &p, actor).await;
    seed(&store, &c, actor).await;
    // This provider can coexist with the selected one only while it is not ready here.
    let mut other = provider();
    other.plugin_version = "2.0.0".into();
    store
        .upsert_installation(&installation_input(&other, actor))
        .await
        .unwrap();
    let mut missing = artifact(&other);
    missing.artifact_status = domain::PluginArtifactInstanceStatus::Missing;
    missing.is_current = false;
    let before = store.upsert_artifact_instance(&missing).await.unwrap();
    let new_version = "3.0.0";
    let refresh = RefreshPluginArtifactInstanceInput {
        node_id: before.node_id.clone(),
        installation_id: other.id,
        observed_artifact: Some(before.clone()),
        touch_checked_at: true,
        local_version: Some(new_version.into()),
        local_checksum: None,
        local_path: before.local_path.clone(),
        manifest_fingerprint: None,
        artifact_status: domain::PluginArtifactInstanceStatus::Ready,
        last_error: None,
        reset_runtime_status: false,
        checked_at: time::OffsetDateTime::now_utc(),
    };

    let mut fence = store.pool().begin().await.unwrap();
    lock_managed_dependency_scope(&mut fence).await.unwrap();
    let blocker: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *fence)
        .await
        .unwrap();
    let worker = store.clone();
    let pending = tokio::spawn(async move { worker.refresh_artifact_instance(&refresh).await });
    // Wait for the actual scope-lock request, not a scheduler delay or a task-start signal.
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "select exists(select 1 from pg_stat_activity where $1 = any(pg_blocking_pids(pid))
                 and query like '%pg_advisory_xact_lock%managed-workspace:%')",
            )
            .bind(blocker)
            .fetch_one(store.pool())
            .await
            .unwrap();
            if waiting {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("refresh did not wait for the held dependency fence");
    sqlx::query("select id from extension_installations where id=$1 for update nowait")
        .bind(other.id)
        .fetch_one(&mut *fence)
        .await
        .expect("scope fence must precede installation row lock");
    // This is the version change performed by a conflict-update on the same installation.
    sqlx::query("update extension_installations set artifact_version=$2 where id=$1")
        .bind(other.id)
        .bind(new_version)
        .execute(&mut *fence)
        .await
        .unwrap();
    fence.commit().await.unwrap();
    let error = tokio::time::timeout(Duration::from_secs(10), pending)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert!(matches!(error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PluginDependency { code, dependents, .. })
            if code == "plugin_dependency_in_use" && dependents == &["fixture.publisher/consumer@1.0.0"]));
    assert_eq!(
        store
            .get_artifact_instance("node-a", other.id)
            .await
            .unwrap(),
        Some(before)
    );
}

#[tokio::test]
async fn refresh_waits_for_non_bus_to_managed_upsert_before_classifying_the_provider() {
    // The fixture has its own database. Observe actual blockers, not task-start notifications.
    async fn next_waiter(pool: &sqlx::PgPool, exclude: i32) -> (i32, Vec<i32>, bool) {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Some(waiter) = sqlx::query_as(
                    "select pid, pg_blocking_pids(pid), wait_event = 'advisory' from pg_stat_activity
                     where datname = current_database() and pid <> $1 and wait_event_type = 'Lock'
                       and cardinality(pg_blocking_pids(pid)) > 0",
                )
                .bind(exclude)
                .fetch_optional(pool)
                .await
                .unwrap()
                {
                    return waiter;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("expected writer to be blocked in PostgreSQL")
    }

    let (store, actor) = store().await;
    seed(&store, &provider(), actor).await;
    seed(&store, &consumer(), actor).await;
    let mut other = provider();
    other.plugin_version = "2.0.0".into();
    let managed = installation_input(&other, actor);
    let mut non_bus = installation_input(&other, actor);
    non_bus.contract_version = "1flowbase.data_source/v1".into();
    let original = store.upsert_installation(&non_bus).await.unwrap();
    assert_eq!(original.contract_version, non_bus.contract_version);

    // Stall the real conflict-update on the installation row while it owns the dependency fence.
    let mut gate = store.pool().begin().await.unwrap();
    let gate_pid: i32 = sqlx::query_scalar(
        "select pg_backend_pid() from extension_installations where id=$1 for update",
    )
    .bind(other.id)
    .fetch_one(&mut *gate)
    .await
    .unwrap();
    let writer = store.clone();
    let upsert = tokio::spawn(async move { writer.upsert_installation(&managed).await });
    let (upsert_pid, blockers, _) = next_waiter(store.pool(), gate_pid).await;
    assert!(blockers.contains(&gate_pid));

    // Unfenced classification still sees non-bus and waits on the row instead of the fence.
    let refresh = RefreshPluginArtifactInstanceInput {
        installation_id: other.id,
        node_id: "node-a".into(),
        observed_artifact: None,
        touch_checked_at: true,
        local_version: Some(other.plugin_version.clone()),
        local_checksum: None,
        local_path: Some("/fixture/installed".into()),
        manifest_fingerprint: None,
        artifact_status: domain::PluginArtifactInstanceStatus::Ready,
        last_error: None,
        reset_runtime_status: false,
        checked_at: time::OffsetDateTime::now_utc(),
    };
    let writer = store.clone();
    let refresh = tokio::spawn(async move { writer.refresh_artifact_instance(&refresh).await });
    let (_, blockers, advisory) = next_waiter(store.pool(), upsert_pid).await;
    gate.commit().await.unwrap();
    let (upsert, refresh) = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::join!(upsert, refresh)
    })
    .await
    .unwrap();
    assert!(
        advisory && blockers.contains(&upsert_pid),
        "refresh must queue behind the reclassification fence before taking row locks"
    );
    let installed = upsert.unwrap().unwrap();
    assert_eq!(installed.id, other.id);
    assert_eq!(installed.contract_version, other.contract_version);
    let error = refresh.unwrap().unwrap_err();
    assert!(matches!(error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PluginDependency { code, dependents, .. })
            if code == "plugin_dependency_in_use" && dependents == &["fixture.publisher/consumer@1.0.0"]));
    assert_eq!(
        store
            .get_artifact_instance("node-a", other.id)
            .await
            .unwrap(),
        None,
        "the refused second ready provider must not leave an artifact row"
    );
}
