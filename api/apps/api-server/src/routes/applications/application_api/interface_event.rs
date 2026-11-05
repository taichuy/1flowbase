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
use std::sync::Arc;
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
}
pub(crate) fn port(
    store: MainDurableStore,
    changes: Arc<tokio::sync::Notify>,
    statuses: crate::workers::event_workflow::EventWorkflowStatuses,
) -> Arc<dyn ConsoleInterfacePort<WorkflowEventInput, WorkflowEventOutput>> {
    Arc::new(WorkflowEventAdapter {
        store,
        changes,
        statuses,
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
                let service = WorkflowEventTriggerService::new(self.store.for_actor(actor.clone()));
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
