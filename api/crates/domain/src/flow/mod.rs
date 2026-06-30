use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

pub use crate::system_defaults::flow_document::{FLOW_SCHEMA_VERSION, WORKFLOW_SYNC_TIMEOUT_MS};
pub use crate::system_defaults::runtime_policy::FLOW_AUTOSAVE_INTERVAL_SECONDS;

pub const FLOW_HISTORY_LIMIT: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowChangeKind {
    Layout,
    Logical,
}

impl FlowChangeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Layout => "layout",
            Self::Logical => "logical",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowVersionTrigger {
    Autosave,
    Restore,
}

impl FlowVersionTrigger {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Autosave => "autosave",
            Self::Restore => "restore",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowRecord {
    pub id: Uuid,
    pub application_id: Uuid,
    pub created_by: Uuid,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowDraftRecord {
    pub id: Uuid,
    pub flow_id: Uuid,
    pub schema_version: String,
    pub document: serde_json::Value,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowVersionRecord {
    pub id: Uuid,
    pub flow_id: Uuid,
    pub sequence: i64,
    pub trigger: FlowVersionTrigger,
    pub change_kind: FlowChangeKind,
    pub summary: String,
    pub summary_is_custom: bool,
    pub is_protected: bool,
    pub document: serde_json::Value,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowEditorState {
    pub flow: FlowRecord,
    pub draft: FlowDraftRecord,
    pub versions: Vec<FlowVersionRecord>,
    pub autosave_interval_seconds: u16,
}

pub fn default_flow_document(flow_id: Uuid) -> serde_json::Value {
    crate::system_defaults::flow_document::default_flow_document(flow_id)
}

pub fn default_flow_document_for_application(
    application_type: crate::ApplicationType,
    flow_id: Uuid,
) -> serde_json::Value {
    crate::system_defaults::flow_document::default_flow_document_for_application(
        application_type,
        flow_id,
    )
}
