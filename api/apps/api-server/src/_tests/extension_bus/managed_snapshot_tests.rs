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

pub(super) struct RuntimeFixture {
    pub(super) host: Arc<runtime_extension_host::RuntimeExtensionHost>,
    pub(super) services: Arc<ApiRuntimeServices>,
    pub(super) composition: Arc<ManagedExtensionComposition>,
    pub(super) delivery: Arc<ApiLifecycleFactDelivery>,
    pub(super) store: MainDurableStore,
}
impl RuntimeFixture {
    pub(super) fn new(state: &crate::app_state::ApiState) -> Self {
        let assembly = crate::extension_bus::assemble_extension_graph_input(
            crate::api_workspace_root().unwrap(),
            crate::extension_bus::DEFAULT_PLUGIN_SET_PATH,
            vec![],
        )
        .unwrap();
        let graph = Arc::new(assembly.compile_graph().unwrap());
        let native = assembly.compile_lifecycle_subscriber_plan(&graph).unwrap();
        let bindings = crate::host_extensions::lifecycle::production_lifecycle_handler_factories(
            Arc::new(storage_ephemeral::MemoryEventBus::new()),
        )
        .unwrap()
        .activate(assembly.host_extension_manifests())
        .unwrap();
        let (delivery, catalog) = ApiLifecycleFactDelivery::bind(&native, bindings).unwrap();
        let store = state
            .store
            .clone()
            .with_lifecycle_publication_catalog(catalog.clone());
        let host = Arc::new(
            runtime_extension_host::RuntimeExtensionHost::new_with_artifact_resolver(
                time::OffsetDateTime::now_utc(),
                Arc::new(ApiRuntimeArtifactResolver::new(
                    store.clone(),
                    &state.api_node_id,
                    &state.provider_install_root,
                )),
            )
            .unwrap(),
        );
        host.mark_ready().unwrap();
        let services = Arc::new(
            ApiRuntimeServices::new_with_runtime_backend(host.clone(), graph)
                .unwrap()
                .with_managed_composition(
                    store.clone(),
                    state.api_node_id.clone(),
                    assembly.module_descriptors().to_vec(),
                ),
        );
        let composition = services.managed_composition().unwrap();
        composition.attach_native_lifecycle_plan(native).unwrap();
        catalog
            .attach_workspace_source(Arc::new(ManagedWorkspacePublicationSource(Arc::downgrade(
                &composition,
            ))))
            .unwrap();
        Self {
            host,
            services,
            delivery: Arc::new(delivery.with_managed(&composition)),
            composition,
            store,
        }
    }
}

fn mixed_package() -> Vec<u8> {
    let mut a = manifest("a");
    let original: serde_json::Value = serde_yaml::from_str(include_str!(
        "../../../../../plugins/fixtures/acme.composition-a/manifest.yaml"
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
    .with_node_id(&state.api_node_id);
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
    authority
        .grant(
            &actor,
            installations[0].installation.id,
            grant("a", "event.subscribe"),
        )
        .await
        .unwrap();
    // The new authority revision changes a graph; retain a current-graph durable target too.
    runtime
        .composition
        .rebuild_installation(installations[0].installation.id)
        .await
        .unwrap();
    let before_restart = runtime.composition.snapshot(workspace).await.unwrap();
    create(&runtime.store, actor_id, workspace).await;
    runtime
        .store
        .retry_lifecycle_fact(
            old.event_id,
            &old.subscriber_id,
            worker_id,
            old.claim_id.unwrap(),
            time::OffsetDateTime::now_utc(),
            "restart fixture retains original target",
        )
        .await
        .unwrap();
    let pending = runtime
        .store
        .claim_lifecycle_facts(worker_id, 32, time::Duration::minutes(5))
        .await
        .unwrap();
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
