use std::sync::Arc;

use control_plane::application_public_api::workflow_schedule::{
    GetWorkflowScheduleTriggerCommand, ReplaceWorkflowScheduleTriggerCommand,
    WorkflowScheduleTriggerService,
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::{
    to_workflow_schedule_trigger_response, WorkflowScheduleTriggerBody,
    WorkflowScheduleTriggerResponse,
};
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(crate) enum WorkflowScheduleInput {
    Get {
        application_id: Uuid,
    },
    Replace {
        application_id: Uuid,
        body: WorkflowScheduleTriggerBody,
    },
}

impl InterfaceContract for WorkflowScheduleInput {
    const CONTRACT_ID: &'static str = "console-workflow-schedule-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum WorkflowScheduleOutput {
    Optional(Option<WorkflowScheduleTriggerResponse>),
    Required(WorkflowScheduleTriggerResponse),
}

impl WorkflowScheduleOutput {
    pub(super) fn into_optional(self) -> Result<Option<WorkflowScheduleTriggerResponse>, ApiError> {
        match self {
            Self::Optional(value) => Ok(value),
            _ => Err(output_error()),
        }
    }

    pub(super) fn into_required(self) -> Result<WorkflowScheduleTriggerResponse, ApiError> {
        match self {
            Self::Required(value) => Ok(value),
            _ => Err(output_error()),
        }
    }
}

fn output_error() -> ApiError {
    control_plane::errors::ControlPlaneError::InvalidInput("workflow_schedule_output").into()
}

impl InterfaceContract for WorkflowScheduleOutput {
    const CONTRACT_ID: &'static str = "console-workflow-schedule-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct WorkflowScheduleAdapter {
    store: MainDurableStore,
}

pub(crate) fn port(
    store: MainDurableStore,
) -> Arc<dyn ConsoleInterfacePort<WorkflowScheduleInput, WorkflowScheduleOutput>> {
    Arc::new(WorkflowScheduleAdapter { store })
}

impl ConsoleInterfacePort<WorkflowScheduleInput, WorkflowScheduleOutput>
    for WorkflowScheduleAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: WorkflowScheduleInput,
    ) -> ConsoleInterfaceFuture<'a, WorkflowScheduleOutput> {
        Box::pin(async move {
            let result: Result<WorkflowScheduleOutput, ApiError> = async {
                let actor = principal.actor();
                let service =
                    WorkflowScheduleTriggerService::new(self.store.for_actor(actor.clone()));
                let output = match input {
                    WorkflowScheduleInput::Get { application_id } => {
                        let trigger = service
                            .get_trigger(GetWorkflowScheduleTriggerCommand {
                                actor_user_id: actor.user_id,
                                application_id,
                            })
                            .await?
                            .map(to_workflow_schedule_trigger_response);
                        WorkflowScheduleOutput::Optional(trigger)
                    }
                    WorkflowScheduleInput::Replace {
                        application_id,
                        body,
                    } => {
                        let trigger = service
                            .replace_trigger(ReplaceWorkflowScheduleTriggerCommand {
                                actor_user_id: actor.user_id,
                                application_id,
                                enabled: body.enabled,
                                cron: body.cron,
                                timezone: body.timezone,
                                input_payload: body.input_payload,
                            })
                            .await?;
                        WorkflowScheduleOutput::Required(to_workflow_schedule_trigger_response(
                            trigger,
                        ))
                    }
                };
                Ok(output)
            }
            .await;
            result.map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "applications.workflow-schedule.get",
        binding_id: "http.console.applications.workflow-schedule.get.v1",
        method: "GET",
        path: "/api/console/applications/:application_id/workflow-schedule-trigger",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.workflow-schedule.replace",
        binding_id: "http.console.applications.workflow-schedule.replace.v1",
        method: "PUT",
        path: "/api/console/applications/:application_id/workflow-schedule-trigger",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<WorkflowScheduleInput, WorkflowScheduleOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-workflow-schedule",
        "api-server.console-workflow-schedule.graph.v1",
        DECLARATIONS,
        port,
    )
}
