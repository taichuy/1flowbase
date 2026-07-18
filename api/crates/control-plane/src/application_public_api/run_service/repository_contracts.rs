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
