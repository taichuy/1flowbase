//! Event configuration and source-owned ephemeral observation contracts.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventTriggerPhase {
    Subscribed,
    Published,
    Consumed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventDefinitionOrigin {
    Custom,
    Core,
    Plugin,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventDefinitionRecord {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub contract_id: String,
    pub contract_version: String,
    pub name: String,
    pub description: String,
    pub payload_schema: Value,
    pub enabled: bool,
    pub created_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// Read-only projection of registered sources. Empty phases mean no source adapter is available.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventCatalogEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<EventCatalogSource>,
    pub id: Option<Uuid>,
    pub enabled: bool,
    pub owner_id: String,
    pub contract_id: String,
    pub contract_version: String,
    pub name: String,
    pub description: String,
    pub payload_schema: Value,
    pub origin: EventDefinitionOrigin,
    pub supported_phases: Vec<EventTriggerPhase>,
}

impl From<EventDefinitionRecord> for EventCatalogEntry {
    fn from(value: EventDefinitionRecord) -> Self {
        Self {
            source: None,
            id: Some(value.id),
            enabled: value.enabled,
            owner_id: value.workspace_id.to_string(),
            contract_id: value.contract_id,
            contract_version: value.contract_version,
            name: value.name,
            description: value.description,
            payload_schema: value.payload_schema,
            origin: EventDefinitionOrigin::Custom,
            supported_phases: vec![
                EventTriggerPhase::Subscribed,
                EventTriggerPhase::Published,
                EventTriggerPhase::Consumed,
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateEventDefinitionInput {
    pub contract_id: String,
    pub contract_version: String,
    pub name: String,
    pub description: String,
    pub payload_schema: Value,
    pub enabled: bool,
}

/// Contract identity and creator are immutable; create another definition for a new version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateEventDefinitionInput {
    pub name: String,
    pub description: String,
    pub payload_schema: Value,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowEventTriggerInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<EventSourceSelection>,
    pub enabled: bool,
    pub contract_id: String,
    pub contract_version: String,
    pub trigger_phase: EventTriggerPhase,
    /// Workflow input name -> RFC6901 pointer into the lifecycle signal.
    pub input_mapping: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowEventTriggerRecord {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub application_id: Uuid,
    #[serde(flatten)]
    pub configuration: WorkflowEventTriggerInput,
    pub updated_by: Uuid,
    pub updated_at: OffsetDateTime,
}

/// RFC6901 string form (the empty string selects the complete document).
pub fn is_json_pointer(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    if !value.starts_with('/') {
        return false;
    }
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '~' && !matches!(chars.next(), Some('0' | '1')) {
            return false;
        }
    }
    true
}

#[cfg(test)]
#[path = "_tests/configuration.rs"]
mod tests;

mod source;
pub use source::*;
