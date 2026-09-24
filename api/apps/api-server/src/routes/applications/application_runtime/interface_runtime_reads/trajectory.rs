use super::*;
use control_plane::ports::{
    ProviderTrajectoryBody, ProviderTrajectoryPage, ProviderTrajectoryRepository,
    TrajectorySelection,
};

impl ApplicationRuntimeReadsAdapter {
    pub(super) async fn client_trajectory_page(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_id: Uuid,
        query: provider_trajectory::ClientTrajectoryQuery,
    ) -> Result<control_plane::ports::ClientTrajectoryPage, ApiError> {
        self.visible_trajectory_run(actor, application_id, run_id)
            .await?;
        self.store
            .client_trajectory_filtered_page(
                run_id,
                query.node_run_id,
                query.cursor,
                query.limit.unwrap_or(50),
                TrajectorySelection {
                    request_id: query.request_id,
                    target_id: query.focus_step_id,
                },
            )
            .await
            .map_err(|error| {
                if error.is::<control_plane::ports::TrajectoryTargetNotFound>() {
                    ApiError::from(ControlPlaneError::NotFound("trajectory_target"))
                } else {
                    ApiError::from(error)
                }
            })
    }
    pub(super) async fn client_trajectory_section(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_id: Uuid,
        step_id: Uuid,
        query: provider_trajectory::ClientTrajectoryQuery,
    ) -> Result<control_plane::ports::ClientTrajectorySection, ApiError> {
        self.visible_trajectory_run(actor, application_id, run_id)
            .await?;
        self.store
            .client_trajectory_section(
                run_id,
                query.node_run_id,
                step_id,
                query.section.as_deref().unwrap_or("overview"),
                query.cursor,
                query.limit.unwrap_or(8),
            )
            .await?
            .ok_or_else(|| ControlPlaneError::NotFound("client_trajectory_section").into())
    }
    pub(super) async fn run_trajectory_page(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_id: Uuid,
        query: provider_trajectory::ProviderTrajectoryQuery,
    ) -> Result<ProviderTrajectoryPage, ApiError> {
        self.visible_trajectory_run(actor, application_id, run_id)
            .await?;
        self.store
            .provider_trajectory_filtered_page(
                run_id,
                None,
                query.cursor,
                query.limit.unwrap_or(50),
                TrajectorySelection {
                    request_id: query.request_id,
                    target_id: query.focus_event_id,
                },
            )
            .await
            .map_err(|error| {
                if error.is::<control_plane::ports::TrajectoryTargetNotFound>() {
                    ApiError::from(ControlPlaneError::NotFound("trajectory_target"))
                } else {
                    ApiError::from(error)
                }
            })
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
        self.store
            .provider_trajectory_filtered_page(
                run_id,
                Some(node_run_id),
                query.cursor,
                query.limit.unwrap_or(50),
                TrajectorySelection {
                    request_id: query.request_id,
                    target_id: query.focus_event_id,
                },
            )
            .await
            .map_err(|error| {
                if error.is::<control_plane::ports::TrajectoryTargetNotFound>() {
                    ApiError::from(ControlPlaneError::NotFound("trajectory_target"))
                } else {
                    ApiError::from(error)
                }
            })
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

impl ApplicationRuntimeReadsAdapter {
    pub(super) async fn workflow_trajectory_page(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_id: Uuid,
        query: control_plane::ports::WorkflowTrajectoryQuery,
    ) -> Result<control_plane::ports::WorkflowTrajectoryPage, ApiError> {
        self.visible_trajectory_run(actor, application_id, run_id)
            .await?;
        self.store
            .workflow_trajectory_page(application_id, run_id, query)
            .await
            .map_err(|error| {
                if error.is::<control_plane::ports::InvalidWorkflowTrajectoryQuery>() {
                    ControlPlaneError::InvalidInput("workflow_trajectory_query").into()
                } else {
                    ApiError::from(error)
                }
            })
    }

    pub(super) async fn workflow_trajectory_body(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_id: Uuid,
        event_id: &str,
    ) -> Result<control_plane::ports::WorkflowTrajectoryBody, ApiError> {
        self.visible_trajectory_run(actor, application_id, run_id)
            .await?;
        self.store
            .workflow_trajectory_body(application_id, run_id, event_id)
            .await?
            .ok_or_else(|| ControlPlaneError::NotFound("workflow_event").into())
    }
}
