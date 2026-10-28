//! Host-facing durable communication ports. Credentials intentionally do not implement Debug.
use crate::ports::SaveNotificationAuthorizationInput;
use async_trait::async_trait;
use domain::*;
use serde_json::Value;
use uuid::Uuid;

#[path = "communication_sessions.rs"]
mod sessions;
pub use sessions::*;

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
    pub message: CommunicationInput,
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
    /// Ordered channel-wide classification scan, bounded to at most 100 input receipts.
    async fn scan_communication_admissions(
        &self,
        _workspace_id: Uuid,
        _instance_id: Uuid,
        _limit: u32,
    ) -> anyhow::Result<Vec<CommunicationPendingAdmission>> {
        anyhow::bail!("communication_sessions_unsupported")
    }
    /// Ordered received input IDs awaiting host classification; no state is changed.
    async fn pending_communication_admissions(
        &self,
        _scope: &CommunicationConversationScope,
    ) -> anyhow::Result<Vec<Uuid>> {
        anyhow::bail!("communication_sessions_unsupported")
    }
    /// Exactly-once admission; a replay returns the existing membership. First admission
    /// creates the active logical session and open turn under the external-chat lock.
    async fn admit_communication_input(
        &self,
        _scope: &CommunicationConversationScope,
        _input_id: Uuid,
    ) -> anyhow::Result<Option<CommunicationAdmission>> {
        anyhow::bail!("communication_sessions_unsupported")
    }
    /// Classifier-owned completion (for example a command). Keep commands pending until
    /// their effect and durable reply receipt commit, so later inputs cannot overtake them.
    /// Never deletes history.
    async fn exclude_communication_input(
        &self,
        _scope: &CommunicationConversationScope,
        _input_id: Uuid,
    ) -> anyhow::Result<bool> {
        anyhow::bail!("communication_sessions_unsupported")
    }
    async fn active_communication_session(
        &self,
        _scope: &CommunicationConversationScope,
    ) -> anyhow::Result<Option<CommunicationSession>> {
        anyhow::bail!("communication_sessions_unsupported")
    }
    /// Caller has already validated model availability/authorization. Only future turns change.
    async fn select_communication_model(
        &self,
        _scope: &CommunicationConversationScope,
        _model: Option<&str>,
    ) -> anyhow::Result<bool> {
        anyhow::bail!("communication_sessions_unsupported")
    }
    /// Atomic fence and rotation. Receipt key is stable across command redelivery. Host
    /// cancels returned runs before acknowledging the command or executing the new session.
    async fn rotate_communication_session(
        &self,
        _scope: &CommunicationConversationScope,
        _receipt_key: &str,
    ) -> anyhow::Result<Option<CommunicationSessionRotation>> {
        anyhow::bail!("communication_sessions_unsupported")
    }
    async fn claim_communication_turn(
        &self,
        _workspace_id: Uuid,
        _instance_id: Uuid,
        _lease_seconds: u32,
        _master_key: &str,
    ) -> anyhow::Result<Option<ClaimedCommunicationTurn>> {
        anyhow::bail!("communication_sessions_unsupported")
    }
    async fn renew_communication_turn_claim(
        &self,
        _claim: &CommunicationTurnClaim,
        _lease_seconds: u32,
    ) -> anyhow::Result<bool> {
        anyhow::bail!("communication_sessions_unsupported")
    }
    async fn communication_turn_inputs(
        &self,
        _claim: &CommunicationTurnClaim,
        _master_key: &str,
    ) -> anyhow::Result<Option<CommunicationTurnInputs>> {
        anyhow::bail!("communication_sessions_unsupported")
    }
    async fn record_communication_turn_attachment(
        &self,
        _claim: &CommunicationTurnClaim,
        _update: CommunicationAttachmentUpdate,
    ) -> anyhow::Result<bool> {
        anyhow::bail!("communication_sessions_unsupported")
    }
    /// Reserve once before dispatch. Reclaiming a turn retains the original run ID.
    async fn link_communication_turn_run(
        &self,
        _claim: &CommunicationTurnClaim,
        _run_id: Uuid,
    ) -> anyhow::Result<bool> {
        anyhow::bail!("communication_sessions_unsupported")
    }
    /// The checkpoint and consumed watermark are committed by one transaction.
    async fn checkpoint_communication_turn(
        &self,
        _claim: &CommunicationTurnClaim,
        _checkpoint: CommunicationTurnCheckpoint,
    ) -> anyhow::Result<bool> {
        anyhow::bail!("communication_sessions_unsupported")
    }
    /// Final watermark CAS and SendStarted are one atomic operation. The winner sends
    /// once; Pending resumes the same run from its durable continuation, never redispatches.
    async fn seal_communication_turn(
        &self,
        _claim: &CommunicationTurnClaim,
        _candidate: CommunicationTurnFinalCandidate,
        _master_key: &str,
    ) -> anyhow::Result<CommunicationTurnSeal> {
        anyhow::bail!("communication_sessions_unsupported")
    }
    /// Only for an explicit operator retry of a known failed send, using persisted content.
    async fn reserve_communication_turn_send(
        &self,
        _claim: &CommunicationTurnClaim,
    ) -> anyhow::Result<Option<String>> {
        anyhow::bail!("communication_sessions_unsupported")
    }
    async fn record_communication_turn_progress(
        &self,
        _claim: &CommunicationTurnClaim,
        _progress: CommunicationTurnProgress,
    ) -> anyhow::Result<bool> {
        anyhow::bail!("communication_sessions_unsupported")
    }
    async fn communication_management_history(
        &self,
        workspace_id: Uuid,
        conversation_id: Uuid,
        cursor: Option<Uuid>,
        limit: u32,
    ) -> anyhow::Result<CommunicationPage<CommunicationMessage>>;
    async fn communication_management_conversations(
        &self,
        workspace_id: Uuid,
        query: &CommunicationManagementQuery,
    ) -> anyhow::Result<CommunicationManagementPage<CommunicationConversationListItem>>;
    async fn communication_management_messages(
        &self,
        workspace_id: Uuid,
        query: &CommunicationManagementQuery,
    ) -> anyhow::Result<CommunicationManagementPage<CommunicationMessageListItem>>;
    async fn communication_management_message(
        &self,
        workspace_id: Uuid,
        message_id: Uuid,
    ) -> anyhow::Result<Option<CommunicationMessageDetail>>;

    /// Original application is read from the linked durable run, independent of later channel edits.
    async fn communication_run_application_id(
        &self,
        workspace_id: Uuid,
        run_id: Uuid,
    ) -> anyhow::Result<Option<Uuid>>;

    /// Restore trusted communication authority from a committed run association, not request JSON.
    async fn communication_context_for_run(
        &self,
        workspace_id: Uuid,
        run_id: Uuid,
    ) -> anyhow::Result<Option<(CommunicationHistoryScope, CommunicationBindingRecord)>>;

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
    ) -> anyhow::Result<Option<CommunicationMessage>>;
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
    ) -> anyhow::Result<CommunicationPage<CommunicationMessage>>;
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
