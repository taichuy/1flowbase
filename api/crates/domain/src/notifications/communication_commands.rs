use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommunicationCommandSetting {
    pub command_id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommunicationCommandSourceKind {
    Builtin,
    Plugin,
    User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationCommandView {
    pub command_id: String,
    pub name: String,
    pub description: String,
    pub usage: String,
    pub source_kind: CommunicationCommandSourceKind,
    pub source_name: String,
    pub installation_id: Option<Uuid>,
    pub enabled: bool,
    pub available: bool,
    pub unavailable_reason: Option<String>,
}

/// Host-normalized command context -> event payload. Identity never comes from slash text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommunicationEventCommandBinding {
    pub command_id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub contract_id: String,
    pub contract_version: String,
    pub input_mapping: std::collections::BTreeMap<String, String>,
}
