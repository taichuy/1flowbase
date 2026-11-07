use super::{
    to_workflow_event_trigger_response, WorkflowEventTriggerBody, WorkflowEventTriggerResponse,
};
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};
use control_plane::application_public_api::workflow_event::WorkflowEventTriggerService;
use interface_runtime::{InterfaceContract, UserPrincipal};
use std::sync::{Arc, Weak};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

pub(crate) enum WorkflowEventInput {
    Get {
        application_id: Uuid,
    },
    Replace {
        application_id: Uuid,
        body: WorkflowEventTriggerBody,
    },
}
impl InterfaceContract for WorkflowEventInput {
    const CONTRACT_ID: &'static str = "console-workflow-event-input";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema([
            ("application_id", mp::text_schema()),
            ("mutating", serde_json::json!({"type":"boolean"})),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        let (id, mutating) = match self {
            Self::Get { application_id } => (application_id, false),
            Self::Replace { application_id, .. } => (application_id, true),
        };
        Some(serde_json::json!({"application_id":id,"mutating":mutating}))
    }
}
pub(crate) enum WorkflowEventOutput {
    Optional(Option<WorkflowEventTriggerResponse>),
    Required(WorkflowEventTriggerResponse),
}
impl WorkflowEventOutput {
    pub(super) fn into_optional(self) -> Result<Option<WorkflowEventTriggerResponse>, ApiError> {
        match self {
            Self::Optional(value) => Ok(value),
            _ => Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "workflow_event_output",
            )
            .into()),
        }
    }
    pub(super) fn into_required(self) -> Result<WorkflowEventTriggerResponse, ApiError> {
        match self {
            Self::Required(value) => Ok(value),
            _ => Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "workflow_event_output",
            )
            .into()),
        }
    }
}
impl InterfaceContract for WorkflowEventOutput {
    const CONTRACT_ID: &'static str = "console-workflow-event-output";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<serde_json::Value> {
        Some(
            serde_json::json!({"type":"object","properties":{"present":{"type":"boolean"}},"required":["present"],"additionalProperties":false}),
        )
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        Some(
            serde_json::json!({"present": match self { Self::Optional(value) => value.is_some(), Self::Required(_) => true }}),
        )
    }
}
struct WorkflowEventAdapter {
    store: MainDurableStore,
    changes: Arc<tokio::sync::Notify>,
    statuses: crate::workers::event_workflow::EventWorkflowStatuses,
    sources: Arc<crate::event_center::catalog::EventSourceCatalog>,
    runtime: control_plane::event_center::EventCenterRuntime,
    // The registry owns this adapter, so the generation is borrowed, never retained.
    boot: Option<Weak<crate::extension_bus::ExtensionBootSnapshot>>,
}
pub(crate) fn port(
    state: &crate::app_state::ApiState,
) -> Arc<dyn ConsoleInterfacePort<WorkflowEventInput, WorkflowEventOutput>> {
    Arc::new(WorkflowEventAdapter {
        store: state.store.clone(),
        changes: state.event_workflow_changes.clone(),
        statuses: state.event_workflow_statuses.clone(),
        sources: state.event_sources.clone(),
        runtime: state.event_center.clone(),
        boot: state.extension_boot_snapshot.as_ref().map(Arc::downgrade),
    })
}
impl ConsoleInterfacePort<WorkflowEventInput, WorkflowEventOutput> for WorkflowEventAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: WorkflowEventInput,
    ) -> ConsoleInterfaceFuture<'a, WorkflowEventOutput> {
        Box::pin(async move {
            let result: Result<WorkflowEventOutput, ApiError> = async {
                let actor = principal.actor();
                let _generation = self.boot.as_ref().map(|boot| {
                    boot.upgrade().ok_or_else(crate::generation_dispatch::unavailable)
                }).transpose()?;
                let registry = _generation.as_ref().and_then(|boot| boot.interface_registry()).map(|registry| registry.snapshot());
                let entries = self.sources.registered_events_with_interfaces(actor, &self.runtime, registry.as_deref());
                let service = WorkflowEventTriggerService::new(self.store.for_actor(actor.clone())).with_registered_events(entries);
                Ok(match input {
                    WorkflowEventInput::Get { application_id } => WorkflowEventOutput::Optional(
                        service
                            .get_trigger(actor.user_id, application_id)
                            .await?
                            .map(to_workflow_event_trigger_response)
                            .transpose()?,
                    ),
                    WorkflowEventInput::Replace {
                        application_id,
                        body,
                    } => {
                        let previous = control_plane_contracts::ports::EventCenterRepository::get_workflow_event_trigger(
                            &self.store.for_actor(actor.clone()), actor.current_workspace_id, application_id,
                        ).await?;
                        let record = service
                            .replace_trigger(actor.user_id, application_id, body.into_domain())
                            .await?;
                        if record.configuration.enabled && previous.is_none_or(|previous| !previous.configuration.enabled) {
                            self.statuses.request_activation(&record);
                        }
                        self.changes.notify_one();
                        WorkflowEventOutput::Required(to_workflow_event_trigger_response(record)?)
                    }
                })
            }
            .await;
            result.map_err(ConsoleInterfaceTargetError)
        })
    }
}
pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "applications.workflow-event.get",
        binding_id: "http.console.applications.workflow-event.get.v1",
        method: "GET",
        path: "/api/console/applications/:application_id/workflow-event-trigger",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.workflow-event.replace",
        binding_id: "http.console.applications.workflow-event.replace.v1",
        method: "PUT",
        path: "/api/console/applications/:application_id/workflow-event-trigger",
        mutating: true,
    },
];
pub(crate) fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<WorkflowEventInput, WorkflowEventOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-workflow-event",
        "api-server.console-workflow-event.graph.v1",
        DECLARATIONS,
        port,
    )
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/workflow_event_ownership.rs"]
mod ownership_tests;

impl crate::event_center::api_projection::ApiEventInput for WorkflowEventInput {
    fn project_document_input(
        &self,
        operation: &crate::event_center::api_projection::ApiEventOperationContext,
    ) -> Result<serde_json::Value, std::sync::Arc<str>> {
        Ok(match self {
            Self::Get { application_id } => {
                crate::routes::applications_group::event_projection::request(
                    operation,
                    serde_json::json!({"application_id":application_id}),
                    serde_json::json!({}),
                    serde_json::Value::Null,
                )
            }
            Self::Replace {
                application_id,
                body,
            } => crate::routes::applications_group::event_projection::request(
                operation,
                serde_json::json!({"application_id":application_id}),
                serde_json::json!({}),
                crate::event_center::api_projection::to_value(body)?,
            ),
        })
    }
}

impl crate::event_center::api_projection::ApiEventOutput for WorkflowEventOutput {
    fn project_document_output(
        &self,
        _operation: &crate::event_center::api_projection::ApiEventOperationContext,
    ) -> Result<serde_json::Value, std::sync::Arc<str>> {
        match self {
            Self::Optional(value) => crate::event_center::api_projection::to_value(value),
            Self::Required(value) => crate::event_center::api_projection::to_value(value),
        }
    }
}
