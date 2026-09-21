use super::*;
use control_plane::ports::{
    ProviderTrajectoryBody, ProviderTrajectoryPage, ProviderTrajectoryRepository,
};

impl ApplicationRuntimeReadsAdapter {
    pub(super) async fn run_trajectory_page(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_id: Uuid,
        query: provider_trajectory::ProviderTrajectoryQuery,
    ) -> Result<ProviderTrajectoryPage, ApiError> {
        self.visible_trajectory_run(actor, application_id, run_id)
            .await?;
        Ok(self
            .store
            .provider_run_trajectory_page(run_id, query.cursor, query.limit.unwrap_or(50))
            .await?)
    }
    pub(super) async fn run_payload(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_id: Uuid,
        section: control_plane::ports::ApplicationRunPayloadSection,
    ) -> Result<serde_json::Value, ApiError> {
        let application = self.visible_application(actor, application_id).await?;
        let payload = self
            .store
            .application_run_payload(application_id, run_id, section)
            .await?
            .ok_or(ControlPlaneError::NotFound("flow_run"))?;
        super::super::interface_trace_payloads::preview_run_payload(
            self.store.clone(),
            self.file_storage_registry.as_ref(),
            application.workspace_id,
            application_id,
            run_id,
            section,
            payload,
        )
        .await
    }

    async fn visible_trajectory_run(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_id: Uuid,
    ) -> Result<(), ApiError> {
        self.visible_application(actor, application_id).await?;
        <_ as OrchestrationRuntimeRepository>::get_flow_run_metadata(
            &self.store,
            application_id,
            run_id,
        )
        .await?
        .ok_or(ControlPlaneError::NotFound("flow_run"))?;
        Ok(())
    }

    pub(super) async fn trajectory_page(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_id: Uuid,
        node_run_id: Uuid,
        query: provider_trajectory::ProviderTrajectoryQuery,
    ) -> Result<ProviderTrajectoryPage, ApiError> {
        self.visible_trajectory_run(actor, application_id, run_id)
            .await?;
        Ok(self
            .store
            .provider_trajectory_page(run_id, node_run_id, query.cursor, query.limit.unwrap_or(50))
            .await?)
    }

    pub(super) async fn trajectory_body(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_id: Uuid,
        node_run_id: Uuid,
        event_id: Uuid,
        query: provider_trajectory::ProviderTrajectoryQuery,
    ) -> Result<ProviderTrajectoryBody, ApiError> {
        self.visible_trajectory_run(actor, application_id, run_id)
            .await?;
        self.store
            .provider_trajectory_body(
                run_id,
                node_run_id,
                event_id,
                query.cursor,
                query.limit.unwrap_or(8),
                query.view,
            )
            .await?
            .ok_or_else(|| ControlPlaneError::NotFound("provider_protocol_observation").into())
    }
}
