use std::sync::Arc;

use axum::{
    body::Body,
    http::{header::CONTENT_TYPE, Response, StatusCode},
};
use control_plane::{
    errors::ControlPlaneError,
    orchestration_runtime::debug_artifacts::{
        build_runtime_debug_artifact_object_path, build_runtime_debug_artifact_preview,
        inline_budget_for_kind, RUNTIME_DEBUG_ARTIFACT_CONTENT_TYPE_JSON,
        RUNTIME_DEBUG_ARTIFACT_RETENTION_ACTIVE,
    },
    ports::{
        CreateRuntimeDebugArtifactInput, FileManagementRepository, GetRuntimeDebugArtifactInput,
        OrchestrationRuntimeRepository, UpdateFlowRunPayloadsInput, UpdateNodeRunPayloadsInput,
        UpdateRunEventPayloadInput,
    },
};
use serde_json::Value;
use storage_durable::MainDurableStore;
use uuid::Uuid;

use crate::{app_state::ApiState, error_response::ApiError};

const APPLICATION_INPUT_QUERY_KEYS: &[&str] = &["query", "question", "prompt", "message", "input"];

struct RuntimeDebugArtifactScope {
    workspace_id: Uuid,
    application_id: Uuid,
    flow_run_id: Option<Uuid>,
    node_run_id: Option<Uuid>,
    run_event_id: Option<Uuid>,
}

struct RuntimeDebugArtifactWriter {
    state: Arc<ApiState>,
    storage: domain::FileStorageRecord,
    driver: Arc<dyn storage_object::FileStorageDriver>,
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

    let writer = RuntimeDebugArtifactWriter::new(state.clone()).await?;
    let flow_scope = RuntimeDebugArtifactScope {
        workspace_id,
        application_id,
        flow_run_id: Some(detail.flow_run.id),
        node_run_id: None,
        run_event_id: None,
    };
    let flow_input_query = application_run_query(&detail.flow_run.input_payload);
    let flow_input_model = application_run_model(&detail.flow_run.input_payload);
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
        )
    } else {
        flow_input_payload
    };
    let (flow_output_payload, flow_output_changed) = writer
        .offload_value(
            &flow_scope,
            "flow_output_payload",
            detail.flow_run.output_payload.clone(),
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
        let original_input_payload = node_run.input_payload.clone();
        let (input_payload, input_changed) = writer
            .offload_value(
                &node_scope,
                "node_input_payload",
                original_input_payload.clone(),
            )
            .await?;
        let input_payload = if input_changed && node_run.node_type == "start" {
            with_start_node_input_summary(input_payload, &original_input_payload)
        } else {
            input_payload
        };
        let (output_payload, output_changed) = writer
            .offload_value(
                &node_scope,
                "node_output_payload",
                node_run.output_payload.clone(),
            )
            .await?;
        let (error_payload, error_changed) = match node_run.error_payload.clone() {
            Some(error_payload) => {
                let (payload, changed) = writer
                    .offload_value(&node_scope, "node_error_payload", error_payload)
                    .await?;
                (Some(payload), changed)
            }
            None => (None, false),
        };
        let (metrics_payload, metrics_changed) = writer
            .offload_value(
                &node_scope,
                "node_metrics_payload",
                node_run.metrics_payload.clone(),
            )
            .await?;
        let (debug_payload, debug_changed) = writer
            .offload_value(
                &node_scope,
                "node_debug_payload",
                node_run.debug_payload.clone(),
            )
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

pub(super) fn application_run_query(payload: &Value) -> Option<String> {
    if let Some(query) = string_field(payload, "query") {
        return Some(query);
    }

    if let Some(input_text) = string_field(payload, "input_text") {
        return Some(input_text);
    }

    for selector in [
        "node-start.query",
        "node-start.question",
        "node-start.prompt",
        "node-start.message",
        "node-start.input",
        "start.query",
        "start.question",
        "start.prompt",
        "start.message",
        "start.input",
    ] {
        if let Some(value) = string_field(payload, selector) {
            return Some(value);
        }
    }

    let object = payload.as_object()?;
    for key in ["node-start", "start"] {
        if let Some(value) = object.get(key).and_then(immediate_input_text) {
            return Some(value);
        }
    }

    for value in object.values() {
        if let Some(value) = immediate_input_text(value) {
            return Some(value);
        }
    }

    None
}

pub(super) fn application_run_model(payload: &Value) -> Option<String> {
    if let Some(model) = string_field(payload, "model") {
        return Some(model);
    }

    for selector in ["node-start.model", "start.model"] {
        if let Some(value) = string_field(payload, selector) {
            return Some(value);
        }
    }

    let object = payload.as_object()?;
    for key in ["node-start", "start"] {
        if let Some(value) = object.get(key).and_then(immediate_model_value) {
            return Some(value);
        }
    }

    for value in object.values() {
        if let Some(value) = immediate_model_value(value) {
            return Some(value);
        }
    }

    None
}

pub(super) fn application_run_answer(payload: &Value) -> Option<String> {
    for key in ["answer", "text", "content", "message"] {
        if let Some(value) = string_field(payload, key) {
            return Some(value);
        }
    }

    if is_runtime_debug_artifact_payload(payload) {
        return string_field(payload, "preview");
    }

    let object = payload.as_object()?;
    if let Some(error) = object.get("error").and_then(|value| value.get("message")) {
        if let Some(error) = error
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(error.to_string());
        }
    }

    None
}

fn immediate_input_text(value: &Value) -> Option<String> {
    let object = value.as_object()?;
    for key in APPLICATION_INPUT_QUERY_KEYS {
        if let Some(value) = object
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(value.to_string());
        }
    }
    None
}

fn immediate_model_value(value: &Value) -> Option<String> {
    let object = value.as_object()?;
    object
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn is_runtime_debug_artifact_payload(value: &Value) -> bool {
    value
        .get("__runtime_debug_artifact")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn with_application_run_input_summary(
    mut payload: Value,
    query: Option<&str>,
    model: Option<&str>,
) -> Value {
    let Some(object) = payload.as_object_mut() else {
        return payload;
    };
    if let Some(query) = query {
        object.insert("query".to_string(), Value::String(query.to_string()));
    }
    if let Some(model) = model {
        object.insert("model".to_string(), Value::String(model.to_string()));
    }
    payload
}

fn with_start_node_input_summary(mut payload: Value, full_input: &Value) -> Value {
    let Some(object) = payload.as_object_mut() else {
        return payload;
    };
    let start_payload = start_node_input_payload(full_input);
    let query = immediate_input_text(start_payload)
        .or_else(|| application_run_query(full_input))
        .unwrap_or_default();
    let model = immediate_model_value(start_payload)
        .or_else(|| application_run_model(full_input))
        .unwrap_or_default();

    object.insert("query".to_string(), Value::String(query));
    object.insert("model".to_string(), Value::String(model));
    object.insert(
        "files".to_string(),
        named_array_value(start_payload, &["files", "attachments"])
            .map(|value| Value::Array(value.clone()))
            .unwrap_or_else(|| Value::Array(Vec::new())),
    );
    object.insert(
        "history".to_string(),
        placeholder_array_for_named_array(start_payload, &["history", "messages"]),
    );
    object.insert(
        "tools".to_string(),
        placeholder_array_for_named_array(
            start_payload,
            &["tools", "tool_registry", "tool_definitions"],
        ),
    );

    payload
}

fn start_node_input_payload(payload: &Value) -> &Value {
    payload
        .get("node-start")
        .or_else(|| payload.get("start"))
        .unwrap_or(payload)
}

fn placeholder_array_for_named_array(payload: &Value, keys: &[&str]) -> Value {
    if named_array_value(payload, keys).is_none_or(Vec::is_empty) {
        return Value::Array(Vec::new());
    }

    Value::Array(vec![Value::String("...".to_string())])
}

fn named_array_value<'a>(payload: &'a Value, keys: &[&str]) -> Option<&'a Vec<Value>> {
    let object = payload.as_object()?;
    for key in keys {
        if let Some(array) = object.get(*key).and_then(Value::as_array) {
            return Some(array);
        }
    }

    for value in object.values() {
        if let Some(array) = named_array_value(value, keys) {
            return Some(array);
        }
    }

    None
}

fn is_safe_to_persist_debug_artifact_previews(status: domain::FlowRunStatus) -> bool {
    matches!(
        status,
        domain::FlowRunStatus::Succeeded
            | domain::FlowRunStatus::Failed
            | domain::FlowRunStatus::Cancelled
    )
}

pub async fn load_runtime_debug_artifact_response(
    state: Arc<ApiState>,
    workspace_id: Uuid,
    application_id: Uuid,
    artifact_id: Uuid,
) -> Result<Response<Body>, ApiError> {
    let artifact =
        <MainDurableStore as OrchestrationRuntimeRepository>::get_runtime_debug_artifact(
            &state.store,
            &GetRuntimeDebugArtifactInput {
                workspace_id,
                application_id,
                artifact_id,
            },
        )
        .await?
        .ok_or(ControlPlaneError::NotFound("runtime_debug_artifact"))?;
    let storage = <MainDurableStore as FileManagementRepository>::get_file_storage(
        &state.store,
        artifact.storage_id,
    )
    .await?
    .ok_or(ControlPlaneError::NotFound("file_storage"))?;
    if !storage.enabled {
        return Err(ControlPlaneError::Conflict("file_storage_disabled").into());
    }
    let driver = state
        .file_storage_registry
        .get(&storage.driver_type)
        .ok_or(ControlPlaneError::Conflict("storage_driver_not_registered"))?;
    let object = driver
        .open_read(storage_object::OpenReadInput {
            config_json: &storage.config_json,
            object_path: &artifact.storage_ref,
        })
        .await?;
    let content_type = object.content_type.unwrap_or(artifact.content_type);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, content_type)
        .body(Body::from(object.bytes))
        .unwrap())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn application_query_prefers_start_query_over_tool_schema_payload() {
        let payload = json!({
            "node-start": {
                "query": "ping",
                "model": "gpt-test",
                "compatibility": {
                    "tools": [
                        {
                            "function": {
                                "description": "path to the file to read.",
                                "parameters": {
                                    "properties": {
                                        "file_path": {
                                            "description": "path to the file to read."
                                        }
                                    }
                                }
                            }
                        }
                    ]
                }
            }
        });

        assert_eq!(application_run_query(&payload), Some("ping".into()));
        assert_eq!(application_run_model(&payload), Some("gpt-test".into()));
    }

    #[test]
    fn application_query_reads_persisted_artifact_preview_summary() {
        let payload = json!({
            "__runtime_debug_artifact": true,
            "artifact_ref": Uuid::now_v7().to_string(),
            "preview": "{\"node-start\":{\"compatibility\":{\"tools\":[]}}}",
            "query": "总结退款政策",
            "model": "deepseek-chat"
        });

        assert_eq!(application_run_query(&payload), Some("总结退款政策".into()));
        assert_eq!(
            application_run_model(&payload),
            Some("deepseek-chat".into())
        );
    }

    #[test]
    fn flow_input_artifact_preview_keeps_application_query_and_model() {
        let preview = json!({
            "__runtime_debug_artifact": true,
            "artifact_ref": Uuid::now_v7().to_string(),
            "preview": "{\"node-start\":{\"compatibility\":{\"tools\":[]}}}"
        });

        let preview = with_application_run_input_summary(preview, Some("ping"), Some("gpt-test"));

        assert_eq!(preview["query"], json!("ping"));
        assert_eq!(preview["model"], json!("gpt-test"));
    }

    #[test]
    fn start_node_input_artifact_preview_keeps_lightweight_start_fields() {
        let preview = json!({
            "__runtime_debug_artifact": true,
            "artifact_ref": Uuid::now_v7().to_string(),
            "preview": "{\"query\":\"truncated"
        });
        let full_input = json!({
            "query": "总结退款政策",
            "model": "deepseek-chat",
            "files": [{ "name": "refund.md" }],
            "history": [
                { "role": "user", "content": "旧问题" }
            ],
            "compatibility": {
                "tools": [
                    { "name": "read_file" }
                ]
            }
        });

        let preview = with_start_node_input_summary(preview, &full_input);

        assert_eq!(preview["query"], json!("总结退款政策"));
        assert_eq!(preview["model"], json!("deepseek-chat"));
        assert_eq!(preview["files"], json!([{ "name": "refund.md" }]));
        assert_eq!(preview["history"], json!(["..."]));
        assert_eq!(preview["tools"], json!(["..."]));
    }

    #[test]
    fn application_answer_reads_preferred_output_fields() {
        let payload = json!({
            "answer": "退款政策摘要"
        });

        assert_eq!(
            application_run_answer(&payload),
            Some("退款政策摘要".into())
        );
    }
}
