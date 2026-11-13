use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// User-configurable event producers. A command never represents an interface invocation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventDefinitionKind {
    #[default]
    Interface,
    CommunicationCommand,
}

impl EventDefinitionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Interface => "interface",
            Self::CommunicationCommand => "communication_command",
        }
    }
}

/// Business flags remain opaque here; the receiving workflow owns their parsing and validation.
pub fn communication_command_event_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["arguments", "input_id", "conversation_id", "instance_id"],
        "properties": {
            "arguments": {"type": "string", "maxLength": 16384},
            "input_id": {"type": "string", "format": "uuid"},
            "conversation_id": {"type": "string", "format": "uuid"},
            "instance_id": {"type": "string", "format": "uuid"}
        }
    })
}

/// Identity fields are supplied by the Host, never extracted from command text.
pub fn communication_command_event_mapping() -> BTreeMap<String, String> {
    ["arguments", "input_id", "conversation_id", "instance_id"]
        .into_iter()
        .map(|field| (field.to_owned(), format!("/{field}")))
        .collect()
}
