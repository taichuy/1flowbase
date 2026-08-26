use anyhow::{anyhow, Result};
use domain::{
    ApplicationApiSection, ApplicationRecord, ApplicationSections, ApplicationTag, ApplicationType,
    WorkflowTriggerType,
};
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

use super::flow_mapper::flow_sections;

#[derive(Debug, Clone)]
pub struct StoredApplicationRow {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub application_type: String,
    pub workflow_trigger_type: Option<String>,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub icon_type: Option<String>,
    pub icon_background: Option<String>,
    pub created_by: Uuid,
    pub updated_at: OffsetDateTime,
    pub release_version: i64,
    pub release_digest: Option<String>,
    pub current_flow_id: Option<Uuid>,
    pub current_draft_id: Option<Uuid>,
    pub api_enabled: bool,
    pub has_application_api_keys: bool,
    pub has_application_api_mapping: bool,
    pub active_publication_id: Option<Uuid>,
    pub tags: Value,
}

pub struct PgApplicationMapper;

impl PgApplicationMapper {
    pub fn to_application_record(row: StoredApplicationRow) -> Result<ApplicationRecord> {
        let application_type = parse_application_type(&row.application_type)?;
        let workflow_trigger_type = row
            .workflow_trigger_type
            .as_deref()
            .map(parse_workflow_trigger_type)
            .transpose()?;

        Ok(ApplicationRecord {
            id: row.id,
            workspace_id: row.workspace_id,
            application_type,
            workflow_trigger_type,
            name: row.name,
            description: row.description,
            icon: row.icon,
            icon_type: row.icon_type,
            icon_background: row.icon_background,
            created_by: row.created_by,
            updated_at: row.updated_at,
            release_version: row.release_version,
            release_digest: row.release_digest,
            tags: serde_json::from_value::<Vec<ApplicationTag>>(row.tags)?,
            sections: application_sections(
                application_type,
                row.current_flow_id,
                row.current_draft_id,
                ApplicationApiSectionState {
                    api_enabled: row.api_enabled,
                    has_application_api_keys: row.has_application_api_keys,
                    has_application_api_mapping: row.has_application_api_mapping,
                    active_publication_id: row.active_publication_id,
                },
            ),
        })
    }
}

pub fn parse_workflow_trigger_type(value: &str) -> Result<WorkflowTriggerType> {
    WorkflowTriggerType::parse(value)
        .ok_or_else(|| anyhow!("unknown workflow_trigger_type: {value}"))
}

#[derive(Debug, Clone, Copy)]
pub struct ApplicationApiSectionState {
    pub api_enabled: bool,
    pub has_application_api_keys: bool,
    pub has_application_api_mapping: bool,
    pub active_publication_id: Option<Uuid>,
}

impl ApplicationApiSectionState {
    pub fn planned() -> Self {
        Self {
            api_enabled: false,
            has_application_api_keys: false,
            has_application_api_mapping: false,
            active_publication_id: None,
        }
    }
}

pub fn parse_application_type(value: &str) -> Result<ApplicationType> {
    match value {
        "agent_flow" => Ok(ApplicationType::AgentFlow),
        "workflow" => Ok(ApplicationType::Workflow),
        _ => Err(anyhow!("unknown application_type: {value}")),
    }
}

pub fn planned_sections(application_type: ApplicationType) -> ApplicationSections {
    application_sections(
        application_type,
        None,
        None,
        ApplicationApiSectionState::planned(),
    )
}

pub fn application_sections(
    application_type: ApplicationType,
    current_flow_id: Option<Uuid>,
    current_draft_id: Option<Uuid>,
    api_state: ApplicationApiSectionState,
) -> ApplicationSections {
    let mut sections = flow_sections(application_type, current_flow_id, current_draft_id);
    let has_active_publication = api_state.active_publication_id.is_some();
    let api_ready = api_state.has_application_api_keys
        && api_state.has_application_api_mapping
        && has_active_publication;
    sections.api = ApplicationApiSection {
        status: if api_ready { "active" } else { "planned" }.to_string(),
        credential_kind: "application_api_key".to_string(),
        invoke_routing_mode: "api_key_bound_application".to_string(),
        invoke_path_template: Some("/api/agent/v1/runs".to_string()),
        api_capability_status: if has_active_publication {
            if api_state.api_enabled {
                "enabled"
            } else {
                "disabled"
            }
        } else {
            "not_published"
        }
        .to_string(),
        credentials_status: if api_state.has_application_api_keys {
            "configured"
        } else {
            "missing"
        }
        .to_string(),
    };
    sections
}
