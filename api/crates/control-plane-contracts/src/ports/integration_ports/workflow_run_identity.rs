use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Safe authorization identity only: no permission lists, cookies, tokens or passwords.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCredentialIdentity {
    pub user_id: Uuid,
    pub tenant_id: Uuid,
    pub workspace_id: Uuid,
    pub active_role_code: String,
    pub session_version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRunIdentitySnapshot {
    pub flow_run_id: Uuid,
    /// Unavailable creators stay unavailable for this run; their historical ownership is preserved.
    pub creator: Option<WorkflowCredentialIdentity>,
    /// Non-user execution principals do not acquire their owner's Console identity.
    pub executor: Option<WorkflowCredentialIdentity>,
}

/// Current lifecycle/ownership facts used by the trusted host before preparation and dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowCredentialRun {
    pub flow_run_id: Uuid,
    pub application_id: Uuid,
    pub workspace_id: Uuid,
    pub tenant_id: Uuid,
    pub application_created_by: Uuid,
    pub executor_user_id: Uuid,
    pub status: domain::FlowRunStatus,
}

#[async_trait]
pub trait WorkflowRunIdentityRepository: Send + Sync {
    async fn get_workflow_credential_run(
        &self,
        flow_run_id: Uuid,
    ) -> anyhow::Result<Option<WorkflowCredentialRun>>;

    async fn get_workflow_run_identity(
        &self,
        flow_run_id: Uuid,
    ) -> anyhow::Result<Option<WorkflowRunIdentitySnapshot>>;

    /// First writer wins atomically. A replay returns the immutable original snapshot.
    async fn freeze_workflow_run_identity(
        &self,
        snapshot: &WorkflowRunIdentitySnapshot,
    ) -> anyhow::Result<WorkflowRunIdentitySnapshot>;
}
