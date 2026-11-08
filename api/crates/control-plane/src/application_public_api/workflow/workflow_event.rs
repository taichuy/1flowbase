//! Workflow ownership of event configuration and immutable publication invocation.
//! Subscription/queue lifecycle is owned by the host worker, not this service.
use anyhow::Result;
use control_plane_contracts::ports::EventCenterRepository;
use domain::event_center::{
    EventCatalogEntry, WorkflowEventTriggerInput, WorkflowEventTriggerRecord,
};
use serde_json::Value;
use uuid::Uuid;

use super::{
    run_service::ApplicationPublishedFlowRunRepository,
    workflow_invocation::{
        InvokeWorkflowCommand, WorkflowInvocationResult, WorkflowInvocationService,
        WorkflowInvocationTrigger,
    },
    workflow_start_http_inputs::{
        build_workflow_start_event_input_payload, parse_workflow_start_event_inputs,
    },
};
use crate::{
    application_public_api::{
        ensure_application_edit_permission, ensure_application_view_permission,
    },
    errors::ControlPlaneError,
    event_center::EventCenterConfigurationService,
    ports::{
        ApplicationCompiledPlanRepository, ApplicationPublicationRepository, ApplicationRepository,
    },
};

pub struct WorkflowEventTriggerService<R> {
    repository: R,
    registered_events: Vec<EventCatalogEntry>,
}

impl<R: ApplicationRepository + EventCenterRepository + Clone> WorkflowEventTriggerService<R> {
    pub fn new(repository: R) -> Self {
        Self {
            repository,
            registered_events: Vec::new(),
        }
    }

    /// Only the composition root supplies the authorized registry projection.
    pub fn with_registered_events(mut self, entries: Vec<EventCatalogEntry>) -> Self {
        self.registered_events = entries;
        self
    }

    pub async fn get_trigger(
        &self,
        actor_user_id: Uuid,
        application_id: Uuid,
    ) -> Result<Option<WorkflowEventTriggerRecord>> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        let application = self
            .repository
            .get_application(actor.current_workspace_id, application_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;
        ensure_application_view_permission(&self.repository, &actor, &application).await?;
        ensure_event_workflow(&application)?;
        self.repository
            .get_workflow_event_trigger(actor.current_workspace_id, application_id)
            .await
    }

    pub async fn replace_trigger(
        &self,
        actor_user_id: Uuid,
        application_id: Uuid,
        mut configuration: WorkflowEventTriggerInput,
    ) -> Result<WorkflowEventTriggerRecord>
    where
        R: ApplicationPublicationRepository + ApplicationCompiledPlanRepository,
    {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        let application = self
            .repository
            .get_application(actor.current_workspace_id, application_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;
        ensure_application_edit_permission(&self.repository, &actor, &application).await?;
        ensure_event_workflow(&application)?;
        // Retain the persisted legacy field, but it is not an independent activation switch.
        configuration.enabled = true;
        // A saved draft configuration does not activate a subscription. When a publication
        // exists the active immutable input contract, rather than the mutable draft, governs.
        if let Some(publication) = self
            .repository
            .load_active_application_publication(application_id)
            .await?
        {
            if configuration.enabled {
                let contract = parse_workflow_start_event_inputs(&publication.document_snapshot)?;
                validate_event_input_mapping(&contract, &configuration)?;
                let record = self
                    .repository
                    .get_application_compiled_plan(publication.compiled_plan_id)
                    .await?
                    .ok_or(ControlPlaneError::NotFound("compiled_plan"))?;
                let plan = serde_json::from_value(record.plan)?;
                crate::orchestration_runtime::compile_context::ensure_compiled_plan_runnable(
                    &plan,
                )?;
            }
        }
        let configuration_owner = EventCenterConfigurationService::with_registered_events(
            self.repository.clone(),
            actor.current_workspace_id,
            self.registered_events.clone(),
        );
        if let Some(previous) = self
            .repository
            .get_workflow_event_trigger(actor.current_workspace_id, application_id)
            .await?
        {
            if previous.configuration == configuration && previous.updated_by == actor_user_id {
                configuration_owner
                    .validate_workflow_trigger(&actor, &configuration)
                    .await?;
                return Ok(previous);
            }
        }
        configuration_owner
            .save_workflow_trigger(&actor, application_id, configuration)
            .await
    }

    /// Host activation gate. A receiver captures this publication and its trigger configuration
    /// once; runtime refresh must retire the receiver before replacing either snapshot.
    pub async fn prepare_subscription(
        &self,
        trigger: &WorkflowEventTriggerRecord,
    ) -> Result<super::publications::ApplicationPublicationVersionRecord>
    where
        R: ApplicationPublicationRepository + ApplicationCompiledPlanRepository,
    {
        let actor = self
            .repository
            .load_actor_context_for_user(trigger.updated_by)
            .await?;
        if actor.current_workspace_id != trigger.workspace_id {
            return Err(ControlPlaneError::PermissionDenied("workspace_changed").into());
        }
        let application = self
            .repository
            .get_application(trigger.workspace_id, trigger.application_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;
        ensure_event_workflow(&application)?;
        ensure_application_edit_permission(&self.repository, &actor, &application).await?;
        EventCenterConfigurationService::with_registered_events(
            self.repository.clone(),
            trigger.workspace_id,
            self.registered_events.clone(),
        )
        .validate_workflow_trigger(&actor, &listening_configuration(&trigger.configuration))
        .await?;
        let publication = self
            .repository
            .load_active_application_publication(trigger.application_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("application_publication"))?;
        if !publication.api_enabled {
            return Err(ControlPlaneError::InvalidInput("application_disabled").into());
        }
        if publication.workspace_id != trigger.workspace_id {
            return Err(ControlPlaneError::InvalidInput("workspace_id").into());
        }
        let contract = parse_workflow_start_event_inputs(&publication.document_snapshot)?;
        validate_event_input_mapping(&contract, &trigger.configuration)?;
        let record = self
            .repository
            .get_application_compiled_plan(publication.compiled_plan_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("compiled_plan"))?;
        crate::orchestration_runtime::compile_context::ensure_compiled_plan_runnable(
            &serde_json::from_value(record.plan)?,
        )?;
        Ok(publication)
    }

    /// Host integration captures immutable publication + trigger at activation time. Revalidation
    /// rejects stale subscriptions; queued signals are never reinterpreted using new configuration.
    /// The host owns delivery guard release and execution through the published run executor.
    pub async fn invoke_event(
        &self,
        command: InvokeWorkflowEventCommand,
    ) -> Result<WorkflowEventInvocationOutcome>
    where
        R: ApplicationPublicationRepository
            + ApplicationCompiledPlanRepository
            + ApplicationPublishedFlowRunRepository
            + 'static,
    {
        validate_event_context(&command)?;
        let snapshot = &command.trigger;
        let Some(current) = self
            .repository
            .get_workflow_event_trigger(snapshot.workspace_id, snapshot.application_id)
            .await?
        else {
            return Ok(WorkflowEventInvocationOutcome::Skipped {
                reason: "trigger_removed",
            });
        };
        if current != *snapshot {
            return Ok(WorkflowEventInvocationOutcome::Skipped {
                reason: "trigger_changed",
            });
        }
        let application = self
            .repository
            .get_application(snapshot.workspace_id, snapshot.application_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;
        ensure_event_workflow(&application)?;
        // Permission removal invalidates an already established subscription.
        let actor = self
            .repository
            .load_actor_context_for_user(snapshot.updated_by)
            .await?;
        if actor.current_workspace_id != snapshot.workspace_id {
            return Ok(WorkflowEventInvocationOutcome::Skipped {
                reason: "workspace_changed",
            });
        }
        ensure_application_edit_permission(&self.repository, &actor, &application).await?;
        EventCenterConfigurationService::with_registered_events(
            self.repository.clone(),
            snapshot.workspace_id,
            self.registered_events.clone(),
        )
        .validate_workflow_trigger(&actor, &listening_configuration(&snapshot.configuration))
        .await?;
        let Some(active) = self
            .repository
            .load_active_application_publication(application.id)
            .await?
        else {
            return Ok(WorkflowEventInvocationOutcome::Skipped {
                reason: "application_unpublished",
            });
        };
        if !active.api_enabled {
            return Ok(WorkflowEventInvocationOutcome::Skipped {
                reason: "application_disabled",
            });
        }
        // The active publication is replaced in place and keeps its database ID.
        // Compare the frozen contents as well, including changes after host handoff validation.
        if active != command.publication
            || command.publication.workspace_id != snapshot.workspace_id
            || command.publication.application_id != snapshot.application_id
        {
            return Ok(WorkflowEventInvocationOutcome::Skipped {
                reason: "publication_changed",
            });
        }
        let contract = parse_workflow_start_event_inputs(&command.publication.document_snapshot)?;
        let node_input_payload = build_workflow_start_event_input_payload(
            &contract,
            &snapshot.configuration.input_mapping,
            &command.context,
        )?;
        let key = format!(
            "workflow-event:{}:{}:{}",
            application.id, command.subscription_id, command.signal_id
        );
        Ok(WorkflowEventInvocationOutcome::Invoked(
            WorkflowInvocationService::new(self.repository.clone())
                .invoke(InvokeWorkflowCommand {
                    actor_user_id: snapshot.updated_by,
                    publication: command.publication,
                    node_input_payload,
                    trigger: WorkflowInvocationTrigger::Event {
                        signal_id: command.signal_id,
                        event_id: command.event_id,
                        source_context: command
                            .context
                            .get("source_context")
                            .cloned()
                            .map(serde_json::from_value)
                            .transpose()?,
                        workflow_ancestor_ids: command
                            .context
                            .get("workflow_ancestor_ids")
                            .cloned()
                            .map(serde_json::from_value)
                            .transpose()?
                            .unwrap_or_default(),
                        subscription_id: command.subscription_id,
                        trigger_phase: snapshot.configuration.trigger_phase,
                        idempotency_key: key,
                    },
                })
                .await?,
        ))
    }
}

#[derive(Debug, Clone)]
pub struct InvokeWorkflowEventCommand {
    /// Immutable records captured when the host establishes the subscription.
    pub trigger: WorkflowEventTriggerRecord,
    pub publication: super::publications::ApplicationPublicationVersionRecord,
    /// Unique identity of this lifecycle signal, including subscribed/consumed signals.
    pub signal_id: Uuid,
    /// Original published event identity, absent for subscription establishment.
    pub event_id: Option<String>,
    pub subscription_id: Uuid,
    /// Canonical serialized lifecycle signal, with phase-specific metadata and payload.
    pub context: Value,
}

#[derive(Debug, Clone)]
pub enum WorkflowEventInvocationOutcome {
    Invoked(WorkflowInvocationResult),
    Skipped { reason: &'static str },
}

fn ensure_event_workflow(application: &domain::ApplicationRecord) -> Result<()> {
    if application.application_type != domain::ApplicationType::Workflow
        || application.workflow_trigger_type != Some(domain::WorkflowTriggerType::Event)
    {
        return Err(ControlPlaneError::InvalidInput("workflow_trigger_type").into());
    }
    Ok(())
}

pub(crate) fn validate_event_input_mapping(
    contract: &super::workflow_start_http_inputs::WorkflowStartHttpInputs,
    configuration: &WorkflowEventTriggerInput,
) -> Result<()> {
    for (name, pointer) in &configuration.input_mapping {
        if !domain::event_center::is_json_pointer(pointer)
            || !contract.fields().iter().any(|field| field.key() == name)
        {
            return Err(ControlPlaneError::InvalidInput("input_mapping").into());
        }
    }
    for field in contract.fields() {
        if field.required()
            && field.default_value().is_none()
            && !configuration.input_mapping.contains_key(field.key())
        {
            return Err(ControlPlaneError::InvalidInput("input_mapping").into());
        }
    }
    Ok(())
}

fn validate_event_context(command: &InvokeWorkflowEventCommand) -> Result<()> {
    use domain::event_center::EventTriggerPhase;
    let phase = match command.trigger.configuration.trigger_phase {
        EventTriggerPhase::Subscribed => "subscribed",
        EventTriggerPhase::Published => "published",
        EventTriggerPhase::Consumed => "consumed",
    };
    let context = &command.context;
    let uuid_at = |field| {
        context
            .get(field)
            .and_then(Value::as_str)
            .and_then(|id| Uuid::parse_str(id).ok())
    };
    let source_context: Option<domain::event_center::EventSourceContext> = context
        .get("source_context")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|_| ControlPlaneError::InvalidInput("event_context"))?;
    match (&command.trigger.configuration.source, &source_context) {
        (None, None) => {}
        (Some(selected), Some(source))
            if selected.source_id == source.source_id
                && selected.scope == source.scope
                && selected
                    .filters
                    .iter()
                    .all(|(key, value)| source.attributes.get(key) == Some(value)) => {}
        _ => return Err(ControlPlaneError::InvalidInput("event_source_context").into()),
    }
    let original = match command.trigger.configuration.trigger_phase {
        EventTriggerPhase::Subscribed => None,
        EventTriggerPhase::Published => match &source_context {
            Some(source) => source.original_event_id.clone(),
            None => Some(command.signal_id.to_string()),
        },
        EventTriggerPhase::Consumed => context
            .get("source_event_id")
            .and_then(Value::as_str)
            .map(str::to_owned),
    };
    if let Some(source) = &source_context {
        if command.trigger.configuration.trigger_phase != EventTriggerPhase::Subscribed
            && (source.original_event_id != original
                || original.as_ref().is_none_or(|id| id.trim().is_empty()))
        {
            return Err(ControlPlaneError::InvalidInput("event_source_context").into());
        }
    }
    if uuid_at("workspace_id") != Some(command.trigger.workspace_id)
        || uuid_at("event_id") != Some(command.signal_id)
        || context.get("phase").and_then(Value::as_str) != Some(phase)
        || context.get("contract_id").and_then(Value::as_str)
            != Some(command.trigger.configuration.contract_id.as_str())
        || original != command.event_id
        || (command.trigger.configuration.trigger_phase == EventTriggerPhase::Consumed
            && original.is_none())
    {
        return Err(ControlPlaneError::InvalidInput("event_context").into());
    }
    Ok(())
}

// Old archives may retain enabled=false. Validate the actual event capability regardless;
// the published application's state owns whether a listener may be installed.
fn listening_configuration(input: &WorkflowEventTriggerInput) -> WorkflowEventTriggerInput {
    let mut configuration = input.clone();
    configuration.enabled = true;
    configuration
}

#[cfg(test)]
#[path = "_tests/workflow_event.rs"]
mod tests;
