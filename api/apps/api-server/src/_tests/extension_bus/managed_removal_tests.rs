//! Real snapshots, installed artifacts and durable rows: uninstall retires only idle history.
use super::*;

struct Fixture {
    _state: Arc<crate::app_state::ApiState>,
    runtime: RuntimeFixture,
    management: PluginManagementService<
        storage_durable_postgres::MainDurableStore,
        crate::provider_runtime::ApiProviderRuntime,
    >,
    actor: domain::ActorContext,
    installation_id: Uuid,
    node_id: String,
}

impl Fixture {
    async fn new() -> Self {
        let (state, _) = test_api_state_with_database_url().await;
        let runtime = RuntimeFixture::new(&state);
        let actor_id: Uuid = sqlx::query_scalar("select id from users where account='root'")
            .fetch_one(runtime.store.pool())
            .await
            .unwrap();
        let actor = AuthRepository::load_actor_context_for_user(&runtime.store, actor_id)
            .await
            .unwrap();
        let management = PluginManagementService::new(
            runtime.store.clone(),
            crate::provider_runtime::ApiProviderRuntime::new(runtime.services.clone()),
            state.official_plugin_source.clone(),
            &state.provider_install_root,
        )
        .with_node_id(&state.api_node_id);
        let installation_id = management
            .install_uploaded_plugin(InstallUploadedPluginCommand {
                actor_user_id: actor_id,
                file_name: "removal-fixture.1flowbasepkg".into(),
                package_bytes: package(&manifest("a")),
            })
            .await
            .unwrap()
            .installation
            .id;
        management
            .assign_plugin(AssignPluginCommand {
                actor_user_id: actor_id,
                installation_id,
            })
            .await
            .unwrap();
        let authority = PluginContributionAuthorityService::new(
            runtime.store.clone(),
            HostContributionGrantPolicy::root_composition(),
        )
        .with_node_id(&state.api_node_id);
        for permission in ["event.subscribe", "event.publish"] {
            authority
                .grant(&actor, installation_id, grant("a", permission))
                .await
                .unwrap();
        }
        management
            .enable_plugin(EnablePluginCommand {
                actor_user_id: actor_id,
                installation_id,
            })
            .await
            .unwrap();
        Self {
            _state: state,
            runtime,
            management,
            actor,
            installation_id,
            node_id: state.api_node_id.clone(),
        }
    }
    async fn disable(&self) {
        self.management
            .disable_plugin(DisablePluginCommand {
                actor_user_id: self.actor.user_id,
                installation_id: self.installation_id,
            })
            .await
            .unwrap();
    }
    async fn executions(&self) -> ManagedExecutionState {
        self.runtime
            .composition
            .governance()
            .managed_execution_state(self.actor.current_workspace_id, self.installation_id, None)
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn removal_preparation_retires_idle_snapshots_but_queries_do_not() {
    let fixture = Fixture::new().await;
    let composition = &fixture.runtime.composition;
    let id = fixture.installation_id;
    let old = composition
        .snapshot(fixture.actor.current_workspace_id)
        .await
        .unwrap();
    assert!(
        composition
            .clone()
            .prepare_managed_artifact_removal(&[id])
            .await
            .is_err(),
        "an active graph cannot be silently retired"
    );
    let reference = old.freeze_reference().unwrap();
    fixture.disable().await;
    let before = fixture.executions().await;
    assert!(!before.executions.is_empty());
    assert!(before.executions.iter().all(|execution| !execution.current));
    assert!(composition
        .guard_managed_artifact_removal(&[id])
        .await
        .is_err());
    assert_eq!(
        serde_json::to_value(fixture.executions().await).unwrap(),
        serde_json::to_value(&before).unwrap(),
        "read-only guard cannot retire history"
    );
    assert!(
        composition
            .clone()
            .prepare_managed_artifact_removal(&[id])
            .await
            .is_err(),
        "a frozen invocation owns the whole old graph"
    );
    drop(
        old.freeze_reference()
            .expect("failed retirement must reopen admission"),
    );
    drop(reference);
    let removal = composition
        .clone()
        .prepare_managed_artifact_removal(&[id])
        .await
        .unwrap();
    assert!(fixture.executions().await.executions.is_empty());
    assert!(
        old.freeze_reference().is_err(),
        "retired target cannot resurrect"
    );
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(30),
            composition.rebuild_installation(id)
        )
        .await
        .is_err(),
        "removal guard must hold assembly through the caller's filesystem/DB work"
    );
    drop(removal);
    drop(
        composition
            .guard_managed_artifact_removal(&[id])
            .await
            .unwrap(),
    );
    fixture.runtime.host.stop().await.unwrap();
}

#[tokio::test]
async fn removal_preparation_preserves_durable_backlog_and_reopens_failed_retirement() {
    let fixture = Fixture::new().await;
    let composition = &fixture.runtime.composition;
    let old = composition
        .snapshot(fixture.actor.current_workspace_id)
        .await
        .unwrap();
    create(
        &fixture.runtime.store,
        fixture.actor.user_id,
        fixture.actor.current_workspace_id,
    )
    .await;
    fixture.disable().await;
    let before = fixture.executions().await;
    assert!(
        !before.deliveries.is_empty(),
        "fixture must own genuine durable deliveries"
    );
    assert!(composition
        .clone()
        .prepare_managed_artifact_removal(&[fixture.installation_id])
        .await
        .is_err());
    assert_eq!(
        serde_json::to_value(fixture.executions().await).unwrap(),
        serde_json::to_value(before).unwrap(),
        "failed removal cannot discard backlog or snapshots"
    );
    drop(
        old.freeze_reference()
            .expect("backlog rejection rolls back the lifetime gate"),
    );
    fixture.runtime.host.stop().await.unwrap();
}

#[tokio::test]
async fn family_uninstall_disables_and_retires_without_host_restart() {
    let fixture = Fixture::new().await;
    let old = fixture
        .runtime
        .composition
        .snapshot(fixture.actor.current_workspace_id)
        .await
        .unwrap();
    let reference = old.freeze_reference().unwrap();
    let command = || DeletePluginFamilyCommand {
        actor_user_id: fixture.actor.user_id,
        provider_code: "acme.composition-a".into(),
    };
    assert!(
        fixture.management.delete_family(command()).await.is_err(),
        "uninstall disables first but must preserve genuinely referenced artifacts"
    );
    let installation = fixture
        .runtime
        .store
        .get_installation(fixture.installation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        installation.desired_state,
        domain::PluginDesiredState::Disabled
    );
    let artifact = fixture
        .runtime
        .store
        .get_artifact_instance(&fixture.node_id, fixture.installation_id)
        .await
        .unwrap()
        .unwrap();
    assert!(std::path::Path::new(artifact.local_path.as_deref().unwrap()).exists());
    drop(reference);
    let task = fixture.management.delete_family(command()).await.unwrap();
    assert_eq!(task.status, domain::PluginTaskStatus::Succeeded);
    assert!(fixture.executions().await.executions.is_empty());
    assert!(old.freeze_reference().is_err());
    let installation = fixture
        .runtime
        .store
        .get_installation(fixture.installation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        installation.desired_state,
        domain::PluginDesiredState::Disabled
    );
    fixture.runtime.host.stop().await.unwrap();
}

#[tokio::test]
async fn cancelled_removal_caller_does_not_cancel_owned_retirement() {
    let fixture = Fixture::new().await;
    let composition = &fixture.runtime.composition;
    let old = composition
        .snapshot(fixture.actor.current_workspace_id)
        .await
        .unwrap();
    fixture.disable().await;
    let assembly_owner = composition
        .clone()
        .prepare_managed_artifact_removal(&[])
        .await
        .unwrap();
    let ids = [fixture.installation_id];
    let mut pending = Box::pin(composition.clone().prepare_managed_artifact_removal(&ids));
    // Poll exactly through admission/spawn. The held assembly guard makes completion
    // impossible, so this deterministically cancels the caller while its owner is queued.
    std::future::poll_fn(|cx| {
        assert!(std::future::Future::poll(pending.as_mut(), cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(pending);
    drop(assembly_owner);
    composition
        .wait_owned_shutdown(std::time::Duration::from_secs(5))
        .await
        .unwrap();
    assert!(fixture.executions().await.executions.is_empty());
    assert!(old.freeze_reference().is_err());
    // Caller cancellation does not authorize filesystem or durable installation deletion.
    let artifact = fixture
        .runtime
        .store
        .get_artifact_instance(&fixture.node_id, fixture.installation_id)
        .await
        .unwrap()
        .unwrap();
    assert!(std::path::Path::new(artifact.local_path.as_deref().unwrap()).exists());
    fixture.runtime.host.stop().await.unwrap();
}

#[tokio::test]
async fn family_uninstall_reconciles_interrupted_disable_with_missing_sibling() {
    let fixture = Fixture::new().await;
    let old = fixture
        .runtime
        .composition
        .snapshot(fixture.actor.current_workspace_id)
        .await
        .unwrap();
    let mut sibling_manifest = manifest("a");
    sibling_manifest["version"] = "1.0.1".into();
    sibling_manifest["managed"]["module"]["module_version"] = "1.0.1".into();
    let sibling = fixture
        .management
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: fixture.actor.user_id,
            file_name: "removal-missing-sibling.1flowbasepkg".into(),
            package_bytes: package(&sibling_manifest),
        })
        .await
        .unwrap()
        .installation;
    assert_eq!(sibling.desired_state, domain::PluginDesiredState::Disabled);
    let artifact = fixture
        .runtime
        .store
        .get_artifact_instance(&fixture.node_id, sibling.id)
        .await
        .unwrap()
        .unwrap();
    std::fs::remove_dir_all(artifact.local_path.as_deref().unwrap()).unwrap();
    sqlx::query("update extension_artifact_instances set artifact_status='missing', runtime_status='inactive', local_path=null where node_id=$1 and installation_id=$2")
        .bind(&fixture.node_id).bind(sibling.id).execute(fixture.runtime.store.pool()).await.unwrap();
    // Exact state left by cancellation after disable's durable write but before its
    // runtime rebuild acquires assembly. The production repository writes the state;
    // deliberately do not invoke the runtime to manufacture that interrupted boundary.
    fixture
        .runtime
        .store
        .update_desired_state(&UpdatePluginDesiredStateInput {
            installation_id: fixture.installation_id,
            desired_state: domain::PluginDesiredState::Disabled,
            actor_user_id: fixture.actor.user_id,
        })
        .await
        .unwrap();
    assert!(
        fixture
            .executions()
            .await
            .executions
            .iter()
            .any(|execution| execution.current),
        "fixture must retain the stale current graph after persisting Disabled"
    );
    let task = fixture
        .management
        .delete_family(DeletePluginFamilyCommand {
            actor_user_id: fixture.actor.user_id,
            provider_code: "acme.composition-a".into(),
        })
        .await
        .unwrap();
    assert_eq!(task.status, domain::PluginTaskStatus::Succeeded);
    assert!(fixture.executions().await.executions.is_empty());
    assert!(old.freeze_reference().is_err());
    let removed = fixture
        .runtime
        .store
        .get_artifact_instance(&fixture.node_id, fixture.installation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        removed.artifact_status,
        domain::PluginArtifactInstanceStatus::Missing
    );
    fixture.runtime.host.stop().await.unwrap();
}
