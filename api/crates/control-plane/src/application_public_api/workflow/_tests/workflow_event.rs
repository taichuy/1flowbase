use super::super::workflow_start_http_inputs::{
    build_workflow_start_event_input_payload, parse_workflow_start_event_inputs,
};
use super::*;
use serde_json::json;
use std::collections::BTreeMap;

fn configuration() -> WorkflowEventTriggerInput {
    WorkflowEventTriggerInput {
        source: None,
        enabled: true,
        contract_id: "custom.invoice".into(),
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
    // Preserve the pre-interface-rule consumer contract using a persisted legacy record.
    let now = time::OffsetDateTime::now_utc();
    let mut definition = domain::event_center::EventDefinitionRecord {
        interface_event: None,
        id: Uuid::now_v7(),
        workspace_id: actor.current_workspace_id,
        contract_id: configuration().contract_id,
        name: "Invoice".into(),
        description: String::new(),
        payload_schema: json!({"type":"object"}),
        enabled: true,
        created_by: actor.user_id,
        created_at: now,
        updated_at: now,
    };
    repository
        .create_event_definition(&definition)
        .await
        .unwrap();
    let service = WorkflowEventTriggerService::new(repository.clone());
    // Unpublished configuration is persisted without starting or manufacturing a run.
    let mut legacy_request = configuration();
    legacy_request.enabled = false;
    let draft_trigger = service
        .replace_trigger(user_id, app.id, legacy_request)
        .await
        .unwrap();
    assert!(
        draft_trigger.configuration.enabled,
        "legacy client flags cannot disable a publication-owned listener"
    );
    assert!(service.prepare_subscription(&draft_trigger).await.is_err());
    let unchanged = service
        .replace_trigger(user_id, app.id, configuration())
        .await
        .unwrap();
    assert_eq!(
        unchanged, draft_trigger,
        "no-op save must preserve the activation snapshot"
    );
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: user_id,
            application_id: app.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    repository.set_active_publication_document_snapshot(app.id, document());
    // Legacy backups retain this field. It must no longer suppress a published listener.
    let mut legacy = repository
        .get_workflow_event_trigger(app.workspace_id, app.id)
        .await
        .unwrap()
        .unwrap();
    legacy.configuration.enabled = false;
    repository
        .save_workflow_event_trigger(&legacy)
        .await
        .unwrap();
    assert!(repository
        .list_workflow_event_triggers(app.workspace_id)
        .await
        .unwrap()
        .iter()
        .any(|row| row.id == legacy.id));
    let trigger = service.get_trigger(user_id, app.id).await.unwrap().unwrap();
    let publication = service.prepare_subscription(&trigger).await.unwrap();
    let signal_id = Uuid::now_v7();
    let mut command = InvokeWorkflowEventCommand {
        trigger,
        publication: publication.clone(),
        signal_id,
        event_id: Some(signal_id.to_string()),
        subscription_id: Uuid::now_v7(),
        context: json!({"workspace_id":app.workspace_id,"event_id":signal_id,"contract_id":"custom.invoice","phase":"published","payload":{"invoice/id":"A-001"}}),
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
    let publications = ApplicationPublicationService::new(repository.clone());
    publications
        .set_api_enabled(
            crate::application_public_api::publications::SetApplicationApiEnabledCommand {
                actor_user_id: user_id,
                application_id: app.id,
                api_enabled: false,
            },
        )
        .await
        .unwrap();
    assert!(service
        .prepare_subscription(&command.trigger)
        .await
        .is_err());
    assert!(matches!(
        service.invoke_event(command.clone()).await.unwrap(),
        WorkflowEventInvocationOutcome::Skipped {
            reason: "application_disabled"
        }
    ));
    publications
        .set_api_enabled(
            crate::application_public_api::publications::SetApplicationApiEnabledCommand {
                actor_user_id: user_id,
                application_id: app.id,
                api_enabled: true,
            },
        )
        .await
        .unwrap();
    command.publication = service
        .prepare_subscription(&command.trigger)
        .await
        .unwrap();
    publications
        .unpublish(
            crate::application_public_api::publications::UnpublishApplicationCommand {
                actor_user_id: user_id,
                application_id: app.id,
            },
        )
        .await
        .unwrap();
    assert!(service
        .prepare_subscription(&command.trigger)
        .await
        .is_err());
    assert!(matches!(
        service.invoke_event(command.clone()).await.unwrap(),
        WorkflowEventInvocationOutcome::Skipped {
            reason: "application_unpublished"
        }
    ));
    publications
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: user_id,
            application_id: app.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    repository.set_active_publication_document_snapshot(app.id, document());
    command.publication = service
        .prepare_subscription(&command.trigger)
        .await
        .unwrap();
    definition.enabled = false;
    assert!(repository
        .update_event_definition(&definition)
        .await
        .unwrap());
    assert!(service
        .prepare_subscription(&command.trigger)
        .await
        .is_err());
    assert!(service.invoke_event(command.clone()).await.is_err());
    definition.enabled = true;
    assert!(repository
        .update_event_definition(&definition)
        .await
        .unwrap());
    let mut invalid_workspace = command.clone();
    invalid_workspace.context["workspace_id"] = json!(Uuid::now_v7());
    assert!(service.invoke_event(invalid_workspace).await.is_err());
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: user_id,
            application_id: app.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let republished = repository
        .load_active_application_publication(app.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(republished.id, command.publication.id);
    assert_ne!(
        republished.document_snapshot,
        command.publication.document_snapshot
    );
    assert!(matches!(
        service.invoke_event(command.clone()).await.unwrap(),
        WorkflowEventInvocationOutcome::Skipped {
            reason: "publication_changed"
        }
    ));
    // The preceding republish intentionally used the default draft to prove stale rejection.
    // Restore a valid event Start contract before testing a configuration revision.
    repository.set_active_publication_document_snapshot(app.id, document());
    let mut changed = configuration();
    changed
        .input_mapping
        .insert("invoice".into(), "/payload/revised".into());
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

#[tokio::test]
async fn native_source_string_identity_and_causal_metadata_survive_invocation_without_body_archive()
{
    use crate::application_public_api::{
        mapping::ApplicationApiMappingConfig,
        publications::{ApplicationPublicationService, PublishApplicationCommand},
        ApplicationPublicApiTestHarness,
    };
    use domain::event_center::*;
    let user = Uuid::from_u128(0x11111111111111111111111111111111);
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_workflow_application(user, "Native source workflow");
    harness.set_workflow_trigger_type(application.id, domain::WorkflowTriggerType::Event);
    let repository = harness.repository();
    let scope = EventSourceScope::Workspace {
        workspace_id: application.workspace_id,
    };
    let descriptor = EventSourceDescriptor {
        source_id: "native.fact.source".into(),
        owner_id: "native".into(),
        point_id: "native.point".into(),
        contract_id: "native.fact".into(),
        name: "Fact".into(),
        description: String::new(),
        payload_schema: json!({"type":"object"}),
        origin: EventDefinitionOrigin::Core,
        scope: scope.clone(),
        binding: EventSourceBinding::Active,
        coverage: EventSourceCoverage::Live,
        supported_phases: vec![EventTriggerPhase::Published],
    };
    let service = WorkflowEventTriggerService::new(repository.clone())
        .with_registered_events(vec![descriptor.into()]);
    let mut selected = configuration();
    selected.contract_id = "native.fact".into();
    selected.source = Some(EventSourceSelection {
        filters: Default::default(),

        source_id: "native.fact.source".into(),
        scope: scope.clone(),
    });
    service
        .replace_trigger(user, application.id, selected)
        .await
        .unwrap();
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: user,
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    repository.set_active_publication_document_snapshot(application.id, document());
    let trigger = service
        .get_trigger(user, application.id)
        .await
        .unwrap()
        .unwrap();
    let publication = service.prepare_subscription(&trigger).await.unwrap();
    let signal = Uuid::now_v7();
    let ancestor = Uuid::now_v7();
    let source = EventSourceContext {
        attributes: Default::default(),

        registration_id: Uuid::now_v7(),
        scope,
        source_id: "native.fact.source".into(),
        point_id: "native.point".into(),
        original_event_id: Some("native:not-a-uuid:42".into()),
        producer_id: Some("real-producer".into()),
    };
    let command = InvokeWorkflowEventCommand {
        subscription_id: trigger.id,
        trigger,
        publication,
        signal_id: signal,
        event_id: source.original_event_id.clone(),
        context: json!({"workspace_id":application.workspace_id,"event_id":signal,"contract_id":"native.fact","phase":"published","source_context":source,"workflow_ancestor_ids":[ancestor,application.id],"payload":{"invoice/id":"native-invoice","secret":"never-store-source-body-in-trigger-metadata"}}),
    };
    let WorkflowEventInvocationOutcome::Invoked(invoked) =
        service.invoke_event(command.clone()).await.unwrap()
    else {
        panic!("native source must invoke");
    };
    let metadata = &invoked.flow_run.input_payload["trigger"];
    assert_eq!(metadata["event_id"], "native:not-a-uuid:42");
    assert_eq!(
        metadata["source_context"]["original_event_id"],
        "native:not-a-uuid:42"
    );
    assert_eq!(
        metadata["workflow_ancestor_ids"],
        json!([ancestor, application.id])
    );
    assert_eq!(
        invoked.flow_run.input_payload["event-start"]["invoice"],
        "native-invoice"
    );
    assert!(!serde_json::to_string(metadata)
        .unwrap()
        .contains("never-store-source-body"));
    let mut mismatched = command.clone();
    mismatched.context["source_context"]["scope"] = json!({"kind":"system"});
    assert!(service.invoke_event(mismatched).await.is_err());
    let mut missing = command;
    missing.context["source_context"]["original_event_id"] = Value::Null;
    assert!(service.invoke_event(missing).await.is_err());
    assert!(
        WorkflowEventTriggerService::new(repository)
            .prepare_subscription(
                &service
                    .get_trigger(user, application.id)
                    .await
                    .unwrap()
                    .unwrap()
            )
            .await
            .is_err(),
        "restored receivers revalidate source availability"
    );
}
