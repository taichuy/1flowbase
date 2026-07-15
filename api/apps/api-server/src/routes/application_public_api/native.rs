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
        callback_resume::{
            ApplicationPublishedCallbackResumeService, PublishedCallbackResumeSource,
            PublishedCallbackResumeTarget, ResumePublishedCallbackCommand,
        },
        model_catalog::{
            extract_agent_model_catalog_from_start_node, AgentModelCapabilities,
            AgentModelDescriptor, AgentModelReasoning,
        },
        native::{
            ApplicationNativeRunService, CancelNativeRunCommand, CreateNativeRunCommand,
            GetNativeRunCommand, NativeRunRequest, NativeRunResult, NativeRunValidationError,
        },
        publications::{ApplicationPublicationService, LoadActiveApplicationPublicationCommand},
        run_service::native_result_from_run_detail,
    },
    file_management::{FileUploadService, UploadFileCommand},
    orchestration_runtime::{
        debug_stream_events, OrchestrationRuntimeService, StartPublishedFlowRunCommand,
    },
    ports::AuthRepository,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tracing::error;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    provider_runtime::ApiProviderRuntime,
    response::ApiSuccess,
    routes::{application_public_api::sse, files::UploadedFileResponse},
    runtime_activity::{scope_application_activity, ApplicationActivityKind},
};

pub(crate) fn api_provider_runtime(state: &ApiState) -> ApiProviderRuntime {
    ApiProviderRuntime::new_with_activity(
        state.provider_runtime.clone(),
        state.runtime_activity.clone(),
    )
}

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer_segments: Option<Value>,
    pub required_action: Option<Value>,
    pub tool_calls: Option<Value>,
    pub usage: Option<Value>,
    pub error: Option<Value>,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NativeModelListResponse {
    pub object: &'static str,
    pub data: Vec<NativeModelObject>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NativeModelObject {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_context_window: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_compact_token_limit: Option<u64>,
    pub capabilities: NativeModelCapabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<NativeModelReasoning>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NativeModelCapabilities {
    pub reasoning: bool,
    pub tool_call: bool,
    pub multimodal: bool,
    pub structured_output: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NativeModelReasoning {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_effort: Option<String>,
    pub supported_efforts: Vec<String>,
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

#[utoipa::path(
    get,
    path = "/api/agent/v1/models",
    operation_id = "list_native_agent_models",
    responses(
        (status = 200, body = NativeModelListResponse),
        (status = 401, body = NativeErrorBody),
        (status = 403, body = NativeErrorBody),
        (status = 409, body = NativeErrorBody)
    )
)]
pub async fn list_native_models(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<NativeModelListResponse>, NativeApiError> {
    let bearer_token = bearer_token(&headers)?;
    let actor = ApplicationApiKeyService::new(state.store.clone())
        .with_last_used_cache(state.infrastructure.cache_store())
        .authenticate_bearer_token(&bearer_token)
        .await
        .map_err(|_| native_error(NativeRunValidationError::NotAuthenticated))?;
    let publication = ApplicationPublicationService::new(state.store.clone())
        .load_active_publication(LoadActiveApplicationPublicationCommand {
            application_id: actor.application_id,
        })
        .await
        .map_err(|_| native_error(NativeRunValidationError::ApplicationNotPublished))?;
    let models = extract_agent_model_catalog_from_start_node(&publication.document_snapshot)
        .into_iter()
        .map(NativeModelObject::from)
        .collect();

    Ok(Json(NativeModelListResponse {
        object: "list",
        data: models,
    }))
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
        NativeRunValidationError::InvalidModelParameters(field) => NativeApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_model_parameters",
            format!("invalid native request model parameter field: {field}"),
        ),
        NativeRunValidationError::InvalidToolResults(message) => {
            NativeApiError::new(StatusCode::BAD_REQUEST, "tool_results", message)
        }
        NativeRunValidationError::InvalidState => NativeApiError::new(
            StatusCode::CONFLICT,
            "invalid_state",
            "run is not in a valid state for this operation",
        ),
        NativeRunValidationError::IdempotencyConflict => NativeApiError::new(
            StatusCode::CONFLICT,
            "idempotency_conflict",
            "idempotency key was already used with a different request",
        ),
    }
}

impl From<AgentModelDescriptor> for NativeModelObject {
    fn from(model: AgentModelDescriptor) -> Self {
        Self {
            id: model.id,
            name: model.name,
            context_window: model.context_window,
            max_context_window: model.max_context_window.or(model.context_window),
            max_output_tokens: model.max_output_tokens,
            auto_compact_token_limit: model.auto_compact_token_limit,
            capabilities: NativeModelCapabilities::from(model.capabilities),
            reasoning: model.reasoning.map(NativeModelReasoning::from),
        }
    }
}

impl From<AgentModelCapabilities> for NativeModelCapabilities {
    fn from(capabilities: AgentModelCapabilities) -> Self {
        Self {
            reasoning: capabilities.reasoning,
            tool_call: capabilities.tool_call,
            multimodal: capabilities.multimodal,
            structured_output: capabilities.structured_output,
        }
    }
}

impl From<AgentModelReasoning> for NativeModelReasoning {
    fn from(reasoning: AgentModelReasoning) -> Self {
        Self {
            default_effort: reasoning.default_effort,
            supported_efforts: reasoning.supported_efforts,
        }
    }
}

pub(crate) fn service_error(error: anyhow::Error) -> NativeApiError {
    if error
        .downcast_ref::<control_plane::errors::ControlPlaneError>()
        .is_some_and(|error| {
            matches!(
                error,
                control_plane::errors::ControlPlaneError::NotAuthenticated
            )
        })
    {
        return NativeApiError::new(
            StatusCode::UNAUTHORIZED,
            "not_authenticated",
            "invalid application API key",
        );
    }
    if let Some(control_plane::errors::ControlPlaneError::PermissionDenied(reason)) =
        error.downcast_ref::<control_plane::errors::ControlPlaneError>()
    {
        return NativeApiError::new(StatusCode::FORBIDDEN, reason, error.to_string());
    }
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
    let message = error.to_string();
    if is_llm_tool_result_validation_error(&message) {
        return NativeApiError::new(StatusCode::BAD_REQUEST, "tool_results", message);
    }
    NativeApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", message)
}

fn is_llm_tool_result_validation_error(message: &str) -> bool {
    [
        "llm tool callback response requires tool_results",
        "llm tool callback result is missing tool_call_id",
        "unexpected tool result for ",
        "duplicate tool result for ",
        "missing tool result for ",
    ]
    .iter()
    .any(|prefix| message.starts_with(prefix))
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
    let mut request: NativeRunRequest = serde_json::from_value(value)
        .map_err(|error| NativeApiError::new(StatusCode::BAD_REQUEST, "body", error.to_string()))?;
    request.protocol_compatibility_mode = Some("native-v1".to_string());
    Ok(request)
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
        answer_segments: run
            .answer_segments
            .and_then(|segments| serde_json::to_value(segments).ok()),
        required_action: run
            .required_action
            .and_then(|value| serde_json::to_value(value).ok()),
        tool_calls: run.tool_calls,
        usage: run.usage.and_then(|value| serde_json::to_value(value).ok()),
        error: run.error.and_then(|value| serde_json::to_value(value).ok()),
        created_at: run.created_at.to_string(),
    }
}

pub(crate) fn published_run_metadata(flow_run: &domain::FlowRunRecord) -> Value {
    json!({
        "title": flow_run.title,
        "expand_id": flow_run.external_user,
        "external_user": flow_run.external_user,
        "external_conversation_id": flow_run.external_conversation_id,
        "external_trace_id": flow_run.external_trace_id,
        "compatibility_mode": flow_run.compatibility_mode,
        "idempotency_key": flow_run.idempotency_key,
        "request": {
            "conversation": {
                "id": flow_run.external_conversation_id,
                "user": flow_run.external_user,
            }
        }
    })
}

pub(crate) async fn execute_blocking_native_run(
    state: Arc<ApiState>,
    bearer_token: String,
    run: NativeRunResult,
) -> Result<NativeRunResult, NativeApiError> {
    let _execution_activity = state.runtime_activity.start(
        run.application_id,
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
    .with_provider_request_log_queue(state.infrastructure.task_queue());
    let execution_result = scope_application_activity(
        run.application_id,
        runtime_service.start_published_flow_run(StartPublishedFlowRunCommand {
            application_id: run.application_id,
            flow_run_id: run.id,
        }),
    )
    .await;
    match execution_result {
        Ok(detail) => Ok(native_result_from_run_detail(&detail, run.metadata.clone())),
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
    path = "/api/agent/v1/runs",
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
    let _http_activity = state
        .runtime_activity
        .start(run.application_id, ApplicationActivityKind::HttpRequest);

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
        state.clone(),
        run.clone(),
        include_workflow_events,
        None,
        None,
        sender,
    ));

    let background_state = state.clone();
    tokio::spawn(async move {
        let _execution_activity = background_state.runtime_activity.start(
            run.application_id,
            ApplicationActivityKind::ApplicationExecution,
        );
        let runtime_service = OrchestrationRuntimeService::new(
            background_state.store.clone(),
            api_provider_runtime(&background_state),
            background_state.runtime_engine.clone(),
            background_state.provider_secret_master_key.clone(),
        )
        .with_node_artifact_context(
            background_state.api_node_id.clone(),
            background_state.provider_install_root.clone(),
        )
        .with_file_storage_registry(background_state.file_storage_registry.clone())
        .with_llm_routing_counter_store(background_state.infrastructure.cache_store())
        .with_provider_request_log_queue(background_state.infrastructure.task_queue())
        .with_runtime_event_stream(background_state.runtime_event_stream.clone());
        if let Err(runtime_error) = scope_application_activity(
            run.application_id,
            runtime_service.start_published_flow_run(StartPublishedFlowRunCommand {
                application_id: run.application_id,
                flow_run_id: run.id,
            }),
        )
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

    let sse_activity = state
        .runtime_activity
        .start(run.application_id, ApplicationActivityKind::SseConnection);
    let stream = sse::NativeRunSseStream::new(receiver).map(move |event| {
        let _keep_alive = &sse_activity;
        event
    });

    Ok(Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response())
}

#[utoipa::path(
    get,
    path = "/api/agent/v1/runs/{run_id}",
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
    path = "/api/agent/v1/runs/{run_id}/cancel",
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
    path = "/api/agent/v1/runs/{run_id}/resume",
    request_body = ResumeNativeRunBody,
    params(("run_id" = String, Path, description = "Published run id")),
    responses(
        (status = 200, body = NativeRunResponse),
        (status = 401, body = NativeErrorBody),
        (status = 403, body = NativeErrorBody),
        (status = 404, body = NativeErrorBody),
        (status = 409, body = NativeErrorBody)
    )
)]
pub async fn resume_native_run(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
    Json(body): Json<ResumeNativeRunBody>,
) -> Result<Response, NativeApiError> {
    let bearer_token = bearer_token(&headers)?;
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
    let result =
        ApplicationPublishedCallbackResumeService::new(state.store.clone(), runtime_service)
            .with_last_used_cache(state.infrastructure.cache_store())
            .resume_callback(ResumePublishedCallbackCommand {
                bearer_token,
                target: PublishedCallbackResumeTarget::FlowRun {
                    flow_run_id: run_id,
                    callback_task_id: body.callback_task_id,
                },
                source: PublishedCallbackResumeSource::NativeAgent,
                response_payload: body.response_payload,
                response_mode: body.response_mode,
            })
            .await
            .map_err(service_error)?;
    let run = control_plane::application_public_api::run_service::native_result_from_run_detail(
        &result.detail,
        published_run_metadata(&result.detail.flow_run),
    );

    Ok(Json(ApiSuccess::new(to_native_run_response(run))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/agent/v1/files",
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
