use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApplicationType {
    AgentFlow,
    Workflow,
}

impl ApplicationType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AgentFlow => "agent_flow",
            Self::Workflow => "workflow",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowTriggerType {
    Extension,
    Schedule,
}

impl WorkflowTriggerType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Extension => "extension",
            Self::Schedule => "schedule",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "extension" => Some(Self::Extension),
            "schedule" => Some(Self::Schedule),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApplicationPublicationStatus {
    Published,
    Unpublished,
}

impl ApplicationPublicationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Published => "published",
            Self::Unpublished => "unpublished",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationOrchestrationSection {
    pub status: String,
    pub subject_kind: String,
    pub subject_status: String,
    pub current_subject_id: Option<Uuid>,
    pub current_draft_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationApiSection {
    pub status: String,
    pub credential_kind: String,
    pub invoke_routing_mode: String,
    pub invoke_path_template: Option<String>,
    pub api_capability_status: String,
    pub credentials_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationLogsSection {
    pub status: String,
    pub runs_capability_status: String,
    pub run_object_kind: String,
    pub log_retention_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationMonitoringSection {
    pub status: String,
    pub metrics_capability_status: String,
    pub metrics_object_kind: String,
    pub tracing_config_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationSections {
    pub orchestration: ApplicationOrchestrationSection,
    pub api: ApplicationApiSection,
    pub logs: ApplicationLogsSection,
    pub monitoring: ApplicationMonitoringSection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationTag {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationTagCatalogEntry {
    pub id: Uuid,
    pub name: String,
    pub application_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationEnvironmentVariable {
    pub application_id: Uuid,
    pub name: String,
    pub value_type: String,
    pub value: serde_json::Value,
    pub description: String,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationRecord {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub application_type: ApplicationType,
    pub workflow_trigger_type: Option<WorkflowTriggerType>,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub icon_type: Option<String>,
    pub icon_background: Option<String>,
    pub created_by: Uuid,
    pub updated_at: OffsetDateTime,
    pub tags: Vec<ApplicationTag>,
    pub sections: ApplicationSections,
}
