//! Joined command receipt → admission → effect → event worker → persisted workflow result.
use super::*;
use crate::workers::event_workflow::{spawn_event_workflow_loop, EventWorkflowStatuses};
use control_plane::{
    application::{ApplicationService, CreateApplicationCommand},
    application_public_api::{
        mapping::ApplicationApiMappingConfig,
        publications::{ApplicationPublicationService, PublishApplicationCommand},
        workflow_event::WorkflowEventTriggerService,
    },
    flow::{FlowService, SaveFlowDraftCommand},
};
use domain::event_center::{EventTriggerPhase, WorkflowEventTriggerInput};
use futures_util::FutureExt;
use std::{collections::BTreeMap, panic::AssertUnwindSafe};

async fn published_workflow(fixture: &Fixture, definition: &EventDefinitionRecord) -> (Uuid, Uuid) {
    let actor = &fixture.actor;
    let store = fixture.state.store.for_actor(actor.clone());
    let application = ApplicationService::new(store.clone())
        .create_application(CreateApplicationCommand {
            actor_user_id: actor.user_id,
            application_type: ApplicationType::Workflow,
            workflow_trigger_type: Some(WorkflowTriggerType::Event),
            workflow_trigger_config: None,
            name: "Command to event workflow fixture".into(),
            description: String::new(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();
    let flows = FlowService::new(store.clone());
    let editor = flows
        .get_or_create_editor_state(actor.user_id, application.id)
        .await
        .unwrap();
    let mut document =
        domain::default_flow_document_for_application(ApplicationType::Workflow, editor.flow.id);
    document["graph"]["nodes"][0]["config"]["input_fields"] = json!([{
        "key":"message", "label":"Message", "inputType":"text", "valueType":"string",
        "source":"body", "required":true
    }]);
    document["graph"]["nodes"][1]["bindings"] =
        json!({"message":{"kind":"selector","value":["node-workflow-start","message"]}});
    document["graph"]["nodes"][1]["outputs"] =
        json!([{"key":"message","title":"Message","valueType":"string"}]);
    flows
        .save_draft(SaveFlowDraftCommand {
            actor_user_id: actor.user_id,
            application_id: application.id,
            document,
            change_kind: FlowChangeKind::Logical,
            summary: "Deterministic Start/End command event workflow".into(),
        })
        .await
        .unwrap();
    let publication = ApplicationPublicationService::new(store.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor.user_id,
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: false,
        })
        .await
        .unwrap();
    WorkflowEventTriggerService::new(store)
        .replace_trigger(
            actor.user_id,
            application.id,
            WorkflowEventTriggerInput {
                source: None,
                enabled: true,
                contract_id: definition.contract_id.clone(),
                trigger_phase: EventTriggerPhase::Published,
                input_mapping: BTreeMap::from([("message".into(), "/payload/message".into())]),
            },
        )
        .await
        .unwrap();
    (application.id, publication.id)
}

async fn run_count(fixture: &Fixture, application_id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM flow_runs WHERE application_id=$1")
        .bind(application_id)
        .fetch_one(fixture.state.store.pool())
        .await
        .unwrap()
}

fn published_count(fixture: &Fixture) -> usize {
    fixture
        .state
        .event_center
        .snapshot(fixture.key.0)
        .activity
        .iter()
        .filter(|row| row.phase == EventTriggerPhase::Published && row.admission.is_none())
        .count()
}

#[tokio::test]
async fn command_receipt_runs_real_event_worker_and_persists_mapped_workflow_result() {
    let mut fixture = Fixture::new().await;
    let definition = configure(&mut fixture).await;
    let (application_id, publication_id) = published_workflow(&fixture, &definition).await;
    // Keep the actual serving generation alive for the worker's published executor/MCP factory.
    let _router = crate::app_with_state_and_config(
        fixture.state.clone(),
        &crate::test_support::support::test_config(),
    );
    let owner = crate::generation_dispatch::GenerationDispatch::acquire(&fixture.state)
        .expect("the serving router must own the compiled generation");
    let statuses = EventWorkflowStatuses::default();
    let worker = spawn_event_workflow_loop(fixture.state.clone(), owner, statuses.clone());
    let completed = AssertUnwindSafe(tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if statuses.snapshot(fixture.key.0).iter().any(|row| {
                row.application_id == application_id && row.activation == "active"
            }) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(published_count(&fixture), 0);
        assert_eq!(run_count(&fixture, application_id).await, 0);

        // Empty arguments fail the actual event schema before publication, with an explicit
        // command reply. They cannot become ordinary chat input or start this workflow.
        let (invalid_scope, invalid_input) = fixture.receive("invalid", "empty-joined", "/notify").await;
        super::super::super::admission::admit_available(&fixture.state, fixture.key)
            .await.unwrap();
        fixture.effect().await;
        let invalid = fixture.state.notification_service
            .command_for_input(&invalid_scope, invalid_input).await.unwrap().unwrap();
        assert_eq!(invalid.failure_code.as_deref(), Some("event_command_publish_rejected"));
        assert!(invalid.reply.is_some());
        assert!(!fixture.state.notification_service.pending_admissions(&invalid_scope)
            .await.unwrap().contains(&invalid_input));
        assert_eq!(published_count(&fixture), 0);
        assert_eq!(run_count(&fixture, application_id).await, 0);

        // All production boundaries are exercised: receive commits a durable input receipt,
        // admission classifies it, and the real command effect publishes into the same runtime
        // whose only business consumer is the real event workflow worker started above.
        let (scope, input) = fixture.receive("alice", "joined-success", "/notify command payload").await;
        super::super::super::admission::admit_available(&fixture.state, fixture.key)
            .await.unwrap();
        fixture.effect().await;
        let command = fixture.state.notification_service.command_for_input(&scope, input)
            .await.unwrap().unwrap();
        assert_eq!(command.failure_code, None);
        assert!(command.reply.unwrap().text.contains("1 targets accepted, 0 targets rejected"));
        let outcome = loop {
            if let Some(row) = statuses.snapshot(fixture.key.0).into_iter().find(|row| {
                row.application_id == application_id && row.last_delivery == Some("succeeded")
            }) {
                break row;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        };
        let run_id = outcome.last_run_id.expect("successful worker run identity");
        let (status, mode, publication, mapped_input, output):
            (String, String, Uuid, serde_json::Value, serde_json::Value) = sqlx::query_as(
                "SELECT status::text,run_mode,publication_version_id,input_payload,output_payload FROM flow_runs WHERE id=$1 AND application_id=$2",
            ).bind(run_id).bind(application_id).fetch_one(fixture.state.store.pool()).await.unwrap();
        assert_eq!(status, "succeeded");
        assert_eq!(mode, "workflow_event_run");
        assert_eq!(publication, publication_id);
        assert_eq!(mapped_input["node-workflow-start"]["message"], "command payload");
        assert!(mapped_input["trigger"].get("context").is_none());
        assert_eq!(output, json!({"message":"command payload"}));
        assert_eq!(published_count(&fixture), 1);
        assert_eq!(run_count(&fixture, application_id).await, 1);

        // Reprocessing effects cannot replay the already completed command into another run.
        fixture.effect().await;
        assert_eq!(published_count(&fixture), 1);
        assert_eq!(run_count(&fixture, application_id).await, 1);

        // Revocation between actual admission and effects must be checked live. The admitted
        // request stays without a success reply and must not add an event or workflow run.
        let (revoked_scope, revoked_input) = fixture.receive("revoked", "joined-revoked", "/notify denied payload").await;
        super::super::super::admission::admit_available(&fixture.state, fixture.key)
            .await.unwrap();
        assert!(fixture.state.notification_service.command_for_input(&revoked_scope, revoked_input)
            .await.unwrap().is_some());
        sqlx::query("UPDATE users SET status='disabled' WHERE id=$1")
            .bind(fixture.actor.user_id).execute(fixture.state.store.pool()).await.unwrap();
        let (_stop, receiver) = watch::channel(false);
        let rejected = super::super::super::effects::process_next(&fixture.state, fixture.key, receiver)
            .await.expect_err("a revoked channel actor cannot publish");
        assert!(rejected.to_string().contains("actor_disabled"));
        let revoked = fixture.state.notification_service.command_for_input(&revoked_scope, revoked_input)
            .await.unwrap().unwrap();
        assert!(revoked.reply.is_none());
        assert_eq!(published_count(&fixture), 1);
        assert_eq!(run_count(&fixture, application_id).await, 1);
    })).catch_unwind().await;
    // The generation-owned worker must be joined even when the bounded observation times out.
    tokio::time::timeout(Duration::from_secs(5), worker.shutdown())
        .await
        .unwrap();
    match completed {
        Ok(result) => {
            result.expect("joined command to event workflow path must complete deterministically")
        }
        Err(panic) => std::panic::resume_unwind(panic),
    }
    assert!(fixture
        .state
        .event_center
        .snapshot(fixture.key.0)
        .queues
        .is_empty());
}
