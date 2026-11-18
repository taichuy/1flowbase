use super::*;
use crate::routes::_tests::time_response_cases::assert_timestamp_mapping;

#[test]
fn environment_variable_response_timestamp_contract() {
    assert_timestamp_mapping(|at| {
        let response =
            to_application_environment_variable(domain::ApplicationEnvironmentVariable {
                application_id: Uuid::nil(),
                name: "setting".into(),
                value_type: "string".into(),
                value: serde_json::json!("value"),
                description: "description".into(),
                updated_at: at,
            })?;
        Ok(vec![response.updated_at])
    });
}

fn application(at: time::OffsetDateTime) -> domain::ApplicationRecord {
    domain::ApplicationRecord {
        id: Uuid::nil(),
        workspace_id: Uuid::nil(),
        application_type: domain::ApplicationType::AgentFlow,
        workflow_trigger_type: None,
        name: "application".into(),
        description: "description".into(),
        icon: None,
        icon_type: None,
        icon_background: None,
        created_by: Uuid::nil(),
        updated_at: at,
        release_version: 0,
        release_digest: None,
        tags: vec![],
        sections: domain::ApplicationSections {
            orchestration: domain::ApplicationOrchestrationSection {
                status: "planned".into(),
                subject_kind: "agent_flow".into(),
                subject_status: "unconfigured".into(),
                current_subject_id: None,
                current_draft_id: None,
            },
            api: domain::ApplicationApiSection {
                status: "planned".into(),
                credential_kind: "application_api_key".into(),
                invoke_routing_mode: "api_key_bound_application".into(),
                invoke_path_template: None,
                api_capability_status: "not_published".into(),
                credentials_status: "missing".into(),
            },
            logs: domain::ApplicationLogsSection {
                status: "planned".into(),
                runs_capability_status: "planned".into(),
                run_object_kind: "application_run".into(),
                log_retention_status: "planned".into(),
            },
            monitoring: domain::ApplicationMonitoringSection {
                status: "planned".into(),
                metrics_capability_status: "planned".into(),
                metrics_object_kind: "application_metrics".into(),
                tracing_config_status: "planned".into(),
            },
        },
    }
}

#[test]
fn application_summary_response_timestamp_contract() {
    assert_timestamp_mapping(|at| Ok(vec![to_application_summary(application(at))?.updated_at]));
}

#[test]
fn application_detail_response_timestamp_contract() {
    assert_timestamp_mapping(|at| Ok(vec![to_application_detail(application(at))?.updated_at]));
}
