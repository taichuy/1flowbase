use domain::{ApplicationType, RoleScopeKind, WorkflowTriggerType};
use serde_json::json;
use storage_postgres::mappers::application_mapper::{
    parse_application_type, planned_sections, PgApplicationMapper, StoredApplicationRow,
};
use time::OffsetDateTime;
use uuid::Uuid;

#[test]
fn parse_application_type_accepts_known_storage_values() {
    assert!(matches!(
        parse_application_type("agent_flow").unwrap(),
        ApplicationType::AgentFlow
    ));
    assert!(matches!(
        parse_application_type("workflow").unwrap(),
        ApplicationType::Workflow
    ));
}

#[test]
fn parse_application_type_rejects_unknown_storage_value() {
    let error = parse_application_type("form").unwrap_err();

    assert_eq!(error.to_string(), "unknown application_type: form");
}

#[test]
fn parse_workflow_trigger_type_accepts_known_storage_values() {
    assert!(matches!(
        storage_postgres::mappers::application_mapper::parse_workflow_trigger_type("extension")
            .unwrap(),
        WorkflowTriggerType::Extension
    ));
    assert!(matches!(
        storage_postgres::mappers::application_mapper::parse_workflow_trigger_type("schedule")
            .unwrap(),
        WorkflowTriggerType::Schedule
    ));
    assert!(
        storage_postgres::mappers::application_mapper::parse_workflow_trigger_type("manual")
            .is_err()
    );
}

#[test]
fn planned_sections_expose_public_api_template_before_configuration() {
    let sections = planned_sections(ApplicationType::Workflow);

    assert_eq!(sections.orchestration.status, "planned");
    assert_eq!(sections.orchestration.subject_kind, "workflow");
    assert_eq!(sections.orchestration.subject_status, "unconfigured");
    assert_eq!(sections.orchestration.current_subject_id, None);
    assert_eq!(sections.orchestration.current_draft_id, None);

    assert_eq!(sections.api.status, "planned");
    assert_eq!(sections.api.credential_kind, "application_api_key");
    assert_eq!(
        sections.api.invoke_routing_mode,
        "api_key_bound_application"
    );
    assert_eq!(
        sections.api.invoke_path_template.as_deref(),
        Some("/api/agent/v1/runs")
    );
    assert_eq!(sections.api.api_capability_status, "not_published");
    assert_eq!(sections.api.credentials_status, "missing");

    assert_eq!(sections.logs.status, "planned");
    assert_eq!(sections.logs.runs_capability_status, "planned");
    assert_eq!(sections.logs.run_object_kind, "application_run");
    assert_eq!(sections.logs.log_retention_status, "planned");

    assert_eq!(sections.monitoring.status, "planned");
    assert_eq!(sections.monitoring.metrics_capability_status, "planned");
    assert_eq!(
        sections.monitoring.metrics_object_kind,
        "application_metrics"
    );
    assert_eq!(sections.monitoring.tracing_config_status, "planned");
}

#[test]
fn application_mapper_maps_tags_type_and_ready_sections() {
    let application_id = Uuid::now_v7();
    let workspace_id = Uuid::now_v7();
    let creator_id = Uuid::now_v7();
    let current_flow_id = Uuid::now_v7();
    let current_draft_id = Uuid::now_v7();
    let tag_id = Uuid::now_v7();

    let record = PgApplicationMapper::to_application_record(StoredApplicationRow {
        id: application_id,
        workspace_id,
        application_type: "agent_flow".into(),
        workflow_trigger_type: None,
        name: "Support Agent".into(),
        description: "Routes support requests".into(),
        icon: Some("sparkles".into()),
        icon_type: Some("lucide".into()),
        icon_background: Some("#0f172a".into()),
        created_by: creator_id,
        updated_at: OffsetDateTime::now_utc(),
        release_version: 0,
        release_digest: None,
        current_flow_id: Some(current_flow_id),
        current_draft_id: Some(current_draft_id),
        api_enabled: false,
        has_application_api_keys: false,
        has_application_api_mapping: false,
        active_publication_id: None,
        tags: json!([
            {
                "id": tag_id,
                "name": "Support"
            }
        ]),
    })
    .unwrap();

    assert_eq!(record.id, application_id);
    assert_eq!(record.workspace_id, workspace_id);
    assert!(matches!(
        record.application_type,
        ApplicationType::AgentFlow
    ));
    assert_eq!(record.tags.len(), 1);
    assert_eq!(record.tags[0].id, tag_id);
    assert_eq!(record.tags[0].name, "Support");

    assert_eq!(record.sections.orchestration.status, "ready");
    assert_eq!(record.sections.orchestration.subject_kind, "agent_flow");
    assert_eq!(record.sections.orchestration.subject_status, "editable");
    assert_eq!(
        record.sections.orchestration.current_subject_id,
        Some(current_flow_id)
    );
    assert_eq!(
        record.sections.orchestration.current_draft_id,
        Some(current_draft_id)
    );
    assert_eq!(record.sections.logs.status, "ready");
    assert_eq!(record.sections.logs.runs_capability_status, "queryable");
}

#[test]
fn application_mapper_marks_api_section_active_when_key_mapping_and_publication_exist() {
    let application_id = Uuid::now_v7();
    let current_flow_id = Uuid::now_v7();
    let current_draft_id = Uuid::now_v7();
    let publication_id = Uuid::now_v7();

    let record = PgApplicationMapper::to_application_record(StoredApplicationRow {
        id: application_id,
        workspace_id: Uuid::now_v7(),
        application_type: "agent_flow".into(),
        workflow_trigger_type: None,
        name: "Support Agent".into(),
        description: "Routes support requests".into(),
        icon: None,
        icon_type: None,
        icon_background: None,
        created_by: Uuid::now_v7(),
        updated_at: OffsetDateTime::now_utc(),
        release_version: 2,
        release_digest: Some("digest-v2".into()),
        current_flow_id: Some(current_flow_id),
        current_draft_id: Some(current_draft_id),
        api_enabled: true,
        has_application_api_keys: true,
        has_application_api_mapping: true,
        active_publication_id: Some(publication_id),
        tags: json!([]),
    })
    .unwrap();

    assert_eq!(record.sections.api.status, "active");
    assert_eq!(record.sections.api.credential_kind, "application_api_key");
    assert_eq!(
        record.sections.api.invoke_routing_mode,
        "api_key_bound_application"
    );
    assert_eq!(
        record.sections.api.invoke_path_template.as_deref(),
        Some("/api/agent/v1/runs")
    );
    assert_eq!(record.sections.api.api_capability_status, "enabled");
    assert_eq!(record.sections.api.credentials_status, "configured");
}

#[test]
fn decode_role_scope_kind_maps_legacy_and_default_scope_values() {
    assert!(matches!(
        crate::repositories::decode_role_scope_kind("system"),
        RoleScopeKind::System
    ));
    assert!(matches!(
        crate::repositories::decode_role_scope_kind("app"),
        RoleScopeKind::System
    ));
    assert!(matches!(
        crate::repositories::decode_role_scope_kind("workspace"),
        RoleScopeKind::Workspace
    ));
    assert!(matches!(
        crate::repositories::decode_role_scope_kind("unknown"),
        RoleScopeKind::Workspace
    ));
}
