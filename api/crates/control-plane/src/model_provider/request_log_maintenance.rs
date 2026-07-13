use anyhow::Result;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    ports::{
        ClearModelProviderRequestLogsBatchInput, DeleteModelProviderRequestLogsInput,
        ListModelProviderRequestLogsPageInput, ModelProviderRequestLogsPage,
        OrchestrationRuntimeRepository, MODEL_PROVIDER_REQUEST_LOG_DELETE_BATCH_LIMIT,
    },
};

use super::{shared::ensure_model_provider_permission, ModelProviderService};

pub struct DeleteSelectedModelProviderRequestLogsCommand {
    pub actor: domain::ActorContext,
    pub attempt_ids: Vec<Uuid>,
}

pub struct ClearModelProviderRequestLogsBatchCommand {
    pub actor: domain::ActorContext,
    pub continuation: ClearModelProviderRequestLogsContinuation,
}

pub struct ListModelProviderRequestLogsCommand {
    pub actor: domain::ActorContext,
    pub application_name: Option<String>,
    pub provider_instance_id: Option<Uuid>,
    pub model_id: Option<String>,
    pub status: Option<String>,
    pub zero_output_only: bool,
    pub started_after: Option<OffsetDateTime>,
    pub started_before: Option<OffsetDateTime>,
    pub page: i64,
    pub page_size: i64,
}

pub enum ClearModelProviderRequestLogsContinuation {
    Start,
    Continue {
        snapshot_created_before: OffsetDateTime,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClearModelProviderRequestLogsBatchView {
    pub deleted_count: u64,
    pub has_more: bool,
    pub snapshot_created_before: OffsetDateTime,
}

impl<R, H> ModelProviderService<R, H>
where
    R: OrchestrationRuntimeRepository,
{
    pub async fn list_request_logs(
        &self,
        command: ListModelProviderRequestLogsCommand,
    ) -> Result<ModelProviderRequestLogsPage> {
        ensure_model_provider_permission(&command.actor, "view", self.use_case)?;
        self.repository
            .list_model_provider_request_logs_page(ListModelProviderRequestLogsPageInput {
                scope_id: command.actor.current_workspace_id,
                application_name: command.application_name,
                provider_instance_id: command.provider_instance_id,
                model_id: command.model_id,
                status: command.status,
                zero_output_only: command.zero_output_only,
                started_after: command.started_after,
                started_before: command.started_before,
                page: command.page,
                page_size: command.page_size,
            })
            .await
    }

    pub async fn delete_selected_request_logs(
        &self,
        command: DeleteSelectedModelProviderRequestLogsCommand,
    ) -> Result<u64> {
        ensure_model_provider_permission(&command.actor, "manage", self.use_case)?;
        if command.attempt_ids.is_empty()
            || command.attempt_ids.len() > MODEL_PROVIDER_REQUEST_LOG_DELETE_BATCH_LIMIT
        {
            return Err(ControlPlaneError::InvalidInput("attempt_ids").into());
        }

        self.repository
            .delete_model_provider_request_logs(DeleteModelProviderRequestLogsInput {
                scope_id: command.actor.current_workspace_id,
                attempt_ids: command.attempt_ids,
            })
            .await
    }

    pub async fn clear_request_logs_batch(
        &self,
        command: ClearModelProviderRequestLogsBatchCommand,
    ) -> Result<ClearModelProviderRequestLogsBatchView> {
        ensure_model_provider_permission(&command.actor, "manage", self.use_case)?;
        let result = self
            .repository
            .clear_model_provider_request_logs_batch(ClearModelProviderRequestLogsBatchInput {
                scope_id: command.actor.current_workspace_id,
                snapshot_created_before: match command.continuation {
                    ClearModelProviderRequestLogsContinuation::Start => None,
                    ClearModelProviderRequestLogsContinuation::Continue {
                        snapshot_created_before,
                    } => Some(snapshot_created_before),
                },
            })
            .await?;
        Ok(ClearModelProviderRequestLogsBatchView {
            deleted_count: result.deleted_count,
            has_more: result.has_more,
            snapshot_created_before: result.snapshot_created_before,
        })
    }
}
