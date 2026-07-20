use serde_json::{json, Value};
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

use super::{
    publications::ApplicationPublicationVersionRecord,
    run_service::{
        public_freeze_workflow_run_input_environment, ApplicationPublishedFlowRunRepository,
        WorkflowRunTriggerContext,
    },
};
use crate::{
    flow_run_title::build_flow_run_title,
    ports::{ApplicationCompiledPlanRepository, ApplicationRepository, CreateFlowRunInput},
};

#[derive(Debug, Clone)]
pub enum WorkflowInvocationTrigger {
    Http {
        api_key_id: Option<Uuid>,
        interface_id: String,
        route_template: String,
        method: String,
        principal: String,
        response_mode: String,
    },
    Schedule {
        trigger_id: Uuid,
        cron: String,
        timezone: String,
        scheduled_at: OffsetDateTime,
        idempotency_key: String,
    },
}

#[derive(Debug, Clone)]
pub struct InvokeWorkflowCommand {
    pub actor_user_id: Uuid,
    pub publication: ApplicationPublicationVersionRecord,
    pub node_input_payload: Value,
    pub trigger: WorkflowInvocationTrigger,
}

#[derive(Debug, Clone)]
pub struct WorkflowInvocationResult {
    pub flow_run: domain::FlowRunRecord,
    pub compiled_plan: domain::CompiledPlanRecord,
    pub created: bool,
}

pub struct WorkflowInvocationService<R> {
    repository: R,
}

#[derive(Debug, Error)]
pub enum WorkflowInvocationError {
    #[error("published workflow compiled plan is unavailable")]
    CompiledPlanUnavailable,
    #[error("workflow invocation input is invalid")]
    InvalidInput(#[source] anyhow::Error),
    #[error("workflow invocation repository operation failed")]
    Repository(#[source] anyhow::Error),
}

impl<R> WorkflowInvocationService<R>
where
    R: ApplicationRepository
        + ApplicationCompiledPlanRepository
        + ApplicationPublishedFlowRunRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn invoke(
        &self,
        command: InvokeWorkflowCommand,
    ) -> std::result::Result<WorkflowInvocationResult, WorkflowInvocationError> {
        let publication = command.publication;
        let compiled_plan = self
            .repository
            .get_application_compiled_plan(publication.compiled_plan_id)
            .await
            .map_err(WorkflowInvocationError::Repository)?
            .ok_or(WorkflowInvocationError::CompiledPlanUnavailable)?;
        let environment_variables = self
            .repository
            .list_application_environment_variables(
                publication.workspace_id,
                publication.application_id,
            )
            .await
            .map_err(WorkflowInvocationError::Repository)?;
        let input_payload = public_freeze_workflow_run_input_environment(
            command.node_input_payload,
            &environment_variables,
            command.trigger.run_context(),
        )
        .map_err(|error| WorkflowInvocationError::InvalidInput(error.into()))?;

        if let Some(idempotency_key) = command.trigger.idempotency_key() {
            if let Some(flow_run) = self
                .repository
                .find_published_flow_run_by_idempotency_key(
                    publication.application_id,
                    None,
                    idempotency_key,
                )
                .await
                .map_err(WorkflowInvocationError::Repository)?
            {
                return Ok(WorkflowInvocationResult {
                    flow_run,
                    compiled_plan,
                    created: false,
                });
            }
        }

        let created = self
            .repository
            .create_published_flow_run(&CreateFlowRunInput {
                actor_user_id: command.actor_user_id,
                application_id: publication.application_id,
                flow_id: publication.flow_id,
                flow_draft_id: compiled_plan.draft_id,
                compiled_plan_id: publication.compiled_plan_id,
                debug_session_id: String::new(),
                flow_schema_version: publication.flow_schema_version.clone(),
                document_hash: publication.document_hash.clone(),
                run_mode: command.trigger.run_mode(),
                target_node_id: None,
                title: build_flow_run_title(None, &command.trigger.title()),
                status: domain::FlowRunStatus::Queued,
                input_payload,
                started_at: command.trigger.started_at(),
                api_key_id: command.trigger.api_key_id(),
                publication_version_id: Some(publication.id),
                external_user: None,
                external_conversation_id: None,
                external_trace_id: Some(command.trigger.external_trace_id()),
                compatibility_mode: None,
                idempotency_key: command.trigger.idempotency_key().map(str::to_string),
            })
            .await
            .map_err(WorkflowInvocationError::Repository)?;
        if created.created {
            self.repository
                .append_published_run_event(&crate::ports::AppendRunEventInput {
                    flow_run_id: created.flow_run.id,
                    node_run_id: None,
                    event_type: command.trigger.event_type().to_string(),
                    payload: command
                        .trigger
                        .event_payload(publication.id, publication.application_id),
                })
                .await
                .map_err(WorkflowInvocationError::Repository)?;
        }

        Ok(WorkflowInvocationResult {
            flow_run: created.flow_run,
            compiled_plan,
            created: created.created,
        })
    }
}

impl WorkflowInvocationTrigger {
    fn run_context(&self) -> WorkflowRunTriggerContext<'_> {
        match self {
            Self::Http { .. } => WorkflowRunTriggerContext::Extension,
            Self::Schedule {
                scheduled_at,
                timezone,
                ..
            } => WorkflowRunTriggerContext::Schedule {
                scheduled_at: *scheduled_at,
                timezone,
            },
        }
    }

    fn run_mode(&self) -> domain::FlowRunMode {
        match self {
            Self::Http { .. } => domain::FlowRunMode::WorkflowHttpRun,
            Self::Schedule { .. } => domain::FlowRunMode::WorkflowScheduleRun,
        }
    }

    fn api_key_id(&self) -> Option<Uuid> {
        match self {
            Self::Http { api_key_id, .. } => *api_key_id,
            Self::Schedule { .. } => None,
        }
    }

    fn started_at(&self) -> OffsetDateTime {
        match self {
            Self::Http { .. } => OffsetDateTime::now_utc(),
            Self::Schedule { scheduled_at, .. } => *scheduled_at,
        }
    }

    fn title(&self) -> String {
        match self {
            Self::Http { route_template, .. } => format!("Workflow HTTP {route_template}"),
            Self::Schedule { .. } => "Scheduled workflow".to_string(),
        }
    }

    fn external_trace_id(&self) -> String {
        match self {
            Self::Http { interface_id, .. } => format!("workflow-http:{interface_id}"),
            Self::Schedule { trigger_id, .. } => format!("workflow-schedule:{trigger_id}"),
        }
    }

    fn idempotency_key(&self) -> Option<&str> {
        match self {
            Self::Http { .. } => None,
            Self::Schedule {
                idempotency_key, ..
            } => Some(idempotency_key),
        }
    }

    fn event_type(&self) -> &'static str {
        match self {
            Self::Http { .. } => "workflow_extension_run_started",
            Self::Schedule { .. } => "workflow_schedule_run_enqueued",
        }
    }

    fn event_payload(&self, publication_version_id: Uuid, application_id: Uuid) -> Value {
        match self {
            Self::Http {
                api_key_id,
                interface_id,
                route_template,
                method,
                principal,
                response_mode,
            } => json!({
                "api_key_id": api_key_id,
                "principal": principal,
                "application_id": application_id,
                "publication_version_id": publication_version_id,
                "trigger_source": "workflow_extension",
                "operation_id": interface_id,
                "route_template": route_template,
                "method": method,
                "response_mode": response_mode,
            }),
            Self::Schedule {
                trigger_id,
                cron,
                timezone,
                scheduled_at,
                ..
            } => json!({
                "trigger_source": "workflow_schedule",
                "trigger_id": trigger_id,
                "application_id": application_id,
                "operation_id": format!("workflow_schedule:{trigger_id}"),
                "publication_version_id": publication_version_id,
                "cron": cron,
                "timezone": timezone,
                "scheduled_at": scheduled_at,
            }),
        }
    }
}
