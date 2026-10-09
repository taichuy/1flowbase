use super::*;
use crate::{
    application::ApplicationNonCrudConsoleOperation as Operation, errors::ControlPlaneError,
    ports::*,
};

fn scope_valid(scope: &AgentLogsDeleteScope) -> Result<()> {
    scope
        .bounds()
        .map_err(|_| ControlPlaneError::InvalidInput("agent_logs_delete_time_range"))?;
    // Jobs establish their own boundary; preview cannot masquerade as a historical receipt.
    if scope
        .ingested_at_before()
        .map_err(|_| ControlPlaneError::InvalidInput("agent_logs_delete_job_boundary"))?
        .is_some()
    {
        return Err(ControlPlaneError::InvalidInput("agent_logs_delete_job_boundary").into());
    }
    Ok(())
}
fn map_error(error: anyhow::Error) -> anyhow::Error {
    match error.to_string().as_str() {
        "agent_logs.delete_job_active" => {
            ControlPlaneError::Conflict("agent_logs_delete_job_active").into()
        }
        "agent_logs.delete_job_conflict" => {
            ControlPlaneError::Conflict("agent_logs_delete_job_conflict").into()
        }
        "agent_logs.application_type" => {
            ControlPlaneError::InvalidInput("agent_logs_application_type").into()
        }
        _ => error,
    }
}
impl<R> AgentLogsService<R>
where
    R: ApplicationRepository + OrchestrationRuntimeRepository + Clone,
{
    async fn deletion_application(
        &self,
        actor: &domain::ActorContext,
        app: Uuid,
        operation: Operation,
    ) -> Result<domain::ApplicationRecord> {
        let application = crate::application::ApplicationService::new(self.repository.clone())
            .load_application_for_non_crud_console_operation_for_actor(actor, app, operation)
            .await?;
        if application.application_type != domain::ApplicationType::AgentLogs {
            return Err(ControlPlaneError::InvalidInput("agent_logs_application_type").into());
        }
        Ok(application)
    }
    pub async fn preview_deletion(
        &self,
        actor: &domain::ActorContext,
        app: Uuid,
        scope: AgentLogsDeleteScope,
    ) -> Result<AgentLogsDeletePreview> {
        scope_valid(&scope)?;
        let application = self
            .deletion_application(actor, app, Operation::LogsDeletePreview)
            .await?;
        self.repository
            .preview_agent_logs_delete(app, application.workspace_id, &scope)
            .await
    }
    pub async fn start_deletion(
        &self,
        actor: &domain::ActorContext,
        app: Uuid,
        input: AgentLogsDeleteJobCreate,
    ) -> Result<AgentLogsDeleteJob> {
        scope_valid(&input.scope)?;
        if input.scope.batch_size().is_none() || input.job_id.is_nil() {
            return Err(ControlPlaneError::InvalidInput("agent_logs_delete_job_input").into());
        }
        let application = self
            .deletion_application(actor, app, Operation::LogsDeleteJobCreate)
            .await?;
        self.repository
            .create_agent_logs_delete_job(app, application.workspace_id, &input)
            .await
            .map_err(map_error)
    }
    pub async fn deletion_job(
        &self,
        actor: &domain::ActorContext,
        app: Uuid,
        id: Option<Uuid>,
    ) -> Result<Option<AgentLogsDeleteJob>> {
        let operation = if id.is_some() {
            Operation::LogsDeleteJobGet
        } else {
            Operation::LogsDeleteJobLatest
        };
        let application = self.deletion_application(actor, app, operation).await?;
        let job = self
            .repository
            .get_agent_logs_delete_job(app, application.workspace_id, id)
            .await?;
        if id.is_some() && job.is_none() {
            return Err(ControlPlaneError::NotFound("agent_logs_delete_job").into());
        }
        Ok(job)
    }
    pub async fn stop_deletion(
        &self,
        actor: &domain::ActorContext,
        app: Uuid,
        id: Uuid,
    ) -> Result<AgentLogsDeleteJob> {
        let application = self
            .deletion_application(actor, app, Operation::LogsDeleteJobStop)
            .await?;
        self.repository
            .stop_agent_logs_delete_job(app, application.workspace_id, id)
            .await?
            .ok_or_else(|| ControlPlaneError::NotFound("agent_logs_delete_job").into())
    }
}
impl<R: OrchestrationRuntimeRepository> AgentLogsService<R> {
    /// Trusted background command. Database locks release on process death; pending work persists.
    pub async fn advance_next_deletion(&self) -> Result<bool> {
        let Some(job) = self.repository.next_agent_logs_delete_job().await? else {
            return Ok(false);
        };
        if let Err(error) = self
            .repository
            .advance_agent_logs_delete_job(job.job_id)
            .await
        {
            // An uncertain COMMIT is reconciled by the repository before marking failure.
            self.repository
                .fail_agent_logs_delete_job(job.job_id, job.deleted_records)
                .await?;
            return Err(error);
        }
        Ok(true)
    }
}
