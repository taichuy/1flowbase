use super::*;
use domain::event_center::{EventTriggerPhase, WorkflowEventTriggerInput};
use std::collections::BTreeMap;

#[test]
fn event_archive_import_preserves_contract_and_mapping_but_disables_subscription() {
    let configuration = WorkflowEventTriggerInput {
        source: None,
        enabled: true,
        contract_id: "custom.invoice".into(),
        contract_version: "1".into(),
        trigger_phase: EventTriggerPhase::Published,
        input_mapping: BTreeMap::from([("invoice".into(), "/payload/id".into())]),
    };
    let mut entry = ApplicationArchiveEntry {
        template_id: String::new(),
        release_version: 1,
        exported_from_system_version: String::new(),
        exported_at: String::new(),
        dependencies: Vec::new(),
        application: ApplicationArchiveApplication {
            application_type: "workflow".into(),
            workflow_trigger_type: Some("event".into()),
            name: "Invoice".into(),
            description: String::new(),
            icon: None,
            icon_type: None,
            icon_background: None,
        },
        flow_document: serde_json::json!({"graph":{"nodes":[{"id":"start","type":"workflow_start","config":{"input_fields":[{"key":"invoice","valueType":"string","required":true}]}}]}}),
        workflow_trigger_config: Some(WorkflowTriggerTemplateConfig::Event {
            configuration: configuration.clone(),
        }),
    };
    let (kind, config, extension) =
        archive_workflow_trigger_config(domain::ApplicationType::Workflow, &entry).unwrap();
    assert_eq!(kind, Some(domain::WorkflowTriggerType::Event));
    assert!(extension.is_none());
    let Some(CreateWorkflowTriggerConfig::Event {
        configuration: imported,
    }) = config
    else {
        panic!("event must retain its distinct configuration");
    };
    assert!(!imported.enabled);
    assert_eq!(imported.contract_id, configuration.contract_id);
    assert_eq!(imported.input_mapping, configuration.input_mapping);
    entry.application.workflow_trigger_type = Some("schedule".into());
    assert!(archive_workflow_trigger_config(domain::ApplicationType::Workflow, &entry).is_err());
}

#[test]
fn create_rejects_event_configuration_for_schedule_or_nonworkflow_application() {
    use crate::application::validation_policy::validate_create_workflow_trigger_config;
    let configuration = CreateWorkflowTriggerConfig::Event {
        configuration: WorkflowEventTriggerInput {
            source: None,
            enabled: false,
            contract_id: "custom.invoice".into(),
            contract_version: "1".into(),
            trigger_phase: EventTriggerPhase::Consumed,
            input_mapping: BTreeMap::new(),
        },
    };
    assert!(validate_create_workflow_trigger_config(
        domain::ApplicationType::Workflow,
        Some(domain::WorkflowTriggerType::Event),
        Some(&configuration)
    )
    .is_ok());
    assert!(validate_create_workflow_trigger_config(
        domain::ApplicationType::Workflow,
        Some(domain::WorkflowTriggerType::Schedule),
        Some(&configuration)
    )
    .is_err());
    assert!(validate_create_workflow_trigger_config(
        domain::ApplicationType::AgentFlow,
        None,
        Some(&configuration)
    )
    .is_err());
}
