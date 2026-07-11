use std::{collections::BTreeMap, sync::Arc};

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{header::CONTENT_TYPE, HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    Json, Router,
};
use control_plane::{
    application_public_api::{
        mapping::{WorkflowExtensionHttpMethod, WorkflowExtensionResponseMode},
        workflow_extension::{
            CreateWorkflowExtensionRunCommand, WorkflowExtensionRequestParameters,
            WorkflowExtensionRunError, WorkflowExtensionRunService,
        },
    },
    orchestration_runtime::{OrchestrationRuntimeService, StartPublishedFlowRunCommand},
};
use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::{
    app_state::ApiState,
    routes::application_public_api::native::{
        api_provider_runtime, bearer_token, service_error, NativeApiError,
    },
    runtime_activity::{scope_application_activity, ApplicationActivityKind},
};

#[derive(Debug, Serialize)]
struct WorkflowExtensionAcceptedResponse {
    run_id: String,
    status: String,
}

pub fn router() -> Router<Arc<ApiState>> {
    Router::new().route("/:slug", axum::routing::any(invoke_workflow_extension))
}

pub async fn invoke_workflow_extension(
    State(state): State<Arc<ApiState>>,
    method: Method,
    headers: HeaderMap,
    Path(slug): Path<String>,
    uri: axum::http::Uri,
    body: Bytes,
) -> Result<Response, NativeApiError> {
    let bearer_token = bearer_token(&headers)?;
    let method = workflow_extension_method(&method)?;
    let parameters = request_parameters(&slug, uri.query(), &headers, &body)?;
    let run = WorkflowExtensionRunService::new(state.store.clone())
        .with_last_used_cache(state.infrastructure.cache_store())
        .create_run(CreateWorkflowExtensionRunCommand {
            bearer_token,
            slug,
            method,
            parameters,
        })
        .await
        .map_err(workflow_extension_error)?;
    let _http_activity = state
        .runtime_activity
        .start(run.application_id, ApplicationActivityKind::HttpRequest);
    let execution = spawn_workflow_extension_execution(state.clone(), run.application_id, run.id);

    if run.response_mode == WorkflowExtensionResponseMode::Async {
        return Ok(accepted_response(run.id, run.status));
    }

    match tokio::time::timeout(
        std::time::Duration::from_millis(run.sync_timeout_ms),
        execution,
    )
    .await
    {
        Ok(joined) => {
            let detail = joined
                .map_err(|error| service_error(anyhow::anyhow!(error)))?
                .map_err(service_error)?;
            match detail.flow_run.status {
                domain::FlowRunStatus::Succeeded => {
                    Ok((StatusCode::OK, Json(detail.flow_run.output_payload)).into_response())
                }
                domain::FlowRunStatus::Queued
                | domain::FlowRunStatus::Running
                | domain::FlowRunStatus::WaitingCallback
                | domain::FlowRunStatus::WaitingHuman
                | domain::FlowRunStatus::Paused => Ok(accepted_response(
                    detail.flow_run.id,
                    detail.flow_run.status,
                )),
                domain::FlowRunStatus::Failed => Err(NativeApiError::new(
                    StatusCode::CONFLICT,
                    "workflow_failed",
                    detail
                        .flow_run
                        .error_payload
                        .as_ref()
                        .and_then(|value| value.get("message"))
                        .and_then(Value::as_str)
                        .unwrap_or("workflow run failed"),
                )),
                domain::FlowRunStatus::Cancelled => Err(NativeApiError::new(
                    StatusCode::CONFLICT,
                    "workflow_cancelled",
                    "workflow run was cancelled",
                )),
            }
        }
        Err(_) => Ok(accepted_response(run.id, domain::FlowRunStatus::Running)),
    }
}

fn spawn_workflow_extension_execution(
    state: Arc<ApiState>,
    application_id: uuid::Uuid,
    flow_run_id: uuid::Uuid,
) -> tokio::task::JoinHandle<anyhow::Result<domain::ApplicationRunDetail>> {
    tokio::spawn(async move {
        let _execution_activity = state.runtime_activity.start(
            application_id,
            ApplicationActivityKind::ApplicationExecution,
        );
        let runtime_service = OrchestrationRuntimeService::new(
            state.store.clone(),
            api_provider_runtime(&state),
            state.runtime_engine.clone(),
            state.provider_secret_master_key.clone(),
        )
        .with_node_artifact_context(
            state.api_node_id.clone(),
            state.provider_install_root.clone(),
        )
        .with_file_storage_registry(state.file_storage_registry.clone())
        .with_llm_routing_counter_store(state.infrastructure.cache_store())
        .with_provider_request_log_queue(state.infrastructure.task_queue())
        .with_runtime_event_stream(state.runtime_event_stream.clone());
        scope_application_activity(
            application_id,
            runtime_service.start_published_flow_run(StartPublishedFlowRunCommand {
                application_id,
                flow_run_id,
            }),
        )
        .await
    })
}

fn workflow_extension_method(
    method: &Method,
) -> Result<WorkflowExtensionHttpMethod, NativeApiError> {
    if method == Method::GET {
        Ok(WorkflowExtensionHttpMethod::Get)
    } else if method == Method::POST {
        Ok(WorkflowExtensionHttpMethod::Post)
    } else if method == Method::PUT {
        Ok(WorkflowExtensionHttpMethod::Put)
    } else if method == Method::PATCH {
        Ok(WorkflowExtensionHttpMethod::Patch)
    } else if method == Method::DELETE {
        Ok(WorkflowExtensionHttpMethod::Delete)
    } else if method == Method::HEAD {
        Ok(WorkflowExtensionHttpMethod::Head)
    } else if method == Method::OPTIONS {
        Ok(WorkflowExtensionHttpMethod::Options)
    } else {
        Err(NativeApiError::new(
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "HTTP method is not supported for workflow extension APIs",
        ))
    }
}

fn request_parameters(
    slug: &str,
    raw_query: Option<&str>,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<WorkflowExtensionRequestParameters, NativeApiError> {
    let path = BTreeMap::from([("slug".to_string(), Value::String(slug.to_string()))]);
    let query = parse_urlencoded_object(raw_query.unwrap_or_default());
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let (form, body) = if content_type.split(';').next().is_some_and(|value| {
        value
            .trim()
            .eq_ignore_ascii_case("application/x-www-form-urlencoded")
    }) {
        (
            parse_urlencoded_object(std::str::from_utf8(body).map_err(|_| {
                NativeApiError::new(StatusCode::BAD_REQUEST, "form", "invalid form body")
            })?),
            json!({}),
        )
    } else if body.is_empty() {
        (Map::new(), json!({}))
    } else {
        let body = serde_json::from_slice(body).map_err(|_| {
            NativeApiError::new(StatusCode::BAD_REQUEST, "json", "invalid JSON body")
        })?;
        (Map::new(), body)
    };

    Ok(WorkflowExtensionRequestParameters {
        path,
        query,
        form,
        body,
    })
}

fn parse_urlencoded_object(raw: &str) -> Map<String, Value> {
    form_urlencoded::parse(raw.as_bytes())
        .map(|(key, value)| (key.into_owned(), Value::String(value.into_owned())))
        .collect()
}

fn workflow_extension_error(error: WorkflowExtensionRunError) -> NativeApiError {
    match error {
        WorkflowExtensionRunError::NotAuthenticated => NativeApiError::new(
            StatusCode::UNAUTHORIZED,
            "not_authenticated",
            "invalid application API key",
        ),
        WorkflowExtensionRunError::ExtensionNotFound => NativeApiError::new(
            StatusCode::NOT_FOUND,
            "workflow_extension_not_found",
            "workflow extension API was not found",
        ),
        WorkflowExtensionRunError::ApplicationNotPublished => NativeApiError::new(
            StatusCode::CONFLICT,
            "application_not_published",
            "workflow extension API is not enabled",
        ),
        WorkflowExtensionRunError::Forbidden => NativeApiError::new(
            StatusCode::FORBIDDEN,
            "workflow_extension_forbidden",
            "application API key cannot invoke this workflow extension",
        ),
        WorkflowExtensionRunError::MethodNotAllowed => NativeApiError::new(
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "HTTP method does not match the workflow extension API configuration",
        ),
        WorkflowExtensionRunError::TriggerTypeMismatch => NativeApiError::new(
            StatusCode::CONFLICT,
            "workflow_trigger_type_mismatch",
            "workflow trigger type does not allow extension API invocation",
        ),
        WorkflowExtensionRunError::InvalidMapping => NativeApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_mapping",
            "workflow extension API mapping is invalid",
        ),
    }
}

fn accepted_response(run_id: uuid::Uuid, status: domain::FlowRunStatus) -> Response {
    (
        StatusCode::ACCEPTED,
        Json(WorkflowExtensionAcceptedResponse {
            run_id: run_id.to_string(),
            status: status.as_str().to_string(),
        }),
    )
        .into_response()
}
