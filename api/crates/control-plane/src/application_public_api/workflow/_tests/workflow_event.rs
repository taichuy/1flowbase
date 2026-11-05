use super::super::workflow_start_http_inputs::{
    build_workflow_start_event_input_payload, parse_workflow_start_event_inputs,
};
use super::*;
use serde_json::json;
use std::collections::BTreeMap;

fn configuration() -> WorkflowEventTriggerInput {
    WorkflowEventTriggerInput {
        enabled: true,
        contract_id: "custom.invoice".into(),
        contract_version: "1".into(),
        trigger_phase: domain::event_center::EventTriggerPhase::Published,
        input_mapping: BTreeMap::from([("invoice".into(), "/payload/invoice~1id".into())]),
    }
}
fn document() -> Value {
    json!({"graph":{"nodes":[{"id":"event-start","type":"workflow_start","config":{"input_fields":[
        {"key":"invoice","valueType":"string","required":true},
        {"key":"attempt","valueType":"number","defaultValue":1}
    ]}}]}})
}
#[test]
fn event_mapping_uses_lifecycle_context_and_start_defaults_without_http_source() {
    let contract = parse_workflow_start_event_inputs(&document()).unwrap();
    let config = configuration();
    validate_event_input_mapping(&contract, &config).unwrap();
    let input = build_workflow_start_event_input_payload(
        &contract,
        &config.input_mapping,
        &json!({"payload":{"invoice/id":"A-001"}}),
    )
    .unwrap();
    assert_eq!(
        input,
        json!({"event-start":{"invoice":"A-001","attempt":1}})
    );
}
#[test]
fn event_mapping_rejects_missing_required_mapping_unknown_target_and_invalid_pointer() {
    let contract = parse_workflow_start_event_inputs(&document()).unwrap();
    let mut config = configuration();
    config.input_mapping.clear();
    assert!(validate_event_input_mapping(&contract, &config).is_err());
    config.input_mapping.insert("env".into(), "/payload".into());
    assert!(validate_event_input_mapping(&contract, &config).is_err());
    config.input_mapping = BTreeMap::from([("invoice".into(), "/bad~2escape".into())]);
    assert!(validate_event_input_mapping(&contract, &config).is_err());
    config = configuration();
    assert!(build_workflow_start_event_input_payload(
        &contract,
        &config.input_mapping,
        &json!({"phase":"consumed","source_event_id":"source"})
    )
    .is_err());
}
#[test]
fn archive_event_configuration_round_trips_identity_phase_and_mapping() {
    use crate::application::WorkflowTriggerTemplateConfig;
    let config = WorkflowTriggerTemplateConfig::Event {
        configuration: configuration(),
    };
    let encoded = serde_json::to_value(&config).unwrap();
    assert_eq!(encoded["kind"], "event");
    assert_eq!(encoded["configuration"]["trigger_phase"], "published");
    assert_eq!(
        serde_json::from_value::<WorkflowTriggerTemplateConfig>(encoded).unwrap(),
        config
    );
}

#[tokio::test]
async fn event_invocation_freezes_publication_maps_context_and_deduplicates_signal() {
    use crate::application_public_api::{
        mapping::ApplicationApiMappingConfig,
        publications::{ApplicationPublicationService, PublishApplicationCommand},
        ApplicationPublicApiTestHarness,
    };
    let user_id = Uuid::from_u128(0x11111111111111111111111111111111);
    let harness = ApplicationPublicApiTestHarness::new();
    let app = harness.seed_workflow_application(user_id, "Event workflow");
    harness.set_workflow_trigger_type(app.id, domain::WorkflowTriggerType::Event);
    let repository = harness.repository();
    let actor = repository
        .load_actor_context_for_user(user_id)
        .await
        .unwrap();
    EventCenterConfigurationService::new(repository.clone())
        .create_definition(
            &actor,
            domain::event_center::CreateEventDefinitionInput {
                contract_id: "custom.invoice".into(),
                contract_version: "1".into(),
                name: "Invoice".into(),
                description: String::new(),
                payload_schema: json!({"type":"object"}),
                enabled: true,
            },
        )
        .await
        .unwrap();
    let service = WorkflowEventTriggerService::new(repository.clone());
    // Unpublished configuration is persisted without starting or manufacturing a run.
    service
        .replace_trigger(user_id, app.id, configuration())
        .await
        .unwrap();
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: user_id,
            application_id: app.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: false,
        })
        .await
        .unwrap();
    repository.set_active_publication_document_snapshot(app.id, document());
    let trigger = service.get_trigger(user_id, app.id).await.unwrap().unwrap();
    let publication = repository
        .load_active_application_publication(app.id)
        .await
        .unwrap()
        .unwrap();
    let signal_id = Uuid::now_v7();
    let command = InvokeWorkflowEventCommand {
        trigger,
        publication: publication.clone(),
        signal_id,
        event_id: Some(signal_id),
        subscription_id: Uuid::now_v7(),
        context: json!({"workspace_id":app.workspace_id,"event_id":signal_id,"contract_id":"custom.invoice","contract_version":"1","phase":"published","payload":{"invoice/id":"A-001"}}),
    };
    let WorkflowEventInvocationOutcome::Invoked(first) =
        service.invoke_event(command.clone()).await.unwrap()
    else {
        panic!("active event subscription should invoke");
    };
    assert!(first.created);
    assert_eq!(
        first.flow_run.run_mode,
        domain::FlowRunMode::WorkflowEventRun
    );
    assert_eq!(first.flow_run.publication_version_id, Some(publication.id));
    assert_eq!(
        first.flow_run.input_payload["event-start"]["invoice"],
        "A-001"
    );
    assert_eq!(
        first.flow_run.input_payload["trigger"]["signal_id"],
        signal_id.to_string()
    );
    assert!(first.flow_run.input_payload["trigger"]
        .get("context")
        .is_none());
    assert!(first.flow_run.input_payload.get("payload").is_none());
    let WorkflowEventInvocationOutcome::Invoked(second) =
        service.invoke_event(command.clone()).await.unwrap()
    else {
        panic!("repeat signal should resolve the same run");
    };
    assert!(!second.created);
    assert_eq!(first.flow_run.id, second.flow_run.id);
    let mut invalid_workspace = command.clone();
    invalid_workspace.context["workspace_id"] = json!(Uuid::now_v7());
    assert!(service.invoke_event(invalid_workspace).await.is_err());
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: user_id,
            application_id: app.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: false,
        })
        .await
        .unwrap();
    assert!(matches!(
        service.invoke_event(command.clone()).await.unwrap(),
        WorkflowEventInvocationOutcome::Skipped {
            reason: "publication_changed"
        }
    ));
    let mut changed = configuration();
    changed.enabled = false;
    service
        .replace_trigger(user_id, app.id, changed)
        .await
        .unwrap();
    assert!(matches!(
        service.invoke_event(command).await.unwrap(),
        WorkflowEventInvocationOutcome::Skipped {
            reason: "trigger_changed"
        }
    ));
}

#[tokio::test]
async fn event_configuration_obeys_application_permissions_and_trigger_kind() {
    use crate::application_public_api::ApplicationPublicApiTestHarness;
    let user = Uuid::from_u128(0x11111111111111111111111111111111);
    let harness = ApplicationPublicApiTestHarness::new_with_permissions(Vec::new());
    let application = harness.seed_workflow_application(user, "No event edit permission");
    harness.set_workflow_trigger_type(application.id, domain::WorkflowTriggerType::Event);
    let service = WorkflowEventTriggerService::new(harness.repository());
    let error = service
        .replace_trigger(user, application.id, configuration())
        .await
        .unwrap_err();
    assert!(error
        .downcast_ref::<ControlPlaneError>()
        .is_some_and(|error| matches!(error, ControlPlaneError::PermissionDenied(_))));
    let allowed = ApplicationPublicApiTestHarness::new();
    let application = allowed.seed_workflow_application(user, "Extension workflow");
    assert!(WorkflowEventTriggerService::new(allowed.repository())
        .get_trigger(user, application.id)
        .await
        .is_err());
    allowed.set_workflow_trigger_type(application.id, domain::WorkflowTriggerType::Event);
    assert!(
        super::super::workflow_schedule::WorkflowScheduleTriggerService::new(allowed.repository())
            .get_trigger(
                super::super::workflow_schedule::GetWorkflowScheduleTriggerCommand {
                    actor_user_id: user,
                    application_id: application.id
                }
            )
            .await
            .is_err()
    );
}

#[test]
fn event_mapping_rejects_objects_for_scalar_start_fields_without_stringifying() {
    let contract = parse_workflow_start_event_inputs(&document()).unwrap();
    let mapping = BTreeMap::from([("invoice".into(), "/payload".into())]);
    assert!(build_workflow_start_event_input_payload(
        &contract,
        &mapping,
        &json!({"payload":{"invoice":"A"}})
    )
    .is_err());
}
