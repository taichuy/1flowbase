use super::*;
use control_plane::ports::{
    ProviderTrajectoryBody, ProviderTrajectoryPage, ProviderTrajectoryRepository,
};

impl ApplicationRuntimeReadsAdapter {
    async fn visible_trajectory_run(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_id: Uuid,
    ) -> Result<(), ApiError> {
        self.visible_application(actor, application_id).await?;
        <_ as OrchestrationRuntimeRepository>::get_flow_run(&self.store, application_id, run_id)
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
    ) -> Result<ProviderTrajectoryBody, ApiError> {
        self.visible_trajectory_run(actor, application_id, run_id)
            .await?;
        self.store
            .provider_trajectory_body(run_id, node_run_id, event_id)
            .await?
            .ok_or_else(|| ControlPlaneError::NotFound("provider_protocol_observation").into())
    }
}
