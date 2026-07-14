use std::sync::Arc;

use axum::{
    body::Body,
    http::{Response, StatusCode, header::CONTENT_TYPE},
};
use control_plane::{
    errors::ControlPlaneError,
    ports::{
        FileManagementRepository, GetRuntimeDebugArtifactInput, OrchestrationRuntimeRepository,
    },
};
use serde_json::Value;
use storage_durable::MainDurableStore;
use uuid::Uuid;

use crate::{app_state::ApiState, error_response::ApiError};

const APPLICATION_INPUT_QUERY_KEYS: &[&str] = &["query", "question", "prompt", "message", "input"];

pub(crate) fn application_run_query(payload: &Value) -> Option<String> {
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

pub(crate) fn application_run_model(payload: &Value) -> Option<String> {
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

pub(crate) fn application_run_system(payload: &Value) -> Option<String> {
    if let Some(system) = string_field(payload, "system") {
        return Some(system);
    }

    for selector in ["node-start.system", "start.system"] {
        if let Some(value) = string_field(payload, selector) {
            return Some(value);
        }
    }

    let object = payload.as_object()?;
    for key in ["node-start", "start"] {
        if let Some(value) = object.get(key).and_then(immediate_system_value) {
            return Some(value);
        }
    }

    for value in object.values() {
        if let Some(value) = immediate_system_value(value) {
            return Some(value);
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

fn immediate_system_value(value: &Value) -> Option<String> {
    let object = value.as_object()?;
    object
        .get("system")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(super) fn is_runtime_debug_artifact_payload(value: &Value) -> bool {
    value
        .get("__runtime_debug_artifact")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub(super) fn with_debug_artifact_field_path(mut payload: Value, field_path: &[String]) -> Value {
    let Some(object) = payload.as_object_mut() else {
        return payload;
    };
    if field_path.is_empty() {
        return payload;
    }

    object.insert(
        "artifact_scope".to_string(),
        Value::String("field".to_string()),
    );
    object.insert(
        "field_path".to_string(),
        Value::Array(field_path.iter().cloned().map(Value::String).collect()),
    );
    payload
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(super) fn with_application_run_input_summary(
    mut payload: Value,
    query: Option<&str>,
    model: Option<&str>,
    system: Option<&str>,
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
    if let Some(system) = system {
        object.insert("system".to_string(), Value::String(system.to_string()));
    }
    payload
}

pub(super) fn should_keep_runtime_payload_field_inline(field_path: &[String]) -> bool {
    field_path.iter().any(|key| {
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

pub(super) fn is_safe_to_persist_debug_artifact_previews(status: domain::FlowRunStatus) -> bool {
    matches!(
        status,
        domain::FlowRunStatus::Succeeded
            | domain::FlowRunStatus::Failed
            | domain::FlowRunStatus::Cancelled
            | domain::FlowRunStatus::WaitingCallback
            | domain::FlowRunStatus::WaitingHuman
    )
}

pub(crate) async fn load_runtime_debug_artifact_response(
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

    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, content_type)
        .body(Body::from(object.bytes))
        .map_err(ApiError::from)
}

pub(crate) async fn load_runtime_debug_artifact_json_value(
    state: Arc<ApiState>,
    workspace_id: Uuid,
    application_id: Uuid,
    artifact_id: Uuid,
) -> Result<Value, ApiError> {
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

    serde_json::from_slice(&object.bytes)
        .map_err(|_| ControlPlaneError::Conflict("runtime_debug_artifact_not_json").into())
}
