//! Durable communication state. Provider download credentials are never part of public records.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommunicationBindingInput {
    pub application_id: Uuid,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub file_table_id: Option<Uuid>,
    #[serde(default)]
    pub mcp_instance_ids: Vec<String>,
    pub system_prompt: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationBindingRecord {
    pub workspace_id: Uuid,
    pub instance_id: Uuid,
    pub actor_user_id: Uuid,
    #[serde(flatten)]
    pub config: CommunicationBindingInput,
    pub plugin_mcp_instance_id: String,
    pub revision: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationConversation {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub instance_id: Uuid,
    pub external_identity: String,
    pub external_conversation_key: String,
    pub sender_display_name: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommunicationProcessingStatus {
    Pending,
    Processing,
    Succeeded,
    Failed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommunicationSendStatus {
    None,
    Pending,
    Sending,
    Succeeded,
    Failed,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommunicationAttachmentStatus {
    Pending,
    Ready,
    Failed,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationMessage {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub external_message_key: String,
    pub received_at_ms: i64,
    pub content: Option<String>,
    pub processing_status: CommunicationProcessingStatus,
    /// Current backend eligibility; clients must not derive this from visible status alone.
    pub can_retry: bool,
    pub run_id: Option<Uuid>,
    pub final_reply: Option<String>,
    pub send_status: CommunicationSendStatus,
    pub failure_code: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationAttachment {
    pub id: Uuid,
    pub message_id: Uuid,
    pub provider_attachment_id: String,
    pub file_name: String,
    pub media_type: String,
    pub size_bytes: Option<i64>,
    pub status: CommunicationAttachmentStatus,
    pub file_table_id: Option<Uuid>,
    pub file_record_id: Option<Uuid>,
    pub failure_code: Option<String>,
}
/// Host constructs this scope from the active channel execution; tools cannot deserialize it.
#[derive(Debug, Clone)]
pub struct CommunicationHistoryScope {
    pub workspace_id: Uuid,
    pub instance_id: Uuid,
    pub external_identity: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationPage<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<Uuid>,
}
