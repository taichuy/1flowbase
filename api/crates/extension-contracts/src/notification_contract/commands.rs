//! Channel command metadata and event payloads. Host owns execution identity and reply routing.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{identity, invalid, ContractResult};
use crate::ContributionId;

pub const COMMUNICATION_COMMAND_POINT: &str = "1flowbase.communication.command.requested";
pub const COMMUNICATION_COMMAND_CONTRACT_VERSION: &str = "1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommunicationCommandDescriptor {
    /// Stable publisher/plugin/local identity, independent of the user-facing spelling.
    pub command_id: String,
    /// Slash-free command spelling. Host owns the slash parser and builtin names.
    pub name: String,
    pub description: String,
    pub usage: String,
    pub contribution_id: ContributionId,
    #[serde(default)]
    pub default_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommunicationCommandRequest {
    pub command_id: String,
    /// Original argument text after the recognized command token; never a workflow prompt.
    pub arguments: String,
    pub channel_id: String,
    /// The existing durable history conversation UUID, not the AI session identity.
    pub conversation_id: String,
    pub session_id: String,
    pub input_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommunicationCommandReply {
    /// Host binds the destination to the durable request; workers cannot select recipients.
    pub text: String,
}

pub fn communication_command_name_is_valid(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.as_bytes()[0].is_ascii_lowercase()
        && name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'-' | b'_'))
}

pub fn communication_command_name_is_reserved(name: &str) -> bool {
    matches!(name, "new" | "model" | "help" | "model-user") || name.starts_with("model-user-")
}

impl CommunicationCommandDescriptor {
    pub fn validate(&self) -> ContractResult {
        if !identity(&self.command_id)
            || !communication_command_name_is_valid(&self.name)
            || self.description.trim().is_empty()
            || self.description.len() > 4096
            || self.usage.len() > 4096
        {
            return Err(invalid(
                "command requires bounded identity, name, description and usage",
            ));
        }
        let token = self.usage.split_whitespace().next().unwrap_or_default();
        if token != format!("/{}", self.name) {
            return Err(invalid(
                "command usage must start with its exact slash name",
            ));
        }
        Ok(())
    }

    pub fn validate_plugin_namespace(&self, publisher: &str, plugin_code: &str) -> ContractResult {
        self.validate()?;
        let prefix = format!("{publisher}/{plugin_code}/");
        if communication_command_name_is_reserved(&self.name)
            || !self
                .command_id
                .strip_prefix(&prefix)
                .is_some_and(|local| !local.is_empty() && !local.contains('/') && identity(local))
        {
            return Err(invalid(
                "plugin command requires its publisher/plugin namespace and a non-reserved name",
            ));
        }
        Ok(())
    }
}

impl CommunicationCommandRequest {
    pub fn validate(&self) -> ContractResult {
        if !identity(&self.command_id)
            || self.arguments.len() > 16_384
            || [
                &self.channel_id,
                &self.conversation_id,
                &self.session_id,
                &self.input_id,
            ]
            .iter()
            .any(|id| !identity(id) || id.len() > 128)
        {
            return Err(invalid(
                "command event requires bounded request identity and arguments",
            ));
        }
        Ok(())
    }

    pub fn event_schema() -> Value {
        let id = json!({"type":"string","minLength":1,"maxLength":128});
        json!({
            "type":"object","additionalProperties":false,
            "required":["command_id","arguments","channel_id","conversation_id","session_id","input_id"],
            "properties":{
                "command_id":{"type":"string","minLength":1,"maxLength":256},
                "arguments":{"type":"string","maxLength":16384},
                "channel_id":id,"conversation_id":id,"session_id":id,"input_id":id
            }
        })
    }
}

impl CommunicationCommandReply {
    pub fn validate(&self) -> ContractResult {
        if self.text.trim().is_empty() || self.text.len() > 32_768 {
            return Err(invalid("command reply requires bounded non-empty text"));
        }
        Ok(())
    }
}
