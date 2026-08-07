use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::ports::CreateFlowRunInput;

#[derive(Debug, Clone)]
pub struct CreatePublishedFlowRunResult {
    pub flow_run: domain::FlowRunRecord,
    pub created: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateAssistantConversationInput {
    pub conversation_id: Uuid,
    pub workspace_id: Uuid,
    pub application_id: Uuid,
    pub actor_user_id: Uuid,
    /// A read-only legacy run that seeds this new conversation without changing
    /// the legacy run or copying its messages into a second ledger.
    pub seed_legacy_flow_run_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssistantConversationRecord {
    pub conversation_id: Uuid,
    pub workspace_id: Uuid,
    pub application_id: Uuid,
    pub created_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListAssistantConversationsInput {
    pub workspace_id: Uuid,
    pub application_id: Uuid,
    pub actor_user_id: Uuid,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssistantConversationSummary {
    pub conversation_id: Option<Uuid>,
    pub legacy_flow_run_id: Option<Uuid>,
    pub latest_flow_run_id: Option<Uuid>,
    pub title: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssistantConversationPage {
    pub items: Vec<AssistantConversationSummary>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssistantConversationMessage {
    pub id: String,
    pub flow_run_id: Uuid,
    pub role: String,
    pub content: String,
    pub created_at: OffsetDateTime,
}

#[async_trait]
pub trait ApplicationPublishedFlowRunRepository: Send + Sync {
    async fn create_published_flow_run(
        &self,
        input: &CreateFlowRunInput,
    ) -> Result<CreatePublishedFlowRunResult>;

    async fn find_published_flow_run_by_idempotency_key(
        &self,
        application_id: Uuid,
        api_key_id: Option<Uuid>,
        idempotency_key: &str,
    ) -> Result<Option<domain::FlowRunRecord>>;

    async fn append_published_run_event(
        &self,
        input: &crate::ports::AppendRunEventInput,
    ) -> Result<domain::RunEventRecord>;

    async fn create_assistant_conversation(
        &self,
        input: &CreateAssistantConversationInput,
    ) -> Result<AssistantConversationRecord> {
        let _ = input;
        anyhow::bail!("create_assistant_conversation not implemented")
    }

    async fn get_assistant_conversation(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<Option<AssistantConversationRecord>> {
        let _ = (workspace_id, application_id, actor_user_id, conversation_id);
        anyhow::bail!("get_assistant_conversation not implemented")
    }

    async fn list_assistant_conversations(
        &self,
        input: &ListAssistantConversationsInput,
    ) -> Result<AssistantConversationPage> {
        let _ = input;
        anyhow::bail!("list_assistant_conversations not implemented")
    }

    async fn list_assistant_conversation_messages(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<Vec<AssistantConversationMessage>> {
        let _ = (workspace_id, application_id, actor_user_id, conversation_id);
        anyhow::bail!("list_assistant_conversation_messages not implemented")
    }

    async fn list_assistant_legacy_snapshot_messages(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Vec<AssistantConversationMessage>> {
        let _ = (workspace_id, application_id, actor_user_id, flow_run_id);
        anyhow::bail!("list_assistant_legacy_snapshot_messages not implemented")
    }
}

#[derive(Debug, Clone)]
pub struct CancelPublishedFlowRunInput {
    pub flow_run_id: Uuid,
    pub from_status: domain::FlowRunStatus,
    pub output_payload: Value,
    pub error_payload: Option<Value>,
    pub flow_run_event_payload: Value,
    pub terminal_event_payload: Value,
    pub finished_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListWaitingCallbackPublishedRunsInput {
    pub application_id: Uuid,
    pub api_key_id: Uuid,
    pub external_user: String,
    pub external_conversation_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PublishedRunNodeUsage {
    pub metrics_usage: Option<Value>,
    pub output_usage: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PublishedRunPendingCallback {
    pub id: Uuid,
    pub flow_run_id: Uuid,
    pub node_run_id: Uuid,
    pub callback_kind: String,
    pub request_payload: Option<Value>,
    pub tool_calls: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct PublishedRunStreamState {
    pub status: domain::FlowRunStatus,
    pub output_payload: Value,
    pub error_payload: Option<Value>,
    pub node_usages: Vec<PublishedRunNodeUsage>,
    pub latest_pending_callback: Option<PublishedRunPendingCallback>,
}

#[async_trait]
pub trait ApplicationPublishedRunControlRepository: Send + Sync {
    async fn get_published_flow_run(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Option<domain::FlowRunRecord>>;

    async fn find_published_flow_run_by_provider_response_id(
        &self,
        application_id: Uuid,
        api_key_id: Uuid,
        provider_response_id: &str,
    ) -> Result<Option<domain::FlowRunRecord>>;

    async fn cancel_published_flow_run(
        &self,
        input: &CancelPublishedFlowRunInput,
    ) -> Result<crate::ports::CommitFlowRunTerminalReceipt>;

    async fn cancel_published_pending_callback_tasks_for_run(
        &self,
        flow_run_id: Uuid,
        completed_at: OffsetDateTime,
    ) -> Result<Vec<domain::CallbackTaskRecord>>;

    async fn list_waiting_callback_published_flow_run_ids_for_conversation(
        &self,
        input: &ListWaitingCallbackPublishedRunsInput,
    ) -> Result<Vec<Uuid>>;

    async fn get_published_callback_task(
        &self,
        callback_task_id: Uuid,
    ) -> Result<Option<domain::CallbackTaskRecord>>;

    async fn get_published_run_stream_state(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<PublishedRunStreamState>>;
}
