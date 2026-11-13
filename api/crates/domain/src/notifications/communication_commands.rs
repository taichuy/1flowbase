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
    pub input_mapping: std::collections::BTreeMap<String, String>,
}

/// A channel owns one ordered configuration list for every command source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CommunicationCommandConfiguration {
    Registered(CommunicationCommandSetting),
    Event(CommunicationEventCommandBinding),
}

impl CommunicationCommandConfiguration {
    pub fn command_id(&self) -> &str {
        match self {
            Self::Registered(value) => &value.command_id,
            Self::Event(value) => &value.command_id,
        }
    }

    pub fn registered(&self) -> Option<&CommunicationCommandSetting> {
        match self {
            Self::Registered(value) => Some(value),
            Self::Event(_) => None,
        }
    }

    pub fn event(&self) -> Option<&CommunicationEventCommandBinding> {
        match self {
            Self::Event(value) => Some(value),
            Self::Registered(_) => None,
        }
    }
}
