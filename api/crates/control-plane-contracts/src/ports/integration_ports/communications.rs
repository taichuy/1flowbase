//! Host-facing durable communication ports. Credentials intentionally do not implement Debug.
use crate::ports::SaveNotificationAuthorizationInput;
use async_trait::async_trait;
use domain::*;
use serde_json::Value;
use uuid::Uuid;

pub struct CommunicationReceiveMessage {
    pub provider_message_id: String,
    pub conversation_id: String,
    pub sender_id: String,
    pub sender_display_name: Option<String>,
    pub received_at_ms: i64,
    pub text: Option<String>,
    pub attachments: Vec<CommunicationReceiveAttachment>,
}
pub struct CommunicationReceiveAttachment {
    pub provider_attachment_id: String,
    pub file_name: String,
    pub media_type: String,
    pub size_bytes: Option<i64>,
    /// Opaque private descriptor encrypted with channel-specific AAD.
    pub reference: Value,
}
pub struct CommitCommunicationReceiveInput {
    /// Complete next authorization/secrets, fenced against the pre-receive revision.
    pub authorization: SaveNotificationAuthorizationInput,
    pub candidate_cursor: Option<Value>,
    pub messages: Vec<CommunicationReceiveMessage>,
}
pub struct ClaimedCommunicationInput {
    pub binding: CommunicationBindingRecord,
    pub conversation: CommunicationConversation,
    pub message: CommunicationMessage,
    pub claim_id: Uuid,
    pub attachments: Vec<CommunicationPendingAttachment>,
}
pub struct CommunicationPendingAttachment {
    pub record: CommunicationAttachment,
    pub reference: Value,
}
#[derive(Debug, Clone)]
pub struct CommunicationClaim {
    pub workspace_id: Uuid,
    pub instance_id: Uuid,
    pub message_id: Uuid,
    pub claim_id: Uuid,
}
#[derive(Debug, Clone)]
pub struct CommunicationAttachmentUpdate {
    pub attachment_id: Uuid,
    pub file_table_id: Option<Uuid>,
    pub file_record_id: Option<Uuid>,
    pub failure_code: Option<String>,
}
#[derive(Debug, Clone)]
pub enum CommunicationProgress {
    RunStarted { run_id: Uuid },
    RunSucceeded { final_reply: String },
    RunFailed { failure_code: String },
    SendStarted,
    SendSucceeded,
    SendFailed { failure_code: String },
    SendUnknown { failure_code: String },
}
#[async_trait]
pub trait CommunicationRepository: Send + Sync {
    async fn save_communication_binding(
        &self,
        binding: CommunicationBindingRecord,
    ) -> anyhow::Result<CommunicationBindingRecord>;
    async fn communication_binding(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
    ) -> anyhow::Result<Option<CommunicationBindingRecord>>;
    async fn enabled_communication_bindings(
        &self,
    ) -> anyhow::Result<Vec<CommunicationBindingRecord>>;
    async fn communication_receive_cursor(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
        master_key: &str,
    ) -> anyhow::Result<Option<Value>>;
    /// Inbox, private attachment descriptors, cursor and authorization CAS commit together.
    async fn commit_communication_receive(
        &self,
        input: CommitCommunicationReceiveInput,
    ) -> anyhow::Result<Option<i64>>;
    async fn communication_conversations(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
        external_identity: Option<&str>,
        cursor: Option<Uuid>,
        limit: u32,
    ) -> anyhow::Result<CommunicationPage<CommunicationConversation>>;
    async fn communication_conversation(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
        conversation_id: Uuid,
    ) -> anyhow::Result<Option<CommunicationConversation>>;
    async fn communication_file(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
        external_identity: Option<&str>,
        attachment_id: Uuid,
    ) -> anyhow::Result<Option<CommunicationAttachment>>;
    async fn communication_messages(
        &self,
        scope: &CommunicationHistoryScope,
        conversation_id: Uuid,
        cursor: Option<Uuid>,
        limit: u32,
    ) -> anyhow::Result<CommunicationPage<CommunicationMessage>>;
    async fn communication_files(
        &self,
        scope: &CommunicationHistoryScope,
        conversation_id: Uuid,
        cursor: Option<Uuid>,
        limit: u32,
    ) -> anyhow::Result<CommunicationPage<CommunicationAttachment>>;
    async fn claim_communication_input(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
        lease_seconds: u32,
        master_key: &str,
    ) -> anyhow::Result<Option<ClaimedCommunicationInput>>;
    async fn renew_communication_claim(
        &self,
        claim: &CommunicationClaim,
        lease_seconds: u32,
    ) -> anyhow::Result<bool>;
    async fn record_communication_attachment(
        &self,
        claim: &CommunicationClaim,
        update: CommunicationAttachmentUpdate,
    ) -> anyhow::Result<bool>;
    async fn record_communication_progress(
        &self,
        claim: &CommunicationClaim,
        progress: CommunicationProgress,
    ) -> anyhow::Result<bool>;
    /// Explicit operator retry; unknown outbound delivery is deliberately not retryable.
    async fn retry_communication_message(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
        message_id: Uuid,
    ) -> anyhow::Result<bool>;
}
