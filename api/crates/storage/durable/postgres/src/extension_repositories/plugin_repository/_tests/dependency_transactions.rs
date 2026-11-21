//! Real adapter/transaction regressions; execute in Root's single centralized PostgreSQL batch.
use super::dependency_tests::{consumer, provider};
use super::*;
use control_plane_contracts::ports::BootstrapRepository;
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
async fn seed(store: &PgControlPlaneStore, i: &domain::PluginInstallationRecord, actor: Uuid) {
    store
        .upsert_installation(&UpsertPluginInstallationInput {
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
        })
        .await
        .unwrap();
    store.upsert_artifact_instance(&artifact(i)).await.unwrap();
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
