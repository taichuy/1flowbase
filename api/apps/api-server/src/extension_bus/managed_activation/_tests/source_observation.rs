//! Actual managed graph publication, SDK transactions and original dispatcher observation.
use super::*;
use crate::{
    app_state::ApiState,
    extension_bus::ManagedWorkspacePublicationSource,
    host_extensions::lifecycle::ApiLifecycleFactDelivery,
    provider_runtime::{ApiProviderRuntime, ApiRuntimeArtifactResolver, ApiRuntimeServices},
};
use control_plane::{
    event_center::{
        EventCenterRuntime, EventCenterRuntimePolicy, EventCenterSignalBody,
        EventRuntimeSubscription, EventSourceTarget,
    },
    lifecycle_outbox_dispatcher::{LifecycleOutboxDispatcher, LifecycleSourceObserver},
    plugin_management::*,
};
use control_plane_contracts::ports::*;
use domain::event_center::*;
use extension_contracts::*;
use futures_util::FutureExt;
use serde_json::{json, Value};
use std::{future::Future, panic::AssertUnwindSafe, path::PathBuf, time::Duration};

// Shared construction code only; none of the legacy test tree is mounted.
include!("../../../_tests/extension_bus/managed/runtime_fixture.rs");

struct Fixture {
    state: Arc<ApiState>,
    runtime: RuntimeFixture,
    actor: domain::ActorContext,
    center: EventCenterRuntime,
    observer: Arc<LifecycleSourceObserver>,
}
impl Fixture {
    async fn new() -> Self {
        let (state, _) = crate::test_support::support::test_api_state_with_database_url().await;
        let user: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE account='root'")
            .fetch_one(state.store.pool())
            .await
            .unwrap();
        let actor = ApplicationRepository::load_actor_context_for_user(&state.store, user)
            .await
            .unwrap();
        let mut infrastructure = (*state.infrastructure).clone();
        // This is the same local EventBus provider used by production; no replacement source bus.
        infrastructure.set_event_bus(Arc::new(storage_ephemeral::MemoryEventBus::new()));
        let center = EventCenterRuntime::new(
            infrastructure.event_bus(),
            Some(EventCenterRuntimePolicy {
                bus_limits: EventBusLimits {
                    max_items: 128,
                    max_json_bytes: 4_000_000,
                    max_waiters: 32,
                    max_topics: 128,
                    max_consumers: 128,
                    max_topic_name_bytes: 64_000,
                },
                observation_max_bytes: 4_000_000,
                observation_ttl_ms: 60_000,
            }),
        )
        .unwrap();
        let (store, sources) = crate::event_center::catalog::EventSourceCatalog::attach_local(
            &mut infrastructure,
            state.store.clone(),
            &center,
            None,
        )
        .unwrap();
        let observer = sources.lifecycle_observer.as_ref().unwrap().clone();
        let mut next = (*state).clone();
        next.store = store;
        next.runtime_event_stream = infrastructure.runtime_event_stream().unwrap();
        next.infrastructure = Arc::new(infrastructure);
        next.event_center = center.clone();
        next.event_sources = sources;
        let state = Arc::new(next);
        let runtime = RuntimeFixture::new(&state);
        runtime
            .composition
            .attach_lifecycle_source_observer(observer.clone())
            .await
            .unwrap();
        Self {
            state,
            runtime,
            actor,
            center,
            observer,
        }
    }
    fn scope(&self) -> EventSourceScope {
        EventSourceScope::Workspace {
            workspace_id: self.actor.current_workspace_id,
        }
    }
    fn management(&self) -> PluginManagementService<MainDurableStore, ApiProviderRuntime> {
        PluginManagementService::new(
            self.runtime.store.clone(),
            ApiProviderRuntime::new(self.runtime.services.clone()),
            self.state.official_plugin_source.clone(),
            &self.state.provider_install_root,
        )
        .with_node_id(&self.state.api_node_id)
        .with_managed_activation()
    }
    async fn install(&self, manifest: Value) -> domain::PluginInstallationRecord {
        let installed = self
            .management()
            .install_uploaded_plugin(InstallUploadedPluginCommand {
                actor_user_id: self.actor.user_id,
                file_name: "source-observation.1flowbasepkg".into(),
                package_bytes: package(&manifest),
            })
            .await
            .unwrap();
        self.management()
            .assign_plugin(AssignPluginCommand {
                actor_user_id: self.actor.user_id,
                installation_id: installed.installation.id,
            })
            .await
            .unwrap();
        self.management()
            .enable_plugin(EnablePluginCommand {
                actor_user_id: self.actor.user_id,
                installation_id: installed.installation.id,
            })
            .await
            .unwrap();
        installed.installation
    }
    fn source(&self, contract: &str) -> control_plane::event_center::EventRegisteredSourceSnapshot {
        self.center
            .snapshot_sources(self.scope())
            .sources
            .into_iter()
            .find(|source| source.descriptor.contract_id == contract)
            .expect("real candidate must register its source")
    }
    fn target(
        &self,
        contract: &str,
        phase: EventTriggerPhase,
    ) -> control_plane::event_center::EventCenterSubscription {
        let source = self.source(contract);
        self.center
            .subscribe_source(
                EventRuntimeSubscription {
                    subscription_id: Uuid::now_v7(),
                    workspace_id: self.actor.current_workspace_id,
                    contract_id: contract.into(),
                    trigger_phase: phase,
                    consumer_id: Uuid::now_v7(),
                },
                EventSourceTarget {
                    scope: self.scope(),
                    source_id: source.descriptor.source_id,
                    workflow_id: Uuid::now_v7(),
                },
            )
            .unwrap()
    }
    fn dispatcher(&self) -> LifecycleOutboxDispatcher<MainDurableStore> {
        LifecycleOutboxDispatcher::new(
            self.runtime.store.clone(),
            self.runtime.delivery.clone(),
            Arc::new(crate::ApiLifecycleDeliveryCompletion::default()),
        )
        .with_lifecycle_observer(self.observer.clone())
    }
    async fn create_model(&self) -> Uuid {
        self.runtime
            .store
            .create_model_definition(&CreateModelDefinitionInput {
                bound_storage_id: None,
                actor_user_id: self.actor.user_id,
                scope_kind: domain::DataModelScopeKind::Workspace,
                scope_id: self.actor.current_workspace_id,
                data_source_instance_id: None,
                source_kind: domain::DataModelSourceKind::MainSource,
                external_resource_key: None,
                external_table_id: None,
                external_capability_snapshot: None,
                template_provider: "source-observation".into(),
                template_code: "event".into(),
                template_version: "1".into(),
                code: format!("source_{}", Uuid::now_v7().simple()),
                title: "Actual managed publication".into(),
                description: None,
                status: domain::DataModelStatus::Published,
                protection: Default::default(),
            })
            .await
            .unwrap()
            .id
    }
}

async fn owned(fixture: &Fixture, checks: impl Future<Output = ()>) {
    let result = AssertUnwindSafe(tokio::time::timeout(Duration::from_secs(45), checks))
        .catch_unwind()
        .await;
    fixture.runtime.composition.close_owned_admission();
    let shutdown = fixture
        .runtime
        .composition
        .wait_for_shutdown(Duration::from_secs(5))
        .await;
    // Stop the actual host even when an operation failed to drain; assertions must not leave
    // owned SDK processes running after a failed fixture.
    let stopped = fixture.runtime.host.stop().await;
    if shutdown.is_ok() {
        fixture.runtime.composition.cleanup_after_shutdown().await;
    }
    stopped.unwrap();
    shutdown.unwrap();
    match result {
        Ok(result) => result.expect("actual managed source fixture exceeded its bounded wait"),
        Err(panic) => std::panic::resume_unwind(panic),
    }
    assert!(fixture
        .center
        .snapshot_sources(fixture.scope())
        .sources
        .iter()
        .all(
            |source| ![MANAGED_PROCESSED_EVENT_ID, COMMUNICATION_COMMAND_POINT]
                .contains(&source.descriptor.point_id.as_str())
        ));
}

fn manifest(name: &str, handler: &str) -> Value {
    let mut raw: Value = serde_yaml::from_str(include_str!(
        "../../../../../../plugins/fixtures/acme.composition-a/manifest.yaml"
    ))
    .unwrap();
    let module = format!("acme.composition-{name}");
    raw["plugin_id"] = module.clone().into();
    raw["data_models"] = json!([]);
    raw["managed"] = json!({
        "module":{"bus_version":"v1","module_id":module,"module_version":"1.0.0","module_kind":"runtime","contributions":[{
            "contribution_id":format!("{module}.events"),"contributor_module_id":module,
            "point_id":if name=="a" {MANAGED_CREATE_EVENT_POINT} else {MANAGED_PROCESSED_EVENT_ID},"contract_version":"1",
            "required_permissions":if name=="a" {vec!["event.subscribe","event.publish"]} else {vec!["event.subscribe"]},"mode":"append"
        }]},
        "execution_bindings":[{"contribution_id":format!("{module}.events"),"execution_mode":"process_per_call",
            "runtime":{"protocol":"stdio_json","entry":"bin/worker.py"},"handler":handler}]
    });
    if name == "a" {
        raw["managed"]["module"]["extension_points"] = json!([{
            "point_id":MANAGED_PROCESSED_EVENT_ID,"owner_module_id":module,"point_kind":"event_stream",
            "contract":{"contract_id":MANAGED_PROCESSED_EVENT_ID,"contract_version":"1","payload_schema":processed_schema()},
            "scope":"workspace","cardinality":"many","ordering":"lexicographic","failure":"isolate_contribution",
            "delivery":"after_commit_durable","lifecycle":"workspace_assignment","allowed_permissions":["event.subscribe","event.publish"],"override_policy":"sealed"
        }]);
    }
    raw
}

fn processed_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"properties":{"model_id":{"type":"string","maxLength":128},"status":{"type":"string","maxLength":32,"enum":["processed"]},"result_reference":{"anyOf":[{"type":"string","maxLength":512},{"type":"null"}]}},"required":["model_id","status","result_reference"]})
}

fn package(manifest: &Value) -> Vec<u8> {
    let yaml = serde_yaml::to_string(manifest).unwrap();
    plugin_framework::parse_plugin_manifest(&yaml).unwrap();
    let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    let mut header = tar::Header::new_gnu();
    header.set_size(yaml.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive
        .append_data(&mut header, "manifest.yaml", yaml.as_bytes())
        .unwrap();
    let worker = PathBuf::from(
        std::env::var_os("MANAGED_EVENT_WORKER_FIXTURE")
            .expect("prebuild runtime-extension-sdk --example managed_event_worker"),
    );
    assert!(
        worker.is_absolute() && worker.is_file(),
        "actual SDK example executable required"
    );
    archive
        .append_path_with_name(worker, "bin/worker.py")
        .unwrap();
    archive.into_inner().unwrap().finish().unwrap()
}

fn original_handoffs(
    fixture: &Fixture,
    contract: &str,
) -> Vec<control_plane::event_center::EventRuntimeActivity> {
    fixture
        .center
        .snapshot_sources(fixture.scope())
        .activity
        .into_iter()
        .filter(|row| {
            row.contract_id == contract
                && row.phase == EventTriggerPhase::Consumed
                && row.admission.is_none()
        })
        .collect()
}

#[tokio::test]
async fn system_event_center_managed_source_candidate_refresh_retained_graph_and_cleanup() {
    let fixture = Fixture::new().await;
    owned(&fixture, async {
        assert!(fixture
            .center
            .snapshot_sources(fixture.scope())
            .sources
            .is_empty());
        let installation = fixture.install(manifest("a", "publish")).await;
        let source = fixture.source(MANAGED_PROCESSED_EVENT_ID);
        assert_eq!(source.descriptor.origin, EventDefinitionOrigin::Plugin);
        assert_eq!(source.descriptor.scope, fixture.scope());
        assert_eq!(source.descriptor.owner_id, "acme.composition-a");
        assert_eq!(source.descriptor.point_id, MANAGED_PROCESSED_EVENT_ID);
        assert_eq!(
            source.descriptor.payload_schema["properties"]["payload"],
            processed_schema()
        );
        assert_eq!(
            source.descriptor.supported_phases,
            vec![EventTriggerPhase::Published, EventTriggerPhase::Consumed]
        );
        assert!(
            fixture
                .center
                .snapshot_sources(fixture.scope())
                .activity
                .is_empty(),
            "candidate registration is not publication or consumption"
        );
        let previous = fixture
            .runtime
            .composition
            .snapshot(fixture.actor.current_workspace_id)
            .await
            .unwrap();
        let reference = previous.freeze_reference().unwrap();
        fixture
            .management()
            .disable_plugin(DisablePluginCommand {
                actor_user_id: fixture.actor.user_id,
                installation_id: installation.id,
            })
            .await
            .unwrap();
        assert!(!fixture
            .runtime
            .composition
            .snapshot(fixture.actor.current_workspace_id)
            .await
            .unwrap()
            .graph
            .points()
            .iter()
            .any(|point| point.descriptor().point_id.as_str() == MANAGED_PROCESSED_EVENT_ID));
        fixture
            .runtime
            .composition
            .refresh_lifecycle_source_observations()
            .await;
        let retained = fixture.source(MANAGED_PROCESSED_EVENT_ID);
        assert_eq!(retained.descriptor.origin, EventDefinitionOrigin::Plugin);
        assert_eq!(retained.descriptor.scope, fixture.scope());
        assert_eq!(
            retained.descriptor.payload_schema,
            source.descriptor.payload_schema
        );
        assert_eq!(
            retained.descriptor.supported_phases,
            vec![EventTriggerPhase::Consumed],
            "retained original consumers cannot accept new publications"
        );
        assert!(fixture
            .runtime
            .composition
            .snapshots
            .lock()
            .await
            .retained
            .values()
            .flatten()
            .any(|snapshot| Arc::ptr_eq(snapshot, &previous)));
        assert!(fixture
            .center
            .snapshot_sources(fixture.scope())
            .activity
            .is_empty());
        drop(reference);
        fixture
            .runtime
            .composition
            .reclaim_retained_snapshots()
            .await;
        assert!(fixture
            .center
            .snapshot_sources(fixture.scope())
            .sources
            .iter()
            .all(|source| source.descriptor.contract_id != MANAGED_PROCESSED_EVENT_ID));
        assert!(fixture
            .center
            .snapshot_sources(EventSourceScope::System)
            .sources
            .is_empty());
    })
    .await;
}

#[tokio::test]
async fn system_event_center_managed_source_actual_commit_dispatch_failure_and_independent_ack() {
    let fixture = Fixture::new().await;
    owned(&fixture, async {
        fixture.install(manifest("a", "publish")).await;
        fixture.install(manifest("b", "attack.correlation")).await;
        fixture.install(manifest("c", "ack")).await;
        let published = fixture.target(MANAGED_PROCESSED_EVENT_ID, EventTriggerPhase::Published);
        let consumed = fixture.target(MANAGED_PROCESSED_EVENT_ID, EventTriggerPhase::Consumed);
        let model = fixture.create_model().await;
        assert!(published.try_recv().unwrap().is_none(), "native commit alone cannot fabricate a managed publication");
        let dispatcher = fixture.dispatcher();
        assert!(dispatcher.run_once().await.unwrap() > 0);
        let delivery = published.try_recv().unwrap().expect("actual SDK publisher must commit its output");
        let signal = delivery.signal();
        let EventCenterSignalBody::Published { payload } = &signal.body else { panic!("real managed fact required") };
        let fact: ManagedEventFact = serde_json::from_value(payload.clone()).unwrap();
        assert_eq!(fact.payload["model_id"], model.to_string());
        assert_eq!(fact.workspace_id, fixture.actor.current_workspace_id.to_string());
        assert_eq!(signal.source_context.as_ref().unwrap().scope, fixture.scope());
        assert_eq!(signal.source_context.as_ref().unwrap().original_event_id.as_deref(), Some(fact.event_id.as_str()));
        assert!(jsonschema::validator_for(&fixture.source(MANAGED_PROCESSED_EVENT_ID).descriptor.payload_schema).unwrap().is_valid(payload));
        let event_id = Uuid::parse_str(&fact.event_id).unwrap();
        let persisted: Vec<u8> = sqlx::query_scalar("SELECT canonical_payload FROM lifecycle_outbox WHERE event_id=$1").bind(event_id).fetch_one(fixture.runtime.store.pool()).await.unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&persisted).unwrap(), *payload);
        assert_eq!(original_handoffs(&fixture, MANAGED_PROCESSED_EVENT_ID).len(), 0);
        drop(delivery); // EventCenter receiver dequeue is not the original subscriber handoff.
        assert!(consumed.try_recv().unwrap().is_none());
        assert_eq!(dispatcher.run_once().await.unwrap(), 2);
        let statuses: Vec<(String,String,i32)> = sqlx::query_as("SELECT subscriber_id,status,attempt_count FROM lifecycle_outbox_deliveries WHERE event_id=$1 ORDER BY subscriber_id")
            .bind(event_id).fetch_all(fixture.runtime.store.pool()).await.unwrap();
        assert_eq!(statuses.len(), 2);
        assert!(statuses.iter().any(|(id,status,attempts)| id.ends_with("acme.composition-b.events") && status=="pending" && *attempts==1));
        assert!(statuses.iter().any(|(id,status,attempts)| id.ends_with("acme.composition-c.events") && status=="delivered" && *attempts==1));
        let failure: String = sqlx::query_scalar("SELECT last_error FROM lifecycle_outbox_deliveries WHERE event_id=$1 AND status='pending'").bind(event_id).fetch_one(fixture.runtime.store.pool()).await.unwrap();
        assert!(failure.contains("uncorrelated or oversized response"), "the original retry must come from the actual SDK correlation rejection: {failure}");
        let handoffs = original_handoffs(&fixture, MANAGED_PROCESSED_EVENT_ID);
        assert_eq!(handoffs.len(), 2, "failed handler still owns an original Consumed fact");
        for handoff in &handoffs {
            assert!(handoff.original_consumer_id.as_ref().is_some_and(|id| !id.is_empty()));
            assert!(statuses.iter().any(|(id,_,_)| Some(id)==handoff.original_subscriber_id.as_ref()));
            assert_eq!(handoff.source_context.as_ref().unwrap().original_event_id.as_deref(), Some(fact.event_id.as_str()));
        }
        let mut observed_subscribers = Vec::new();
        while let Some(delivery) = consumed.try_recv().unwrap() {
            let EventCenterSignalBody::Consumed { source_event_id, subscription_id, .. } = &delivery.signal().body else { panic!("original handoff signal required") };
            assert_eq!(source_event_id, &fact.event_id);
            assert_eq!(delivery.signal().source_context.as_ref().unwrap().scope, fixture.scope());
            observed_subscribers.push(subscription_id.clone());
        }
        observed_subscribers.sort();
        assert_eq!(observed_subscribers, statuses.iter().map(|(id,_,_)| id.clone()).collect::<Vec<_>>());
        fixture.observer.refresh_queue_snapshots(fixture.runtime.store.lifecycle_source_queue_snapshots().await.unwrap());
        let source = fixture.source(MANAGED_PROCESSED_EVENT_ID);
        assert!(source.queues.iter().any(|queue| queue.subscriber_id.ends_with("acme.composition-b.events") && queue.buffered==Some(1)));
        assert!(source.queues.iter().any(|queue| queue.subscriber_id.ends_with("acme.composition-c.events") && queue.buffered==Some(0)));
        assert_eq!(original_handoffs(&fixture, MANAGED_PROCESSED_EVENT_ID), handoffs, "read-only refresh cannot consume again");
        assert!(!serde_json::to_string(&handoffs).unwrap().contains("result_reference"));
        assert!(fixture.center.snapshot_sources(EventSourceScope::System).activity.is_empty());
        drop(published); drop(consumed);
    }).await;
}

#[path = "communication_source.rs"]
mod communication_source;
