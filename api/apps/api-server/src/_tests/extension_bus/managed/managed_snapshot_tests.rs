//! Root #2007 AC-008 / AUTH-05/09. Real SDK processes, official package intake and durable
//! Outbox rows cross G1 -> G2 and reconstructed runtime/composition instances. Never skipped.
use super::managed_activation_tests::{hook_input, hook_invocation, principal};
use super::managed_event_authority_tests::{grant, manifest, package};
use crate::extension_bus::{ManagedExtensionComposition, ManagedWorkspacePublicationSource};
use crate::host_extensions::lifecycle::ApiLifecycleFactDelivery;
use crate::provider_runtime::{ApiProviderRuntime, ApiRuntimeArtifactResolver, ApiRuntimeServices};
use control_plane::{
    lifecycle_outbox_dispatcher::{
        LifecycleDeliveryCompletionPort, LifecycleFactDeliveryCompletion,
        LifecycleFactDeliveryPort, LifecycleOutboxDispatcher,
    },
    plugin_management::*,
    ports::AuthRepository,
};
use control_plane_contracts::ports::*;
use extension_contracts::*;
use plugin_framework::extension_bus::ContributionId;
use std::{path::PathBuf, sync::Arc};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

include!("runtime_fixture.rs");

fn mixed_package() -> Vec<u8> {
    let mut a = manifest("a");
    let original: serde_json::Value = serde_yaml::from_str(include_str!(
        "../../../../../../plugins/fixtures/acme.composition-a/manifest.yaml"
    ))
    .unwrap();
    let original = &original["managed"];
    a["managed"]["module"]["contributions"]
        .as_array_mut()
        .unwrap()
        .push(
            original["module"]["contributions"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["contribution_id"] == "acme.composition-a.first")
                .unwrap()
                .clone(),
        );
    let mut hook = original["execution_bindings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["contribution_id"] == "acme.composition-a.first")
        .unwrap()
        .clone();
    hook["runtime"]["entry"] = "bin/hook".into();
    a["managed"]["execution_bindings"]
        .as_array_mut()
        .unwrap()
        .push(hook);
    let bytes = serde_yaml::to_string(&a).unwrap().into_bytes();
    plugin_framework::parse_plugin_manifest(std::str::from_utf8(&bytes).unwrap()).unwrap();
    let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive
        .append_data(&mut header, "manifest.yaml", bytes.as_slice())
        .unwrap();
    for (variable, entry) in [
        ("MANAGED_EVENT_WORKER_FIXTURE", "bin/worker.py"),
        ("MANAGED_HOOK_WORKER_FIXTURE", "bin/hook"),
    ] {
        let source = PathBuf::from(
            std::env::var_os(variable).expect("CI must prebuild both real managed SDK examples"),
        );
        assert!(
            source.is_absolute() && source.is_file(),
            "real SDK executable required: {variable}"
        );
        archive.append_path_with_name(source, entry).unwrap();
    }
    archive.into_inner().unwrap().finish().unwrap()
}

pub(super) async fn create(store: &MainDurableStore, actor: Uuid, workspace: Uuid) {
    store
        .create_model_definition(&CreateModelDefinitionInput {
            bound_storage_id: None,
            actor_user_id: actor,
            scope_kind: domain::DataModelScopeKind::Workspace,
            scope_id: workspace,
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            template_provider: "acme-fixture".into(),
            template_code: "snapshot".into(),
            template_version: "1".into(),
            code: format!("snapshot_{}", Uuid::now_v7().simple()),
            title: "Frozen generation".into(),
            description: None,
            status: domain::DataModelStatus::Published,
            protection: domain::DataModelProtection::default(),
        })
        .await
        .unwrap();
}

struct Completion;
impl LifecycleDeliveryCompletionPort for Completion {
    fn complete(&self, _: CompletionOutcome<LifecycleFactDeliveryCompletion>) {}
}
fn paused(error: anyhow::Error, reason: LifecycleDeliveryPauseReason) {
    assert_eq!(
        error
            .downcast_ref::<LifecycleDeliveryBlocked>()
            .map(|e| e.0),
        Some(reason)
    );
}

#[tokio::test]
async fn root_2007_ac_008_snapshot_restart() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let actor_id: Uuid = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let actor = AuthRepository::load_actor_context_for_user(&state.store, actor_id)
        .await
        .unwrap();
    let workspace = actor.current_workspace_id;
    let runtime = RuntimeFixture::new(&state);
    let management = PluginManagementService::new(
        runtime.store.clone(),
        ApiProviderRuntime::new(runtime.services.clone()),
        state.official_plugin_source.clone(),
        &state.provider_install_root,
    )
    .with_node_id(&state.api_node_id)
    .with_managed_activation();
    let authority = PluginContributionAuthorityService::new(
        runtime.store.clone(),
        HostContributionGrantPolicy::root_composition(),
    );
    let mut installations = Vec::new();
    for name in ["a", "b", "c"] {
        let installed = management
            .install_uploaded_plugin(InstallUploadedPluginCommand {
                actor_user_id: actor_id,
                file_name: format!("acme.composition-{name}.1flowbasepkg"),
                package_bytes: if name == "a" {
                    mixed_package()
                } else {
                    package(&manifest(name))
                },
            })
            .await
            .unwrap();
        let id = installed.installation.id;
        management
            .assign_plugin(AssignPluginCommand {
                actor_user_id: actor_id,
                installation_id: id,
            })
            .await
            .unwrap();
        authority
            .grant(&actor, id, grant(name, "event.subscribe"))
            .await
            .unwrap();
        if name == "a" {
            authority
                .grant(&actor, id, grant(name, "event.publish"))
                .await
                .unwrap();
            authority
                .grant(
                    &actor,
                    id,
                    GrantContributionPermission {
                        contribution_id: "acme.composition-a.first".into(),
                        permission: "hook.model_definitions.create.before".into(),
                        resource_scope: domain::ContributionResourceScope::Workspace,
                        permission_contract_id: "managed-hook".into(),
                        permission_contract_version: "1".into(),
                    },
                )
                .await
                .unwrap();
        }
        if name != "c" {
            management
                .enable_plugin(EnablePluginCommand {
                    actor_user_id: actor_id,
                    installation_id: id,
                })
                .await
                .unwrap();
        }
        installations.push(installed);
    }
    let g1 = runtime.composition.snapshot(workspace).await.unwrap();
    create(&runtime.store, actor_id, workspace).await;
    let worker_id = Uuid::now_v7();
    let records = runtime
        .store
        .claim_lifecycle_facts(worker_id, 32, time::Duration::minutes(5))
        .await
        .unwrap();
    let old = records
        .iter()
        .find(|r| r.subscriber_id.ends_with("acme.composition-a.events"))
        .unwrap()
        .clone();
    assert_eq!(old.graph_fingerprint, g1.graph.fingerprint().as_str());
    for record in records
        .iter()
        .filter(|r| r.subscriber_id != old.subscriber_id)
    {
        runtime.delivery.deliver(record).await.unwrap();
        runtime
            .store
            .mark_lifecycle_fact_delivered(
                record.event_id,
                &record.subscriber_id,
                worker_id,
                record.claim_id.unwrap(),
            )
            .await
            .unwrap();
    }

    // A real SDK Hook has passed host admission and is waiting on an explicit file barrier.
    let hook_path = PathBuf::from(installations[0].local_artifact.local_path.as_ref().unwrap())
        .join("bin/hook");
    let owner = runtime.composition.clone();
    let frozen = g1.clone();
    let running = tokio::spawn(async move {
        owner
            .execute_hook(
                &frozen,
                &ContributionId::new("acme.composition-a.first").unwrap(),
                principal(workspace, actor_id),
                hook_invocation(&frozen),
                hook_input("fixture_barrier"),
            )
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while !hook_path.with_extension("started").is_file() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: actor_id,
            installation_id: installations[2].installation.id,
        })
        .await
        .unwrap();
    let g2 = runtime.composition.snapshot(workspace).await.unwrap();
    assert_ne!(g1.graph.fingerprint(), g2.graph.fingerprint());
    assert!(
        !running.is_finished(),
        "G2 publication must not complete or replace the admitted G1 worker"
    );
    std::fs::write(hook_path.with_extension("release"), "release").unwrap();
    assert_eq!(
        running.await.unwrap().unwrap(),
        ManagedHookOutcome::Continue
    );
    assert!(Arc::ptr_eq(
        &g1,
        &runtime
            .composition
            .event_snapshot_for_graph(&old)
            .await
            .unwrap()
            .unwrap()
    ));
    // Both a subsequent phase on G1 and G1's durable target keep the exact original handler.
    assert_eq!(
        runtime
            .composition
            .execute_hook(
                &g1,
                &ContributionId::new("acme.composition-a.first").unwrap(),
                principal(workspace, actor_id),
                hook_invocation(&g1),
                hook_input("identify")
            )
            .await
            .unwrap(),
        ManagedHookOutcome::Deny {
            classification: "fixture.first".into()
        }
    );
    runtime.delivery.deliver(&old).await.unwrap();
    let event_path = PathBuf::from(installations[0].local_artifact.local_path.as_ref().unwrap())
        .join("bin/worker.py");
    let frame: ManagedEventHostFrame = serde_json::from_str(
        std::fs::read_to_string(event_path.with_extension("trace"))
            .unwrap()
            .lines()
            .last()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(frame.graph_fingerprint, old.graph_fingerprint);
    assert_eq!(
        frame.execution_identity,
        *g1.bindings
            .get(&ContributionId::new("acme.composition-a.events").unwrap())
            .unwrap()
            .handle
            .identity()
    );
    assert_eq!(frame.handler, "publish");
    let e2_count: i64 = sqlx::query_scalar(
        "select count(*) from lifecycle_outbox where contract_id='acme.composition-a.processed'",
    )
    .fetch_one(runtime.store.pool())
    .await
    .unwrap();
    assert_eq!(e2_count, 1);

    // Mounted executables are pinned by content. A replacement or missing old path cannot run.
    let original_bytes = std::fs::read(&event_path).unwrap();
    std::fs::write(&event_path, b"replaced artifact").unwrap();
    paused(
        runtime.delivery.deliver(&old).await.unwrap_err(),
        LifecycleDeliveryPauseReason::FrozenHandlerUnavailable,
    );
    std::fs::write(&event_path, original_bytes).unwrap();
    let saved = event_path.with_extension("saved");
    std::fs::rename(&event_path, &saved).unwrap();
    paused(
        runtime.delivery.deliver(&old).await.unwrap_err(),
        LifecycleDeliveryPauseReason::FrozenHandlerUnavailable,
    );
    std::fs::rename(&saved, &event_path).unwrap();

    // A retained graph never resurrects revoked permission; the failure is a durable pause reason.
    let grants = authority
        .query(&actor, installations[0].installation.id)
        .await
        .unwrap();
    let authorization_id = grants
        .authorizations
        .iter()
        .find(|g| g.permission == "event.subscribe")
        .unwrap()
        .id;
    authority
        .revoke(
            &actor,
            installations[0].installation.id,
            RevokeContributionPermission {
                authorization_id,
                expected_revision: grants.revision,
            },
        )
        .await
        .unwrap();
    paused(
        runtime.delivery.deliver(&old).await.unwrap_err(),
        LifecycleDeliveryPauseReason::AuthorityRevoked,
    );
    let stale_retry = runtime
        .store
        .retry_lifecycle_fact(
            old.event_id,
            &old.subscriber_id,
            worker_id,
            old.claim_id.unwrap(),
            time::OffsetDateTime::now_utc(),
            "revoked claim must not regain pending state",
        )
        .await
        .unwrap_err();
    assert!(stale_retry.is::<LifecycleClaimLost>(), "{stale_retry:#}");
    let exact_resume = ResumeManagedLifecycleDelivery {
        event_id: old.event_id,
        subscriber_id: old.subscriber_id.clone(),
        expected: ManagedFrozenExecutionTarget {
            graph_fingerprint: old.graph_fingerprint.clone(),
            handler_id: old.handler_id.clone(),
            handler_version: old.handler_version.clone(),
        },
    };
    authority
        .grant(
            &actor,
            installations[0].installation.id,
            grant("a", "event.subscribe"),
        )
        .await
        .unwrap();
    let still_paused = runtime
        .store
        .managed_lifecycle_delivery(installations[0].installation.id, workspace, &exact_resume)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        still_paused.status,
        LifecycleOutboxStatus::Paused,
        "regrant never implicitly resumes a revoked delivery"
    );
    assert_eq!(
        still_paused.pause_reason,
        Some(LifecycleDeliveryPauseReason::AuthorityRevoked)
    );
    assert!(still_paused.claim_id.is_none());
    ManagedExecutionService::new(runtime.store.clone(), runtime.composition.governance())
        .resume(
            &actor,
            installations[0].installation.id,
            exact_resume.clone(),
        )
        .await
        .unwrap();
    let resumed = runtime
        .store
        .managed_lifecycle_delivery(installations[0].installation.id, workspace, &exact_resume)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(resumed.status, LifecycleOutboxStatus::Pending);
    assert_eq!(resumed.graph_fingerprint, old.graph_fingerprint);
    assert_eq!(resumed.handler_id, old.handler_id);
    assert_eq!(resumed.handler_version, old.handler_version);
    // The new authority revision changes a graph; retain a current-graph durable target too.
    runtime
        .composition
        .rebuild_installation(installations[0].installation.id)
        .await
        .unwrap();
    let before_restart = runtime.composition.snapshot(workspace).await.unwrap();
    create(&runtime.store, actor_id, workspace).await;
    let pending = runtime
        .store
        .claim_lifecycle_facts(worker_id, 32, time::Duration::minutes(5))
        .await
        .unwrap();
    let reclaimed_old = pending
        .iter()
        .find(|record| record.event_id == old.event_id && record.subscriber_id == old.subscriber_id)
        .expect("explicitly resumed original target must acquire a fresh claim");
    assert!(reclaimed_old.claim_id.is_some());
    assert_ne!(reclaimed_old.claim_id, old.claim_id);
    assert_eq!(reclaimed_old.graph_fingerprint, old.graph_fingerprint);
    assert_eq!(reclaimed_old.handler_id, old.handler_id);
    assert_eq!(reclaimed_old.handler_version, old.handler_version);
    let current = pending
        .iter()
        .find(|r| {
            r.contract_id == MANAGED_CREATE_EVENT_ID
                && r.event_id != old.event_id
                && r.subscriber_id.ends_with("acme.composition-a.events")
        })
        .unwrap()
        .clone();
    assert_eq!(
        current.graph_fingerprint,
        before_restart.graph.fingerprint().as_str()
    );
    for record in &pending {
        runtime
            .store
            .retry_lifecycle_fact(
                record.event_id,
                &record.subscriber_id,
                worker_id,
                record.claim_id.unwrap(),
                time::OffsetDateTime::now_utc(),
                "simulate process restart",
            )
            .await
            .unwrap();
    }
    runtime.host.stop().await.unwrap();
    drop(management);
    drop(g1);
    drop(g2);
    drop(before_restart);
    drop(runtime);

    // Real reconstruction: only persisted installation/grants/artifacts are reused, no Arc graph.
    let restarted = RuntimeFixture::new(&state);
    restarted
        .composition
        .rebuild_installation(installations[0].installation.id)
        .await
        .unwrap();
    let restored = restarted.composition.snapshot(workspace).await.unwrap();
    assert_eq!(
        restored.graph.fingerprint().as_str(),
        current.graph_fingerprint
    );
    let new_target = restored
        .publication_plan(MANAGED_CREATE_EVENT_ID, "v1")
        .unwrap()
        .subscribers
        .into_iter()
        .find(|s| s.subscriber_id == current.subscriber_id)
        .unwrap();
    assert_ne!(
        new_target.handler_version, current.handler_version,
        "a reset mount counter must not impersonate the old instance"
    );
    paused(
        restarted.delivery.deliver(&old).await.unwrap_err(),
        LifecycleDeliveryPauseReason::FrozenGraphUnavailable,
    );
    paused(
        restarted.delivery.deliver(&current).await.unwrap_err(),
        LifecycleDeliveryPauseReason::FrozenHandlerUnavailable,
    );
    let dispatcher = LifecycleOutboxDispatcher::new(
        restarted.store.clone(),
        restarted.delivery.clone(),
        Arc::new(Completion),
    );
    assert!(dispatcher.run_once().await.unwrap() >= 2);
    let old_state: (String, String) = sqlx::query_as("select status, pause_reason from lifecycle_outbox_deliveries where event_id=$1 and subscriber_id=$2")
        .bind(old.event_id).bind(&old.subscriber_id).fetch_one(restarted.store.pool()).await.unwrap();
    assert_eq!(
        old_state,
        ("paused".into(), "frozen_graph_unavailable".into())
    );
    let current_state: (String, String) = sqlx::query_as("select status, pause_reason from lifecycle_outbox_deliveries where event_id=$1 and subscriber_id=$2")
        .bind(current.event_id).bind(&current.subscriber_id).fetch_one(restarted.store.pool()).await.unwrap();
    assert_eq!(
        current_state,
        ("paused".into(), "frozen_handler_unavailable".into())
    );
    assert_eq!(
        dispatcher.run_once().await.unwrap(),
        0,
        "unavailable work must not loop as pending"
    );
    restarted.host.stop().await.unwrap();
    drop(dispatcher);
    drop(restarted);

    // A further restart with the old artifact missing cannot reconstruct from another version.
    std::fs::rename(&event_path, &saved).unwrap();
    let missing = RuntimeFixture::new(&state);
    let rebuilt = missing
        .composition
        .rebuild_installation(installations[0].installation.id)
        .await;
    std::fs::rename(&saved, &event_path).unwrap();
    assert!(rebuilt.is_err());
    assert!(missing.composition.snapshot(workspace).await.is_none());
    paused(
        missing.delivery.deliver(&old).await.unwrap_err(),
        LifecycleDeliveryPauseReason::FrozenGraphUnavailable,
    );
    let retained: i64 = sqlx::query_scalar("select count(*) from lifecycle_outbox_deliveries where event_id=$1 and subscriber_id=$2 and status='paused'")
        .bind(old.event_id).bind(&old.subscriber_id).fetch_one(missing.store.pool()).await.unwrap();
    assert_eq!(retained, 1);
    missing.host.stop().await.unwrap();
}

#[tokio::test]
async fn boot_refresh_rebuilds_each_workspace_once_with_all_bindings() {
    exercise_boot_refresh(false).await;
}

#[tokio::test]
async fn boot_refresh_rejects_old_binding_reenabled_after_package_selection() {
    exercise_boot_refresh(true).await;
}

async fn exercise_boot_refresh(reenable_after_selection: bool) {
    use runtime_core::runtime_backend::{RuntimeArtifactReference, RuntimeBackendError};
    use runtime_extension_host::RuntimeArtifactResolver;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingResolver {
        resolver: ApiRuntimeArtifactResolver,
        activations: AtomicUsize,
        store: MainDurableStore,
        reenable: std::sync::Mutex<Option<UpdatePluginDesiredStateInput>>,
    }
    #[async_trait::async_trait]
    impl RuntimeArtifactResolver for CountingResolver {
        async fn resolve(
            &self,
            artifact: &RuntimeArtifactReference,
        ) -> Result<PathBuf, RuntimeBackendError> {
            self.activations.fetch_add(1, Ordering::Relaxed);
            let reenable = self.reenable.lock().unwrap().take();
            if let Some(input) = reenable {
                // A surviving binding reaches the resolver only after prepare_packages has
                // filtered the disabled one. Simulate the other node's durable write now.
                self.store.update_desired_state(&input).await.unwrap();
            }
            self.resolver.resolve(artifact).await
        }
    }

    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let actor_id: Uuid = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let actor = AuthRepository::load_actor_context_for_user(&state.store, actor_id)
        .await
        .unwrap();
    let resolver = Arc::new(CountingResolver {
        resolver: ApiRuntimeArtifactResolver::new(
            state.store.clone(),
            &state.api_node_id,
            &state.provider_install_root,
        ),
        activations: AtomicUsize::new(0),
        store: state.store.clone(),
        reenable: std::sync::Mutex::new(None),
    });
    let runtime = RuntimeFixture::with_artifact_resolver(&state, resolver.clone(), false);
    let management = PluginManagementService::new(
        runtime.store.clone(),
        ApiProviderRuntime::new(runtime.services.clone()),
        state.official_plugin_source.clone(),
        &state.provider_install_root,
    )
    .with_node_id(&state.api_node_id)
    .with_managed_activation();
    let authority = PluginContributionAuthorityService::new(
        runtime.store.clone(),
        HostContributionGrantPolicy::root_composition(),
    );
    let mut ids = Vec::new();
    for name in ["a", "b"] {
        let id = management
            .install_uploaded_plugin(InstallUploadedPluginCommand {
                actor_user_id: actor_id,
                file_name: format!("acme.composition-{name}.1flowbasepkg"),
                package_bytes: package(&manifest(name)),
            })
            .await
            .unwrap()
            .installation
            .id;
        management
            .assign_plugin(AssignPluginCommand {
                actor_user_id: actor_id,
                installation_id: id,
            })
            .await
            .unwrap();
        authority
            .grant(&actor, id, grant(name, "event.subscribe"))
            .await
            .unwrap();
        if name == "a" {
            authority
                .grant(&actor, id, grant(name, "event.publish"))
                .await
                .unwrap();
        }
        management
            .enable_plugin(EnablePluginCommand {
                actor_user_id: actor_id,
                installation_id: id,
            })
            .await
            .unwrap();
        ids.push(id);
    }
    let before = runtime
        .composition
        .snapshot(actor.current_workspace_id)
        .await
        .unwrap();
    assert!(ids
        .iter()
        .all(|id| before.bindings.values().any(|b| b.installation.id == *id)));
    let module = plugin_framework::extension_bus::compile_managed_interface_module([
        "notifications.instances.list",
    ])
    .unwrap();
    runtime
        .composition
        .attach_interface_module(module.clone())
        .unwrap();
    resolver.activations.store(0, Ordering::Relaxed);
    if reenable_after_selection {
        let target = UpdatePluginDesiredStateInput {
            installation_id: ids[1],
            desired_state: domain::PluginDesiredState::Disabled,
            actor_user_id: actor_id,
        };
        let disabled = runtime.store.update_desired_state(&target).await.unwrap();
        *resolver.reenable.lock().unwrap() = Some(UpdatePluginDesiredStateInput {
            desired_state: domain::PluginDesiredState::ActiveRequested,
            ..target
        });
        let error = runtime
            .composition
            .refresh_boot_snapshots()
            .await
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("managed candidate authority or installation revision changed"));
        assert_eq!(resolver.activations.load(Ordering::Relaxed), 1);
        assert!(Arc::ptr_eq(
            &before,
            &runtime
                .composition
                .snapshot(actor.current_workspace_id)
                .await
                .unwrap()
        ));
        let reenabled = runtime
            .store
            .get_installation(ids[1])
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            reenabled.desired_state,
            domain::PluginDesiredState::ActiveRequested
        );
        assert_ne!(disabled.updated_at, reenabled.updated_at);
        runtime.host.stop().await.unwrap();
        return;
    }
    runtime.composition.refresh_boot_snapshots().await.unwrap();
    let after = runtime
        .composition
        .snapshot(actor.current_workspace_id)
        .await
        .unwrap();
    assert_eq!(
        resolver.activations.load(Ordering::Relaxed),
        before.bindings.len()
    );
    assert_eq!(before.bindings.len(), after.bindings.len());
    for (id, binding) in &before.bindings {
        assert_eq!(after.bindings[id].handle, binding.handle);
    }
    assert!(module.extension_points.iter().all(|declared| after
        .graph
        .points()
        .iter()
        .any(|point| point.descriptor() == declared)));
    runtime.host.stop().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn boot_refresh_holds_db_lock_until_visible_and_pinned_before_release() {
    use futures_util::FutureExt;

    let (state, database_url) = crate::_tests::support::test_api_state_with_database_url().await;
    let actor_id: Uuid = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let actor = AuthRepository::load_actor_context_for_user(&state.store, actor_id)
        .await
        .unwrap();
    let runtime = RuntimeFixture::new(&state);
    let management = PluginManagementService::new(
        runtime.store.clone(),
        ApiProviderRuntime::new(runtime.services.clone()),
        state.official_plugin_source.clone(),
        &state.provider_install_root,
    )
    .with_node_id(&state.api_node_id)
    .with_managed_activation();
    let authority = PluginContributionAuthorityService::new(
        runtime.store.clone(),
        HostContributionGrantPolicy::root_composition(),
    );
    let id = management
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: actor_id,
            file_name: "acme.composition-a.1flowbasepkg".into(),
            package_bytes: package(&manifest("a")),
        })
        .await
        .unwrap()
        .installation
        .id;
    management
        .assign_plugin(AssignPluginCommand {
            actor_user_id: actor_id,
            installation_id: id,
        })
        .await
        .unwrap();
    authority
        .grant(&actor, id, grant("a", "event.subscribe"))
        .await
        .unwrap();
    management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: actor_id,
            installation_id: id,
        })
        .await
        .unwrap();

    let before = runtime
        .composition
        .snapshot(actor.current_workspace_id)
        .await
        .unwrap();
    let module = plugin_framework::extension_bus::compile_managed_interface_module([
        "notifications.instances.list",
    ])
    .unwrap();
    runtime
        .composition
        .attach_interface_module(module.clone())
        .unwrap();

    type PinnedSnapshots = std::collections::BTreeMap<
        Uuid,
        Arc<api_host_kernel::extension_bus::ManagedWorkspaceSnapshot>,
    >;
    let pinned_once: Arc<std::sync::OnceLock<PinnedSnapshots>> =
        Arc::new(std::sync::OnceLock::new());

    sqlx::query("delete from event_catalog")
        .execute(runtime.store.pool())
        .await
        .unwrap();
    eprintln!("boot lock probe: fixture ready; starting rollback refresh");
    // 1. Pre-commit failure after SQL staging rolls back visible state AND leaves OnceLock unpinned.
    *runtime.composition.before_authority_release.lock().unwrap() = Some(Box::pin(async {
        eprintln!("boot lock probe: rollback attempt reached pre-commit hook");
        anyhow::bail!("simulated pre-commit failure")
    }));
    let state_fail = state.clone();
    let pin_fail = pinned_once.clone();
    let err = runtime
        .composition
        .refresh_boot_snapshots_with_event_catalog_and_pin(
            move |snapshots| {
                Box::pin(async move {
                    let catalog =
                        crate::event_center::persistence::snapshot(&state_fail, &snapshots).await?;
                    eprintln!(
                        "boot lock probe: rollback catalog prepared; acquiring authority batch"
                    );
                    Ok(catalog)
                })
            },
            move |published| {
                pin_fail
                    .set(published.clone())
                    .map_err(|_| anyhow::anyhow!("already pinned"))
            },
        )
        .await
        .err()
        .unwrap();
    assert!(err.to_string().contains("simulated pre-commit failure"));
    assert!(pinned_once.get().is_none());
    assert!(Arc::ptr_eq(
        &before,
        &runtime
            .composition
            .snapshot(actor.current_workspace_id)
            .await
            .unwrap()
    ));

    let rolled_back_events: i64 = sqlx::query_scalar("select count(*) from event_catalog")
        .fetch_one(runtime.store.pool())
        .await
        .unwrap();
    assert_eq!(rolled_back_events, 0, "staged catalog must roll back");

    // 2. Real event_catalog SQL runs inside the authority transaction; before release, both
    //    local snapshots mutex AND PG locks are held and OnceLock is still unset; after commit,
    //    event_catalog rows are durable and OnceLock is pinned to the exact published snapshot.
    // Open the independent observer before acquiring the authority lease. Never check out
    // a second connection from the fixture's one-slot pool inside the publication barrier.
    let mut conn = <sqlx::PgConnection as sqlx::Connection>::connect(&database_url)
        .await
        .unwrap();
    eprintln!("boot lock probe: rollback verified; starting commit refresh");
    let composition_ref = runtime.composition.clone();
    let workspace_id = actor.current_workspace_id;
    let pin_check = pinned_once.clone();
    *runtime.composition.before_authority_release.lock().unwrap() = Some(Box::pin(async move {
        eprintln!("boot lock probe: SQL staged, before authority release");
        assert!(
            pin_check.get().is_none(),
            "OnceLock must not pin before COMMIT"
        );
        // Poll the actual reader without relying on scheduling a timer on another worker.
        assert!(
            tokio::task::unconstrained(composition_ref.current_snapshots())
                .now_or_never()
                .is_none(),
            "readers must remain blocked on snapshots mutex until authority lease commits"
        );
        let mut tx = sqlx::Acquire::begin(&mut conn).await.unwrap();
        sqlx::query("set local lock_timeout = '50ms'")
            .execute(&mut *tx)
            .await
            .unwrap();
        let event_lock_free: bool = sqlx::query_scalar(
            "select pg_try_advisory_xact_lock(hashtextextended('1flowbase.event_catalog', 0))",
        )
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        assert!(
            !event_lock_free,
            "1flowbase.event_catalog advisory lock must be held inside stage_managed_settings_templates"
        );
        let uncommitted_visible: i64 = sqlx::query_scalar("select count(*) from event_catalog")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        assert_eq!(
            uncommitted_visible, 0,
            "event catalog SQL must be staged inside the uncommitted authority transaction"
        );
        let lock_err =
            sqlx::query("update extension_installations set updated_at = now() where id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await
                .unwrap_err();
        assert_eq!(
            lock_err
                .as_database_error()
                .and_then(|error| error.code())
                .as_deref(),
            Some("55P03"),
            "competing installation write must fail with lock_not_available: {lock_err}"
        );
        tx.rollback().await.unwrap();
        Ok(())
    }));
    let state_ok = state.clone();
    let composition_at_pin = runtime.composition.clone();
    let pin_ok = pinned_once.clone();
    let returned = runtime
        .composition
        .refresh_boot_snapshots_with_event_catalog_and_pin(
            move |snapshots| {
                Box::pin(async move {
                    let catalog =
                        crate::event_center::persistence::snapshot(&state_ok, &snapshots).await?;
                    eprintln!(
                        "boot lock probe: commit catalog prepared; acquiring authority batch"
                    );
                    Ok(catalog)
                })
            },
            move |published| {
                eprintln!("boot lock probe: committed; pinning published snapshots");
                assert!(
                    tokio::task::unconstrained(composition_at_pin.current_snapshots())
                        .now_or_never()
                        .is_none(),
                    "readers must still be blocked while the committed snapshot is pinned"
                );
                pin_ok
                    .set(published.clone())
                    .map_err(|_| anyhow::anyhow!("already pinned"))
            },
        )
        .await
        .unwrap();
    eprintln!("boot lock probe: publication and inline reclamation returned");
    let after = runtime.composition.snapshot(workspace_id).await.unwrap();
    assert!(
        !Arc::ptr_eq(&before, &after),
        "refresh must replace the snapshot"
    );
    assert!(Arc::ptr_eq(&returned[&workspace_id], &after));
    assert!(Arc::ptr_eq(
        &pinned_once.get().unwrap()[&workspace_id],
        &after
    ));
    let committed_events: i64 = sqlx::query_scalar("select count(*) from event_catalog")
        .fetch_one(runtime.store.pool())
        .await
        .unwrap();
    assert!(
        committed_events > 0,
        "event catalog must commit with authority lease"
    );
    runtime.host.stop().await.unwrap();
}

fn management_for_snapshot(
    state: &crate::app_state::ApiState,
    runtime: &RuntimeFixture,
) -> PluginManagementService<MainDurableStore, ApiProviderRuntime> {
    PluginManagementService::new(
        runtime.store.clone(),
        ApiProviderRuntime::new(runtime.services.clone()),
        state.official_plugin_source.clone(),
        &state.provider_install_root,
    )
    .with_node_id(&state.api_node_id)
    .with_managed_activation()
}

async fn install_event_candidate(
    runtime: &RuntimeFixture,
    management: &PluginManagementService<MainDurableStore, ApiProviderRuntime>,
    actor: &domain::ActorContext,
    name: &str,
    version: &str,
) -> Uuid {
    let mut declaration = manifest(name);
    declaration["version"] = version.into();
    declaration["managed"]["module"]["module_version"] = version.into();
    let installed = management
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: actor.user_id,
            file_name: format!("acme.composition-{name}.1flowbasepkg"),
            package_bytes: package(&declaration),
        })
        .await
        .unwrap();
    let id = installed.installation.id;
    // The first installation needs an assignment before granting; a staged sibling
    // may instead receive candidate grants via the already assigned family.
    if version == "1.0.0" {
        management
            .assign_plugin(AssignPluginCommand {
                actor_user_id: actor.user_id,
                installation_id: id,
            })
            .await
            .unwrap();
    }
    let authority = PluginContributionAuthorityService::new(
        runtime.store.clone(),
        HostContributionGrantPolicy::root_composition(),
    )
    .with_node_id(installed.local_artifact.node_id);
    authority
        .grant(actor, id, grant(name, "event.subscribe"))
        .await
        .unwrap();
    if name == "a" {
        authority
            .grant(actor, id, grant(name, "event.publish"))
            .await
            .unwrap();
    }
    id
}

#[tokio::test]
async fn empty_boot_refresh_rejects_first_active_managed_binding_added_during_preparation() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let actor_id = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let actor = AuthRepository::load_actor_context_for_user(&state.store, actor_id)
        .await
        .unwrap();
    let runtime = RuntimeFixture::new(&state);
    runtime
        .composition
        .attach_interface_module(
            plugin_framework::extension_bus::compile_managed_interface_module([
                "notifications.instances.list",
            ])
            .unwrap(),
        )
        .unwrap();
    assert!(runtime.composition.current_snapshots().await.is_empty());
    let pinned = Arc::new(std::sync::OnceLock::new());
    let pin = pinned.clone();
    let writer_state = state.clone();
    let error = runtime
        .composition
        .refresh_boot_snapshots_with_event_catalog_and_pin(
            move |snapshots| {
                Box::pin(async move {
                    assert!(snapshots.is_empty());
                    let writer = RuntimeFixture::new(&writer_state);
                    let management = management_for_snapshot(&writer_state, &writer);
                    let id =
                        install_event_candidate(&writer, &management, &actor, "a", "1.0.0").await;
                    management
                        .enable_plugin(EnablePluginCommand {
                            actor_user_id: actor_id,
                            installation_id: id,
                        })
                        .await
                        .unwrap();
                    writer.host.stop().await.unwrap();
                    crate::event_center::persistence::snapshot(&writer_state, &snapshots).await
                })
            },
            move |published| {
                assert!(pin.set(published.clone()).is_ok());
                Ok(())
            },
        )
        .await
        .err()
        .expect("empty boot unexpectedly published before observing the new binding");
    assert!(
        error
            .to_string()
            .contains("managed candidate authority scope changed"),
        "{error:#}"
    );
    assert!(pinned.get().is_none());
    assert!(runtime.composition.current_snapshots().await.is_empty());
    let catalog_rows = || async {
        sqlx::query_scalar::<_, i64>("select count(*) from event_catalog")
            .fetch_one(state.store.pool())
            .await
            .unwrap()
    };
    assert_eq!(catalog_rows().await, 0);

    let writer = RuntimeFixture::new(&state);
    let id = sqlx::query_scalar("select id from extension_installations where desired_state = 'active_requested' and plugin_id like 'acme.composition-a%'")
        .fetch_one(state.store.pool()).await.unwrap();
    management_for_snapshot(&state, &writer)
        .disable_plugin(DisablePluginCommand {
            actor_user_id: actor_id,
            installation_id: id,
        })
        .await
        .unwrap();
    let boot_state = state.clone();
    let pin = pinned.clone();
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
    assert!(visible.is_empty());
    assert!(runtime.composition.current_snapshots().await.is_empty());
    assert!(pinned.get().unwrap().is_empty());
    assert!(
        catalog_rows().await > 0,
        "empty boot still commits the host catalog"
    );
    writer.host.stop().await.unwrap();
    runtime.host.stop().await.unwrap();
}

#[tokio::test]
async fn boot_refresh_rejects_new_active_managed_binding_added_during_preparation() {
    exercise_new_boot_binding(false).await;
}

#[tokio::test]
async fn boot_refresh_rejects_first_binding_in_a_previously_unseen_workspace() {
    exercise_new_boot_binding(true).await;
}

async fn exercise_new_boot_binding(new_workspace: bool) {
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
    let a = install_event_candidate(&runtime, &management, &actor, "a", "1.0.0").await;
    management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: actor_id,
            installation_id: a,
        })
        .await
        .unwrap();
    let workspace = actor.current_workspace_id;
    let before = runtime.composition.snapshot(workspace).await.unwrap();
    assert_eq!(before.bindings.len(), 1);
    runtime
        .composition
        .attach_interface_module(
            plugin_framework::extension_bus::compile_managed_interface_module([
                "notifications.instances.list",
            ])
            .unwrap(),
        )
        .unwrap();
    let pinned = Arc::new(std::sync::OnceLock::new());
    let pin = pinned.clone();
    let catalog_rows = |store: MainDurableStore| async move {
        sqlx::query_scalar::<_, i64>("select count(*) from event_catalog")
            .fetch_one(store.pool())
            .await
            .unwrap()
    };
    assert_eq!(catalog_rows(runtime.store.clone()).await, 0);
    let writer_actor = if new_workspace {
        let workspace_id = Uuid::now_v7();
        sqlx::query(
            "insert into workspaces (id, tenant_id, name) values ($1, $2, 'new boot scope')",
        )
        .bind(workspace_id)
        .bind(actor.tenant_id)
        .execute(state.store.pool())
        .await
        .unwrap();
        let writer = state
            .store
            .upsert_root_user(
                workspace_id,
                "boot-scope-writer",
                "boot-scope-writer@example.test",
                "unused-test-password-hash",
                "Boot writer",
                "Boot writer",
            )
            .await
            .unwrap();
        let actor = AuthRepository::load_actor_context_for_user(&state.store, writer.id)
            .await
            .unwrap();
        assert_eq!(actor.current_workspace_id, workspace_id);
        actor
    } else {
        actor
    };
    let state_for_writer = state.clone();
    let error = runtime
        .composition
        .refresh_boot_snapshots_with_event_catalog_and_pin(
            move |snapshots| {
                Box::pin(async move {
                    assert_eq!(snapshots[&workspace].bindings.len(), 1);
                    // A separate composition owns the concurrent write; it does not share the boot assembly mutex.
                    let state = state_for_writer;
                    let writer = RuntimeFixture::new(&state);
                    let management = management_for_snapshot(&state, &writer);
                    let added = if new_workspace {
                        // A is already active in W1. Its first assignment in W2 is a new
                        // active authority scope even though the installation already exists.
                        management
                            .assign_plugin(AssignPluginCommand {
                                actor_user_id: writer_actor.user_id,
                                installation_id: a,
                            })
                            .await
                            .unwrap();
                        let authority = PluginContributionAuthorityService::new(
                            writer.store.clone(),
                            HostContributionGrantPolicy::root_composition(),
                        )
                        .with_node_id(&state.api_node_id);
                        for permission in ["event.subscribe", "event.publish"] {
                            authority
                                .grant(&writer_actor, a, grant("a", permission))
                                .await
                                .unwrap();
                        }
                        writer.composition.rebuild_installation(a).await.unwrap();
                        a
                    } else {
                        let b = install_event_candidate(
                            &writer,
                            &management,
                            &writer_actor,
                            "b",
                            "1.0.0",
                        )
                        .await;
                        management
                            .enable_plugin(EnablePluginCommand {
                                actor_user_id: writer_actor.user_id,
                                installation_id: b,
                            })
                            .await
                            .unwrap();
                        b
                    };
                    let new = writer
                        .composition
                        .snapshot(writer_actor.current_workspace_id)
                        .await
                        .unwrap();
                    assert!(new
                        .bindings
                        .values()
                        .any(|binding| binding.installation.id == added));
                    writer.host.stop().await.unwrap();
                    crate::event_center::persistence::snapshot(&state, &snapshots).await
                })
            },
            move |published| {
                pin.set(published.clone())
                    .map_err(|_| anyhow::anyhow!("already pinned"))
            },
        )
        .await
        .err()
        .expect("boot published a candidate that omits the concurrent binding");
    assert!(
        error
            .to_string()
            .contains("managed candidate authority scope changed"),
        "{error:#}"
    );
    assert_eq!(catalog_rows(runtime.store.clone()).await, 0);
    assert!(pinned.get().is_none());
    assert!(Arc::ptr_eq(
        &before,
        &runtime.composition.snapshot(workspace).await.unwrap()
    ));
    runtime.host.stop().await.unwrap();
}

#[tokio::test]
async fn managed_switch_rejects_replaced_ready_node_artifact_after_preparation() {
    use runtime_core::runtime_backend::{RuntimeArtifactReference, RuntimeBackendError};
    use runtime_extension_host::RuntimeArtifactResolver;

    struct ReplacingResolver {
        resolver: ApiRuntimeArtifactResolver,
        store: MainDurableStore,
        node_id: String,
        target: std::sync::OnceLock<Uuid>,
    }
    #[async_trait::async_trait]
    impl RuntimeArtifactResolver for ReplacingResolver {
        async fn resolve(
            &self,
            artifact: &RuntimeArtifactReference,
        ) -> Result<PathBuf, RuntimeBackendError> {
            let path = self.resolver.resolve(artifact).await?;
            if let Some(&target) = self
                .target
                .get()
                .filter(|target| target.to_string() == artifact.as_str())
            {
                // prepare_package has already captured the row, and the first switch lease is released.
                let result = sqlx::query("update extension_artifact_instances set manifest_fingerprint = 'sha256:replacement' where node_id = $1 and installation_id = $2 and artifact_status = 'ready'")
                    .bind(&self.node_id).bind(target).execute(self.store.pool()).await.unwrap();
                assert_eq!(result.rows_affected(), 1);
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
    let resolver = Arc::new(ReplacingResolver {
        resolver: ApiRuntimeArtifactResolver::new(
            state.store.clone(),
            &state.api_node_id,
            &state.provider_install_root,
        ),
        store: state.store.clone(),
        node_id: state.api_node_id.clone(),
        target: std::sync::OnceLock::new(),
    });
    let runtime = RuntimeFixture::with_artifact_resolver(&state, resolver.clone(), false);
    let management = management_for_snapshot(&state, &runtime);
    let current = install_event_candidate(&runtime, &management, &actor, "a", "1.0.0").await;
    management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: actor_id,
            installation_id: current,
        })
        .await
        .unwrap();
    let target = install_event_candidate(&runtime, &management, &actor, "a", "2.0.0").await;
    resolver.target.set(target).unwrap();
    let workspace = actor.current_workspace_id;
    let before = runtime.composition.snapshot(workspace).await.unwrap();
    let authority_facts = || async {
        sqlx::query_as::<_, (time::OffsetDateTime, i64)>(
            "select i.updated_at, r.revision from extension_installations i join plugin_contribution_authorization_revisions r on r.installation_id = i.id where i.id = $1 and r.workspace_id = $2",
        ).bind(target).bind(workspace).fetch_one(runtime.store.pool()).await.unwrap()
    };
    let expected = authority_facts().await;
    let result = management
        .for_plugin_console_operation(
            domain::ConsolePolicyGroup::other("other.plugins").unwrap(),
            "plugins.families.switch",
        )
        .switch_version(SwitchPluginVersionCommand {
            actor_user_id: actor_id,
            provider_code: "acme.composition-a".into(),
            target_installation_id: target,
        })
        .await;
    let replacement = runtime
        .store
        .get_local_installation(&state.api_node_id, target)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        replacement.artifact.manifest_fingerprint.as_deref(),
        Some("sha256:replacement")
    );
    let error = result.expect_err("switch committed against a different Ready artifact row");
    assert!(
        error
            .to_string()
            .contains("managed candidate node artifact changed during preparation"),
        "{error:#}"
    );
    assert_eq!(authority_facts().await, expected);
    assert_eq!(
        replacement.artifact.artifact_status,
        domain::PluginArtifactInstanceStatus::Ready
    );
    assert_eq!(
        replacement.artifact.runtime_status,
        domain::PluginRuntimeStatus::Inactive
    );
    let source = runtime
        .store
        .get_local_installation(&state.api_node_id, current)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        source.installation.desired_state,
        domain::PluginDesiredState::ActiveRequested
    );
    assert_eq!(
        source.artifact.runtime_status,
        domain::PluginRuntimeStatus::Active
    );
    let assignments = runtime.store.list_assignments(workspace).await.unwrap();
    assert_eq!(
        assignments
            .iter()
            .find(|assignment| assignment.provider_code == "acme.composition-a")
            .unwrap()
            .installation_id,
        current
    );
    assert!(Arc::ptr_eq(
        &before,
        &runtime.composition.snapshot(workspace).await.unwrap()
    ));
    runtime.host.stop().await.unwrap();
}

#[path = "boot_refresh_removal_tests.rs"]
mod boot_refresh_removal_tests;
