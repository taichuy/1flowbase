use std::sync::Arc;

use control_plane::application_public_api::workflow_schedule::{
    GetWorkflowScheduleTriggerCommand, ReplaceWorkflowScheduleTriggerCommand,
    WorkflowScheduleTriggerService,
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::{
    WorkflowScheduleTriggerBody, WorkflowScheduleTriggerResponse,
    to_workflow_schedule_trigger_response,
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
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Get")),
                ("application_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Replace")),
                ("application_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        (
                            "cron",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "timezone",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("input_payload", mp::json_summary_schema()),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Get {
                application_id: _field_application_id,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Get".to_owned())),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
            ]),
            Self::Replace {
                application_id: _field_application_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Replace".to_owned())),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "enabled",
                            serde_json::Value::Bool(*(&(_field_body).enabled)),
                        ),
                        (
                            "cron",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).cron).len()),
                            )]),
                        ),
                        (
                            "timezone",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).timezone).len()),
                            )]),
                        ),
                        (
                            "input_payload",
                            mp::json_summary(&(_field_body).input_payload),
                        ),
                    ]),
                ),
            ]),
        })
    }

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
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Optional")),
                (
                    "0",
                    serde_json::json!({"anyOf": [mp::object_schema(&[("id",mp::text_schema()), ("workspace_id",mp::text_schema()), ("application_id",mp::text_schema()), ("enabled",serde_json::json!({"type":"boolean"})), ("cron",mp::object_schema(&[("byte_count",mp::count_schema())])), ("timezone",mp::object_schema(&[("byte_count",mp::count_schema())])), ("input_payload",mp::json_summary_schema()), ("created_by",mp::text_schema()), ("updated_by",mp::text_schema()), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema())]), {"type":"null"}]}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Required")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        ("workspace_id", mp::text_schema()),
                        ("application_id", mp::text_schema()),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        (
                            "cron",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "timezone",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("input_payload", mp::json_summary_schema()),
                        ("created_by", mp::text_schema()),
                        ("updated_by", mp::text_schema()),
                        ("created_at", mp::text_schema()),
                        ("updated_at", mp::text_schema()),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Optional(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Optional".to_owned())),
                (
                    "0",
                    match (_field_0).as_ref() {
                        Some(item) => mp::object_value(&[
                            ("id", serde_json::Value::String((&(item).id).to_string())),
                            (
                                "workspace_id",
                                serde_json::Value::String((&(item).workspace_id).to_string()),
                            ),
                            (
                                "application_id",
                                serde_json::Value::String((&(item).application_id).to_string()),
                            ),
                            ("enabled", serde_json::Value::Bool(*(&(item).enabled))),
                            (
                                "cron",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((&(item).cron).len()),
                                )]),
                            ),
                            (
                                "timezone",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((&(item).timezone).len()),
                                )]),
                            ),
                            ("input_payload", mp::json_summary(&(item).input_payload)),
                            (
                                "created_by",
                                serde_json::Value::String((&(item).created_by).to_string()),
                            ),
                            (
                                "updated_by",
                                serde_json::Value::String((&(item).updated_by).to_string()),
                            ),
                            ("created_at", mp::text(&(item).created_at)?),
                            ("updated_at", mp::text(&(item).updated_at)?),
                        ]),
                        None => serde_json::Value::Null,
                    },
                ),
            ]),
            Self::Required(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Required".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "id",
                            serde_json::Value::String((&(_field_0).id).to_string()),
                        ),
                        (
                            "workspace_id",
                            serde_json::Value::String((&(_field_0).workspace_id).to_string()),
                        ),
                        (
                            "application_id",
                            serde_json::Value::String((&(_field_0).application_id).to_string()),
                        ),
                        ("enabled", serde_json::Value::Bool(*(&(_field_0).enabled))),
                        (
                            "cron",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).cron).len()),
                            )]),
                        ),
                        (
                            "timezone",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).timezone).len()),
                            )]),
                        ),
                        ("input_payload", mp::json_summary(&(_field_0).input_payload)),
                        (
                            "created_by",
                            serde_json::Value::String((&(_field_0).created_by).to_string()),
                        ),
                        (
                            "updated_by",
                            serde_json::Value::String((&(_field_0).updated_by).to_string()),
                        ),
                        ("created_at", mp::text(&(_field_0).created_at)?),
                        ("updated_at", mp::text(&(_field_0).updated_at)?),
                    ]),
                ),
            ]),
        })
    }

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
