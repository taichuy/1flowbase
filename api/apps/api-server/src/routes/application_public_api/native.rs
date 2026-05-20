use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use control_plane::{
    application_public_api::{
        api_keys::ApplicationApiKeyService,
        native::{
            ApplicationNativeRunService, CancelNativeRunCommand, CreateNativeRunCommand,
            GetNativeRunCommand, NativeRunRequest, NativeRunResult, NativeRunValidationError,
            ResumeNativeRunCommand,
        },
        run_service::native_result_from_application_run_detail,
    },
    file_management::{FileUploadService, UploadFileCommand},
    orchestration_runtime::{
        debug_stream_events, CompleteCallbackTaskCommand, OrchestrationRuntimeService,
        StartPublishedFlowRunCommand,
    },
    ports::AuthRepository,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;
use tracing::error;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    provider_runtime::ApiProviderRuntime,
    response::ApiSuccess,
    routes::{application_public_api::sse, files::UploadedFileResponse},
};

#[derive(Debug, Deserialize, ToSchema)]
pub struct ResumeNativeRunBody {
    pub callback_task_id: Uuid,
    #[serde(default)]
    pub response_payload: Value,
    #[serde(default)]
    pub response_mode: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NativeRunResponse {
    pub id: Uuid,
    pub application_id: Uuid,
    pub api_key_id: Uuid,
    pub publication_version_id: Uuid,
    pub status: String,
    pub node_input_payload: Value,
    pub metadata: Value,
    pub answer: Option<String>,
    pub reasoning: Option<String>,
    pub required_action: Option<Value>,
    pub tool_calls: Option<Value>,
    pub usage: Option<Value>,
    pub error: Option<Value>,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NativeErrorBody {
    pub code: String,
    pub message: String,
}

#[derive(Debug)]
pub struct NativeApiError {
    pub(crate) status: StatusCode,
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

impl NativeApiError {
    pub(crate) fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
}

impl IntoResponse for NativeApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(NativeErrorBody {
                code: self.code.to_string(),
                message: self.message,
            }),
        )
            .into_response()
    }
}

pub(crate) fn bearer_token(headers: &HeaderMap) -> Result<String, NativeApiError> {
    let raw = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            NativeApiError::new(
                StatusCode::UNAUTHORIZED,
                "not_authenticated",
                "missing Authorization bearer token",
            )
        })?;
    raw.strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            NativeApiError::new(
                StatusCode::UNAUTHORIZED,
                "not_authenticated",
                "invalid Authorization bearer token",
            )
        })
}

pub(crate) fn native_error(error: NativeRunValidationError) -> NativeApiError {
    match error {
        NativeRunValidationError::NotAuthenticated => NativeApiError::new(
            StatusCode::UNAUTHORIZED,
            "not_authenticated",
            "invalid application API key",
        ),
        NativeRunValidationError::ApplicationNotPublished => NativeApiError::new(
            StatusCode::CONFLICT,
            "application_not_published",
            "application has no active published public API version",
        ),
        NativeRunValidationError::Forbidden => NativeApiError::new(
            StatusCode::FORBIDDEN,
            "application_run_forbidden",
            "run does not belong to this application API key",
        ),
        NativeRunValidationError::NotFound => NativeApiError::new(
            StatusCode::NOT_FOUND,
            "application_run_not_found",
            "run was not found",
        ),
        NativeRunValidationError::InvalidMapping => NativeApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_mapping",
            "application public API mapping is invalid",
        ),
        NativeRunValidationError::InvalidState => NativeApiError::new(
            StatusCode::CONFLICT,
            "invalid_state",
            "run is not in a valid state for this operation",
        ),
        NativeRunValidationError::ResumeContinuationNotImplemented => NativeApiError::new(
            StatusCode::NOT_IMPLEMENTED,
            "resume_not_implemented",
            "public callback resume continuation is not implemented",
        ),
    }
}

pub(crate) fn service_error(error: anyhow::Error) -> NativeApiError {
    if let Some(control_plane::errors::ControlPlaneError::NotFound(name)) =
        error.downcast_ref::<control_plane::errors::ControlPlaneError>()
    {
        return NativeApiError::new(StatusCode::NOT_FOUND, name, error.to_string());
    }
    if let Some(control_plane::errors::ControlPlaneError::Conflict(name)) =
        error.downcast_ref::<control_plane::errors::ControlPlaneError>()
    {
        return NativeApiError::new(StatusCode::CONFLICT, name, error.to_string());
    }
    if let Some(control_plane::errors::ControlPlaneError::InvalidInput(name)) =
        error.downcast_ref::<control_plane::errors::ControlPlaneError>()
    {
        return NativeApiError::new(StatusCode::BAD_REQUEST, name, error.to_string());
    }
    if let Some(runtime_core::runtime_acl::RuntimeAclError::PermissionDenied(reason)) =
        error.downcast_ref::<runtime_core::runtime_acl::RuntimeAclError>()
    {
        return NativeApiError::new(StatusCode::FORBIDDEN, reason, error.to_string());
    }
    NativeApiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal_error",
        error.to_string(),
    )
}

fn parse_native_run_request(bytes: Bytes) -> Result<NativeRunRequest, NativeApiError> {
    let value = serde_json::from_slice::<Value>(&bytes)
        .map_err(|_| NativeApiError::new(StatusCode::BAD_REQUEST, "json", "invalid JSON body"))?;
    if let Some(field) = invalid_native_field(&value) {
        return Err(NativeApiError::new(
            StatusCode::BAD_REQUEST,
            field,
            format!("invalid native request field: {field}"),
        ));
    }
    serde_json::from_value(value)
        .map_err(|error| NativeApiError::new(StatusCode::BAD_REQUEST, "body", error.to_string()))
}

fn invalid_native_field(value: &Value) -> Option<&'static str> {
    if !value.is_object() {
        return Some("body");
    }
    let field = |name: &str| value.get(name);
    if !field("query").is_some_and(Value::is_string) {
        return Some("query");
    }
    if field("model").is_some_and(|value| !value.is_string()) {
        return Some("model");
    }
    if field("inputs").is_some_and(|value| !value.is_object()) {
        return Some("inputs");
    }
    if field("history").is_some_and(|value| !value.is_array()) {
        return Some("history");
    }
    if field("attachments").is_some_and(|value| !value.is_array()) {
        return Some("attachments");
    }
    if field("conversation").is_some_and(|value| !value.is_object()) {
        return Some("conversation");
    }
    if field("expand_id").is_some_and(|value| !value.is_string()) {
        return Some("expand_id");
    }
    if field("user_id").is_some() {
        return Some("user_id");
    }
    if field("response_mode").is_some_and(|value| !value.is_string()) {
        return Some("response_mode");
    }
    if field("stream_options").is_some_and(|value| !value.is_object()) {
        return Some("stream_options");
    }
    if field("execution").is_some_and(|value| !value.is_object()) {
        return Some("execution");
    }
    if field("metadata").is_some_and(|value| !value.is_object()) {
        return Some("metadata");
    }
    if field("title").is_some_and(|value| !value.is_string()) {
        return Some("title");
    }
    if field("compatibility_mode").is_some_and(|value| !value.is_string()) {
        return Some("compatibility_mode");
    }
    None
}

pub(crate) fn to_native_run_response(run: NativeRunResult) -> NativeRunResponse {
    NativeRunResponse {
        id: run.id,
        application_id: run.application_id,
        api_key_id: run.api_key_id,
        publication_version_id: run.publication_version_id,
        status: serde_json::to_value(run.status)
            .ok()
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
            .unwrap_or_else(|| "unknown".to_string()),
        node_input_payload: run.node_input_payload,
        metadata: run.metadata,
        answer: run.answer,
        reasoning: run.reasoning,
        required_action: run
            .required_action
            .and_then(|value| serde_json::to_value(value).ok()),
        tool_calls: run.tool_calls,
        usage: run.usage.and_then(|value| serde_json::to_value(value).ok()),
        error: run.error.and_then(|value| serde_json::to_value(value).ok()),
        created_at: run.created_at.to_string(),
    }
}

pub(crate) async fn execute_blocking_native_run(
    state: Arc<ApiState>,
    bearer_token: String,
    run: NativeRunResult,
) -> Result<NativeRunResult, NativeApiError> {
    let runtime_service = OrchestrationRuntimeService::new(
        state.store.clone(),
        ApiProviderRuntime::new(state.provider_runtime.clone()),
        state.runtime_engine.clone(),
        state.provider_secret_master_key.clone(),
    );
    let execution_result = runtime_service
        .start_published_flow_run(StartPublishedFlowRunCommand {
            application_id: run.application_id,
            flow_run_id: run.id,
        })
        .await;
    match execution_result {
        Ok(detail) => Ok(native_result_from_application_run_detail(
            &detail,
            run.metadata.clone(),
        )),
        Err(error) => {
            error!(
                application_id = %run.application_id,
                flow_run_id = %run.id,
                error = %error,
                "blocking native published run reached failed runtime result"
            );
            ApplicationNativeRunService::new(state.store.clone())
                .with_last_used_cache(state.infrastructure.cache_store())
                .get_native_run(GetNativeRunCommand {
                    bearer_token,
                    run_id: run.id,
                })
                .await
                .map_err(native_error)
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/1flowbase/runs",
    request_body = Value,
    responses(
        (status = 201, body = NativeRunResponse),
        (status = 400, body = NativeErrorBody),
        (status = 401, body = NativeErrorBody),
        (status = 403, body = NativeErrorBody),
        (status = 409, body = NativeErrorBody)
    )
)]
pub async fn create_native_run(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, NativeApiError> {
    let bearer_token = bearer_token(&headers)?;
    let request = parse_native_run_request(body)?;
    let response_mode = request.response_mode.clone();
    let include_workflow_events = include_workflow_events(&request);
    let run = ApplicationNativeRunService::new(state.store.clone())
        .with_last_used_cache(state.infrastructure.cache_store())
        .create_native_run(CreateNativeRunCommand {
            bearer_token: bearer_token.clone(),
            request,
        })
        .await
        .map_err(native_error)?;

    if response_mode.as_deref() == Some("streaming") {
        return start_native_run_stream(state, run, include_workflow_events).await;
    }

    if response_mode.as_deref().unwrap_or("blocking") == "blocking" {
        let run = execute_blocking_native_run(state, bearer_token, run).await?;
        return Ok((
            StatusCode::CREATED,
            Json(ApiSuccess::new(to_native_run_response(run))),
        )
            .into_response());
    }

    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_native_run_response(run))),
    )
        .into_response())
}

fn include_workflow_events(request: &NativeRunRequest) -> sse::IncludeWorkflowEvents {
    match request
        .stream_options
        .get("include_workflow_events")
        .and_then(Value::as_str)
    {
        Some("public") => sse::IncludeWorkflowEvents::Public,
        _ => sse::IncludeWorkflowEvents::None,
    }
}

async fn start_native_run_stream(
    state: Arc<ApiState>,
    run: NativeRunResult,
    include_workflow_events: sse::IncludeWorkflowEvents,
) -> Result<Response, NativeApiError> {
    state
        .runtime_event_stream
        .open_run(
            run.id,
            control_plane::ports::RuntimeEventStreamPolicy::debug_default(),
        )
        .await
        .map_err(service_error)?;

    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(sse::send_native_runtime_event_stream(
        state.runtime_event_stream.clone(),
        run.clone(),
        include_workflow_events,
        sender,
    ));

    let background_state = state.clone();
    tokio::spawn(async move {
        let runtime_service = OrchestrationRuntimeService::new(
            background_state.store.clone(),
            ApiProviderRuntime::new(background_state.provider_runtime.clone()),
            background_state.runtime_engine.clone(),
            background_state.provider_secret_master_key.clone(),
        )
        .with_runtime_event_stream(background_state.runtime_event_stream.clone());
        if let Err(runtime_error) = runtime_service
            .start_published_flow_run(StartPublishedFlowRunCommand {
                application_id: run.application_id,
                flow_run_id: run.id,
            })
            .await
        {
            let _ = background_state
                .runtime_event_stream
                .append(
                    run.id,
                    debug_stream_events::flow_failed(
                        run.id,
                        serde_json::json!({ "message": runtime_error.to_string() }),
                    ),
                )
                .await;
            let _ = background_state
                .runtime_event_stream
                .close_run(
                    run.id,
                    control_plane::ports::RuntimeEventCloseReason::Failed,
                )
                .await;
            error!(
                application_id = %run.application_id,
                flow_run_id = %run.id,
                error = %runtime_error,
                "failed to execute native streaming published run"
            );
        }
    });

    Ok(Sse::new(sse::NativeRunSseStream::new(receiver))
        .keep_alive(KeepAlive::default())
        .into_response())
}

#[utoipa::path(
    get,
    path = "/api/1flowbase/runs/{run_id}",
    params(("run_id" = String, Path, description = "Published run id")),
    responses(
        (status = 200, body = NativeRunResponse),
        (status = 401, body = NativeErrorBody),
        (status = 403, body = NativeErrorBody),
        (status = 404, body = NativeErrorBody)
    )
)]
pub async fn get_native_run(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
) -> Result<Json<ApiSuccess<NativeRunResponse>>, NativeApiError> {
    let bearer_token = bearer_token(&headers)?;
    let run = ApplicationNativeRunService::new(state.store.clone())
        .with_last_used_cache(state.infrastructure.cache_store())
        .get_native_run(GetNativeRunCommand {
            bearer_token,
            run_id,
        })
        .await
        .map_err(native_error)?;

    Ok(Json(ApiSuccess::new(to_native_run_response(run))))
}

#[utoipa::path(
    post,
    path = "/api/1flowbase/runs/{run_id}/cancel",
    params(("run_id" = String, Path, description = "Published run id")),
    responses(
        (status = 200, body = NativeRunResponse),
        (status = 401, body = NativeErrorBody),
        (status = 403, body = NativeErrorBody),
        (status = 404, body = NativeErrorBody),
        (status = 409, body = NativeErrorBody)
    )
)]
pub async fn cancel_native_run(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
) -> Result<Json<ApiSuccess<NativeRunResponse>>, NativeApiError> {
    let bearer_token = bearer_token(&headers)?;
    let run = ApplicationNativeRunService::new(state.store.clone())
        .with_last_used_cache(state.infrastructure.cache_store())
        .cancel_native_run(CancelNativeRunCommand {
            bearer_token,
            run_id,
        })
        .await
        .map_err(native_error)?;

    Ok(Json(ApiSuccess::new(to_native_run_response(run))))
}

#[utoipa::path(
    post,
    path = "/api/1flowbase/runs/{run_id}/resume",
    request_body = ResumeNativeRunBody,
    params(("run_id" = String, Path, description = "Published run id")),
    responses(
        (status = 200, body = NativeRunResponse),
        (status = 401, body = NativeErrorBody),
        (status = 403, body = NativeErrorBody),
        (status = 404, body = NativeErrorBody),
        (status = 409, body = NativeErrorBody),
        (status = 501, body = NativeErrorBody)
    )
)]
pub async fn resume_native_run(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
    Json(body): Json<ResumeNativeRunBody>,
) -> Result<Response, NativeApiError> {
    let bearer_token = bearer_token(&headers)?;
    let response_payload = body.response_payload.clone();
    let response_mode = body.response_mode.clone();
    let native_service = ApplicationNativeRunService::new(state.store.clone())
        .with_last_used_cache(state.infrastructure.cache_store());
    let run_result = native_service
        .resume_native_run(ResumeNativeRunCommand {
            bearer_token: bearer_token.clone(),
            run_id,
            callback_task_id: body.callback_task_id,
            response_payload: response_payload.clone(),
            response_mode,
        })
        .await;
    let run = match run_result {
        Ok(run) => run,
        Err(NativeRunValidationError::ResumeContinuationNotImplemented) => {
            let api_actor = ApplicationApiKeyService::new(state.store.clone())
                .with_last_used_cache(state.infrastructure.cache_store())
                .authenticate_bearer_token(&bearer_token)
                .await
                .map_err(|_| NativeRunValidationError::NotAuthenticated)
                .map_err(native_error)?;
            if body.response_mode.as_deref() == Some("streaming") {
                let initial_run = native_service
                    .get_native_run(GetNativeRunCommand {
                        bearer_token: bearer_token.clone(),
                        run_id,
                    })
                    .await
                    .map_err(native_error)?;
                return resume_native_run_stream(
                    state,
                    initial_run,
                    api_actor.creator_user_id,
                    api_actor.application_id,
                    body.callback_task_id,
                    response_payload,
                )
                .await;
            }
            let detail = OrchestrationRuntimeService::new(
                state.store.clone(),
                ApiProviderRuntime::new(state.provider_runtime.clone()),
                state.runtime_engine.clone(),
                state.provider_secret_master_key.clone(),
            )
            .complete_callback_task(CompleteCallbackTaskCommand {
                actor_user_id: api_actor.creator_user_id,
                application_id: api_actor.application_id,
                callback_task_id: body.callback_task_id,
                response_payload,
            })
            .await
            .map_err(service_error)?;
            native_result_from_application_run_detail(
                &detail,
                serde_json::json!({
                    "external_user": detail.flow_run.external_user,
                    "external_conversation_id": detail.flow_run.external_conversation_id,
                    "external_trace_id": detail.flow_run.external_trace_id,
                    "compatibility_mode": detail.flow_run.compatibility_mode,
                    "idempotency_key": detail.flow_run.idempotency_key,
                    "request": {
                        "conversation": {
                            "id": detail.flow_run.external_conversation_id,
                            "user": detail.flow_run.external_user,
                        }
                    }
                }),
            )
        }
        Err(error) => return Err(native_error(error)),
    };

    Ok(Json(ApiSuccess::new(to_native_run_response(run))).into_response())
}

async fn resume_native_run_stream(
    state: Arc<ApiState>,
    initial_run: NativeRunResult,
    actor_user_id: Uuid,
    application_id: Uuid,
    callback_task_id: Uuid,
    response_payload: Value,
) -> Result<Response, NativeApiError> {
    state
        .runtime_event_stream
        .open_run(
            initial_run.id,
            control_plane::ports::RuntimeEventStreamPolicy::debug_default(),
        )
        .await
        .map_err(service_error)?;
    let _ = state
        .runtime_event_stream
        .append(
            initial_run.id,
            debug_stream_events::flow_started(initial_run.id),
        )
        .await;

    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(sse::send_native_runtime_event_stream(
        state.runtime_event_stream.clone(),
        initial_run.clone(),
        sse::IncludeWorkflowEvents::None,
        sender,
    ));

    let background_state = state.clone();
    tokio::spawn(async move {
        let runtime_service = OrchestrationRuntimeService::new(
            background_state.store.clone(),
            ApiProviderRuntime::new(background_state.provider_runtime.clone()),
            background_state.runtime_engine.clone(),
            background_state.provider_secret_master_key.clone(),
        );
        let result = runtime_service
            .complete_callback_task(CompleteCallbackTaskCommand {
                actor_user_id,
                application_id,
                callback_task_id,
                response_payload,
            })
            .await;
        match result {
            Ok(detail) => {
                append_terminal_sse_event(&background_state, &detail.flow_run).await;
            }
            Err(error) => {
                let _ = background_state
                    .runtime_event_stream
                    .append(
                        initial_run.id,
                        debug_stream_events::flow_failed(
                            initial_run.id,
                            serde_json::json!({ "message": error.to_string() }),
                        ),
                    )
                    .await;
                let _ = background_state
                    .runtime_event_stream
                    .close_run(
                        initial_run.id,
                        control_plane::ports::RuntimeEventCloseReason::Failed,
                    )
                    .await;
            }
        }
    });

    Ok(Sse::new(sse::NativeRunSseStream::new(receiver))
        .keep_alive(KeepAlive::default())
        .into_response())
}

async fn append_terminal_sse_event(state: &ApiState, flow_run: &domain::FlowRunRecord) {
    match flow_run.status {
        domain::FlowRunStatus::Succeeded => {
            let _ = state
                .runtime_event_stream
                .append(
                    flow_run.id,
                    debug_stream_events::flow_finished(
                        flow_run.id,
                        flow_run.output_payload.clone(),
                    ),
                )
                .await;
            let _ = state
                .runtime_event_stream
                .close_run(
                    flow_run.id,
                    control_plane::ports::RuntimeEventCloseReason::Finished,
                )
                .await;
        }
        domain::FlowRunStatus::Failed => {
            let _ = state
                .runtime_event_stream
                .append(
                    flow_run.id,
                    debug_stream_events::flow_failed(
                        flow_run.id,
                        flow_run
                            .error_payload
                            .clone()
                            .unwrap_or_else(|| serde_json::json!({ "message": "run failed" })),
                    ),
                )
                .await;
            let _ = state
                .runtime_event_stream
                .close_run(
                    flow_run.id,
                    control_plane::ports::RuntimeEventCloseReason::Failed,
                )
                .await;
        }
        domain::FlowRunStatus::Cancelled => {
            let _ = state
                .runtime_event_stream
                .append(
                    flow_run.id,
                    debug_stream_events::flow_cancelled(flow_run.id),
                )
                .await;
            let _ = state
                .runtime_event_stream
                .close_run(
                    flow_run.id,
                    control_plane::ports::RuntimeEventCloseReason::Cancelled,
                )
                .await;
        }
        _ => {}
    }
}

#[utoipa::path(
    post,
    path = "/api/1flowbase/files",
    responses(
        (status = 201, body = crate::routes::files::UploadedFileResponse),
        (status = 401, body = NativeErrorBody)
    )
)]
pub async fn upload_native_file(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<ApiSuccess<UploadedFileResponse>>), NativeApiError> {
    let bearer_token = bearer_token(&headers)?;
    let api_actor = ApplicationApiKeyService::new(state.store.clone())
        .with_last_used_cache(state.infrastructure.cache_store())
        .authenticate_bearer_token(&bearer_token)
        .await
        .map_err(|_| {
            NativeApiError::new(
                StatusCode::UNAUTHORIZED,
                "not_authenticated",
                "invalid application API key",
            )
        })?;
    let actor =
        AuthRepository::load_actor_context_for_user(&state.store, api_actor.creator_user_id)
            .await
            .map_err(service_error)?;

    let mut file_table_id = None;
    let mut filename = None;
    let mut content_type = None;
    let mut bytes = None;

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        NativeApiError::new(
            StatusCode::BAD_REQUEST,
            "multipart",
            format!("invalid multipart payload: {error}"),
        )
    })? {
        match field.name() {
            Some("file_table_id") => {
                file_table_id = Some(field.text().await.map_err(|error| {
                    NativeApiError::new(
                        StatusCode::BAD_REQUEST,
                        "file_table_id",
                        format!("invalid file_table_id field: {error}"),
                    )
                })?)
            }
            Some("file") => {
                filename = field.file_name().map(str::to_string);
                content_type = field.content_type().map(str::to_string);
                bytes = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|error| {
                            NativeApiError::new(
                                StatusCode::BAD_REQUEST,
                                "file",
                                format!("invalid file field: {error}"),
                            )
                        })?
                        .to_vec(),
                );
            }
            _ => {}
        }
    }

    let file_table_id = file_table_id
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(|| {
            NativeApiError::new(
                StatusCode::BAD_REQUEST,
                "file_table_id",
                "file_table_id is required",
            )
        })?;
    let bytes = bytes.ok_or_else(|| {
        NativeApiError::new(StatusCode::BAD_REQUEST, "file", "file field is required")
    })?;
    let uploaded = FileUploadService::new(
        state.store.clone(),
        state.file_storage_registry.clone(),
        state.runtime_engine.clone(),
    )
    .upload(UploadFileCommand {
        actor,
        file_table_id,
        original_filename: filename.unwrap_or_else(|| "upload.bin".into()),
        content_type,
        bytes,
    })
    .await
    .map_err(service_error)?;

    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(UploadedFileResponse {
            storage_id: uploaded.storage_id.to_string(),
            record: uploaded.record,
        })),
    ))
}
