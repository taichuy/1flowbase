use anyhow::{anyhow, Result};
use async_trait::async_trait;
use control_plane_contracts::ports::{
    WorkflowCredentialRun, WorkflowRunIdentityRepository, WorkflowRunIdentitySnapshot,
};
use sqlx::Row;
use uuid::Uuid;

use crate::PgControlPlaneStore;

#[async_trait]
impl WorkflowRunIdentityRepository for PgControlPlaneStore {
    async fn get_workflow_credential_run(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Option<WorkflowCredentialRun>> {
        let row = sqlx::query(
            "select r.id, r.application_id, r.created_by as executor_user_id, r.status, \
             a.workspace_id, a.created_by as application_created_by, w.tenant_id \
             from flow_runs r join applications a on a.id = r.application_id \
             join workspaces w on w.id = a.workspace_id where r.id = $1",
        )
        .bind(flow_run_id)
        .fetch_optional(self.pool())
        .await?;
        row.map(|row| -> Result<_> {
            let status: String = row.try_get("status")?;
            Ok(WorkflowCredentialRun {
                flow_run_id: row.try_get("id")?,
                application_id: row.try_get("application_id")?,
                workspace_id: row.try_get("workspace_id")?,
                tenant_id: row.try_get("tenant_id")?,
                application_created_by: row.try_get("application_created_by")?,
                executor_user_id: row.try_get("executor_user_id")?,
                status: serde_json::from_value(serde_json::Value::String(status))?,
            })
        })
        .transpose()
    }

    async fn get_workflow_run_identity(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Option<WorkflowRunIdentitySnapshot>> {
        let row = sqlx::query(
            "select creator_identity, executor_identity from workflow_run_identity_snapshots where flow_run_id = $1",
        ).bind(flow_run_id).fetch_optional(self.pool()).await?;
        row.map(|row| -> Result<_> {
            Ok(WorkflowRunIdentitySnapshot {
                flow_run_id,
                creator: serde_json::from_value(row.try_get("creator_identity")?)?,
                executor: serde_json::from_value(row.try_get("executor_identity")?)?,
            })
        })
        .transpose()
    }

    async fn freeze_workflow_run_identity(
        &self,
        snapshot: &WorkflowRunIdentitySnapshot,
    ) -> Result<WorkflowRunIdentitySnapshot> {
        sqlx::query(
            "insert into workflow_run_identity_snapshots (flow_run_id, creator_identity, executor_identity) \
             values ($1, $2, $3) on conflict (flow_run_id) do nothing",
        ).bind(snapshot.flow_run_id)
         .bind(serde_json::to_value(&snapshot.creator)?)
         .bind(serde_json::to_value(&snapshot.executor)?)
         .execute(self.pool()).await?;
        self.get_workflow_run_identity(snapshot.flow_run_id)
            .await?
            .ok_or_else(|| anyhow!("workflow_run_identity_missing"))
    }
}
