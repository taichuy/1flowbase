use super::*;
use crate::test_support::support::{test_api_state_with_database_url, test_config};
use control_plane::{
    application::{ApplicationService, CreateApplicationCommand},
    application_public_api::{
        mapping::ApplicationApiMappingConfig,
        publications::{ApplicationPublicationService, PublishApplicationCommand},
    },
    event_center::{EventCenterRuntime, EventCenterRuntimePolicy},
    flow::{FlowService, SaveFlowDraftCommand},
    ports::AuthRepository,
};
use control_plane_contracts::ports::{EventBusLimits, EventCenterRepository};
use domain::event_center::{EventDefinitionRecord, EventTriggerPhase, WorkflowEventTriggerInput};
use serde_json::json;
use storage_ephemeral::MemoryEventBus;

struct Fixture {
    state: Arc<ApiState>,
    actor: domain::ActorContext,
    snapshot: Snapshot,
    definition: domain::event_center::EventDefinitionRecord,
    bus: Arc<MemoryEventBus>,
}

async fn fixture() -> Fixture {
    fixture_for_phase(EventTriggerPhase::Published, "/payload/invoice").await
}

async fn fixture_for_phase(phase: EventTriggerPhase, pointer: &str) -> Fixture {
    let (mut state, _) = test_api_state_with_database_url().await;
    let bus = Arc::new(MemoryEventBus::new());
    Arc::get_mut(&mut state).unwrap().event_center = EventCenterRuntime::new(
        bus.clone(),
        Some(EventCenterRuntimePolicy {
            bus_limits: EventBusLimits {
                max_items: 16,
                max_json_bytes: 64000,
                max_waiters: 16,
                max_topics: 16,
                max_consumers: 16,
                max_topic_name_bytes: 16000,
            },
            observation_max_bytes: 16000,
            observation_ttl_ms: 60000,
        }),
    )
    .unwrap();
    let user_id: Uuid = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let actor = AuthRepository::load_actor_context_for_user(&state.store, user_id)
        .await
        .unwrap();
    // This fixture exercises legacy custom publication/consumption, not rule creation.
    let now = time::OffsetDateTime::now_utc();
    let definition = EventDefinitionRecord {
        interface_event: None,
        id: Uuid::now_v7(),
        workspace_id: actor.current_workspace_id,
        contract_id: "custom.worker.invoice".into(),
        name: "Invoice".into(),
        description: String::new(),
        enabled: true,
        payload_schema: json!({"type":"object","properties":{"invoice":{"type":"string"}},"required":["invoice"]}),
        created_by: actor.user_id,
        created_at: now,
        updated_at: now,
    };
    state
        .store
        .create_event_definition(&definition)
        .await
        .unwrap();
    let snapshot = workflow_snapshot(&state, &actor, &definition, phase, pointer).await;
    Fixture {
        state,
        actor,
        snapshot,
        definition,
        bus,
    }
}

/// Shared real publication fixture for every lifecycle phase; no worker behavior is mirrored.
async fn workflow_snapshot(
    state: &Arc<ApiState>,
    actor: &domain::ActorContext,
    definition: &domain::event_center::EventDefinitionRecord,
    phase: EventTriggerPhase,
    pointer: &str,
) -> Snapshot {
    let user_id = actor.user_id;
    let store = state.store.for_actor(actor.clone());
    let application = ApplicationService::new(store.clone())
        .create_application(CreateApplicationCommand {
            actor_user_id: user_id,
            application_type: domain::ApplicationType::Workflow,
            workflow_trigger_type: Some(domain::WorkflowTriggerType::Event),
            workflow_trigger_config: None,
            name: "Event worker execution fixture".into(),
            description: String::new(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();
    let flows = FlowService::new(store.clone());
    let editor = flows
        .get_or_create_editor_state(user_id, application.id)
        .await
        .unwrap();
    let mut document = domain::default_flow_document_for_application(
        domain::ApplicationType::Workflow,
        editor.flow.id,
    );
    document["graph"]["nodes"][0]["config"]["input_fields"] = json!([{"key":"invoice","label":"Invoice","inputType":"text","valueType":"string","source":"body","required":true}]);
    document["graph"]["nodes"][1]["bindings"] =
        json!({"invoice":{"kind":"selector","value":["node-workflow-start","invoice"]}});
    document["graph"]["nodes"][1]["outputs"] =
        json!([{"key":"invoice","title":"Invoice","valueType":"string"}]);
    flows
        .save_draft(SaveFlowDraftCommand {
            actor_user_id: user_id,
            application_id: application.id,
            document,
            change_kind: domain::FlowChangeKind::Logical,
            summary: "Deterministic event execution fixture".into(),
        })
        .await
        .unwrap();
    let publication = ApplicationPublicationService::new(store.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: user_id,
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let trigger = WorkflowEventTriggerService::new(store)
        .replace_trigger(
            user_id,
            application.id,
            WorkflowEventTriggerInput {
                source: None,
                enabled: true,
                contract_id: definition.contract_id.clone(),
                trigger_phase: phase,
                input_mapping: BTreeMap::from([("invoice".into(), pointer.into())]),
            },
        )
        .await
        .unwrap();
    Snapshot {
        trigger,
        publication,
    }
}

#[tokio::test]
async fn event_worker_executes_published_workflow_and_shutdown_retires_all_memberships() {
    let Fixture {
        state,
        actor,
        snapshot,
        definition,
        ..
    } = fixture().await;
    let application_id = snapshot.trigger.application_id;
    let publication = snapshot.publication;
    // The real serving router holds the dispatch generation used by the runtime MCP factory.
    let _router = crate::app_with_state_and_config(state.clone(), &test_config());
    let statuses = EventWorkflowStatuses::default();
    let (stop, receiver) = watch::channel(false);
    let task = tokio::spawn(run(state.clone(), statuses.clone(), receiver));
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if statuses
                .snapshot(actor.current_workspace_id)
                .iter()
                .any(|row| row.application_id == application_id && row.activation == "active")
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    state
        .event_center
        .publish(&definition, json!({"invoice":"A-001"}))
        .unwrap();
    let outcome = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(row) = statuses
                .snapshot(actor.current_workspace_id)
                .into_iter()
                .find(|row| {
                    row.application_id == application_id && row.last_delivery == Some("succeeded")
                })
            {
                break row;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    // Always retire receivers before asserting so test failures cannot leak generation tasks.
    let _ = stop.send(true);
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap();
    let outcome =
        outcome.expect("the real event receiver must reach the existing Workflow executor");
    let (run_mode, publication_id, input): (String, Uuid, serde_json::Value) = sqlx::query_as(
        "select run_mode,publication_version_id,input_payload from flow_runs where id=$1",
    )
    .bind(outcome.last_run_id.unwrap())
    .fetch_one(state.store.pool())
    .await
    .unwrap();
    assert_eq!(run_mode, "workflow_event_run");
    assert_eq!(publication_id, publication.id);
    assert_eq!(input["node-workflow-start"]["invoice"], "A-001");
    assert!(input["trigger"].get("context").is_none());
    assert!(state
        .event_center
        .snapshot(actor.current_workspace_id)
        .queues
        .is_empty());
}

#[derive(Clone, Copy)]
enum Invalidation {
    AuthorDisabled,
    TriggerChanged,
    PublicationChanged,
    PublicationDisabled,
}

async fn invalidate(fixture: &Fixture, invalidation: Invalidation) {
    match invalidation {
        Invalidation::AuthorDisabled => {
            sqlx::query("update users set status='disabled' where id=$1")
                .bind(fixture.snapshot.trigger.updated_by)
                .execute(fixture.state.store.pool())
                .await
                .unwrap();
        }
        Invalidation::TriggerChanged => {
            // Keep enabled and the same event identity; even an input-mapping revision
            // must retire the old receiver rather than reinterpret its queued deliveries.
            sqlx::query("update workflow_event_triggers set input_mapping='{}'::jsonb where id=$1")
                .bind(fixture.snapshot.trigger.id)
                .execute(fixture.state.store.pool())
                .await
                .unwrap();
        }
        Invalidation::PublicationDisabled => {
            ApplicationPublicationService::new(fixture.state.store.for_actor(fixture.actor.clone()))
                .set_api_enabled(control_plane::application_public_api::publications::SetApplicationApiEnabledCommand {
                    actor_user_id: fixture.actor.user_id,
                    application_id: fixture.snapshot.trigger.application_id,
                    api_enabled: false,
                }).await.unwrap();
        }
        Invalidation::PublicationChanged => {
            let mut document = fixture.snapshot.publication.document_snapshot.clone();
            document["graph"]["nodes"][0]["config"]["input_fields"][0]["label"] =
                json!("Republished invoice");
            FlowService::new(fixture.state.store.for_actor(fixture.actor.clone()))
                .save_draft(SaveFlowDraftCommand {
                    actor_user_id: fixture.actor.user_id,
                    application_id: fixture.snapshot.trigger.application_id,
                    document,
                    change_kind: domain::FlowChangeKind::Logical,
                    summary: "Change active event publication after queue admission".into(),
                })
                .await
                .unwrap();
            let current = ApplicationPublicationService::new(
                fixture.state.store.for_actor(fixture.actor.clone()),
            )
            .publish_active_version(PublishApplicationCommand {
                actor_user_id: fixture.actor.user_id,
                application_id: fixture.snapshot.trigger.application_id,
                mapping: ApplicationApiMappingConfig::default_native(),
                api_enabled: true,
            })
            .await
            .unwrap();
            // PostgreSQL replaces the active publication row in place. Equality of IDs
            // alone cannot detect that the current published snapshot has changed.
            assert_ne!(
                current.document_snapshot,
                fixture.snapshot.publication.document_snapshot
            );
            assert_ne!(current, fixture.snapshot.publication);
        }
    }
}

async fn rejects_before_handoff(invalidation: Invalidation, idle: bool) {
    let fixture = fixture().await;
    let trigger = &fixture.snapshot.trigger;
    let subscription = Arc::new(
        fixture
            .state
            .event_center
            .subscribe(
                EventRuntimeSubscription {
                    subscription_id: trigger.id,
                    workspace_id: trigger.workspace_id,
                    contract_id: trigger.configuration.contract_id.clone(),
                    trigger_phase: trigger.configuration.trigger_phase,
                    consumer_id: trigger.application_id,
                },
                false,
            )
            .unwrap(),
    );
    // A real consumed observer makes an accidental dequeue visible as both a fact and a
    // queued observer signal, even when execution refuses to create a workflow run.
    let observer = fixture
        .state
        .event_center
        .subscribe(
            EventRuntimeSubscription {
                subscription_id: Uuid::now_v7(),
                trigger_phase: EventTriggerPhase::Consumed,
                consumer_id: Uuid::now_v7(),
                ..subscription.spec().clone()
            },
            false,
        )
        .unwrap();
    let statuses = EventWorkflowStatuses::default();
    statuses.activation(&fixture.snapshot.trigger, "active", None);
    let (_stop, receiver) = watch::channel(false);
    if !idle {
        fixture
            .state
            .event_center
            .publish(&fixture.definition, json!({"invoice":"queued"}))
            .unwrap();
    }
    if !idle {
        invalidate(&fixture, invalidation).await;
    }
    let task = tokio::spawn(consume(
        fixture.state.clone(),
        fixture.snapshot.clone(),
        subscription.clone(),
        statuses.clone(),
        receiver,
    ));
    if idle {
        // Wait until the actual adapter owns a readiness waiter, not a time-based guess.
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if control_plane_contracts::ports::EventBus::event_bus_snapshot(
                    fixture.bus.as_ref(),
                )
                .waiters
                    > 0
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    if idle {
        invalidate(&fixture, invalidation).await;
        fixture
            .state
            .event_center
            .publish(&fixture.definition, json!({"invoice":"after-revocation"}))
            .unwrap();
    }
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap();
    let status = statuses
        .snapshot(fixture.actor.current_workspace_id)
        .into_iter()
        .find(|row| row.application_id == fixture.snapshot.trigger.application_id)
        .unwrap();
    assert_eq!(status.activation, "unavailable");
    let expected = match invalidation {
        Invalidation::AuthorDisabled => "subscription_author_disabled",
        Invalidation::TriggerChanged => "trigger_changed",
        Invalidation::PublicationChanged => "publication_changed",
        Invalidation::PublicationDisabled => "application_disabled",
    };
    assert_eq!(status.reason.as_deref(), Some(expected));
    assert!(status.last_run_id.is_none());
    let snapshot = fixture
        .state
        .event_center
        .snapshot(fixture.actor.current_workspace_id);
    let original = snapshot
        .queues
        .iter()
        .find(|queue| queue.subscription_id == subscription.spec().subscription_id)
        .unwrap();
    assert_eq!(
        (original.buffered, original.consumed, original.in_flight),
        (1, 0, 0)
    );
    assert!(!snapshot
        .activity
        .iter()
        .any(|row| row.phase == EventTriggerPhase::Consumed));
    let observed = snapshot
        .queues
        .iter()
        .find(|queue| queue.subscription_id == observer.spec().subscription_id)
        .unwrap();
    assert_eq!((observed.accepted, observed.buffered), (0, 0));
    let runs: i64 = sqlx::query_scalar("select count(*) from flow_runs where application_id=$1")
        .bind(fixture.snapshot.trigger.application_id)
        .fetch_one(fixture.state.store.pool())
        .await
        .unwrap();
    assert_eq!(runs, 0);
}

#[tokio::test]
async fn queued_revoked_author_cannot_create_a_consumed_fact_or_observer_signal() {
    rejects_before_handoff(Invalidation::AuthorDisabled, false).await;
}

#[tokio::test]
async fn idle_receiver_rechecks_revoked_author_when_work_arrives() {
    rejects_before_handoff(Invalidation::AuthorDisabled, true).await;
}

#[tokio::test]
async fn current_trigger_revision_must_match_before_handoff() {
    rejects_before_handoff(Invalidation::TriggerChanged, false).await;
}

#[tokio::test]
async fn current_publication_must_match_before_handoff() {
    rejects_before_handoff(Invalidation::PublicationChanged, false).await;
}

#[path = "host_lifecycle.rs"]
mod lifecycle;

#[path = "host_execution/joined_sources.rs"]
mod joined_sources;

#[tokio::test]
async fn disabled_publication_rejects_queue_handoff_without_consuming() {
    rejects_before_handoff(Invalidation::PublicationDisabled, false).await;
}
