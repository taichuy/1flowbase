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

    /// Scans enabled schedule triggers and dispatches every trigger whose
    /// cron expression matches `now_utc` in its configured timezone. The
    /// scheduled minute is used as the idempotency anchor, so repeated ticks
    /// within the same minute do not enqueue duplicate runs.
    pub async fn dispatch_due_schedules(
        &self,
        now_utc: OffsetDateTime,
        task_queue: Option<&dyn TaskQueue>,
    ) -> Result<Vec<WorkflowScheduleTickEntry>>
    where
        R: ApplicationPublicationRepository
            + ApplicationCompiledPlanRepository
            + ApplicationPublishedFlowRunRepository
            + Clone,
    {
        let scheduled_at = now_utc
            .replace_second(0)
            .expect("zero seconds is always valid")
            .replace_nanosecond(0)
            .expect("zero nanoseconds is always valid");
        let triggers = self
            .repository
            .list_enabled_workflow_schedule_triggers()
            .await?;
        let mut entries = Vec::new();

        for trigger in triggers {
            let Some(local) =
                resolve_workflow_schedule_local_time(&trigger.timezone, scheduled_at)
            else {
                entries.push(WorkflowScheduleTickEntry {
                    application_id: trigger.application_id,
                    outcome: WorkflowScheduleDispatchOutcome::Skipped {
                        reason: "invalid_timezone",
                    },
                });
                continue;
            };

            if !workflow_schedule_cron_matches(&trigger.cron, local) {
                continue;
            }

            let outcome = self
                .dispatch_due_schedule(
                    DispatchWorkflowScheduleCommand {
                        application_id: trigger.application_id,
                        scheduled_at,
                    },
                    task_queue,
                )
                .await?;
            entries.push(WorkflowScheduleTickEntry {
                application_id: trigger.application_id,
                outcome,
            });
        }

        Ok(entries)
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
        let application = self
            .repository
            .get_application(trigger.workspace_id, trigger.application_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;
        if application.workflow_trigger_type != Some(domain::WorkflowTriggerType::Schedule) {
            return Ok(WorkflowScheduleDispatchOutcome::Skipped {
                reason: "trigger_type_mismatch",
            });
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowScheduleTickEntry {
    pub application_id: Uuid,
    pub outcome: WorkflowScheduleDispatchOutcome,
}

/// Resolves the wall-clock time for a trigger timezone; `None` means the
/// timezone cannot be resolved and the trigger must be skipped.
pub fn resolve_workflow_schedule_local_time(
    timezone: &str,
    now_utc: OffsetDateTime,
) -> Option<OffsetDateTime> {
    use time_tz::{timezones, OffsetDateTimeExt};

    let tz = timezones::get_by_name(timezone)?;
    Some(now_utc.to_timezone(tz))
}

/// Matches a five-field cron expression (minute hour day-of-month month
/// day-of-week) against a local wall-clock minute. Supports `*`, lists,
/// ranges and step values, mirroring `validate_schedule_cron`.
pub fn workflow_schedule_cron_matches(cron: &str, local: OffsetDateTime) -> bool {
    let fields = cron.split_whitespace().collect::<Vec<_>>();
    if fields.len() != 5 {
        return false;
    }

    let minute = i64::from(local.minute());
    let hour = i64::from(local.hour());
    let day_of_month = i64::from(local.day());
    let month = i64::from(u8::from(local.month()));
    let day_of_week = i64::from(local.weekday().number_days_from_sunday());

    let minute_matches = cron_field_matches(fields[0], minute, 0, 59);
    let hour_matches = cron_field_matches(fields[1], hour, 0, 23);
    let day_of_month_matches = cron_field_matches(fields[2], day_of_month, 1, 31);
    let month_matches = cron_field_matches(fields[3], month, 1, 12);
    // Both 0 and 7 mean Sunday in common cron dialects.
    let day_of_week_matches = cron_field_matches(fields[4], day_of_week, 0, 7)
        || (day_of_week == 0 && cron_field_matches(fields[4], 7, 0, 7));

    minute_matches && hour_matches && day_of_month_matches && month_matches && day_of_week_matches
}

fn cron_field_matches(field: &str, value: i64, min: i64, max: i64) -> bool {
    field.split(',').any(|part| {
        let (range, step) = match part.split_once('/') {
            Some((range, step)) => match step.parse::<i64>() {
                Ok(step) if step > 0 => (range, step),
                _ => return false,
            },
            None => (part, 1),
        };

        let (start, end) = if range == "*" {
            (min, max)
        } else if let Some((start, end)) = range.split_once('-') {
            match (start.parse::<i64>(), end.parse::<i64>()) {
                (Ok(start), Ok(end)) if start <= end => (start, end),
                _ => return false,
            }
        } else {
            match range.parse::<i64>() {
                // A bare value with a step (e.g. `9/2`) extends to the field max.
                Ok(start) if step > 1 => (start, max),
                Ok(start) => (start, start),
                Err(_) => return false,
            }
        };

        value >= start && value <= end && (value - start) % step == 0
    })
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
