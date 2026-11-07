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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface_event: Option<InterfaceEventBinding>,
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub contract_id: String,
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
    pub interface_event: Option<InterfaceEventBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub group_path: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<EventCatalogSource>,
    pub id: Option<Uuid>,
    pub scope: EventSourceScope,
    pub enabled: Option<bool>,
    pub owner_id: Option<String>,
    pub contract_id: String,
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
            interface_event: value.interface_event,
            group_path: vec!["interface_events".into()],
            id: Some(value.id),
            scope: EventSourceScope::Workspace {
                workspace_id: value.workspace_id,
            },
            enabled: Some(value.enabled),
            owner_id: None,
            contract_id: value.contract_id,
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
    pub name: String,
    pub description: String,
    pub interface_event: InterfaceEventBinding,
    pub enabled: bool,
}

/// Event identity and creator are immutable; the payload schema is the current definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateEventDefinitionInput {
    pub name: String,
    pub description: String,
    pub interface_event: InterfaceEventBinding,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowEventTriggerInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<EventSourceSelection>,
    pub enabled: bool,
    pub contract_id: String,
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

mod interface;
pub use interface::*;
