use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

use control_plane::{
    errors::ControlPlaneError,
    orchestration_runtime::debug_artifacts::{
        build_runtime_debug_artifact_object_path, build_runtime_debug_artifact_preview,
        inline_budget_for_kind, RUNTIME_DEBUG_ARTIFACT_CONTENT_TYPE_JSON,
        RUNTIME_DEBUG_ARTIFACT_RETENTION_ACTIVE,
    },
    ports::{
        CreateRuntimeDebugArtifactInput, FileManagementRepository, OrchestrationRuntimeRepository,
        UpdateCallbackTaskPayloadsInput, UpdateCheckpointPayloadsInput, UpdateFlowRunPayloadsInput,
        UpdateNodeRunPayloadsInput, UpdateRunEventPayloadInput,
    },
};
use serde_json::{json, Map, Value};
use storage_durable::MainDurableStore;
use uuid::Uuid;

use crate::{app_state::ApiState, error_response::ApiError};

type RuntimeDebugArtifactOffloadFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(Value, bool), ApiError>> + Send + 'a>>;
mod llm_tool_callbacks;
mod payloads;
#[cfg(test)]
mod tests;
mod visible_internal_enrichment;
mod visible_internal_llm_route_traces;

pub(super) use payloads::{
    application_run_model, application_run_query, load_runtime_debug_artifact_json_value,
    load_runtime_debug_artifact_response,
};
use payloads::{
    application_run_system, is_runtime_debug_artifact_payload,
    is_safe_to_persist_debug_artifact_previews, should_keep_runtime_payload_field_inline,
    with_application_run_input_summary, with_debug_artifact_field_path,
};
pub use visible_internal_enrichment::{
    enrich_application_run_detail_visible_internal_llm_route_traces,
    enrich_node_last_run_visible_internal_llm_route_traces,
};
use visible_internal_llm_route_traces::{
    collect_visible_internal_llm_tool_route_traces,
    collect_visible_internal_llm_tool_route_traces_with_branch_node_runs,
    VisibleInternalLlmToolBranchNodeRunPayload,
};

struct RuntimeDebugArtifactScope {
    workspace_id: Uuid,
    application_id: Uuid,
    flow_run_id: Option<Uuid>,
    node_run_id: Option<Uuid>,
    run_event_id: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeDebugArtifactPreviewRequest {
    Auto,
    Fields(Vec<Vec<String>>),
}

impl RuntimeDebugArtifactPreviewRequest {
    fn should_preview_field(&self, field_path: &[String]) -> bool {
        match self {
            Self::Auto => true,
            Self::Fields(field_paths) => field_paths
                .iter()
                .any(|selected_path| selected_path.as_slice() == field_path),
        }
    }

    fn should_descend_into_field(&self, field_path: &[String]) -> bool {
        match self {
            Self::Auto => true,
            Self::Fields(field_paths) => field_paths
                .iter()
                .any(|selected_path| selected_path.starts_with(field_path)),
        }
    }
}

struct RuntimeDebugArtifactWriter {
    state: Arc<ApiState>,
    storage: domain::FileStorageRecord,
    driver: Arc<dyn storage_object::FileStorageDriver>,
    llm_tool_callback_runtime_facts: HashMap<String, LlmToolCallbackRuntimeFacts>,
}

use llm_tool_callbacks::{
    attach_inline_route_traces, collect_llm_tool_callback_runtime_facts,
    collect_llm_tool_callbacks, is_llm_rounds_debug_artifact_missing_tool_index,
    is_llm_rounds_field_path, is_llm_rounds_leaf_field_path, is_tool_calls_field_path,
    with_array_item_count, with_llm_tool_callback_runtime_facts, LlmToolCallbackRuntimeFacts,
};
#[cfg(test)]
use llm_tool_callbacks::{
    execution_status_from_callback_payload, execution_status_from_callback_payload_and_route_trace,
};

pub(super) fn count_llm_tool_callback_trace_items(
    debug_payloads: &[Value],
    callback_tasks: &[domain::CallbackTaskRecord],
) -> usize {
    llm_tool_callbacks::count_llm_tool_callback_trace_items(debug_payloads, callback_tasks)
}

impl RuntimeDebugArtifactWriter {
    async fn new(state: Arc<ApiState>) -> Result<Self, ApiError> {
        let storage =
            <MainDurableStore as FileManagementRepository>::get_default_file_storage(&state.store)
                .await?
                .ok_or(ControlPlaneError::Conflict("file_storage_default_missing"))?;
        if !storage.enabled {
            return Err(ControlPlaneError::Conflict("file_storage_disabled").into());
        }
        let driver = state
            .file_storage_registry
            .get(&storage.driver_type)
            .ok_or(ControlPlaneError::Conflict("storage_driver_not_registered"))?;

        Ok(Self {
            state,
            storage,
            driver,
            llm_tool_callback_runtime_facts: HashMap::new(),
        })
    }

    async fn offload_value(
        &self,
        scope: &RuntimeDebugArtifactScope,
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
            scope.flow_run_id,
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
        <MainDurableStore as OrchestrationRuntimeRepository>::create_runtime_debug_artifact(
            &self.state.store,
            &CreateRuntimeDebugArtifactInput {
                artifact_id: preview.artifact_id,
                workspace_id: scope.workspace_id,
                application_id: scope.application_id,
                flow_run_id: scope.flow_run_id,
                node_run_id: scope.node_run_id,
                run_event_id: scope.run_event_id,
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

    async fn persist_value_artifact(
        &self,
        scope: &RuntimeDebugArtifactScope,
        artifact_kind: &str,
        value: &Value,
    ) -> Result<Uuid, ApiError> {
        let artifact_id = Uuid::now_v7();
        let bytes = serde_json::to_vec(value)?;
        let storage_ref = build_runtime_debug_artifact_object_path(
            scope.workspace_id,
            scope.application_id,
            scope.flow_run_id,
            artifact_id,
        );

        self.driver
            .put_object(storage_object::FileStoragePutInput {
                config_json: &self.storage.config_json,
                object_path: &storage_ref,
                content_type: Some(RUNTIME_DEBUG_ARTIFACT_CONTENT_TYPE_JSON),
                bytes: &bytes,
            })
            .await?;
        <MainDurableStore as OrchestrationRuntimeRepository>::create_runtime_debug_artifact(
            &self.state.store,
            &CreateRuntimeDebugArtifactInput {
                artifact_id,
                workspace_id: scope.workspace_id,
                application_id: scope.application_id,
                flow_run_id: scope.flow_run_id,
                node_run_id: scope.node_run_id,
                run_event_id: scope.run_event_id,
                artifact_kind: artifact_kind.to_string(),
                content_type: RUNTIME_DEBUG_ARTIFACT_CONTENT_TYPE_JSON.to_string(),
                original_size_bytes: bytes.len() as i64,
                preview_size_bytes: 0,
                storage_id: self.storage.id,
                storage_ref,
                retention_state: RUNTIME_DEBUG_ARTIFACT_RETENTION_ACTIVE.to_string(),
            },
        )
        .await?;

        Ok(artifact_id)
    }

    async fn with_llm_tool_callback_index(
        &self,
        scope: &RuntimeDebugArtifactScope,
        mut payload: Value,
        llm_rounds: &Value,
    ) -> Result<Value, ApiError> {
        let Some(object) = payload.as_object() else {
            return Ok(payload);
        };
        if object.contains_key("tool_callbacks") {
            return Ok(payload);
        }

        let mut callbacks =
            collect_llm_tool_callbacks(llm_rounds, &self.llm_tool_callback_runtime_facts);
        attach_inline_route_traces(&mut callbacks, std::slice::from_ref(&payload));
        if callbacks.is_empty() {
            return Ok(payload);
        }

        let mut callback_summaries = Vec::with_capacity(callbacks.len());
        for callback in callbacks {
            let detail_payload = callback.detail_payload();
            let artifact_id = self
                .persist_value_artifact(scope, "node_debug_tool_callback", &detail_payload)
                .await?;

            callback_summaries.push(callback.summary_payload(artifact_id));
        }

        if let Some(object) = payload.as_object_mut() {
            object.insert(
                "tool_callbacks".to_string(),
                Value::Array(callback_summaries),
            );
        }
        Ok(payload)
    }

    async fn with_visible_internal_llm_tool_trace_index(
        &self,
        scope: &RuntimeDebugArtifactScope,
        mut payload: Value,
    ) -> Result<Value, ApiError> {
        let Some(object) = payload.as_object() else {
            return Ok(payload);
        };
        if object.contains_key("visible_internal_llm_tool_trace") {
            return Ok(payload);
        }

        let traces = collect_visible_internal_llm_tool_route_traces(&payload);
        if traces.is_empty() {
            return Ok(payload);
        }

        let mut summaries = Vec::with_capacity(traces.len());
        for trace in traces {
            let artifact_id = self
                .persist_value_artifact(
                    scope,
                    "node_debug_visible_internal_llm_tool_trace",
                    &trace.detail_payload(),
                )
                .await?;
            summaries.push(trace.summary_payload(artifact_id));
        }

        if let Some(object) = payload.as_object_mut() {
            object.insert(
                "visible_internal_llm_tool_trace".to_string(),
                Value::Array(summaries),
            );
        }
        Ok(payload)
    }

    async fn offload_node_debug_payload(
        &self,
        scope: &RuntimeDebugArtifactScope,
        payload: Value,
    ) -> Result<(Value, bool), ApiError> {
        let original_payload = payload.clone();
        let payload = self
            .with_visible_internal_llm_tool_trace_index(scope, payload)
            .await?;
        let trace_changed = payload != original_payload;
        let (payload, fields_changed) = self
            .offload_payload_fields(scope, "node_debug_payload", payload, Vec::new())
            .await?;

        Ok((payload, trace_changed || fields_changed))
    }

    async fn enrich_existing_llm_rounds_preview(
        &self,
        scope: &RuntimeDebugArtifactScope,
        payload: Value,
    ) -> Result<(Value, bool), ApiError> {
        if !is_llm_rounds_debug_artifact_missing_tool_index(&payload) {
            return Ok((payload, false));
        }

        let Some(artifact_id) = payload
            .get("artifact_ref")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
        else {
            return Ok((payload, false));
        };
        let full_llm_rounds = load_runtime_debug_artifact_json_value(
            self.state.clone(),
            scope.workspace_id,
            scope.application_id,
            artifact_id,
        )
        .await?;
        let payload = self
            .with_llm_tool_callback_index(scope, payload, &full_llm_rounds)
            .await?;

        Ok((payload, true))
    }

    fn offload_payload_fields<'a>(
        &'a self,
        scope: &'a RuntimeDebugArtifactScope,
        artifact_kind: &'a str,
        value: Value,
        field_path: Vec<String>,
    ) -> RuntimeDebugArtifactOffloadFuture<'a> {
        Box::pin(async move {
            if is_runtime_debug_artifact_payload(&value) {
                if is_llm_rounds_field_path(&field_path) {
                    return self.enrich_existing_llm_rounds_preview(scope, value).await;
                }

                return Ok((value, false));
            }

            if should_keep_runtime_payload_field_inline(&field_path) {
                return Ok((value, false));
            }

            match value {
                Value::Object(object) => {
                    let mut changed = false;
                    let mut next = Map::with_capacity(object.len());
                    for (key, child) in object {
                        let mut child_path = field_path.clone();
                        child_path.push(key.clone());
                        let (child, child_changed) = self
                            .offload_payload_fields(scope, artifact_kind, child, child_path)
                            .await?;
                        changed |= child_changed;
                        next.insert(key, child);
                    }
                    Ok((Value::Object(next), changed))
                }
                Value::Array(_) | Value::String(_) => {
                    let full_value = value.clone();
                    let (payload, changed) =
                        self.offload_value(scope, artifact_kind, value).await?;
                    let payload = if changed {
                        with_debug_artifact_field_path(payload, &field_path)
                    } else {
                        payload
                    };
                    let payload = if changed && is_tool_calls_field_path(&field_path) {
                        with_array_item_count(payload, &full_value, "tool_call_count")
                    } else {
                        payload
                    };
                    let payload = if changed && is_llm_rounds_field_path(&field_path) {
                        self.with_llm_tool_callback_index(scope, payload, &full_value)
                            .await?
                    } else {
                        payload
                    };
                    if !changed && is_llm_rounds_field_path(&field_path) {
                        let (payload, runtime_facts_changed) = with_llm_tool_callback_runtime_facts(
                            payload,
                            &self.llm_tool_callback_runtime_facts,
                        );
                        return Ok((payload, runtime_facts_changed));
                    }
                    Ok((payload, changed))
                }
                value => Ok((value, false)),
            }
        })
    }

    fn offload_selected_payload_fields<'a>(
        &'a self,
        scope: &'a RuntimeDebugArtifactScope,
        artifact_kind: &'a str,
        value: Value,
        field_path: Vec<String>,
        preview_request: &'a RuntimeDebugArtifactPreviewRequest,
    ) -> RuntimeDebugArtifactOffloadFuture<'a> {
        Box::pin(async move {
            if is_runtime_debug_artifact_payload(&value) {
                if is_llm_rounds_leaf_field_path(&field_path) {
                    return self.enrich_existing_llm_rounds_preview(scope, value).await;
                }

                return Ok((value, false));
            }

            if !preview_request.should_descend_into_field(&field_path) {
                return Ok((value, false));
            }

            if should_keep_runtime_payload_field_inline(&field_path) {
                return Ok((value, false));
            }

            if preview_request.should_preview_field(&field_path) {
                let full_value = value.clone();
                let (payload, changed) = self.offload_value(scope, artifact_kind, value).await?;
                let payload = if changed {
                    with_debug_artifact_field_path(payload, &field_path)
                } else {
                    payload
                };
                let payload = if changed && is_tool_calls_field_path(&field_path) {
                    with_array_item_count(payload, &full_value, "tool_call_count")
                } else {
                    payload
                };
                let payload = if changed && is_llm_rounds_leaf_field_path(&field_path) {
                    self.with_llm_tool_callback_index(scope, payload, &full_value)
                        .await?
                } else {
                    payload
                };
                if !changed && is_llm_rounds_leaf_field_path(&field_path) {
                    let (payload, runtime_facts_changed) = with_llm_tool_callback_runtime_facts(
                        payload,
                        &self.llm_tool_callback_runtime_facts,
                    );
                    return Ok((payload, runtime_facts_changed));
                }
                return Ok((payload, changed));
            }

            match value {
                Value::Object(object) => {
                    let mut changed = false;
                    let mut next = Map::with_capacity(object.len());
                    for (key, child) in object {
                        let mut child_path = field_path.clone();
                        child_path.push(key.clone());
                        let (child, child_changed) = self
                            .offload_selected_payload_fields(
                                scope,
                                artifact_kind,
                                child,
                                child_path,
                                preview_request,
                            )
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

pub async fn offload_application_run_detail_artifacts(
    state: Arc<ApiState>,
    workspace_id: Uuid,
    application_id: Uuid,
    mut detail: domain::ApplicationRunDetail,
) -> Result<domain::ApplicationRunDetail, ApiError> {
    if !is_safe_to_persist_debug_artifact_previews(detail.flow_run.status) {
        return Ok(detail);
    }

    let mut writer = RuntimeDebugArtifactWriter::new(state.clone()).await?;
    writer.llm_tool_callback_runtime_facts =
        collect_llm_tool_callback_runtime_facts(&detail.callback_tasks);
    let flow_scope = RuntimeDebugArtifactScope {
        workspace_id,
        application_id,
        flow_run_id: Some(detail.flow_run.id),
        node_run_id: None,
        run_event_id: None,
    };
    let flow_input_query = application_run_query(&detail.flow_run.input_payload);
    let flow_input_model = application_run_model(&detail.flow_run.input_payload);
    let flow_input_system = application_run_system(&detail.flow_run.input_payload);
    let (flow_input_payload, flow_input_changed) = writer
        .offload_value(
            &flow_scope,
            "flow_input_payload",
            detail.flow_run.input_payload.clone(),
        )
        .await?;
    let flow_input_payload = if flow_input_changed {
        with_application_run_input_summary(
            flow_input_payload,
            flow_input_query.as_deref(),
            flow_input_model.as_deref(),
            flow_input_system.as_deref(),
        )
    } else {
        flow_input_payload
    };
    let (flow_output_payload, flow_output_changed) = writer
        .offload_payload_fields(
            &flow_scope,
            "flow_output_payload",
            detail.flow_run.output_payload.clone(),
            Vec::new(),
        )
        .await?;
    let (flow_error_payload, flow_error_changed) = match detail.flow_run.error_payload.clone() {
        Some(error_payload) => {
            let (payload, changed) = writer
                .offload_value(&flow_scope, "flow_error_payload", error_payload)
                .await?;
            (Some(payload), changed)
        }
        None => (None, false),
    };
    if flow_input_changed || flow_output_changed || flow_error_changed {
        detail.flow_run =
            <MainDurableStore as OrchestrationRuntimeRepository>::update_flow_run_payloads(
                &state.store,
                &UpdateFlowRunPayloadsInput {
                    flow_run_id: detail.flow_run.id,
                    input_payload: flow_input_payload,
                    output_payload: flow_output_payload,
                    error_payload: flow_error_payload,
                },
            )
            .await?;
    }

    for node_run in &mut detail.node_runs {
        let node_scope = RuntimeDebugArtifactScope {
            workspace_id,
            application_id,
            flow_run_id: Some(detail.flow_run.id),
            node_run_id: Some(node_run.id),
            run_event_id: None,
        };
        let (input_payload, input_changed) = writer
            .offload_payload_fields(
                &node_scope,
                "node_input_payload",
                node_run.input_payload.clone(),
                Vec::new(),
            )
            .await?;
        let (output_payload, output_changed) = writer
            .offload_payload_fields(
                &node_scope,
                "node_output_payload",
                node_run.output_payload.clone(),
                Vec::new(),
            )
            .await?;
        let (error_payload, error_changed) = match node_run.error_payload.clone() {
            Some(error_payload) => {
                let (payload, changed) = writer
                    .offload_payload_fields(
                        &node_scope,
                        "node_error_payload",
                        error_payload,
                        Vec::new(),
                    )
                    .await?;
                (Some(payload), changed)
            }
            None => (None, false),
        };
        let (metrics_payload, metrics_changed) = writer
            .offload_payload_fields(
                &node_scope,
                "node_metrics_payload",
                node_run.metrics_payload.clone(),
                Vec::new(),
            )
            .await?;
        let (debug_payload, debug_changed) = writer
            .offload_node_debug_payload(&node_scope, node_run.debug_payload.clone())
            .await?;

        if input_changed || output_changed || error_changed || metrics_changed || debug_changed {
            *node_run =
                <MainDurableStore as OrchestrationRuntimeRepository>::update_node_run_payloads(
                    &state.store,
                    &UpdateNodeRunPayloadsInput {
                        node_run_id: node_run.id,
                        input_payload,
                        output_payload,
                        error_payload,
                        metrics_payload,
                        debug_payload,
                    },
                )
                .await?;
        }
    }

    for checkpoint in &mut detail.checkpoints {
        let checkpoint_scope = RuntimeDebugArtifactScope {
            workspace_id,
            application_id,
            flow_run_id: Some(detail.flow_run.id),
            node_run_id: checkpoint.node_run_id,
            run_event_id: None,
        };
        let (locator_payload, locator_changed) = writer
            .offload_payload_fields(
                &checkpoint_scope,
                "checkpoint_locator_payload",
                checkpoint.locator_payload.clone(),
                Vec::new(),
            )
            .await?;
        let (variable_snapshot, variable_changed) = writer
            .offload_payload_fields(
                &checkpoint_scope,
                "checkpoint_variable_snapshot",
                checkpoint.variable_snapshot.clone(),
                Vec::new(),
            )
            .await?;
        let (external_ref_payload, external_changed) = match checkpoint.external_ref_payload.clone()
        {
            Some(external_ref_payload) => {
                let (payload, changed) = writer
                    .offload_payload_fields(
                        &checkpoint_scope,
                        "checkpoint_external_ref_payload",
                        external_ref_payload,
                        Vec::new(),
                    )
                    .await?;
                (Some(payload), changed)
            }
            None => (None, false),
        };

        if locator_changed || variable_changed || external_changed {
            *checkpoint =
                <MainDurableStore as OrchestrationRuntimeRepository>::update_checkpoint_payloads(
                    &state.store,
                    &UpdateCheckpointPayloadsInput {
                        checkpoint_id: checkpoint.id,
                        locator_payload,
                        variable_snapshot,
                        external_ref_payload,
                    },
                )
                .await?;
        }
    }

    for callback_task in &mut detail.callback_tasks {
        let callback_scope = RuntimeDebugArtifactScope {
            workspace_id,
            application_id,
            flow_run_id: Some(detail.flow_run.id),
            node_run_id: Some(callback_task.node_run_id),
            run_event_id: None,
        };
        let (request_payload, request_changed) = writer
            .offload_payload_fields(
                &callback_scope,
                "callback_task_request_payload",
                callback_task.request_payload.clone(),
                Vec::new(),
            )
            .await?;
        let (response_payload, response_changed) = match callback_task.response_payload.clone() {
            Some(response_payload) => {
                let (payload, changed) = writer
                    .offload_payload_fields(
                        &callback_scope,
                        "callback_task_response_payload",
                        response_payload,
                        Vec::new(),
                    )
                    .await?;
                (Some(payload), changed)
            }
            None => (None, false),
        };
        let (external_ref_payload, external_changed) =
            match callback_task.external_ref_payload.clone() {
                Some(external_ref_payload) => {
                    let (payload, changed) = writer
                        .offload_payload_fields(
                            &callback_scope,
                            "callback_task_external_ref_payload",
                            external_ref_payload,
                            Vec::new(),
                        )
                        .await?;
                    (Some(payload), changed)
                }
                None => (None, false),
            };

        if request_changed || response_changed || external_changed {
            *callback_task =
                <MainDurableStore as OrchestrationRuntimeRepository>::update_callback_task_payloads(
                    &state.store,
                    &UpdateCallbackTaskPayloadsInput {
                        callback_task_id: callback_task.id,
                        request_payload,
                        response_payload,
                        external_ref_payload,
                    },
                )
                .await?;
        }
    }

    for event in &mut detail.events {
        let event_scope = RuntimeDebugArtifactScope {
            workspace_id,
            application_id,
            flow_run_id: Some(detail.flow_run.id),
            node_run_id: event.node_run_id,
            run_event_id: Some(event.id),
        };
        let (payload, changed) = writer
            .offload_value(&event_scope, "run_event_payload", event.payload.clone())
            .await?;
        if changed {
            *event =
                <MainDurableStore as OrchestrationRuntimeRepository>::update_run_event_payload(
                    &state.store,
                    &UpdateRunEventPayloadInput {
                        run_event_id: event.id,
                        payload,
                    },
                )
                .await?;
        }
    }

    Ok(detail)
}

pub async fn offload_trace_node_run_detail_artifacts(
    state: Arc<ApiState>,
    workspace_id: Uuid,
    application_id: Uuid,
    flow_run_id: Uuid,
    mut node_run: domain::NodeRunRecord,
    preview_request: RuntimeDebugArtifactPreviewRequest,
) -> Result<domain::NodeRunRecord, ApiError> {
    let writer = RuntimeDebugArtifactWriter::new(state).await?;
    let node_scope = RuntimeDebugArtifactScope {
        workspace_id,
        application_id,
        flow_run_id: Some(flow_run_id),
        node_run_id: Some(node_run.id),
        run_event_id: None,
    };
    let (input_payload, input_changed) = match &preview_request {
        RuntimeDebugArtifactPreviewRequest::Auto => {
            writer
                .offload_payload_fields(
                    &node_scope,
                    "node_input_payload",
                    node_run.input_payload.clone(),
                    Vec::new(),
                )
                .await?
        }
        RuntimeDebugArtifactPreviewRequest::Fields(_) => {
            writer
                .offload_selected_payload_fields(
                    &node_scope,
                    "node_input_payload",
                    node_run.input_payload.clone(),
                    vec!["node_run".to_string(), "input_payload".to_string()],
                    &preview_request,
                )
                .await?
        }
    };
    let (output_payload, output_changed) = match &preview_request {
        RuntimeDebugArtifactPreviewRequest::Auto => {
            writer
                .offload_payload_fields(
                    &node_scope,
                    "node_output_payload",
                    node_run.output_payload.clone(),
                    Vec::new(),
                )
                .await?
        }
        RuntimeDebugArtifactPreviewRequest::Fields(_) => {
            writer
                .offload_selected_payload_fields(
                    &node_scope,
                    "node_output_payload",
                    node_run.output_payload.clone(),
                    vec!["node_run".to_string(), "output_payload".to_string()],
                    &preview_request,
                )
                .await?
        }
    };
    let (error_payload, error_changed) = match node_run.error_payload.clone() {
        Some(error_payload) => match &preview_request {
            RuntimeDebugArtifactPreviewRequest::Auto => {
                let (payload, changed) = writer
                    .offload_payload_fields(
                        &node_scope,
                        "node_error_payload",
                        error_payload,
                        Vec::new(),
                    )
                    .await?;
                (Some(payload), changed)
            }
            RuntimeDebugArtifactPreviewRequest::Fields(_) => {
                let (payload, changed) = writer
                    .offload_selected_payload_fields(
                        &node_scope,
                        "node_error_payload",
                        error_payload,
                        vec!["node_run".to_string(), "error_payload".to_string()],
                        &preview_request,
                    )
                    .await?;
                (Some(payload), changed)
            }
        },
        None => (None, false),
    };
    let (metrics_payload, metrics_changed) = match &preview_request {
        RuntimeDebugArtifactPreviewRequest::Auto => {
            writer
                .offload_payload_fields(
                    &node_scope,
                    "node_metrics_payload",
                    node_run.metrics_payload.clone(),
                    Vec::new(),
                )
                .await?
        }
        RuntimeDebugArtifactPreviewRequest::Fields(_) => {
            writer
                .offload_selected_payload_fields(
                    &node_scope,
                    "node_metrics_payload",
                    node_run.metrics_payload.clone(),
                    vec!["node_run".to_string(), "metrics_payload".to_string()],
                    &preview_request,
                )
                .await?
        }
    };
    let (debug_payload, debug_changed) = match &preview_request {
        RuntimeDebugArtifactPreviewRequest::Auto => {
            writer
                .offload_node_debug_payload(&node_scope, node_run.debug_payload.clone())
                .await?
        }
        RuntimeDebugArtifactPreviewRequest::Fields(_) => {
            writer
                .offload_selected_payload_fields(
                    &node_scope,
                    "node_debug_payload",
                    node_run.debug_payload.clone(),
                    vec!["node_run".to_string(), "debug_payload".to_string()],
                    &preview_request,
                )
                .await?
        }
    };

    if input_changed || output_changed || error_changed || metrics_changed || debug_changed {
        node_run.input_payload = input_payload;
        node_run.output_payload = output_payload;
        node_run.error_payload = error_payload;
        node_run.metrics_payload = metrics_payload;
        node_run.debug_payload = debug_payload;
    }

    Ok(node_run)
}

pub async fn offload_trace_node_content_artifacts(
    state: Arc<ApiState>,
    workspace_id: Uuid,
    application_id: Uuid,
    flow_run_id: Uuid,
    mut content: domain::ApplicationRunTraceNodeContentRecord,
    preview_request: RuntimeDebugArtifactPreviewRequest,
) -> Result<domain::ApplicationRunTraceNodeContentRecord, ApiError> {
    if !matches!(
        content.content_kind.as_str(),
        "tool_callback" | "fusion" | "route" | "branch" | "callback_task"
    ) {
        return Ok(content);
    }

    let writer = RuntimeDebugArtifactWriter::new(state).await?;
    let scope = RuntimeDebugArtifactScope {
        workspace_id,
        application_id,
        flow_run_id: Some(flow_run_id),
        node_run_id: None,
        run_event_id: None,
    };
    let (payload, changed) = match &preview_request {
        RuntimeDebugArtifactPreviewRequest::Auto => {
            writer
                .offload_payload_fields(
                    &scope,
                    "trace_node_content_payload",
                    content.payload.clone(),
                    Vec::new(),
                )
                .await?
        }
        RuntimeDebugArtifactPreviewRequest::Fields(_) => {
            writer
                .offload_selected_payload_fields(
                    &scope,
                    "trace_node_content_payload",
                    content.payload.clone(),
                    Vec::new(),
                    &preview_request,
                )
                .await?
        }
    };

    if changed {
        content.payload = payload;
    }

    Ok(content)
}
