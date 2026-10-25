//! Pull receive returns candidates only. A plugin must not acknowledge or discard provider
//! messages itself: Host commits the opaque cursor only after a durable, deduplicated inbox.
//! Attachment bytes travel through bounded typed plugin calls, never arbitrary Host URL fetches.
use std::collections::BTreeSet;

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{compile_schema, identity, invalid, ContractResult, NotificationFailureCode};
use crate::ContributionId;

pub const NOTIFICATION_MAX_RECEIVE_MESSAGES: u32 = 100;
pub const NOTIFICATION_MAX_RECEIVE_WAIT_SECONDS: u32 = 30;
pub const NOTIFICATION_MAX_RECEIVE_BATCH_BYTES: usize = 512 * 1024;
/// Transport chunk size, not a total attachment limit. Host streams chunks to its file service.
pub const NOTIFICATION_MAX_ATTACHMENT_CHUNK_BYTES: u32 = 48 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationReceiveDescriptor {
    pub contribution_id: ContributionId,
    /// Plugin-owned default copied into editable channel configuration by Host.
    pub default_system_prompt: String,
    pub mcp_tools: Vec<NotificationMcpToolDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationMcpToolDescriptor {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub operation: NotificationMcpOperation,
}

/// Host executes these operations with immutable workspace/channel/conversation/sender scope.
/// Tool arguments can narrow a query but can never replace the execution scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationMcpOperation {
    HistoryMessages,
    HistoryFiles,
    ReadFile,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationReceiveBatch {
    pub messages: Vec<NotificationReceivedMessage>,
    /// Opaque provider cursor. This is only a candidate, including for empty successful polls.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_cursor: Option<Value>,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationReceivedMessage {
    pub provider_message_id: String,
    pub conversation_id: String,
    pub sender: NotificationSender,
    pub received_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<NotificationReceivedAttachment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationSender {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationReceivedAttachment {
    pub provider_attachment_id: String,
    pub file_name: String,
    pub media_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    pub reference: NotificationAttachmentReference,
}

/// Provider-private download locator, persisted Host-side and never exposed to model or UI.
/// The locator grants no scope or authority: Host must bind it to the stored inbound attachment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationAttachmentReference {
    pub provider_message_id: String,
    pub provider_attachment_id: String,
    pub download_token: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum NotificationReceiveOutcome {
    Succeeded {
        batch: NotificationReceiveBatch,
    },
    Failed {
        failure_code: NotificationFailureCode,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum NotificationAttachmentDownloadOutcome {
    Succeeded {
        offset: u64,
        data_base64: String,
        eof: bool,
    },
    Failed {
        failure_code: NotificationFailureCode,
    },
}

impl NotificationReceiveDescriptor {
    pub fn validate(&self) -> ContractResult {
        if self.default_system_prompt.trim().is_empty() || self.default_system_prompt.len() > 65_536
        {
            return Err(invalid("receive requires a bounded default system prompt"));
        }
        let mut names = BTreeSet::new();
        let mut operations = BTreeSet::new();
        for tool in &self.mcp_tools {
            if !identity(&tool.name)
                || tool.name.contains('/')
                || tool.name.len() > 64
                || !names.insert(&tool.name)
                || !operations.insert(tool.operation)
                || tool.description.trim().is_empty()
                || tool.description.len() > 4096
                || tool.input_schema.get("type").and_then(Value::as_str) != Some("object")
                || serde_json::to_vec(&tool.input_schema).map_or(true, |bytes| bytes.len() > 32_768)
            {
                return Err(invalid("receive MCP tools require unique bounded names and operations with object schemas"));
            }
            compile_schema("receive.mcp_tools.input_schema", &tool.input_schema)?;
        }
        if operations
            != [
                NotificationMcpOperation::HistoryMessages,
                NotificationMcpOperation::HistoryFiles,
                NotificationMcpOperation::ReadFile,
            ]
            .into_iter()
            .collect()
        {
            return Err(invalid(
                "receive requires history messages, history files and read file MCP tools",
            ));
        }
        Ok(())
    }
}

fn bounded_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 1024 && !value.chars().any(char::is_control)
}

impl NotificationAttachmentReference {
    pub fn validate(&self) -> ContractResult {
        if !bounded_id(&self.provider_message_id)
            || !bounded_id(&self.provider_attachment_id)
            || serde_json::to_vec(&self.download_token).map_or(true, |bytes| bytes.len() > 16_384)
        {
            return Err(invalid(
                "attachment requires bounded provider identity and download token",
            ));
        }
        Ok(())
    }
}

impl NotificationReceiveBatch {
    pub fn validate(&self, max_messages: u32) -> ContractResult {
        if max_messages == 0
            || max_messages > NOTIFICATION_MAX_RECEIVE_MESSAGES
            || self.messages.len() > max_messages as usize
            || serde_json::to_vec(self).map_or(true, |bytes| {
                bytes.len() > NOTIFICATION_MAX_RECEIVE_BATCH_BYTES
            })
        {
            return Err(invalid(
                "receive batch exceeds requested message or wire budget",
            ));
        }
        let mut messages = BTreeSet::new();
        for message in &self.messages {
            if !bounded_id(&message.provider_message_id)
                || !bounded_id(&message.conversation_id)
                || !bounded_id(&message.sender.id)
                || message.received_at_ms == 0
                || message
                    .sender
                    .display_name
                    .as_ref()
                    .is_some_and(|name| name.len() > 1024)
                || !messages.insert((&message.conversation_id, &message.provider_message_id))
                || (message
                    .text
                    .as_ref()
                    .is_none_or(|text| text.trim().is_empty())
                    && message.attachments.is_empty())
                || message.attachments.len() > 32
            {
                return Err(invalid(
                    "receive requires unique identified messages with text or attachments",
                ));
            }
            let mut attachments = BTreeSet::new();
            for attachment in &message.attachments {
                attachment.reference.validate()?;
                if !bounded_id(&attachment.provider_attachment_id)
                    || !bounded_id(&attachment.file_name)
                    || attachment.media_type.trim().is_empty()
                    || attachment.media_type.len() > 256
                    || !attachments.insert(&attachment.provider_attachment_id)
                    || attachment.reference.provider_message_id != message.provider_message_id
                    || attachment.reference.provider_attachment_id
                        != attachment.provider_attachment_id
                {
                    return Err(invalid(
                        "attachment identity must match its received message",
                    ));
                }
            }
        }
        Ok(())
    }
}

impl NotificationAttachmentDownloadOutcome {
    pub fn validate(&self, expected_offset: u64, max_bytes: u32) -> ContractResult {
        if max_bytes == 0 || max_bytes > NOTIFICATION_MAX_ATTACHMENT_CHUNK_BYTES {
            return Err(invalid("attachment chunk request exceeds transport budget"));
        }
        if let Self::Succeeded {
            offset,
            data_base64,
            eof,
        } = self
        {
            if *offset != expected_offset
                || data_base64.len() > (max_bytes as usize).div_ceil(3) * 4
            {
                return Err(invalid(
                    "attachment chunk offset or encoded size is invalid",
                ));
            }
            let bytes = STANDARD
                .decode(data_base64)
                .map_err(|_| invalid("attachment chunk requires standard base64"))?;
            if bytes.len() > max_bytes as usize
                || (bytes.is_empty() && !eof)
                || offset.checked_add(bytes.len() as u64).is_none()
            {
                return Err(invalid(
                    "attachment chunk exceeds budget or makes no progress",
                ));
            }
        }
        Ok(())
    }
}
