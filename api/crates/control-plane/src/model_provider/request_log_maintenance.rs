use anyhow::Result;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    ports::{
        ClearModelProviderRequestLogsBatchInput, DeleteModelProviderRequestLogsInput,
        OrchestrationRuntimeRepository, MODEL_PROVIDER_REQUEST_LOG_DELETE_BATCH_LIMIT,
    },
};

use super::{shared::ensure_state_model_permission, ModelProviderService};

pub struct DeleteSelectedModelProviderRequestLogsCommand {
    pub actor: domain::ActorContext,
    pub attempt_ids: Vec<Uuid>,
}

pub struct ClearModelProviderRequestLogsBatchCommand {
    pub actor: domain::ActorContext,
    pub continuation: ClearModelProviderRequestLogsContinuation,
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
    pub async fn delete_selected_request_logs(
        &self,
        command: DeleteSelectedModelProviderRequestLogsCommand,
    ) -> Result<u64> {
        ensure_state_model_permission(&command.actor, "manage")?;
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
        ensure_state_model_permission(&command.actor, "manage")?;
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
