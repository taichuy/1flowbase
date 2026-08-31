use std::{future::Future, pin::Pin, sync::Arc};

use control_plane::{
    application::ApplicationService,
    errors::ControlPlaneError,
    orchestration_runtime::{
        debug_artifacts::{
            build_runtime_debug_artifact_object_path, build_runtime_debug_artifact_preview,
            inline_budget_for_kind, RUNTIME_DEBUG_ARTIFACT_CONTENT_TYPE_JSON,
            RUNTIME_DEBUG_ARTIFACT_RETENTION_ACTIVE,
        },
        trace_projection::{
            build_application_run_trace_projection, merge_trace_node_run_detail,
            projection_status_needs_lazy_rebuild, APPLICATION_RUN_TRACE_PROJECTION_VERSION,
        },
    },
    ports::{
        CreateRuntimeDebugArtifactInput, FileManagementRepository, OrchestrationRuntimeRepository,
    },
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde_json::{Map, Value};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::*;
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(crate) enum ApplicationRuntimeTracePayloadsInput {
    GetNodeContent {
        application_id: Uuid,
        run_id: Uuid,
        trace_node_id: String,
        raw_query: Option<String>,
    },
    GetNodeDetail {
        application_id: Uuid,
        run_id: Uuid,
        trace_node_id: String,
        detail_ref_id: String,
        raw_query: Option<String>,
    },
    GetToolCallbackContent {
        application_id: Uuid,
        run_id: Uuid,
        trace_node_id: String,
        tool_call_id: String,
    },
}

impl InterfaceContract for ApplicationRuntimeTracePayloadsInput {
    const CONTRACT_ID: &'static str = "console-application-runtime-trace-payloads-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum ApplicationRuntimeTracePayloadsOutput {
    NodeContent(ApplicationRunTraceNodeContentResponse),
    NodeDetail(ApplicationRunTraceNodeDetailResponse),
    ToolCallbackContent(ApplicationRunTraceToolCallbackContentResponse),
}

impl InterfaceContract for ApplicationRuntimeTracePayloadsOutput {
    const CONTRACT_ID: &'static str = "console-application-runtime-trace-payloads-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct TracePayloadArtifactWriter {
    store: MainDurableStore,
    storage: domain::FileStorageRecord,
    driver: Arc<dyn storage_object::FileStorageDriver>,
}

impl TracePayloadArtifactWriter {
    async fn new(
        store: MainDurableStore,
        file_storage_registry: &storage_object::FileStorageDriverRegistry,
    ) -> Result<Self, ApiError> {
        let storage = <_ as FileManagementRepository>::get_default_file_storage(&store)
            .await?
            .ok_or(ControlPlaneError::Conflict("file_storage_default_missing"))?;
        if !storage.enabled {
            return Err(ControlPlaneError::Conflict("file_storage_disabled").into());
        }
        let driver = file_storage_registry
            .get(&storage.driver_type)
            .ok_or(ControlPlaneError::Conflict("storage_driver_not_registered"))?;
        Ok(Self {
            store,
            storage,
            driver,
        })
    }

    async fn offload_value(
        &self,
        scope: &TracePayloadArtifactScope,
        artifact_kind: &str,
        value: Value,
    ) -> Result<(Value, bool), ApiError> {
        let artifact_id = Uuid::now_v7();
        let Some(preview) = build_runtime_debug_artifact_preview(
            artifact_id,
            &value,
            inline_budget_for_kind(artifact_kind),
        )?
        else {
            return Ok((value, false));
        };
        let storage_ref = build_runtime_debug_artifact_object_path(
            scope.workspace_id,
            scope.application_id,
            Some(scope.flow_run_id),
            preview.artifact_id,
        );
        self.driver
            .put_object(storage_object::FileStoragePutInput {
                config_json: &self.storage.config_json,
                object_path: &storage_ref,
                content_type: Some(RUNTIME_DEBUG_ARTIFACT_CONTENT_TYPE_JSON),
                bytes: &preview.full_bytes,
            })
            .await?;
        <_ as OrchestrationRuntimeRepository>::create_runtime_debug_artifact(
            &self.store,
            &CreateRuntimeDebugArtifactInput {
                artifact_id: preview.artifact_id,
                workspace_id: scope.workspace_id,
                application_id: scope.application_id,
                flow_run_id: Some(scope.flow_run_id),
                node_run_id: scope.node_run_id,
                run_event_id: None,
                artifact_kind: artifact_kind.to_string(),
                content_type: RUNTIME_DEBUG_ARTIFACT_CONTENT_TYPE_JSON.to_string(),
                original_size_bytes: preview.original_size_bytes,
                preview_size_bytes: preview.preview_size_bytes,
                storage_id: self.storage.id,
                storage_ref,
                retention_state: RUNTIME_DEBUG_ARTIFACT_RETENTION_ACTIVE.to_string(),
            },
        )
        .await?;
        Ok((preview.preview_value, true))
    }

    fn offload_fields<'a>(
        &'a self,
        scope: &'a TracePayloadArtifactScope,
        artifact_kind: &'a str,
        value: Value,
        field_path: Vec<String>,
        preview: &'a TracePayloadPreview,
    ) -> Pin<Box<dyn Future<Output = Result<(Value, bool), ApiError>> + Send + 'a>> {
        Box::pin(async move {
            if is_runtime_debug_artifact_payload(&value)
                || keep_runtime_field_inline(&field_path)
                || !preview.descends_into(&field_path)
            {
                return Ok((value, false));
            }
            let preview_value = preview.previews(&field_path)
                && (!matches!(preview, TracePayloadPreview::Auto)
                    || matches!(value, Value::Array(_) | Value::String(_)));
            if preview_value {
                let (payload, changed) = self.offload_value(scope, artifact_kind, value).await?;
                return Ok((
                    if changed {
                        with_debug_artifact_field_path(payload, &field_path)
                    } else {
                        payload
                    },
                    changed,
                ));
            }
            match value {
                Value::Object(object) => {
                    let mut changed = false;
                    let mut next = Map::with_capacity(object.len());
                    for (key, child) in object {
                        let mut child_path = field_path.clone();
                        child_path.push(key.clone());
                        let (child, child_changed) = self
                            .offload_fields(scope, artifact_kind, child, child_path, preview)
                            .await?;
                        changed |= child_changed;
                        next.insert(key, child);
                    }
                    Ok((Value::Object(next), changed))
                }
                value => Ok((value, false)),
            }
        })
    }
}

struct TracePayloadArtifactScope {
    workspace_id: Uuid,
    application_id: Uuid,
    flow_run_id: Uuid,
    node_run_id: Option<Uuid>,
}

enum TracePayloadPreview {
    Auto,
    Fields(Vec<Vec<String>>),
}

impl TracePayloadPreview {
    fn descends_into(&self, path: &[String]) -> bool {
        match self {
            Self::Auto => true,
            Self::Fields(paths) => paths.iter().any(|candidate| candidate.starts_with(path)),
        }
    }

    fn previews(&self, path: &[String]) -> bool {
        match self {
            Self::Auto => {
                matches!(
                    path.last().map(String::as_str),
                    Some("payload")
                        | Some("input_payload")
                        | Some("output_payload")
                        | Some("error_payload")
                        | Some("metrics_payload")
                        | Some("debug_payload")
                ) || matches!(path.last().map(String::as_str), Some(_)) && path.len() > 0
            }
            Self::Fields(paths) => paths.iter().any(|candidate| candidate.as_slice() == path),
        }
    }
}

struct ApplicationRuntimeTracePayloadsAdapter {
    store: MainDurableStore,
    file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
}

pub(crate) fn trace_payloads_port(
    store: MainDurableStore,
    file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
) -> Arc<
    dyn ConsoleInterfacePort<
        ApplicationRuntimeTracePayloadsInput,
        ApplicationRuntimeTracePayloadsOutput,
    >,
> {
    Arc::new(ApplicationRuntimeTracePayloadsAdapter {
        store,
        file_storage_registry,
    })
}

impl ApplicationRuntimeTracePayloadsAdapter {
    async fn visible_application(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
    ) -> Result<(), ApiError> {
        ApplicationService::new(self.store.for_actor(actor.clone()))
            .get_application(actor.user_id, application_id)
            .await?;
        Ok(())
    }

    async fn projection_status(
        &self,
        application_id: Uuid,
        run_id: Uuid,
    ) -> Result<domain::ApplicationRunTraceProjectionStatusRecord, ApiError> {
        let status =
            <_ as OrchestrationRuntimeRepository>::get_application_run_trace_projection_status(
                &self.store,
                run_id,
                APPLICATION_RUN_TRACE_PROJECTION_VERSION,
            )
            .await?;
        if let Some(status) = status.as_ref() {
            if matches!(
                status.status,
                domain::ApplicationRunTraceProjectionStatus::Pending
                    | domain::ApplicationRunTraceProjectionStatus::Running
                    | domain::ApplicationRunTraceProjectionStatus::Failed
            ) {
                return Ok(status.clone());
            }
        }
        let watermark = <_ as OrchestrationRuntimeRepository>::get_application_run_trace_projection_source_watermark(&self.store, application_id, run_id).await?.ok_or(ControlPlaneError::NotFound("flow_run"))?;
        if !projection_status_needs_lazy_rebuild(status.as_ref(), &watermark) {
            return status
                .ok_or_else(|| ControlPlaneError::Conflict("trace_projection_status").into());
        }
        let source =
            <_ as OrchestrationRuntimeRepository>::get_application_run_trace_projection_source(
                &self.store,
                application_id,
                run_id,
            )
            .await?
            .ok_or(ControlPlaneError::NotFound("flow_run"))?;
        let events =
            <_ as OrchestrationRuntimeRepository>::list_runtime_events(&self.store, run_id, 0)
                .await?;
        let projection = build_application_run_trace_projection(
            &enrich_application_run_detail_visible_internal_llm_route_traces(source, &events),
        )?;
        <_ as OrchestrationRuntimeRepository>::replace_application_run_trace_projection(
            &self.store,
            &projection,
        )
        .await?;
        <_ as OrchestrationRuntimeRepository>::get_application_run_trace_projection_status(
            &self.store,
            run_id,
            APPLICATION_RUN_TRACE_PROJECTION_VERSION,
        )
        .await?
        .ok_or_else(|| ControlPlaneError::Conflict("trace_projection_status").into())
    }

    async fn preview_content(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        run_id: Uuid,
        mut content: domain::ApplicationRunTraceNodeContentRecord,
        preview: TracePayloadPreview,
    ) -> Result<domain::ApplicationRunTraceNodeContentRecord, ApiError> {
        if !matches!(
            content.content_kind.as_str(),
            "tool_callback" | "fusion" | "route" | "branch" | "callback_task"
        ) {
            return Ok(content);
        }
        let writer = TracePayloadArtifactWriter::new(
            self.store.clone(),
            self.file_storage_registry.as_ref(),
        )
        .await?;
        let (payload, changed) = writer
            .offload_fields(
                &TracePayloadArtifactScope {
                    workspace_id,
                    application_id,
                    flow_run_id: run_id,
                    node_run_id: None,
                },
                "trace_node_content_payload",
                content.payload.clone(),
                Vec::new(),
                &preview,
            )
            .await?;
        if changed {
            content.payload = payload;
        }
        Ok(content)
    }

    async fn preview_node_run(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        run_id: Uuid,
        mut node_run: domain::NodeRunRecord,
        preview: TracePayloadPreview,
    ) -> Result<domain::NodeRunRecord, ApiError> {
        let writer = TracePayloadArtifactWriter::new(
            self.store.clone(),
            self.file_storage_registry.as_ref(),
        )
        .await?;
        let scope = TracePayloadArtifactScope {
            workspace_id,
            application_id,
            flow_run_id: run_id,
            node_run_id: Some(node_run.id),
        };
        let path_for = |field: &str| match preview {
            TracePayloadPreview::Auto => Vec::new(),
            TracePayloadPreview::Fields(_) => vec!["node_run".into(), field.into()],
        };
        let (input, input_changed) = writer
            .offload_fields(
                &scope,
                "node_input_payload",
                node_run.input_payload.clone(),
                path_for("input_payload"),
                &preview,
            )
            .await?;
        let (output, output_changed) = writer
            .offload_fields(
                &scope,
                "node_output_payload",
                node_run.output_payload.clone(),
                path_for("output_payload"),
                &preview,
            )
            .await?;
        let (metrics, metrics_changed) = writer
            .offload_fields(
                &scope,
                "node_metrics_payload",
                node_run.metrics_payload.clone(),
                path_for("metrics_payload"),
                &preview,
            )
            .await?;
        let (debug, debug_changed) = writer
            .offload_fields(
                &scope,
                "node_debug_payload",
                node_run.debug_payload.clone(),
                path_for("debug_payload"),
                &preview,
            )
            .await?;
        let (error, error_changed) = match node_run.error_payload.clone() {
            Some(value) => {
                let (value, changed) = writer
                    .offload_fields(
                        &scope,
                        "node_error_payload",
                        value,
                        path_for("error_payload"),
                        &preview,
                    )
                    .await?;
                (Some(value), changed)
            }
            None => (None, false),
        };
        if input_changed || output_changed || metrics_changed || debug_changed || error_changed {
            node_run.input_payload = input;
            node_run.output_payload = output;
            node_run.metrics_payload = metrics;
            node_run.debug_payload = debug;
            node_run.error_payload = error;
        }
        Ok(node_run)
    }

    async fn node_content(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_id: Uuid,
        trace_node_id: String,
        raw_query: Option<String>,
    ) -> Result<ApplicationRunTraceNodeContentResponse, ApiError> {
        self.visible_application(actor, application_id).await?;
        let status = self.projection_status(application_id, run_id).await?;
        let projection_status = to_trace_projection_status_response(&status);
        let trace_node_uuid = parse_trace_projection_node_id(&trace_node_id)?;
        if !projection_is_succeeded(&status) {
            return Ok(ApplicationRunTraceNodeContentResponse {
                trace_node_id,
                node_kind: "trace_projection".into(),
                projection_status,
                content_kind: "trace_projection".into(),
                source_refs: Value::Array(Vec::new()),
                detail_refs: Value::Array(Vec::new()),
                payload: serde_json::json!({}),
            });
        }
        let node = <_ as OrchestrationRuntimeRepository>::get_application_run_trace_node(
            &self.store,
            run_id,
            trace_node_uuid,
        )
        .await?
        .ok_or(ControlPlaneError::NotFound("trace_node"))?;
        let content =
            <_ as OrchestrationRuntimeRepository>::get_application_run_trace_node_content(
                &self.store,
                run_id,
                trace_node_uuid,
            )
            .await?
            .ok_or(ControlPlaneError::NotFound("trace_node_content"))?;
        let content = match parse_preview(raw_query.as_deref()) {
            Some(preview) => {
                self.preview_content(
                    actor.current_workspace_id,
                    application_id,
                    run_id,
                    content,
                    preview,
                )
                .await?
            }
            None => content,
        };
        trace_projection_node_content_response(node, content, projection_status)
    }

    async fn node_detail(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_id: Uuid,
        trace_node_id: String,
        detail_ref_id: String,
        raw_query: Option<String>,
    ) -> Result<ApplicationRunTraceNodeDetailResponse, ApiError> {
        self.visible_application(actor, application_id).await?;
        let status = self.projection_status(application_id, run_id).await?;
        let projection_status = to_trace_projection_status_response(&status);
        let trace_node_uuid = parse_trace_projection_node_id(&trace_node_id)?;
        if !projection_is_succeeded(&status) {
            return Ok(ApplicationRunTraceNodeDetailResponse {
                trace_node_id,
                node_kind: "trace_projection".into(),
                projection_status,
                detail_ref_id,
                detail_kind: "trace_projection".into(),
                source_refs: Value::Array(Vec::new()),
                payload: serde_json::json!({}),
            });
        }
        let node = <_ as OrchestrationRuntimeRepository>::get_application_run_trace_node(
            &self.store,
            run_id,
            trace_node_uuid,
        )
        .await?
        .ok_or(ControlPlaneError::NotFound("trace_node"))?;
        let content =
            <_ as OrchestrationRuntimeRepository>::get_application_run_trace_node_content(
                &self.store,
                run_id,
                trace_node_uuid,
            )
            .await?
            .ok_or(ControlPlaneError::NotFound("trace_node_content"))?;
        let detail_ref = trace_node_content_detail_ref(&content.payload, &detail_ref_id)
            .ok_or(ControlPlaneError::NotFound("trace_node_detail_ref"))?;
        let detail_kind = detail_ref
            .get("detail_kind")
            .and_then(Value::as_str)
            .ok_or(ControlPlaneError::Conflict("trace_node_detail_ref"))?
            .to_string();
        let detail_run_id =
            trace_node_content_source_flow_run_id(&content.payload)?.unwrap_or(run_id);
        let payload = match detail_kind.as_str() {
            "node_run" => {
                let node_runs = <_ as OrchestrationRuntimeRepository>::list_application_run_trace_node_run_details(&self.store, detail_run_id, trace_node_content_node_run_ids(&content.payload)?).await?;
                let node_run = merge_trace_node_run_detail(&node_runs)
                    .ok_or(ControlPlaneError::NotFound("node_run"))?;
                let node_run = match parse_preview(raw_query.as_deref()) {
                    Some(preview) => {
                        self.preview_node_run(
                            actor.current_workspace_id,
                            application_id,
                            detail_run_id,
                            node_run,
                            preview,
                        )
                        .await?
                    }
                    None => node_run,
                };
                trace_node_run_detail_payload(node_run)
            }
            "checkpoints" => {
                serde_json::json!({ "checkpoints": <_ as OrchestrationRuntimeRepository>::list_application_run_trace_checkpoints(&self.store, application_id, detail_run_id, trace_node_content_node_run_ids(&content.payload)?).await?.into_iter().map(to_checkpoint_response).collect::<Vec<_>>() })
            }
            "events" => {
                serde_json::json!({ "events": <_ as OrchestrationRuntimeRepository>::list_application_run_trace_events(&self.store, application_id, detail_run_id, trace_node_content_node_run_ids(&content.payload)?).await?.into_iter().map(to_run_event_response).collect::<Vec<_>>() })
            }
            _ => return Err(ControlPlaneError::NotFound("trace_node_detail_ref").into()),
        };
        Ok(ApplicationRunTraceNodeDetailResponse {
            trace_node_id,
            node_kind: node.node_kind,
            projection_status,
            detail_ref_id,
            detail_kind,
            source_refs: Value::Array(vec![detail_ref]),
            payload,
        })
    }

    async fn tool_callback_content(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_id: Uuid,
        trace_node_id: String,
        tool_call_id: String,
    ) -> Result<ApplicationRunTraceToolCallbackContentResponse, ApiError> {
        self.visible_application(actor, application_id).await?;
        let status = self.projection_status(application_id, run_id).await?;
        let projection_status = to_trace_projection_status_response(&status);
        let trace_node_uuid = parse_trace_projection_node_id(&trace_node_id)?;
        if !projection_is_succeeded(&status) {
            return Ok(ApplicationRunTraceToolCallbackContentResponse {
                trace_node_id,
                tool_call_id,
                projection_status,
                payload: serde_json::json!({}),
            });
        }
        let owner = <_ as OrchestrationRuntimeRepository>::get_application_run_trace_node(
            &self.store,
            run_id,
            trace_node_uuid,
        )
        .await?
        .ok_or(ControlPlaneError::NotFound("trace_node"))?;
        let tool_node =
            find_trace_projection_tool_callback_node(&self.store, run_id, &owner, &tool_call_id)
                .await?;
        let content =
            <_ as OrchestrationRuntimeRepository>::get_application_run_trace_node_content(
                &self.store,
                run_id,
                tool_node.trace_node_id,
            )
            .await?
            .ok_or(ControlPlaneError::NotFound("trace_node_content"))?;
        Ok(ApplicationRunTraceToolCallbackContentResponse {
            trace_node_id: tool_node.trace_node_id.to_string(),
            tool_call_id,
            projection_status,
            payload: content.payload,
        })
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ApplicationRuntimeTracePayloadsInput,
    ) -> Result<ApplicationRuntimeTracePayloadsOutput, ApiError> {
        let actor = principal.actor();
        match input {
            ApplicationRuntimeTracePayloadsInput::GetNodeContent {
                application_id,
                run_id,
                trace_node_id,
                raw_query,
            } => Ok(ApplicationRuntimeTracePayloadsOutput::NodeContent(
                self.node_content(actor, application_id, run_id, trace_node_id, raw_query)
                    .await?,
            )),
            ApplicationRuntimeTracePayloadsInput::GetNodeDetail {
                application_id,
                run_id,
                trace_node_id,
                detail_ref_id,
                raw_query,
            } => Ok(ApplicationRuntimeTracePayloadsOutput::NodeDetail(
                self.node_detail(
                    actor,
                    application_id,
                    run_id,
                    trace_node_id,
                    detail_ref_id,
                    raw_query,
                )
                .await?,
            )),
            ApplicationRuntimeTracePayloadsInput::GetToolCallbackContent {
                application_id,
                run_id,
                trace_node_id,
                tool_call_id,
            } => Ok(ApplicationRuntimeTracePayloadsOutput::ToolCallbackContent(
                self.tool_callback_content(
                    actor,
                    application_id,
                    run_id,
                    trace_node_id,
                    tool_call_id,
                )
                .await?,
            )),
        }
    }
}

impl
    ConsoleInterfacePort<
        ApplicationRuntimeTracePayloadsInput,
        ApplicationRuntimeTracePayloadsOutput,
    > for ApplicationRuntimeTracePayloadsAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ApplicationRuntimeTracePayloadsInput,
    ) -> ConsoleInterfaceFuture<'a, ApplicationRuntimeTracePayloadsOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

fn is_runtime_debug_artifact_payload(value: &Value) -> bool {
    value
        .get("__runtime_debug_artifact")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}
fn with_debug_artifact_field_path(mut value: Value, path: &[String]) -> Value {
    if let Some(object) = value.as_object_mut() {
        if !path.is_empty() {
            object.insert("artifact_scope".into(), Value::String("field".into()));
            object.insert(
                "field_path".into(),
                Value::Array(path.iter().cloned().map(Value::String).collect()),
            );
        }
    }
    value
}
fn keep_runtime_field_inline(path: &[String]) -> bool {
    path.iter().any(|key| {
        matches!(
            key.as_str(),
            "query"
                | "model"
                | "system"
                | "files"
                | "sys"
                | "env"
                | "visible_internal_llm_tool_trace"
        )
    })
}
fn parse_preview(raw_query: Option<&str>) -> Option<TracePayloadPreview> {
    let raw = raw_query?;
    let mut auto = false;
    let mut fields = Vec::new();
    for (key, value) in form_urlencoded::parse(raw.as_bytes()) {
        match key.as_ref() {
            "artifact_preview" if value.as_ref() == "auto" => auto = true,
            "artifact_preview_field" => {
                let path = value
                    .split('.')
                    .map(str::trim)
                    .filter(|part| !part.is_empty())
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>();
                if !path.is_empty() && !fields.iter().any(|existing| existing == &path) {
                    fields.push(path);
                }
            }
            _ => {}
        }
    }
    if !fields.is_empty() {
        Some(TracePayloadPreview::Fields(fields))
    } else {
        auto.then_some(TracePayloadPreview::Auto)
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration { interface_id: "applications.runtime.trace-node.content.get", binding_id: "http.console.applications.runtime.trace-node.content.get.v1", method: "GET", path: "/api/console/applications/:id/logs/runs/:run_id/trace-tree/nodes/:trace_node_id/content", mutating: false },
    ConsoleInterfaceDeclaration { interface_id: "applications.runtime.trace-node.detail.get", binding_id: "http.console.applications.runtime.trace-node.detail.get.v1", method: "GET", path: "/api/console/applications/:id/logs/runs/:run_id/trace-tree/nodes/:trace_node_id/details/:detail_ref_id", mutating: false },
    ConsoleInterfaceDeclaration { interface_id: "applications.runtime.trace-tool-callback.content.get", binding_id: "http.console.applications.runtime.trace-tool-callback.content.get.v1", method: "GET", path: "/api/console/applications/:id/logs/runs/:run_id/trace-tree/nodes/:trace_node_id/tool-callbacks/:tool_call_id/content", mutating: false },
];

pub(crate) fn compile_registry(
    store: MainDurableStore,
    file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-application-runtime-trace-payloads",
        "graph:console-application-runtime-trace-payloads-v1",
        DECLARATIONS,
        trace_payloads_port(store, file_storage_registry),
    )
}
