//! Ingest observations without executing a model or changing credit balances.
use crate::ports::{
    AgentLogsBatch, AgentLogsReceipt, BillingRepository, OrchestrationRuntimeRepository,
};
use anyhow::Result;
use uuid::Uuid;

mod pricing;
mod deletion;

pub struct AgentLogsService<R> {
    repository: R,
}
impl<R: BillingRepository + OrchestrationRuntimeRepository> AgentLogsService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
    pub async fn ingest(
        &self,
        application_id: Uuid,
        scope_id: Uuid,
        api_key_id: Uuid,
        batch: AgentLogsBatch,
    ) -> Result<AgentLogsReceipt> {
        batch
            .validate()
            .map_err(|_| crate::errors::ControlPlaneError::InvalidInput("agent_logs_batch"))?;
        let mut costs = Vec::with_capacity(batch.events.len());
        for event in &batch.events {
            costs.push(
                pricing::rate_usage(
                    &self.repository,
                    event.model_id.as_deref(),
                    &event.occurred_at,
                    event.usage.as_ref(),
                    event.inherited,
                )
                .await?,
            );
        }
        self.repository
            .ingest_agent_logs(application_id, scope_id, api_key_id, &batch, &costs)
            .await
            .map_err(|error| match error.to_string().as_str() {
                "agent_logs.event_conflict" => {
                    crate::errors::ControlPlaneError::Conflict("agent_logs_event_conflict").into()
                }
                "agent_logs.application_type" => {
                    crate::errors::ControlPlaneError::PermissionDenied(
                        "agent_logs_application_type",
                    )
                    .into()
                }
                _ => error,
            })
    }

    /// Trusted maintenance command. Source facts and all non-cost projections remain intact.
    pub async fn reprice_next_record(
        &self,
        application_id: Uuid,
        scope_id: Uuid,
        after: Option<Uuid>,
    ) -> Result<Option<crate::ports::AgentLogsRepriceRecordReceipt>> {
        let Some(record) = self
            .repository
            .next_agent_log_pricing_record(application_id, scope_id, after)
            .await?
        else {
            return Ok(None);
        };
        let mut costs = Vec::with_capacity(record.events.len());
        for event in &record.events {
            let cost = pricing::rate_usage(
                &self.repository,
                event.model_id.as_deref(),
                &event.occurred_at,
                Some(&event.usage),
                event.inherited,
            )
            .await?;
            costs.push((event.event_id.clone(), cost));
        }
        let changed_events = self
            .repository
            .reprice_agent_log_record(application_id, scope_id, record.record_id, &costs)
            .await?;
        Ok(Some(crate::ports::AgentLogsRepriceRecordReceipt {
            record_id: record.record_id,
            usage_events: record.events.len(),
            changed_events,
        }))
    }
}

impl<R> AgentLogsService<R>
where
    R: crate::ports::ApplicationRepository + OrchestrationRuntimeRepository + Clone,
{
    /// Authorized Console command: remove imported records, preserving the application.
    pub async fn delete(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        scope: crate::ports::AgentLogsDeleteScope,
    ) -> Result<crate::ports::AgentLogsDeleteReceipt> {
        scope.bounds().map_err(|_| {
            crate::errors::ControlPlaneError::InvalidInput("agent_logs_delete_time_range")
        })?;
        scope.ingested_at_before().map_err(|_| {
            crate::errors::ControlPlaneError::InvalidInput("agent_logs_delete_ingested_at_before")
        })?;
        let application = crate::application::ApplicationService::new(self.repository.clone())
            .load_application_for_non_crud_console_operation_for_actor(
                actor,
                application_id,
                crate::application::ApplicationNonCrudConsoleOperation::LogsDelete,
            )
            .await?;
        if application.application_type != domain::ApplicationType::AgentLogs {
            return Err(crate::errors::ControlPlaneError::InvalidInput(
                "agent_logs_application_type",
            )
            .into());
        }
        self.repository
            .delete_agent_logs(application_id, application.workspace_id, &scope)
            .await
            .map_err(|error| match error.to_string().as_str() {
                "agent_logs.delete_job_active" => crate::errors::ControlPlaneError::Conflict("agent_logs_delete_job_active").into(),
                "agent_logs.application_type" => {
                    crate::errors::ControlPlaneError::InvalidInput("agent_logs_application_type")
                        .into()
                }
                _ => error,
            })
    }
}
