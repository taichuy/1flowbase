use std::{future::Future, pin::Pin, sync::Arc};

use control_plane::application_public_api::{
    mapping::WorkflowExtensionHttpMethod,
    workflow_extension::{WorkflowExtensionRequestParameters, WorkflowHttpPrincipal},
};
use interface_runtime::{
    AuthenticationAdapterReference, AuthorizationAdapterReference, AuthorizationOperation,
    BindingId, CompiledInterfaceRegistry, ContractIdentity, GraphFingerprint, HandlerReference,
    InterfaceAccess, InterfaceAuditPolicy, InterfaceAuthenticationPolicy,
    InterfaceAuthorizationFuture, InterfaceAuthorizationPort, InterfaceAuthorizationRequest,
    InterfaceContract, InterfaceContracts, InterfaceDefinition, InterfaceErrorPolicy,
    InterfaceExecution, InterfaceExecutionMode, InterfaceHandler, InterfaceHandlerContext,
    InterfaceHandlerFuture, InterfaceId, InterfaceIdentity, InterfaceLifecycle, InterfaceOwner,
    InterfaceScope, InterfaceTargetFailure, InterfaceVersion, InvocationAdapterPlan,
    ProtocolBinding, ProtocolProjection, RegistryCompiler, RouteIdentity, TargetReference,
    UserCredentialKind, UserPrincipal,
};

use super::native::NativeApiError;

pub(crate) const BINDING_ID: &str = "http.workflow-extension.invoke.v1";
const INTERFACE_ID: &str = "workflow-extension.invoke";
const HANDLER: &str = "api-server.workflow-extension.invoke";
const OPERATION: &str = "workflow-extension.invoke";

pub(crate) struct WorkflowExtensionInput {
    pub(crate) request_path: String,
    pub(crate) method: WorkflowExtensionHttpMethod,
    pub(crate) parameters: WorkflowExtensionRequestParameters,
}

impl InterfaceContract for WorkflowExtensionInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[
            (
                "method",
                mp::union_schema(vec![
                    mp::object_schema(&[("variant", mp::tag_schema("Get"))]),
                    mp::object_schema(&[("variant", mp::tag_schema("Post"))]),
                    mp::object_schema(&[("variant", mp::tag_schema("Put"))]),
                    mp::object_schema(&[("variant", mp::tag_schema("Patch"))]),
                    mp::object_schema(&[("variant", mp::tag_schema("Delete"))]),
                    mp::object_schema(&[("variant", mp::tag_schema("Head"))]),
                    mp::object_schema(&[("variant", mp::tag_schema("Options"))]),
                ]),
            ),
            (
                "parameters",
                mp::object_schema(&[
                    (
                        "path",
                        mp::object_schema(&[("item_count", mp::count_schema())]),
                    ),
                    (
                        "query",
                        mp::object_schema(&[("item_count", mp::count_schema())]),
                    ),
                    (
                        "form",
                        mp::object_schema(&[("item_count", mp::count_schema())]),
                    ),
                    ("body", mp::json_summary_schema()),
                ]),
            ),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[("method",match &(self).method {control_plane_contracts::application_public_api::WorkflowExtensionHttpMethod::Get => mp::object_value(&[("variant",serde_json::Value::String("Get".to_owned()))]), control_plane_contracts::application_public_api::WorkflowExtensionHttpMethod::Post => mp::object_value(&[("variant",serde_json::Value::String("Post".to_owned()))]), control_plane_contracts::application_public_api::WorkflowExtensionHttpMethod::Put => mp::object_value(&[("variant",serde_json::Value::String("Put".to_owned()))]), control_plane_contracts::application_public_api::WorkflowExtensionHttpMethod::Patch => mp::object_value(&[("variant",serde_json::Value::String("Patch".to_owned()))]), control_plane_contracts::application_public_api::WorkflowExtensionHttpMethod::Delete => mp::object_value(&[("variant",serde_json::Value::String("Delete".to_owned()))]), control_plane_contracts::application_public_api::WorkflowExtensionHttpMethod::Head => mp::object_value(&[("variant",serde_json::Value::String("Head".to_owned()))]), control_plane_contracts::application_public_api::WorkflowExtensionHttpMethod::Options => mp::object_value(&[("variant",serde_json::Value::String("Options".to_owned()))])}), ("parameters",mp::object_value(&[("path",mp::object_value(&[("item_count",serde_json::json!((&(&(self).parameters).path).len()))])), ("query",mp::object_value(&[("item_count",serde_json::json!((&(&(self).parameters).query).len()))])), ("form",mp::object_value(&[("item_count",serde_json::json!((&(&(self).parameters).form).len()))])), ("body",mp::json_summary(&(&(self).parameters).body))]))]))
    }

    const CONTRACT_ID: &'static str = "workflow-extension-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[expect(
    clippy::large_enum_variant,
    reason = "the typed output is projected immediately into the public protocol response"
)]
pub(crate) enum WorkflowExtensionOutput {
    Accepted {
        run_id: uuid::Uuid,
        status: domain::FlowRunStatus,
    },
    Completed(domain::ApplicationRunDetail),
}

impl InterfaceContract for WorkflowExtensionOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Accepted")),
                ("run_id", mp::text_schema()),
                (
                    "status",
                    mp::union_schema(vec![
                        mp::object_schema(&[("variant", mp::tag_schema("Queued"))]),
                        mp::object_schema(&[("variant", mp::tag_schema("Running"))]),
                        mp::object_schema(&[("variant", mp::tag_schema("WaitingCallback"))]),
                        mp::object_schema(&[("variant", mp::tag_schema("WaitingHuman"))]),
                        mp::object_schema(&[("variant", mp::tag_schema("Paused"))]),
                        mp::object_schema(&[("variant", mp::tag_schema("Succeeded"))]),
                        mp::object_schema(&[("variant", mp::tag_schema("Incomplete"))]),
                        mp::object_schema(&[("variant", mp::tag_schema("Failed"))]),
                        mp::object_schema(&[("variant", mp::tag_schema("Cancelled"))]),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Completed")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "flow_run",
                            mp::object_schema(&[
                                ("id", mp::text_schema()),
                                ("application_id", mp::text_schema()),
                                ("flow_id", mp::text_schema()),
                                ("draft_id", mp::text_schema()),
                                (
                                    "compiled_plan_id",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                ("flow_schema_version", mp::text_schema()),
                                (
                                    "document_hash",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "run_mode",
                                    mp::union_schema(vec![
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("DebugNodePreview"),
                                        )]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("DebugFlowRun"),
                                        )]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("PublishedApiRun"),
                                        )]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("AssistantExecution"),
                                        )]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("WorkflowHttpRun"),
                                        )]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("WorkflowScheduleRun"),
                                        )]),
                                    ]),
                                ),
                                (
                                    "target_node_id",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "title",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "status",
                                    mp::union_schema(vec![
                                        mp::object_schema(&[("variant", mp::tag_schema("Queued"))]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("Running"),
                                        )]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("WaitingCallback"),
                                        )]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("WaitingHuman"),
                                        )]),
                                        mp::object_schema(&[("variant", mp::tag_schema("Paused"))]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("Succeeded"),
                                        )]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("Incomplete"),
                                        )]),
                                        mp::object_schema(&[("variant", mp::tag_schema("Failed"))]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("Cancelled"),
                                        )]),
                                    ]),
                                ),
                                ("input_payload", mp::json_summary_schema()),
                                ("output_payload", mp::json_summary_schema()),
                                (
                                    "error_payload",
                                    serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                                ),
                                ("created_by", mp::text_schema()),
                                (
                                    "publication_version_id",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "external_user",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "external_conversation_id",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "external_trace_id",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "compatibility_mode",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "idempotency_key",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                ("started_at", serde_json::json!({"type":"integer"})),
                                (
                                    "finished_at",
                                    serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                                ),
                                ("created_at", serde_json::json!({"type":"integer"})),
                                ("updated_at", serde_json::json!({"type":"integer"})),
                            ]),
                        ),
                        (
                            "node_runs",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_id",mp::text_schema()), ("node_type",mp::text_schema()), ("node_alias",mp::object_schema(&[("byte_count",mp::count_schema())])), ("status",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Pending"))]), mp::object_schema(&[("variant",mp::tag_schema("Ready"))]), mp::object_schema(&[("variant",mp::tag_schema("Running"))]), mp::object_schema(&[("variant",mp::tag_schema("Streaming"))]), mp::object_schema(&[("variant",mp::tag_schema("WaitingTool"))]), mp::object_schema(&[("variant",mp::tag_schema("WaitingCallback"))]), mp::object_schema(&[("variant",mp::tag_schema("WaitingHuman"))]), mp::object_schema(&[("variant",mp::tag_schema("Retrying"))]), mp::object_schema(&[("variant",mp::tag_schema("Succeeded"))]), mp::object_schema(&[("variant",mp::tag_schema("Failed"))]), mp::object_schema(&[("variant",mp::tag_schema("Cancelled"))]), mp::object_schema(&[("variant",mp::tag_schema("Skipped"))])])), ("input_payload",mp::json_summary_schema()), ("output_payload",mp::json_summary_schema()), ("error_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("metrics_payload",mp::json_summary_schema()), ("debug_payload",mp::json_summary_schema()), ("started_at",serde_json::json!({"type":"integer"})), ("finished_at",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}))])}),
                        ),
                        (
                            "checkpoints",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("status",mp::text_schema()), ("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("locator_payload",mp::json_summary_schema()), ("variable_snapshot",mp::json_summary_schema()), ("external_ref_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("created_at",serde_json::json!({"type":"integer"}))])}),
                        ),
                        (
                            "callback_tasks",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_run_id",mp::text_schema()), ("callback_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("status",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Pending"))]), mp::object_schema(&[("variant",mp::tag_schema("Completed"))]), mp::object_schema(&[("variant",mp::tag_schema("Cancelled"))])])), ("request_payload",mp::json_summary_schema()), ("response_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("external_ref_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("created_at",serde_json::json!({"type":"integer"})), ("completed_at",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}))])}),
                        ),
                        (
                            "events",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("sequence",serde_json::json!({"type":"integer"})), ("event_type",mp::text_schema()), ("payload",mp::json_summary_schema()), ("created_at",serde_json::json!({"type":"integer"}))])}),
                        ),
                        (
                            "stitched_trace",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("source_flow_run",mp::object_schema(&[("id",mp::text_schema()), ("application_id",mp::text_schema()), ("flow_id",mp::text_schema()), ("draft_id",mp::text_schema()), ("compiled_plan_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("flow_schema_version",mp::text_schema()), ("document_hash",mp::object_schema(&[("byte_count",mp::count_schema())])), ("run_mode",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("DebugNodePreview"))]), mp::object_schema(&[("variant",mp::tag_schema("DebugFlowRun"))]), mp::object_schema(&[("variant",mp::tag_schema("PublishedApiRun"))]), mp::object_schema(&[("variant",mp::tag_schema("AssistantExecution"))]), mp::object_schema(&[("variant",mp::tag_schema("WorkflowHttpRun"))]), mp::object_schema(&[("variant",mp::tag_schema("WorkflowScheduleRun"))])])), ("target_node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("title",mp::object_schema(&[("byte_count",mp::count_schema())])), ("status",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Queued"))]), mp::object_schema(&[("variant",mp::tag_schema("Running"))]), mp::object_schema(&[("variant",mp::tag_schema("WaitingCallback"))]), mp::object_schema(&[("variant",mp::tag_schema("WaitingHuman"))]), mp::object_schema(&[("variant",mp::tag_schema("Paused"))]), mp::object_schema(&[("variant",mp::tag_schema("Succeeded"))]), mp::object_schema(&[("variant",mp::tag_schema("Incomplete"))]), mp::object_schema(&[("variant",mp::tag_schema("Failed"))]), mp::object_schema(&[("variant",mp::tag_schema("Cancelled"))])])), ("input_payload",mp::json_summary_schema()), ("output_payload",mp::json_summary_schema()), ("error_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("created_by",mp::text_schema()), ("publication_version_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("external_user",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("external_conversation_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("external_trace_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("compatibility_mode",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("idempotency_key",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("started_at",serde_json::json!({"type":"integer"})), ("finished_at",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]})), ("created_at",serde_json::json!({"type":"integer"})), ("updated_at",serde_json::json!({"type":"integer"}))])), ("node_runs",mp::object_schema(&[("item_count",mp::count_schema())])), ("callback_tasks",mp::object_schema(&[("item_count",mp::count_schema())])), ("events",mp::object_schema(&[("item_count",mp::count_schema())])), ("runtime_events",mp::object_schema(&[("item_count",mp::count_schema())]))])}),
                        ),
                        (
                            "subagent_traces",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("parent_tool_call_id",mp::text_schema()), ("parent_callback_task_id",mp::text_schema()), ("source_flow_run",mp::object_schema(&[("id",mp::text_schema()), ("application_id",mp::text_schema()), ("flow_id",mp::text_schema()), ("draft_id",mp::text_schema()), ("compiled_plan_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("flow_schema_version",mp::text_schema()), ("document_hash",mp::object_schema(&[("byte_count",mp::count_schema())])), ("run_mode",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("DebugNodePreview"))]), mp::object_schema(&[("variant",mp::tag_schema("DebugFlowRun"))]), mp::object_schema(&[("variant",mp::tag_schema("PublishedApiRun"))]), mp::object_schema(&[("variant",mp::tag_schema("AssistantExecution"))]), mp::object_schema(&[("variant",mp::tag_schema("WorkflowHttpRun"))]), mp::object_schema(&[("variant",mp::tag_schema("WorkflowScheduleRun"))])])), ("target_node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("title",mp::object_schema(&[("byte_count",mp::count_schema())])), ("status",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Queued"))]), mp::object_schema(&[("variant",mp::tag_schema("Running"))]), mp::object_schema(&[("variant",mp::tag_schema("WaitingCallback"))]), mp::object_schema(&[("variant",mp::tag_schema("WaitingHuman"))]), mp::object_schema(&[("variant",mp::tag_schema("Paused"))]), mp::object_schema(&[("variant",mp::tag_schema("Succeeded"))]), mp::object_schema(&[("variant",mp::tag_schema("Incomplete"))]), mp::object_schema(&[("variant",mp::tag_schema("Failed"))]), mp::object_schema(&[("variant",mp::tag_schema("Cancelled"))])])), ("input_payload",mp::json_summary_schema()), ("output_payload",mp::json_summary_schema()), ("error_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("created_by",mp::text_schema()), ("publication_version_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("external_user",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("external_conversation_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("external_trace_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("compatibility_mode",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("idempotency_key",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("started_at",serde_json::json!({"type":"integer"})), ("finished_at",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]})), ("created_at",serde_json::json!({"type":"integer"})), ("updated_at",serde_json::json!({"type":"integer"}))])), ("node_runs",mp::object_schema(&[("item_count",mp::count_schema())])), ("callback_tasks",mp::object_schema(&[("item_count",mp::count_schema())])), ("events",mp::object_schema(&[("item_count",mp::count_schema())])), ("runtime_events",mp::object_schema(&[("item_count",mp::count_schema())]))])}),
                        ),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Accepted {
                run_id: _field_run_id,
                status: _field_status,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Accepted".to_owned())),
                (
                    "run_id",
                    serde_json::Value::String((_field_run_id).to_string()),
                ),
                (
                    "status",
                    match _field_status {
                        domain::orchestration::FlowRunStatus::Queued => mp::object_value(&[(
                            "variant",
                            serde_json::Value::String("Queued".to_owned()),
                        )]),
                        domain::orchestration::FlowRunStatus::Running => mp::object_value(&[(
                            "variant",
                            serde_json::Value::String("Running".to_owned()),
                        )]),
                        domain::orchestration::FlowRunStatus::WaitingCallback => {
                            mp::object_value(&[(
                                "variant",
                                serde_json::Value::String("WaitingCallback".to_owned()),
                            )])
                        }
                        domain::orchestration::FlowRunStatus::WaitingHuman => {
                            mp::object_value(&[(
                                "variant",
                                serde_json::Value::String("WaitingHuman".to_owned()),
                            )])
                        }
                        domain::orchestration::FlowRunStatus::Paused => mp::object_value(&[(
                            "variant",
                            serde_json::Value::String("Paused".to_owned()),
                        )]),
                        domain::orchestration::FlowRunStatus::Succeeded => mp::object_value(&[(
                            "variant",
                            serde_json::Value::String("Succeeded".to_owned()),
                        )]),
                        domain::orchestration::FlowRunStatus::Incomplete => mp::object_value(&[(
                            "variant",
                            serde_json::Value::String("Incomplete".to_owned()),
                        )]),
                        domain::orchestration::FlowRunStatus::Failed => mp::object_value(&[(
                            "variant",
                            serde_json::Value::String("Failed".to_owned()),
                        )]),
                        domain::orchestration::FlowRunStatus::Cancelled => mp::object_value(&[(
                            "variant",
                            serde_json::Value::String("Cancelled".to_owned()),
                        )]),
                    },
                ),
            ]),
            Self::Completed(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Completed".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "flow_run",
                            mp::object_value(&[
                                (
                                    "id",
                                    serde_json::Value::String(
                                        (&(&(_field_0).flow_run).id).to_string(),
                                    ),
                                ),
                                (
                                    "application_id",
                                    serde_json::Value::String(
                                        (&(&(_field_0).flow_run).application_id).to_string(),
                                    ),
                                ),
                                (
                                    "flow_id",
                                    serde_json::Value::String(
                                        (&(&(_field_0).flow_run).flow_id).to_string(),
                                    ),
                                ),
                                (
                                    "draft_id",
                                    serde_json::Value::String(
                                        (&(&(_field_0).flow_run).draft_id).to_string(),
                                    ),
                                ),
                                (
                                    "compiled_plan_id",
                                    match (&(&(_field_0).flow_run).compiled_plan_id).as_ref() {
                                        Some(item) => serde_json::Value::String((item).to_string()),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "flow_schema_version",
                                    mp::text(&(&(_field_0).flow_run).flow_schema_version)?,
                                ),
                                (
                                    "document_hash",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_0).flow_run).document_hash).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "run_mode",
                                    match &(&(_field_0).flow_run).run_mode {
                                        domain::orchestration::FlowRunMode::DebugNodePreview => {
                                            mp::object_value(&[(
                                                "variant",
                                                serde_json::Value::String(
                                                    "DebugNodePreview".to_owned(),
                                                ),
                                            )])
                                        }
                                        domain::orchestration::FlowRunMode::DebugFlowRun => {
                                            mp::object_value(&[(
                                                "variant",
                                                serde_json::Value::String(
                                                    "DebugFlowRun".to_owned(),
                                                ),
                                            )])
                                        }
                                        domain::orchestration::FlowRunMode::PublishedApiRun => {
                                            mp::object_value(&[(
                                                "variant",
                                                serde_json::Value::String(
                                                    "PublishedApiRun".to_owned(),
                                                ),
                                            )])
                                        }
                                        domain::orchestration::FlowRunMode::AssistantExecution => {
                                            mp::object_value(&[(
                                                "variant",
                                                serde_json::Value::String(
                                                    "AssistantExecution".to_owned(),
                                                ),
                                            )])
                                        }
                                        domain::orchestration::FlowRunMode::WorkflowHttpRun => {
                                            mp::object_value(&[(
                                                "variant",
                                                serde_json::Value::String(
                                                    "WorkflowHttpRun".to_owned(),
                                                ),
                                            )])
                                        }
                                        domain::orchestration::FlowRunMode::WorkflowScheduleRun => {
                                            mp::object_value(&[(
                                                "variant",
                                                serde_json::Value::String(
                                                    "WorkflowScheduleRun".to_owned(),
                                                ),
                                            )])
                                        }
                                    },
                                ),
                                (
                                    "target_node_id",
                                    match (&(&(_field_0).flow_run).target_node_id).as_ref() {
                                        Some(item) => mp::text(item)?,
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "title",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((&(&(_field_0).flow_run).title).len()),
                                    )]),
                                ),
                                (
                                    "status",
                                    match &(&(_field_0).flow_run).status {
                                        domain::orchestration::FlowRunStatus::Queued => {
                                            mp::object_value(&[(
                                                "variant",
                                                serde_json::Value::String("Queued".to_owned()),
                                            )])
                                        }
                                        domain::orchestration::FlowRunStatus::Running => {
                                            mp::object_value(&[(
                                                "variant",
                                                serde_json::Value::String("Running".to_owned()),
                                            )])
                                        }
                                        domain::orchestration::FlowRunStatus::WaitingCallback => {
                                            mp::object_value(&[(
                                                "variant",
                                                serde_json::Value::String(
                                                    "WaitingCallback".to_owned(),
                                                ),
                                            )])
                                        }
                                        domain::orchestration::FlowRunStatus::WaitingHuman => {
                                            mp::object_value(&[(
                                                "variant",
                                                serde_json::Value::String(
                                                    "WaitingHuman".to_owned(),
                                                ),
                                            )])
                                        }
                                        domain::orchestration::FlowRunStatus::Paused => {
                                            mp::object_value(&[(
                                                "variant",
                                                serde_json::Value::String("Paused".to_owned()),
                                            )])
                                        }
                                        domain::orchestration::FlowRunStatus::Succeeded => {
                                            mp::object_value(&[(
                                                "variant",
                                                serde_json::Value::String("Succeeded".to_owned()),
                                            )])
                                        }
                                        domain::orchestration::FlowRunStatus::Incomplete => {
                                            mp::object_value(&[(
                                                "variant",
                                                serde_json::Value::String("Incomplete".to_owned()),
                                            )])
                                        }
                                        domain::orchestration::FlowRunStatus::Failed => {
                                            mp::object_value(&[(
                                                "variant",
                                                serde_json::Value::String("Failed".to_owned()),
                                            )])
                                        }
                                        domain::orchestration::FlowRunStatus::Cancelled => {
                                            mp::object_value(&[(
                                                "variant",
                                                serde_json::Value::String("Cancelled".to_owned()),
                                            )])
                                        }
                                    },
                                ),
                                (
                                    "input_payload",
                                    mp::json_summary(&(&(_field_0).flow_run).input_payload),
                                ),
                                (
                                    "output_payload",
                                    mp::json_summary(&(&(_field_0).flow_run).output_payload),
                                ),
                                (
                                    "error_payload",
                                    match (&(&(_field_0).flow_run).error_payload).as_ref() {
                                        Some(item) => mp::json_summary(item),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "created_by",
                                    serde_json::Value::String(
                                        (&(&(_field_0).flow_run).created_by).to_string(),
                                    ),
                                ),
                                (
                                    "publication_version_id",
                                    match (&(&(_field_0).flow_run).publication_version_id).as_ref()
                                    {
                                        Some(item) => serde_json::Value::String((item).to_string()),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "external_user",
                                    match (&(&(_field_0).flow_run).external_user).as_ref() {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "external_conversation_id",
                                    match (&(&(_field_0).flow_run).external_conversation_id)
                                        .as_ref()
                                    {
                                        Some(item) => mp::text(item)?,
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "external_trace_id",
                                    match (&(&(_field_0).flow_run).external_trace_id).as_ref() {
                                        Some(item) => mp::text(item)?,
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "compatibility_mode",
                                    match (&(&(_field_0).flow_run).compatibility_mode).as_ref() {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "idempotency_key",
                                    match (&(&(_field_0).flow_run).idempotency_key).as_ref() {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "started_at",
                                    serde_json::json!(
                                        (&(&(_field_0).flow_run).started_at).unix_timestamp()
                                    ),
                                ),
                                (
                                    "finished_at",
                                    match (&(&(_field_0).flow_run).finished_at).as_ref() {
                                        Some(item) => serde_json::json!((item).unix_timestamp()),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "created_at",
                                    serde_json::json!(
                                        (&(&(_field_0).flow_run).created_at).unix_timestamp()
                                    ),
                                ),
                                (
                                    "updated_at",
                                    serde_json::json!(
                                        (&(&(_field_0).flow_run).updated_at).unix_timestamp()
                                    ),
                                ),
                            ]),
                        ),
                        ("node_runs", {
                            if (&(_field_0).node_runs).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array((&(_field_0).node_runs).iter().map(|item| Some(mp::object_value(&[("id",serde_json::Value::String((&(item).id).to_string())), ("flow_run_id",serde_json::Value::String((&(item).flow_run_id).to_string())), ("node_id",mp::text(&(item).node_id)?), ("node_type",mp::text(&(item).node_type)?), ("node_alias",mp::object_value(&[("byte_count",serde_json::json!((&(item).node_alias).len()))])), ("status",match &(item).status {domain::orchestration::NodeRunStatus::Pending => mp::object_value(&[("variant",serde_json::Value::String("Pending".to_owned()))]), domain::orchestration::NodeRunStatus::Ready => mp::object_value(&[("variant",serde_json::Value::String("Ready".to_owned()))]), domain::orchestration::NodeRunStatus::Running => mp::object_value(&[("variant",serde_json::Value::String("Running".to_owned()))]), domain::orchestration::NodeRunStatus::Streaming => mp::object_value(&[("variant",serde_json::Value::String("Streaming".to_owned()))]), domain::orchestration::NodeRunStatus::WaitingTool => mp::object_value(&[("variant",serde_json::Value::String("WaitingTool".to_owned()))]), domain::orchestration::NodeRunStatus::WaitingCallback => mp::object_value(&[("variant",serde_json::Value::String("WaitingCallback".to_owned()))]), domain::orchestration::NodeRunStatus::WaitingHuman => mp::object_value(&[("variant",serde_json::Value::String("WaitingHuman".to_owned()))]), domain::orchestration::NodeRunStatus::Retrying => mp::object_value(&[("variant",serde_json::Value::String("Retrying".to_owned()))]), domain::orchestration::NodeRunStatus::Succeeded => mp::object_value(&[("variant",serde_json::Value::String("Succeeded".to_owned()))]), domain::orchestration::NodeRunStatus::Failed => mp::object_value(&[("variant",serde_json::Value::String("Failed".to_owned()))]), domain::orchestration::NodeRunStatus::Cancelled => mp::object_value(&[("variant",serde_json::Value::String("Cancelled".to_owned()))]), domain::orchestration::NodeRunStatus::Skipped => mp::object_value(&[("variant",serde_json::Value::String("Skipped".to_owned()))])}), ("input_payload",mp::json_summary(&(item).input_payload)), ("output_payload",mp::json_summary(&(item).output_payload)), ("error_payload",match (&(item).error_payload).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("metrics_payload",mp::json_summary(&(item).metrics_payload)), ("debug_payload",mp::json_summary(&(item).debug_payload)), ("started_at",serde_json::json!((&(item).started_at).unix_timestamp())), ("finished_at",match (&(item).finished_at).as_ref() { Some(item) => serde_json::json!((item).unix_timestamp()), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?)
                        }),
                        ("checkpoints", {
                            if (&(_field_0).checkpoints).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).checkpoints)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            (
                                                "id",
                                                serde_json::Value::String((&(item).id).to_string()),
                                            ),
                                            (
                                                "flow_run_id",
                                                serde_json::Value::String(
                                                    (&(item).flow_run_id).to_string(),
                                                ),
                                            ),
                                            (
                                                "node_run_id",
                                                match (&(item).node_run_id).as_ref() {
                                                    Some(item) => serde_json::Value::String(
                                                        (item).to_string(),
                                                    ),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            ("status", mp::text(&(item).status)?),
                                            (
                                                "reason",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).reason).len()),
                                                )]),
                                            ),
                                            (
                                                "locator_payload",
                                                mp::json_summary(&(item).locator_payload),
                                            ),
                                            (
                                                "variable_snapshot",
                                                mp::json_summary(&(item).variable_snapshot),
                                            ),
                                            (
                                                "external_ref_payload",
                                                match (&(item).external_ref_payload).as_ref() {
                                                    Some(item) => mp::json_summary(item),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "created_at",
                                                serde_json::json!(
                                                    (&(item).created_at).unix_timestamp()
                                                ),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("callback_tasks", {
                            if (&(_field_0).callback_tasks).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array((&(_field_0).callback_tasks).iter().map(|item| Some(mp::object_value(&[("id",serde_json::Value::String((&(item).id).to_string())), ("flow_run_id",serde_json::Value::String((&(item).flow_run_id).to_string())), ("node_run_id",serde_json::Value::String((&(item).node_run_id).to_string())), ("callback_kind",mp::object_value(&[("byte_count",serde_json::json!((&(item).callback_kind).len()))])), ("status",match &(item).status {domain::orchestration::CallbackTaskStatus::Pending => mp::object_value(&[("variant",serde_json::Value::String("Pending".to_owned()))]), domain::orchestration::CallbackTaskStatus::Completed => mp::object_value(&[("variant",serde_json::Value::String("Completed".to_owned()))]), domain::orchestration::CallbackTaskStatus::Cancelled => mp::object_value(&[("variant",serde_json::Value::String("Cancelled".to_owned()))])}), ("request_payload",mp::json_summary(&(item).request_payload)), ("response_payload",match (&(item).response_payload).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("external_ref_payload",match (&(item).external_ref_payload).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("created_at",serde_json::json!((&(item).created_at).unix_timestamp())), ("completed_at",match (&(item).completed_at).as_ref() { Some(item) => serde_json::json!((item).unix_timestamp()), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?)
                        }),
                        ("events", {
                            if (&(_field_0).events).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).events)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            (
                                                "id",
                                                serde_json::Value::String((&(item).id).to_string()),
                                            ),
                                            (
                                                "flow_run_id",
                                                serde_json::Value::String(
                                                    (&(item).flow_run_id).to_string(),
                                                ),
                                            ),
                                            (
                                                "node_run_id",
                                                match (&(item).node_run_id).as_ref() {
                                                    Some(item) => serde_json::Value::String(
                                                        (item).to_string(),
                                                    ),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            ("sequence", serde_json::json!(*(&(item).sequence))),
                                            ("event_type", mp::text(&(item).event_type)?),
                                            ("payload", mp::json_summary(&(item).payload)),
                                            (
                                                "created_at",
                                                serde_json::json!(
                                                    (&(item).created_at).unix_timestamp()
                                                ),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("stitched_trace", {
                            if (&(_field_0).stitched_trace).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array((&(_field_0).stitched_trace).iter().map(|item| Some(mp::object_value(&[("source_flow_run",mp::object_value(&[("id",serde_json::Value::String((&(&(item).source_flow_run).id).to_string())), ("application_id",serde_json::Value::String((&(&(item).source_flow_run).application_id).to_string())), ("flow_id",serde_json::Value::String((&(&(item).source_flow_run).flow_id).to_string())), ("draft_id",serde_json::Value::String((&(&(item).source_flow_run).draft_id).to_string())), ("compiled_plan_id",match (&(&(item).source_flow_run).compiled_plan_id).as_ref() { Some(item) => serde_json::Value::String((item).to_string()), None => serde_json::Value::Null }), ("flow_schema_version",mp::text(&(&(item).source_flow_run).flow_schema_version)?), ("document_hash",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).source_flow_run).document_hash).len()))])), ("run_mode",match &(&(item).source_flow_run).run_mode {domain::orchestration::FlowRunMode::DebugNodePreview => mp::object_value(&[("variant",serde_json::Value::String("DebugNodePreview".to_owned()))]), domain::orchestration::FlowRunMode::DebugFlowRun => mp::object_value(&[("variant",serde_json::Value::String("DebugFlowRun".to_owned()))]), domain::orchestration::FlowRunMode::PublishedApiRun => mp::object_value(&[("variant",serde_json::Value::String("PublishedApiRun".to_owned()))]), domain::orchestration::FlowRunMode::AssistantExecution => mp::object_value(&[("variant",serde_json::Value::String("AssistantExecution".to_owned()))]), domain::orchestration::FlowRunMode::WorkflowHttpRun => mp::object_value(&[("variant",serde_json::Value::String("WorkflowHttpRun".to_owned()))]), domain::orchestration::FlowRunMode::WorkflowScheduleRun => mp::object_value(&[("variant",serde_json::Value::String("WorkflowScheduleRun".to_owned()))])}), ("target_node_id",match (&(&(item).source_flow_run).target_node_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("title",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).source_flow_run).title).len()))])), ("status",match &(&(item).source_flow_run).status {domain::orchestration::FlowRunStatus::Queued => mp::object_value(&[("variant",serde_json::Value::String("Queued".to_owned()))]), domain::orchestration::FlowRunStatus::Running => mp::object_value(&[("variant",serde_json::Value::String("Running".to_owned()))]), domain::orchestration::FlowRunStatus::WaitingCallback => mp::object_value(&[("variant",serde_json::Value::String("WaitingCallback".to_owned()))]), domain::orchestration::FlowRunStatus::WaitingHuman => mp::object_value(&[("variant",serde_json::Value::String("WaitingHuman".to_owned()))]), domain::orchestration::FlowRunStatus::Paused => mp::object_value(&[("variant",serde_json::Value::String("Paused".to_owned()))]), domain::orchestration::FlowRunStatus::Succeeded => mp::object_value(&[("variant",serde_json::Value::String("Succeeded".to_owned()))]), domain::orchestration::FlowRunStatus::Incomplete => mp::object_value(&[("variant",serde_json::Value::String("Incomplete".to_owned()))]), domain::orchestration::FlowRunStatus::Failed => mp::object_value(&[("variant",serde_json::Value::String("Failed".to_owned()))]), domain::orchestration::FlowRunStatus::Cancelled => mp::object_value(&[("variant",serde_json::Value::String("Cancelled".to_owned()))])}), ("input_payload",mp::json_summary(&(&(item).source_flow_run).input_payload)), ("output_payload",mp::json_summary(&(&(item).source_flow_run).output_payload)), ("error_payload",match (&(&(item).source_flow_run).error_payload).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("created_by",serde_json::Value::String((&(&(item).source_flow_run).created_by).to_string())), ("publication_version_id",match (&(&(item).source_flow_run).publication_version_id).as_ref() { Some(item) => serde_json::Value::String((item).to_string()), None => serde_json::Value::Null }), ("external_user",match (&(&(item).source_flow_run).external_user).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("external_conversation_id",match (&(&(item).source_flow_run).external_conversation_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("external_trace_id",match (&(&(item).source_flow_run).external_trace_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("compatibility_mode",match (&(&(item).source_flow_run).compatibility_mode).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("idempotency_key",match (&(&(item).source_flow_run).idempotency_key).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("started_at",serde_json::json!((&(&(item).source_flow_run).started_at).unix_timestamp())), ("finished_at",match (&(&(item).source_flow_run).finished_at).as_ref() { Some(item) => serde_json::json!((item).unix_timestamp()), None => serde_json::Value::Null }), ("created_at",serde_json::json!((&(&(item).source_flow_run).created_at).unix_timestamp())), ("updated_at",serde_json::json!((&(&(item).source_flow_run).updated_at).unix_timestamp()))])), ("node_runs",mp::object_value(&[("item_count",serde_json::json!((&(item).node_runs).len()))])), ("callback_tasks",mp::object_value(&[("item_count",serde_json::json!((&(item).callback_tasks).len()))])), ("events",mp::object_value(&[("item_count",serde_json::json!((&(item).events).len()))])), ("runtime_events",mp::object_value(&[("item_count",serde_json::json!((&(item).runtime_events).len()))]))]))).collect::<Option<Vec<_>>>()?)
                        }),
                        ("subagent_traces", {
                            if (&(_field_0).subagent_traces).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array((&(_field_0).subagent_traces).iter().map(|item| Some(mp::object_value(&[("parent_tool_call_id",mp::text(&(item).parent_tool_call_id)?), ("parent_callback_task_id",serde_json::Value::String((&(item).parent_callback_task_id).to_string())), ("source_flow_run",mp::object_value(&[("id",serde_json::Value::String((&(&(item).source_flow_run).id).to_string())), ("application_id",serde_json::Value::String((&(&(item).source_flow_run).application_id).to_string())), ("flow_id",serde_json::Value::String((&(&(item).source_flow_run).flow_id).to_string())), ("draft_id",serde_json::Value::String((&(&(item).source_flow_run).draft_id).to_string())), ("compiled_plan_id",match (&(&(item).source_flow_run).compiled_plan_id).as_ref() { Some(item) => serde_json::Value::String((item).to_string()), None => serde_json::Value::Null }), ("flow_schema_version",mp::text(&(&(item).source_flow_run).flow_schema_version)?), ("document_hash",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).source_flow_run).document_hash).len()))])), ("run_mode",match &(&(item).source_flow_run).run_mode {domain::orchestration::FlowRunMode::DebugNodePreview => mp::object_value(&[("variant",serde_json::Value::String("DebugNodePreview".to_owned()))]), domain::orchestration::FlowRunMode::DebugFlowRun => mp::object_value(&[("variant",serde_json::Value::String("DebugFlowRun".to_owned()))]), domain::orchestration::FlowRunMode::PublishedApiRun => mp::object_value(&[("variant",serde_json::Value::String("PublishedApiRun".to_owned()))]), domain::orchestration::FlowRunMode::AssistantExecution => mp::object_value(&[("variant",serde_json::Value::String("AssistantExecution".to_owned()))]), domain::orchestration::FlowRunMode::WorkflowHttpRun => mp::object_value(&[("variant",serde_json::Value::String("WorkflowHttpRun".to_owned()))]), domain::orchestration::FlowRunMode::WorkflowScheduleRun => mp::object_value(&[("variant",serde_json::Value::String("WorkflowScheduleRun".to_owned()))])}), ("target_node_id",match (&(&(item).source_flow_run).target_node_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("title",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).source_flow_run).title).len()))])), ("status",match &(&(item).source_flow_run).status {domain::orchestration::FlowRunStatus::Queued => mp::object_value(&[("variant",serde_json::Value::String("Queued".to_owned()))]), domain::orchestration::FlowRunStatus::Running => mp::object_value(&[("variant",serde_json::Value::String("Running".to_owned()))]), domain::orchestration::FlowRunStatus::WaitingCallback => mp::object_value(&[("variant",serde_json::Value::String("WaitingCallback".to_owned()))]), domain::orchestration::FlowRunStatus::WaitingHuman => mp::object_value(&[("variant",serde_json::Value::String("WaitingHuman".to_owned()))]), domain::orchestration::FlowRunStatus::Paused => mp::object_value(&[("variant",serde_json::Value::String("Paused".to_owned()))]), domain::orchestration::FlowRunStatus::Succeeded => mp::object_value(&[("variant",serde_json::Value::String("Succeeded".to_owned()))]), domain::orchestration::FlowRunStatus::Incomplete => mp::object_value(&[("variant",serde_json::Value::String("Incomplete".to_owned()))]), domain::orchestration::FlowRunStatus::Failed => mp::object_value(&[("variant",serde_json::Value::String("Failed".to_owned()))]), domain::orchestration::FlowRunStatus::Cancelled => mp::object_value(&[("variant",serde_json::Value::String("Cancelled".to_owned()))])}), ("input_payload",mp::json_summary(&(&(item).source_flow_run).input_payload)), ("output_payload",mp::json_summary(&(&(item).source_flow_run).output_payload)), ("error_payload",match (&(&(item).source_flow_run).error_payload).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("created_by",serde_json::Value::String((&(&(item).source_flow_run).created_by).to_string())), ("publication_version_id",match (&(&(item).source_flow_run).publication_version_id).as_ref() { Some(item) => serde_json::Value::String((item).to_string()), None => serde_json::Value::Null }), ("external_user",match (&(&(item).source_flow_run).external_user).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("external_conversation_id",match (&(&(item).source_flow_run).external_conversation_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("external_trace_id",match (&(&(item).source_flow_run).external_trace_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("compatibility_mode",match (&(&(item).source_flow_run).compatibility_mode).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("idempotency_key",match (&(&(item).source_flow_run).idempotency_key).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("started_at",serde_json::json!((&(&(item).source_flow_run).started_at).unix_timestamp())), ("finished_at",match (&(&(item).source_flow_run).finished_at).as_ref() { Some(item) => serde_json::json!((item).unix_timestamp()), None => serde_json::Value::Null }), ("created_at",serde_json::json!((&(&(item).source_flow_run).created_at).unix_timestamp())), ("updated_at",serde_json::json!((&(&(item).source_flow_run).updated_at).unix_timestamp()))])), ("node_runs",mp::object_value(&[("item_count",serde_json::json!((&(item).node_runs).len()))])), ("callback_tasks",mp::object_value(&[("item_count",serde_json::json!((&(item).callback_tasks).len()))])), ("events",mp::object_value(&[("item_count",serde_json::json!((&(item).events).len()))])), ("runtime_events",mp::object_value(&[("item_count",serde_json::json!((&(item).runtime_events).len()))]))]))).collect::<Option<Vec<_>>>()?)
                        }),
                    ]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "workflow-extension-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct WorkflowExtensionTargetError(pub(crate) NativeApiError);

impl InterfaceContract for WorkflowExtensionTargetError {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "0",
            mp::object_schema(&[
                (
                    "status",
                    serde_json::json!({"type":"integer","minimum":100,"maximum":599}),
                ),
                ("code", mp::text_schema()),
                (
                    "message",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "0",
            mp::object_value(&[
                ("status", serde_json::json!((&(&(self).0).status).as_u16())),
                ("code", mp::text(&(&(self).0).code)?),
                (
                    "message",
                    mp::object_value(&[(
                        "byte_count",
                        serde_json::json!((&(&(self).0).message).len()),
                    )]),
                ),
            ]),
        )]))
    }

    const CONTRACT_ID: &'static str = "workflow-extension-error";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) type WorkflowExtensionFuture<'a> = Pin<
    Box<
        dyn Future<Output = Result<WorkflowExtensionOutput, WorkflowExtensionTargetError>>
            + Send
            + 'a,
    >,
>;

pub(crate) trait WorkflowExtensionPort: Send + Sync + 'static {
    fn invoke<'a>(
        &'a self,
        actor: &'a domain::ActorContext,
        principal: WorkflowHttpPrincipal,
        input: WorkflowExtensionInput,
    ) -> WorkflowExtensionFuture<'a>;
}

struct WorkflowExtensionHandler {
    port: Arc<dyn WorkflowExtensionPort>,
}

impl
    InterfaceHandler<
        WorkflowExtensionInput,
        WorkflowExtensionOutput,
        WorkflowExtensionTargetError,
        UserPrincipal,
    > for WorkflowExtensionHandler
{
    fn invoke(
        &self,
        context: InterfaceHandlerContext<UserPrincipal>,
        input: WorkflowExtensionInput,
    ) -> InterfaceHandlerFuture<WorkflowExtensionOutput, WorkflowExtensionTargetError> {
        let port = Arc::clone(&self.port);
        let actor = context.principal().actor().clone();
        let principal = match context.principal().credential_kind() {
            UserCredentialKind::UserApiKey { api_key_id } => {
                WorkflowHttpPrincipal::UserApiKey { api_key_id }
            }
            UserCredentialKind::CookieSession | UserCredentialKind::ServerDelegation => {
                WorkflowHttpPrincipal::User
            }
        };
        Box::pin(async move {
            port.invoke(&actor, principal, input)
                .await
                .map_err(|error| InterfaceTargetFailure::new("workflow_extension", error))
        })
    }
}

pub(crate) struct WorkflowExtensionAuthorization;

impl InterfaceAuthorizationPort<UserPrincipal> for WorkflowExtensionAuthorization {
    fn adapter_reference(&self) -> AuthorizationAdapterReference {
        AuthorizationAdapterReference::new("api-server.workflow-extension")
            .expect("static adapter is valid")
    }

    fn authorize(
        &self,
        _request: InterfaceAuthorizationRequest<UserPrincipal>,
    ) -> InterfaceAuthorizationFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}

pub(crate) fn compile_registry(
    port: Arc<dyn WorkflowExtensionPort>,
) -> Result<Arc<CompiledInterfaceRegistry>, interface_runtime::RegistryCompilationError> {
    let interface_id = InterfaceId::new(INTERFACE_ID).expect("static interface id is valid");
    let identity = InterfaceIdentity::new(
        interface_id.clone(),
        InterfaceVersion::new("1").expect("static interface version is valid"),
    );
    let contracts = InterfaceContracts::unary(
        contract::<WorkflowExtensionInput>(),
        contract::<WorkflowExtensionOutput>(),
        contract::<WorkflowExtensionTargetError>(),
    );
    let operation = AuthorizationOperation::new(OPERATION).expect("static operation is valid");
    let owner =
        InterfaceOwner::new("api-server.workflow-extension").expect("static owner is valid");
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new("graph:workflow-extension-v1").expect("static graph is valid"),
        [operation.clone()],
        [owner.clone()],
    );
    compiler.register_definition(InterfaceDefinition::new(
        identity.clone(),
        contracts.clone(),
        InterfaceAccess::new(
            interface_runtime::PrincipalProfile::User,
            InterfaceAuthenticationPolicy::Authenticated,
            operation,
            InterfaceScope::Workspace,
        ),
        InterfaceExecution::new(
            InterfaceExecutionMode::Unary,
            HandlerReference::new(HANDLER).expect("static handler is valid"),
            TargetReference::new("control-plane.workflow-extension-run")
                .expect("static target is valid"),
        ),
        InterfaceAuditPolicy::Mutating,
        InterfaceErrorPolicy::TypedTarget,
        InterfaceLifecycle::BootSnapshot,
        owner,
    ))?;
    compiler.register_authentication_adapter(
        &interface_id,
        1,
        interface_runtime::InterfaceExtensionRegistration::new(
            interface_runtime::PluginIdentity::new("api-server.console-authentication")
                .expect("static plugin is valid"),
            interface_runtime::InterfaceExtensionTier::BuiltIn,
            interface_runtime::InterfaceExtensionPoint::AuthenticationAdapter,
            interface_runtime::InterfaceExtensionPermission::Authenticate,
            InterfaceScope::Workspace,
            interface_runtime::InterfaceExtensionIsolation::TrustedInProcess,
            [],
        )
        .expect("built-in authentication registration is valid"),
        interface_runtime::ActivatedAuthenticationAdapter::new(
            interface_runtime::PluginIdentity::new("api-server.console-authentication")
                .expect("static plugin is valid"),
            interface_runtime::InterfaceExtensionTier::BuiltIn,
            AuthenticationAdapterReference::new("api-server.console.require-session")
                .expect("static adapter is valid"),
            interface_runtime::AuthenticationActivationIdentity::new(
                "api-server.console.require-session.activation.v1",
            )
            .expect("static activation is valid"),
            interface_runtime::PrincipalProfile::User,
        ),
    )?;
    compiler.register_binding(
        ProtocolBinding::new(
            BindingId::new(BINDING_ID).expect("static binding is valid"),
            identity,
            contracts,
            ProtocolProjection::http(
                RouteIdentity::new("ANY", "/api/ex/*slug").expect("static route is valid"),
            ),
        ),
        InvocationAdapterPlan::new(
            AuthenticationAdapterReference::new("api-server.console.require-session")
                .expect("static adapter is valid"),
            AuthorizationAdapterReference::new("api-server.workflow-extension")
                .expect("static adapter is valid"),
            None,
        ),
    )?;
    compiler.bind_handler::<WorkflowExtensionInput, WorkflowExtensionOutput, WorkflowExtensionTargetError, UserPrincipal>(
        &interface_id,
        HandlerReference::new(HANDLER).expect("static handler is valid"),
        Arc::new(WorkflowExtensionHandler { port }),
    )?;
    compiler.compile()
}

fn contract<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("static interface contract is valid")
}
