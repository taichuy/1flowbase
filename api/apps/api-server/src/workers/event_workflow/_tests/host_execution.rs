use super::*;
use crate::test_support::support::{test_api_state_with_database_url, test_config};
use control_plane::{
    application::{ApplicationService, CreateApplicationCommand},
    application_public_api::{
        mapping::ApplicationApiMappingConfig,
        publications::{ApplicationPublicationService, PublishApplicationCommand},
    },
    event_center::{EventCenterConfigurationService, EventCenterRuntime, EventCenterRuntimePolicy},
    flow::{FlowService, SaveFlowDraftCommand},
    ports::AuthRepository,
};
use control_plane_contracts::ports::EventBusLimits;
use domain::event_center::{
    CreateEventDefinitionInput, EventTriggerPhase, WorkflowEventTriggerInput,
};
use serde_json::json;
use storage_ephemeral::MemoryEventBus;

#[tokio::test]
async fn event_worker_executes_published_workflow_and_shutdown_retires_all_memberships() {
    let (mut state, _) = test_api_state_with_database_url().await;
    Arc::get_mut(&mut state).unwrap().event_center = EventCenterRuntime::new(
        Arc::new(MemoryEventBus::new()),
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
    // The real serving router holds the dispatch generation used by the runtime MCP factory.
    let _router = crate::app_with_state_and_config(state.clone(), &test_config());
    let user_id: Uuid = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let actor = AuthRepository::load_actor_context_for_user(&state.store, user_id)
        .await
        .unwrap();
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
            api_enabled: false,
        })
        .await
        .unwrap();
    let definition = EventCenterConfigurationService::new(store.clone()).create_definition(&actor, CreateEventDefinitionInput {
        contract_id: "custom.worker.invoice".into(), contract_version: "1".into(), name: "Invoice".into(), description: String::new(), enabled: true,
        payload_schema: json!({"type":"object","properties":{"invoice":{"type":"string"}},"required":["invoice"]}),
    }).await.unwrap();
    WorkflowEventTriggerService::new(store)
        .replace_trigger(
            user_id,
            application.id,
            WorkflowEventTriggerInput {
                enabled: true,
                contract_id: definition.contract_id.clone(),
                contract_version: "1".into(),
                trigger_phase: EventTriggerPhase::Published,
                input_mapping: BTreeMap::from([("invoice".into(), "/payload/invoice".into())]),
            },
        )
        .await
        .unwrap();
    let statuses = EventWorkflowStatuses::default();
    let (stop, receiver) = watch::channel(false);
    let task = tokio::spawn(run(state.clone(), statuses.clone(), receiver));
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if statuses
                .snapshot(actor.current_workspace_id)
                .iter()
                .any(|row| row.application_id == application.id && row.activation == "active")
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
                    row.application_id == application.id && row.last_delivery == Some("succeeded")
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
