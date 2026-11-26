use anyhow::Result;
use control_plane_contracts::ports::{
    ApplicationRepository, FlowRepository, OrchestrationRuntimeRepository,
    RestoreRunArchiveBatchInput, RestoreRunArchiveEntryInput, RunArchiveRepository,
    UpdateFlowRunInput, UpdateFlowRunPayloadsInput,
};
use uuid::Uuid;

use crate::{errors::ControlPlaneError, flow::FlowService};

const RUN_ARCHIVE_VERSION: i32 = 1;

pub struct RunArchiveService<R> {
    repository: R,
}

impl<R> RunArchiveService<R>
where
    R: RunArchiveRepository + OrchestrationRuntimeRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn restore_run_archive_v1<S>(
        &self,
        scoped_repository: S,
        application: &domain::ApplicationRecord,
        actor: &domain::ActorContext,
        job_id: Uuid,
        archive_version: i32,
        entries: Vec<RestoreRunArchiveEntryInput>,
    ) -> Result<Vec<(String, Uuid)>>
    where
        S: ApplicationRepository + FlowRepository + Clone,
    {
        if archive_version != RUN_ARCHIVE_VERSION {
            return Err(ControlPlaneError::InvalidInput("archive_version").into());
        }

        let editor_state = FlowService::new(scoped_repository)
            .get_or_create_editor_state(actor.user_id, application.id)
            .await?;
        let batch = self
            .repository
            .restore_run_archive_entries(&RestoreRunArchiveBatchInput {
                job_id,
                workspace_id: application.workspace_id,
                application_id: application.id,
                actor_user_id: actor.user_id,
                flow_id: editor_state.flow.id,
                flow_draft_id: editor_state.draft.id,
                flow_draft_updated_at: editor_state.draft.updated_at,
                entries,
            })
            .await?;

        for update in batch.final_run_updates {
            OrchestrationRuntimeRepository::update_flow_run_payloads(
                &self.repository,
                &UpdateFlowRunPayloadsInput {
                    flow_run_id: update.target_run_id,
                    input_payload: update.input_payload,
                    output_payload: update.output_payload.clone(),
                    error_payload: update.error_payload.clone(),
                },
            )
            .await?;
            OrchestrationRuntimeRepository::update_flow_run(
                &self.repository,
                &UpdateFlowRunInput {
                    flow_run_id: update.target_run_id,
                    status: update.status,
                    output_payload: update.output_payload,
                    error_payload: update.error_payload,
                    finished_at: update.finished_at,
                },
            )
            .await?;
        }

        Ok(batch.run_mappings)
    }
}
