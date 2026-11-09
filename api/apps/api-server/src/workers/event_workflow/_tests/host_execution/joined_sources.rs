//! Joined evidence: original owners produce and consume; the serving worker executes publications.
use super::*;
use control_plane::{
    application_public_api::workflow_invocation::{
        InvokeWorkflowCommand, WorkflowInvocationService, WorkflowInvocationTrigger,
    },
    event_center::{EventSourceTarget, EventTargetAdmissionStatus},
    lifecycle_outbox_dispatcher::LifecycleOutboxDispatcher,
    orchestration_runtime::{
        project_runtime_event_stream_terminal, start_runtime_debug_event_persister,
        OrchestrationRuntimeService, StartPublishedFlowRunCommand,
    },
    ports::{
        RuntimeEventSourceBinding, RuntimeEventStreamPolicy, RUNTIME_EVENT_REQUIRED_CONTRACT_ID,
    },
};
use control_plane_contracts::ports::{
    CreateModelDefinitionInput, LifecycleOutboxRepository, ModelDefinitionRepository,
};
use domain::event_center::EventSourceScope;
use futures_util::FutureExt;
use std::{future::Future, panic::AssertUnwindSafe};

/// Tokio JoinHandle detaches on drop; retain cancellation for assertion/timeout unwinds too.
struct AbortOnDrop(tokio::task::AbortHandle);
impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Attach before cloning producers, and select only metadata from the actual boot catalog.
async fn joined_fixture(
    native: bool,
    phase: EventTriggerPhase,
    pointer: &str,
) -> (
    Fixture,
    Option<crate::host_extensions::lifecycle::ApiLifecycleFactDelivery>,
) {
    let mut fixture = fixture().await;
    let assembly = crate::extension_bus::assemble_extension_graph_input(
        crate::api_workspace_root().unwrap(),
        crate::extension_bus::DEFAULT_PLUGIN_SET_PATH,
        vec![],
    )
    .unwrap();
    let graph = assembly.compile_graph().unwrap();
    let plan = Arc::new(assembly.compile_lifecycle_subscriber_plan(&graph).unwrap());
    let mut infrastructure = (*fixture.state.infrastructure).clone();
    fixture.bus = Arc::new(MemoryEventBus::new());
    infrastructure.set_event_bus(fixture.bus.clone());
    let center = EventCenterRuntime::new(
        infrastructure.event_bus(),
        Some(EventCenterRuntimePolicy {
            bus_limits: EventBusLimits {
                max_items: 256,
                max_json_bytes: 4_000_000,
                max_waiters: 64,
                max_topics: 128,
                max_consumers: 128,
                max_topic_name_bytes: 64_000,
            },
            observation_max_bytes: 4_000_000,
            observation_ttl_ms: 60_000,
        }),
    )
    .unwrap();
    let (mut store, sources) = crate::event_center::catalog::EventSourceCatalog::attach_local(
        &mut infrastructure,
        fixture.state.store.clone(),
        &center,
        native.then_some(plan.clone()),
    )
    .unwrap();
    let delivery = if native {
        // Intentionally leave the optional publication inactive: the REAL registered native
        // handler fails after dispatcher handoff. This must not erase Consumed or block a run.
        let publication = Arc::new(
            crate::host_extensions::notification_publication::HostOptionalNotificationPublication::default(),
        );
        let bindings =
            crate::host_extensions::lifecycle::production_lifecycle_handler_factories(publication)
                .unwrap()
                .activate(assembly.host_extension_manifests())
                .unwrap();
        let (delivery, catalog) =
            crate::host_extensions::lifecycle::ApiLifecycleFactDelivery::bind(&plan, bindings)
                .unwrap();
        store = store.with_lifecycle_publication_catalog(catalog);
        Some(delivery)
    } else {
        None
    };
    let mut state = (*fixture.state).clone();
    state.store = store;
    state.runtime_event_stream = infrastructure.runtime_event_stream().unwrap();
    state.infrastructure = Arc::new(infrastructure);
    state.event_center = center;
    state.event_sources = sources;
    fixture.state = Arc::new(state);
    let scope = scope(&fixture);
    let entries = fixture
        .state
        .event_sources
        .registered_events(&fixture.actor, &fixture.state.event_center);
    let entry = entries
        .iter()
        .find(|entry| {
            entry.contract_id
                == if native {
                    "model_definition.committed"
                } else {
                    RUNTIME_EVENT_REQUIRED_CONTRACT_ID
                }
                && entry
                    .source
                    .as_ref()
                    .is_some_and(|source| source.selection.scope == scope)
        })
        .expect("source is selectable before its first real publication");
    assert!(entry.supported_phases.contains(&phase));
    let source = entry.source.as_ref().unwrap();
    assert!(entry.enabled.is_none());
    assert!(fixture
        .state
        .event_center
        .snapshot_sources(scope)
        .activity
        .is_empty());
    fixture.snapshot.trigger =
        WorkflowEventTriggerService::new(fixture.state.store.for_actor(fixture.actor.clone()))
            .with_registered_events(entries.clone())
            .replace_trigger(
                fixture.actor.user_id,
                fixture.snapshot.trigger.application_id,
                WorkflowEventTriggerInput {
                    source: Some(source.selection.clone()),
                    enabled: true,
                    contract_id: entry.contract_id.clone(),
                    trigger_phase: phase,
                    input_mapping: BTreeMap::from([("invoice".into(), pointer.into())]),
                },
            )
            .await
            .unwrap();
    (fixture, delivery)
}

fn scope(fixture: &Fixture) -> EventSourceScope {
    EventSourceScope::Workspace {
        workspace_id: fixture.actor.current_workspace_id,
    }
}

async fn with_worker(
    fixture: &Fixture,
    checks: impl Future<Output = ()>,
    statuses: EventWorkflowStatuses,
) {
    let _router = crate::app_with_state_and_config(fixture.state.clone(), &test_config());
    let owner = GenerationDispatch::acquire(&fixture.state).unwrap();
    let worker = spawn_event_workflow_loop(fixture.state.clone(), owner, statuses);
    let result = AssertUnwindSafe(tokio::time::timeout(Duration::from_secs(20), checks))
        .catch_unwind()
        .await;
    tokio::time::timeout(Duration::from_secs(5), worker.shutdown())
        .await
        .unwrap();
    match result {
        Ok(result) => result.expect("joined source workflow timed out"),
        Err(panic) => std::panic::resume_unwind(panic),
    }
    assert!(fixture
        .state
        .event_center
        .snapshot_sources(scope(fixture))
        .workflow_queues
        .is_empty());
}

async fn active(fixture: &Fixture, statuses: &EventWorkflowStatuses) {
    loop {
        if statuses
            .snapshot(fixture.actor.current_workspace_id)
            .iter()
            .any(|row| {
                row.application_id == fixture.snapshot.trigger.application_id
                    && row.activation == "active"
            })
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn succeeded(fixture: &Fixture, statuses: &EventWorkflowStatuses) -> Uuid {
    loop {
        if let Some(id) = statuses
            .snapshot(fixture.actor.current_workspace_id)
            .iter()
            .find(|row| {
                row.application_id == fixture.snapshot.trigger.application_id
                    && row.last_delivery == Some("succeeded")
            })
            .and_then(|row| row.last_run_id)
        {
            return id;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn persisted(fixture: &Fixture, run: Uuid, expected: &str, original_event: &str) {
    let (status, mode, publication, input, output): (String, String, Uuid, serde_json::Value, serde_json::Value) =
        sqlx::query_as("SELECT status::text,run_mode,publication_version_id,input_payload,output_payload FROM flow_runs WHERE id=$1")
            .bind(run).fetch_one(fixture.state.store.pool()).await.unwrap();
    assert_eq!(status, "succeeded");
    assert_eq!(mode, "workflow_event_run");
    assert_eq!(publication, fixture.snapshot.publication.id);
    assert_eq!(input["node-workflow-start"]["invoice"], expected);
    assert_eq!(output, json!({"invoice": expected}));
    let metadata = &input["trigger"];
    assert_eq!(
        metadata["trigger_phase"],
        json!(fixture.snapshot.trigger.configuration.trigger_phase)
    );
    assert_eq!(metadata["event_id"], original_event);
    assert_eq!(
        metadata["subscription_id"],
        fixture.snapshot.trigger.id.to_string()
    );
    assert_eq!(
        metadata["source_context"]["original_event_id"],
        original_event
    );
    assert_eq!(metadata["source_context"]["scope"], json!(scope(fixture)));
    assert_eq!(
        metadata["source_context"]["source_id"],
        fixture
            .snapshot
            .trigger
            .configuration
            .source
            .as_ref()
            .unwrap()
            .source_id
    );
    assert_eq!(
        metadata["workflow_ancestor_ids"],
        json!([fixture.snapshot.trigger.application_id])
    );
    assert!(metadata.get("context").is_none());
    assert!(metadata.get("payload").is_none());
    assert!(metadata["source_context"].get("payload").is_none());
}

#[tokio::test]
async fn system_event_center_real_published_runtime_persister_triggers_workflow_without_recursion()
{
    let (fixture, _) = joined_fixture(false, EventTriggerPhase::Consumed, "/source_event_id").await;
    let statuses = EventWorkflowStatuses::default();
    with_worker(&fixture, async {
        active(&fixture, &statuses).await;
        // A passive workflow target lets us inspect the actual child's outgoing trusted chain.
        let audit = fixture.state.event_center.subscribe_source(
            EventRuntimeSubscription {
                subscription_id: Uuid::now_v7(), workspace_id: fixture.actor.current_workspace_id,
                contract_id: RUNTIME_EVENT_REQUIRED_CONTRACT_ID.into(), trigger_phase: EventTriggerPhase::Published, consumer_id: Uuid::now_v7(),
            },
            EventSourceTarget {
filters : Default::default(),
 scope: scope(&fixture), source_id: RUNTIME_EVENT_REQUIRED_CONTRACT_ID.into(), workflow_id: Uuid::now_v7() },
        ).unwrap();
        // Use the published Start/End plan via the ordinary invocation and execution owners.
        let store = fixture.state.store.for_actor(fixture.actor.clone());
        let original = WorkflowInvocationService::new(store.clone()).invoke(InvokeWorkflowCommand {
            actor_user_id: fixture.actor.user_id,
            publication: fixture.snapshot.publication.clone(),
            node_input_payload: json!({"node-workflow-start":{"invoice":"ordinary-start-end"}}),
            trigger: WorkflowInvocationTrigger::Http {
                api_key_id: None, interface_id: "fixture.ordinary".into(), route_template: "/fixture".into(),
                method: "POST".into(), principal: domain::FlowRunPrincipalKind::User, response_mode: "blocking".into(),
            },
        }).await.unwrap().flow_run;
        let binding = RuntimeEventSourceBinding::workspace(fixture.actor.current_workspace_id);
        fixture.state.runtime_event_stream.open_run_scoped(original.id, RuntimeEventStreamPolicy::debug_default(), binding.clone()).await.unwrap();
        let mut persister = start_runtime_debug_event_persister(store.clone(), fixture.state.runtime_event_stream.clone(), original.id).await;
        let _persister_cleanup = AbortOnDrop(persister.abort_handle());
        let original_queues = fixture.state.event_center.snapshot_sources(scope(&fixture)).sources.into_iter()
            .find(|source| source.descriptor.contract_id == RUNTIME_EVENT_REQUIRED_CONTRACT_ID)
            .map(|source| source.queues).unwrap_or_default();
        // Catch a producer panic too, so the existing required consumer is always retired.
        let execution = AssertUnwindSafe(tokio::time::timeout(Duration::from_secs(10), async {
            OrchestrationRuntimeService::new(
                store, crate::routes::application_public_api::native::api_provider_runtime(&fixture.state),
                fixture.state.runtime_engine.clone(), fixture.state.provider_secret_master_key.clone(),
                fixture.state.infrastructure.provider_transport_store(), fixture.state.model_billing_require_provider_usage,
            ).with_published_plan_cache(fixture.state.infrastructure.published_plan_cache())
                .with_runtime_event_stream(fixture.state.runtime_event_stream.clone())
                .with_runtime_event_source_binding(binding)
                .start_published_flow_run(StartPublishedFlowRunCommand {
                    transport_connection_scope: None, observation_context: None,
                    application_id: original.application_id, flow_run_id: original.id, provider_transport_slot: None,
                }).await
        })).catch_unwind().await;
        // Close even when producer execution fails; do not strand the required receiver.
        if let Ok(Ok(Ok(detail))) = &execution {
            project_runtime_event_stream_terminal(fixture.state.runtime_event_stream.clone(), &detail.flow_run).await;
        } else {
            let _ = fixture.state.runtime_event_stream.close_run(original.id, control_plane::ports::RuntimeEventCloseReason::Failed).await;
        }
        let persisted_owner = tokio::time::timeout(Duration::from_secs(3), &mut persister).await;
        if persisted_owner.is_err() {
            persister.abort();
            let _ = persister.await;
            panic!("original persister failed to retire after producer closure");
        }
        persisted_owner.unwrap().unwrap();
        assert!(!original_queues.is_empty(), "the existing persister must own an original required queue");
        assert_eq!(execution.unwrap().unwrap().unwrap().flow_run.status, domain::FlowRunStatus::Succeeded);
        let child = succeeded(&fixture, &statuses).await;
        let input: serde_json::Value = sqlx::query_scalar("SELECT input_payload FROM flow_runs WHERE id=$1")
            .bind(child).fetch_one(fixture.state.store.pool()).await.unwrap();
        let event_id = input["trigger"]["event_id"].as_str().unwrap();
        assert!(event_id.starts_with(&format!("{}:", original.id)), "native string identity must survive the worker");
        persisted(&fixture, child, event_id, event_id).await;
        let durable: i64 = sqlx::query_scalar("SELECT count(*) FROM runtime_events WHERE flow_run_id=$1")
            .bind(original.id).fetch_one(fixture.state.store.pool()).await.unwrap();
        assert!(durable > 0, "the original persister must write actual runtime events");
        loop {
            let source = fixture.state.event_center.snapshot_sources(scope(&fixture));
            if source.activity.iter().any(|row| row.admission == Some(EventTargetAdmissionStatus::Reentrant)) {
                let handoff = source.activity.iter().find(|row| row.phase == EventTriggerPhase::Consumed && row.admission.is_none()
                    && row.source_context.as_ref().and_then(|context| context.original_event_id.as_deref()) == Some(event_id)).unwrap();
                assert!(original_queues.iter().any(|queue| Some(&queue.subscriber_id) == handoff.original_subscriber_id.as_ref()));
                assert!(handoff.original_consumer_id.is_some());
                assert!(source.activity.iter().all(|row| row.source_context.as_ref().is_some_and(|context| context.scope == scope(&fixture))));
                assert!(source.workflow_queues.iter().any(|queue| queue.subscription_id == fixture.snapshot.trigger.id && queue.consumed > 0));
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let mut child_signal = None;
        let mut original_envelope = None;
        while let Some(delivery) = audit.try_recv().unwrap() {
            let signal = delivery.signal();
            if signal.source_context.as_ref().and_then(|context| context.original_event_id.as_deref()) == Some(event_id) {
                if let control_plane::event_center::EventCenterSignalBody::Published { payload } = &signal.body {
                    original_envelope = Some(payload.clone());
                }
            }
            if signal.source_context.as_ref().and_then(|context| context.original_event_id.as_ref())
                .is_some_and(|id| id.starts_with(&format!("{child}:"))) {
                child_signal = Some(signal.clone());
            }
        }
        let envelope = original_envelope.expect("the child's native string identity must match the actual producer envelope");
        assert_eq!(envelope["event_id"], event_id);
        assert_eq!(envelope["run_id"], original.id.to_string());
        assert!(child_signal.unwrap().workflow_ancestor_ids.contains(&fixture.snapshot.trigger.application_id));
        drop(audit);
    }, statuses.clone()).await;
    let runs: Vec<serde_json::Value> = sqlx::query_scalar("SELECT input_payload FROM flow_runs WHERE application_id=$1 AND run_mode='workflow_event_run'")
        .bind(fixture.snapshot.trigger.application_id).fetch_all(fixture.state.store.pool()).await.unwrap();
    assert!(!runs.is_empty());
    assert!(runs.iter().all(
        |input| input["trigger"]["event_id"].as_str().unwrap().starts_with(
            // Every accepted root fact comes from the ordinary run, never a child workflow run.
            runs[0]["trigger"]["event_id"]
                .as_str()
                .unwrap()
                .split(':')
                .next()
                .unwrap()
        )
    ));
}

async fn native_joined(phase: EventTriggerPhase) {
    let pointer = if phase == EventTriggerPhase::Published {
        "/payload/payload/model_definition_id"
    } else {
        "/source_event_id"
    };
    let (fixture, delivery) = joined_fixture(true, phase, pointer).await;
    let statuses = EventWorkflowStatuses::default();
    with_worker(&fixture, async {
        active(&fixture, &statuses).await;
        let model = ModelDefinitionRepository::create_model_definition(
            &fixture.state.store.for_actor(fixture.actor.clone()),
            &CreateModelDefinitionInput {
                bound_storage_id: None, actor_user_id: fixture.actor.user_id,
                scope_kind: domain::DataModelScopeKind::Workspace, scope_id: fixture.actor.current_workspace_id,
                data_source_instance_id: None, source_kind: domain::DataModelSourceKind::MainSource,
                external_resource_key: None, external_table_id: None, external_capability_snapshot: None,
                template_provider: "joined-native-fixture".into(), template_code: "event".into(), template_version: "1".into(),
                code: format!("joined_{}", Uuid::now_v7().simple()), title: "Joined native transaction".into(),
                description: None, status: domain::DataModelStatus::Published, protection: domain::DataModelProtection::default(),
            },
        ).await.unwrap();
        let committed_scope: Uuid = sqlx::query_scalar("SELECT scope_id FROM model_definitions WHERE id=$1")
            .bind(model.id).fetch_one(fixture.state.store.pool()).await.unwrap();
        assert_eq!(committed_scope, fixture.actor.current_workspace_id);
        let event_id: Uuid = sqlx::query_scalar("SELECT event_id FROM lifecycle_outbox WHERE contract_id='model_definition.committed' AND convert_from(canonical_payload,'UTF8')::jsonb->'payload'->>'model_definition_id'=$1")
            .bind(model.id.to_string()).fetch_one(fixture.state.store.pool()).await.unwrap();
        let before = fixture.state.event_center.snapshot_sources(scope(&fixture));
        assert!(before.activity.iter().any(|row| row.phase == EventTriggerPhase::Published && row.source_context.as_ref().and_then(|c| c.original_event_id.as_ref()) == Some(&event_id.to_string())));
        assert!(!before.activity.iter().any(|row| row.contract_id == "model_definition.committed" && row.phase == EventTriggerPhase::Consumed && row.admission.is_none()));
        let observer = fixture.state.event_sources.lifecycle_observer.as_ref().unwrap();
        let dispatcher = LifecycleOutboxDispatcher::new(
            fixture.state.store.clone(), Arc::new(delivery.unwrap()),
            Arc::new(crate::ApiLifecycleDeliveryCompletion::default()),
        ).with_lifecycle_observer(observer.clone());
        assert!(dispatcher.run_once().await.unwrap() > 0);
        let retries: i64 = sqlx::query_scalar("SELECT count(*) FROM lifecycle_outbox_deliveries WHERE event_id=$1 AND status='pending' AND attempt_count>0 AND last_error LIKE '%not activated%'")
            .bind(event_id).fetch_one(fixture.state.store.pool()).await.unwrap();
        assert!(retries > 0, "the registered handler failure must remain retryable under the original dispatcher");
        observer.refresh_queue_snapshots(fixture.state.store.lifecycle_source_queue_snapshots().await.unwrap());
        let child = succeeded(&fixture, &statuses).await;
        let expected = if phase == EventTriggerPhase::Published { model.id.to_string() } else { event_id.to_string() };
        persisted(&fixture, child, &expected, &event_id.to_string()).await;
        let after = fixture.state.event_center.snapshot_sources(scope(&fixture));
        let source = after.sources.iter().find(|source| source.descriptor.contract_id == "model_definition.committed").unwrap();
        assert_eq!(source.descriptor.scope, scope(&fixture));
        assert!(!source.queues.is_empty());
        let consumed = after.activity.iter().find(|row| row.phase == EventTriggerPhase::Consumed && row.admission.is_none()
            && row.source_context.as_ref().and_then(|c| c.original_event_id.as_ref()) == Some(&event_id.to_string())).unwrap();
        assert!(source.queues.iter().any(|queue| Some(&queue.subscriber_id) == consumed.original_subscriber_id.as_ref()));
        assert!(consumed.original_consumer_id.is_some());
        let serialized = serde_json::to_value(&after.activity).unwrap();
        assert!(!serialized.to_string().contains("model_definition_id"), "ephemeral activity must retain identities, not fact bodies");
        assert!(after.workflow_queues.iter().any(|queue| queue.subscription_id == fixture.snapshot.trigger.id && queue.consumed > 0));
    }, statuses.clone()).await;
}

#[tokio::test]
async fn system_event_center_native_model_commit_published_executes_real_workflow() {
    native_joined(EventTriggerPhase::Published).await;
}

#[tokio::test]
async fn system_event_center_native_model_dispatch_consumed_survives_actual_handler_failure() {
    native_joined(EventTriggerPhase::Consumed).await;
}
