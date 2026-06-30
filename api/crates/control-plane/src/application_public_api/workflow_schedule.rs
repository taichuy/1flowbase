use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use super::run_service::{
    public_compiled_plan_start_node_id, public_freeze_run_input_environment,
    ApplicationPublishedFlowRunRepository,
};
use crate::{
    application_public_api::{
        ensure_application_edit_permission, ensure_application_view_permission,
    },
    errors::ControlPlaneError,
    flow_run_title::build_flow_run_title,
    ports::{
        ApplicationCompiledPlanRepository, ApplicationPublicationRepository, ApplicationRepository,
        CreateFlowRunInput, ReplaceWorkflowScheduleTriggerInput, TaskQueue,
        WorkflowScheduleTriggerRepository,
    },
};

pub const WORKFLOW_SCHEDULE_RUN_QUEUE: &str = "workflow-schedule-runs";
const WORKFLOW_SCHEDULE_COMPATIBILITY_MODE: &str = "workflow_schedule_v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowScheduleTriggerRecord {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub application_id: Uuid,
    pub enabled: bool,
    pub cron: String,
    pub timezone: String,
    pub input_payload: Value,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct ReplaceWorkflowScheduleTriggerCommand {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
    pub enabled: bool,
    pub cron: String,
    pub timezone: String,
    pub input_payload: Value,
}

#[derive(Debug, Clone)]
pub struct GetWorkflowScheduleTriggerCommand {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
}

#[derive(Debug, Clone, Copy)]
pub struct DispatchWorkflowScheduleCommand {
    pub application_id: Uuid,
    pub scheduled_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowScheduleDispatchOutcome {
    Dispatched(WorkflowScheduleDispatchResult),
    Skipped { reason: &'static str },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowScheduleDispatchResult {
    pub run_id: Uuid,
    pub status: domain::FlowRunStatus,
    pub task_id: Option<String>,
}

pub struct WorkflowScheduleTriggerService<R> {
    repository: R,
}

impl<R> WorkflowScheduleTriggerService<R>
where
    R: ApplicationRepository + WorkflowScheduleTriggerRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn get_trigger(
        &self,
        command: GetWorkflowScheduleTriggerCommand,
    ) -> Result<Option<WorkflowScheduleTriggerRecord>> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        let application = self
            .repository
            .get_application(actor.current_workspace_id, command.application_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;
        ensure_application_view_permission(&actor, &application)?;
        if application.application_type != domain::ApplicationType::Workflow {
            return Err(ControlPlaneError::InvalidInput("application_type").into());
        }

        self.repository
            .get_workflow_schedule_trigger(application.id)
            .await
    }

    pub async fn replace_trigger(
        &self,
        command: ReplaceWorkflowScheduleTriggerCommand,
    ) -> Result<WorkflowScheduleTriggerRecord> {
        validate_schedule_cron(&command.cron)?;
        validate_schedule_timezone(&command.timezone)?;
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        let application = self
            .repository
            .get_application(actor.current_workspace_id, command.application_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;
        ensure_application_edit_permission(&actor, &application)?;
        if application.application_type != domain::ApplicationType::Workflow {
            return Err(ControlPlaneError::InvalidInput("application_type").into());
        }

        self.repository
            .replace_workflow_schedule_trigger(&ReplaceWorkflowScheduleTriggerInput {
                actor_user_id: command.actor_user_id,
                workspace_id: application.workspace_id,
                application_id: application.id,
                enabled: command.enabled,
                cron: command.cron,
                timezone: command.timezone,
                input_payload: command.input_payload,
            })
            .await
    }

    pub async fn dispatch_due_schedule(
        &self,
        command: DispatchWorkflowScheduleCommand,
        task_queue: Option<&dyn TaskQueue>,
    ) -> Result<WorkflowScheduleDispatchOutcome>
    where
        R: ApplicationPublicationRepository
            + ApplicationCompiledPlanRepository
            + ApplicationPublishedFlowRunRepository
            + Clone,
    {
        let Some(trigger) = self
            .repository
            .get_workflow_schedule_trigger(command.application_id)
            .await?
        else {
            return Ok(WorkflowScheduleDispatchOutcome::Skipped {
                reason: "not_configured",
            });
        };
        if !trigger.enabled {
            return Ok(WorkflowScheduleDispatchOutcome::Skipped { reason: "disabled" });
        }
        let Some(publication) = self
            .repository
            .load_active_application_publication(trigger.application_id)
            .await?
            .filter(|publication| publication.api_enabled)
        else {
            return Ok(WorkflowScheduleDispatchOutcome::Skipped {
                reason: "application_not_published",
            });
        };
        let compiled_plan = self
            .repository
            .get_application_compiled_plan(publication.compiled_plan_id)
            .await?
            .ok_or(ControlPlaneError::InvalidInput("compiled_plan"))?;
        let start_node_id = public_compiled_plan_start_node_id(&compiled_plan.plan);
        let environment_variables = self
            .repository
            .list_application_environment_variables(trigger.workspace_id, trigger.application_id)
            .await?;
        let input_payload = public_freeze_run_input_environment(
            trigger.input_payload.clone(),
            &environment_variables,
            None,
            start_node_id.as_deref(),
        );
        let idempotency_key =
            schedule_idempotency_key(trigger.application_id, command.scheduled_at);
        if let Some(existing) = self
            .repository
            .find_published_flow_run_by_idempotency_key(
                trigger.application_id,
                None,
                &idempotency_key,
            )
            .await?
        {
            return Ok(WorkflowScheduleDispatchOutcome::Dispatched(
                WorkflowScheduleDispatchResult {
                    run_id: existing.id,
                    status: existing.status,
                    task_id: None,
                },
            ));
        }
        let created = self
            .repository
            .create_published_flow_run(&CreateFlowRunInput {
                actor_user_id: trigger.updated_by,
                application_id: trigger.application_id,
                flow_id: publication.flow_id,
                flow_draft_id: compiled_plan.draft_id,
                compiled_plan_id: publication.compiled_plan_id,
                debug_session_id: String::new(),
                flow_schema_version: publication.flow_schema_version.clone(),
                document_hash: publication.document_hash.clone(),
                run_mode: domain::FlowRunMode::PublishedApiRun,
                target_node_id: None,
                title: build_flow_run_title(None, "Scheduled workflow"),
                status: domain::FlowRunStatus::Queued,
                input_payload,
                started_at: command.scheduled_at,
                api_key_id: None,
                publication_version_id: Some(publication.id),
                external_user: None,
                external_conversation_id: None,
                external_trace_id: Some(format!("workflow-schedule:{}", trigger.application_id)),
                compatibility_mode: Some(WORKFLOW_SCHEDULE_COMPATIBILITY_MODE.to_string()),
                idempotency_key: Some(idempotency_key.clone()),
            })
            .await?;
        let flow_run = created.flow_run;
        self.repository
            .append_published_run_event(&crate::ports::AppendRunEventInput {
                flow_run_id: flow_run.id,
                node_run_id: None,
                event_type: "workflow_schedule_run_enqueued".to_string(),
                payload: json!({
                    "trigger_source": "workflow_schedule",
                    "application_id": trigger.application_id,
                    "publication_version_id": publication.id,
                    "cron": trigger.cron,
                    "timezone": trigger.timezone,
                    "scheduled_at": command.scheduled_at,
                }),
            })
            .await?;
        let task_id = match task_queue {
            Some(queue) => Some(
                queue
                    .enqueue(
                        WORKFLOW_SCHEDULE_RUN_QUEUE,
                        json!({
                            "application_id": trigger.application_id,
                            "flow_run_id": flow_run.id,
                            "scheduled_at": command.scheduled_at,
                        }),
                        Some(&idempotency_key),
                    )
                    .await?,
            ),
            None => None,
        };

        Ok(WorkflowScheduleDispatchOutcome::Dispatched(
            WorkflowScheduleDispatchResult {
                run_id: flow_run.id,
                status: flow_run.status,
                task_id,
            },
        ))
    }
}

fn schedule_idempotency_key(application_id: Uuid, scheduled_at: OffsetDateTime) -> String {
    format!(
        "workflow-schedule:{}:{}",
        application_id,
        scheduled_at.unix_timestamp()
    )
}

fn validate_schedule_cron(cron: &str) -> Result<()> {
    let fields = cron.split_whitespace().collect::<Vec<_>>();
    let valid = fields.len() == 5
        && fields.iter().all(|field| {
            !field.is_empty()
                && field.chars().all(|character| {
                    character.is_ascii_digit() || matches!(character, '*' | ',' | '-' | '/')
                })
        });
    if valid {
        Ok(())
    } else {
        Err(ControlPlaneError::InvalidInput("cron").into())
    }
}

fn validate_schedule_timezone(timezone: &str) -> Result<()> {
    let valid = timezone == "UTC"
        || timezone == "Etc/UTC"
        || timezone.split_once('/').is_some_and(|(region, name)| {
            matches!(
                region,
                "Africa"
                    | "America"
                    | "Antarctica"
                    | "Arctic"
                    | "Asia"
                    | "Atlantic"
                    | "Australia"
                    | "Europe"
                    | "Indian"
                    | "Pacific"
                    | "Etc"
            ) && name
                .split('/')
                .all(|part| !part.is_empty() && part.chars().all(is_timezone_name_character))
        });
    if valid {
        Ok(())
    } else {
        Err(ControlPlaneError::InvalidInput("timezone").into())
    }
}

fn is_timezone_name_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '+')
}
