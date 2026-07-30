use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    Json,
};
use control_plane::application_public_api::{
    callback_resume::{
        ApplicationPublishedCallbackResumeService, PublishedCallbackResumeSource,
        PublishedCallbackResumeTarget, ResumePublishedCallbackCommand,
    },
    client_protocol_envelope::{
        capture_client_protocol_envelope, capture_client_protocol_query,
        capture_source_protocol_request_body, merge_client_protocol_envelopes,
        ClientProtocolIngressPolicy, ANTHROPIC_BETA_HEADER_NAME,
    },
    compat::anthropic::{
        translate_messages_request_with_context_window, AnthropicCompatError,
        AnthropicContextWindowRequest,
    },
    native::{
        ApplicationNativeRunService, CreateNativeRunCommand, NativeRunRequest, NativeRunResult,
        NativeRunStatus,
    },
    protocol_translation::{
        TranslationDecisionKind, TranslationProtocol, TranslationReport,
        TranslationSafeRepresentation,
    },
};
use control_plane::orchestration_runtime::OrchestrationRuntimeService;
use domain::AiNativeOperation;
use orchestration_runtime::execution_state::NativeOperationTerminal;
use plugin_framework::provider_contract::ProtocolContextEnvelope;
use serde::Serialize;
use serde_json::{json, Value};
use tracing::debug;
use utoipa::ToSchema;
use uuid::Uuid;

mod token_count;

use crate::{
    app_state::ApiState,
    provider_runtime::ApiProviderRuntime,
    routes::application_public_api::{
        callback_adapter::correlate_anthropic_callback, compat_sse,
        llm_tool_visibility::external_llm_tool_calls, native,
        tool_callback_ids::encode_anthropic_callback_tool_use_id,
    },
};
use token_count::{anthropic_usage, to_anthropic_count_tokens_response};

#[derive(Debug, Serialize, ToSchema)]
pub struct AnthropicErrorBody {
    #[serde(rename = "type")]
    pub body_type: &'static str,
    pub error: AnthropicErrorObject,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AnthropicErrorObject {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

#[derive(Debug)]
pub enum AnthropicRouteError {
    Compat(Box<AnthropicCompatError>),
    Native(native::NativeApiError),
    RequiredAction,
}

impl From<AnthropicCompatError> for AnthropicRouteError {
    fn from(error: AnthropicCompatError) -> Self {
        Self::Compat(Box::new(error))
    }
}

impl From<native::NativeApiError> for AnthropicRouteError {
    fn from(error: native::NativeApiError) -> Self {
        Self::Native(error)
    }
}

impl IntoResponse for AnthropicRouteError {
    fn into_response(self) -> Response {
        let (status, error) = match self {
            AnthropicRouteError::Compat(error) => (
                StatusCode::BAD_REQUEST,
                AnthropicErrorObject {
                    error_type: error.error_type,
                    message: error.message,
                },
            ),
            AnthropicRouteError::Native(error) => (
                error.status,
                AnthropicErrorObject {
                    error_type: error.code.to_string(),
                    message: error.message,
                },
            ),
            AnthropicRouteError::RequiredAction => (
                StatusCode::CONFLICT,
                AnthropicErrorObject {
                    error_type: "required_action_not_supported".to_string(),
                    message: "waiting states are not supported by compatible endpoints; use the Native API to inspect and resume required_action runs".to_string(),
                },
            ),
        };
        (
            status,
            Json(AnthropicErrorBody {
                body_type: "error",
                error,
            }),
        )
            .into_response()
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AnthropicMessageResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub response_type: &'static str,
    pub role: &'static str,
    pub model: String,
    pub content: Vec<Value>,
    pub stop_reason: &'static str,
    pub usage: AnthropicUsage,
}

#[derive(Debug, Default, Serialize, ToSchema)]
pub struct AnthropicUsage {
    pub input_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AnthropicCountTokensResponse {
    pub input_tokens: u64,
}

#[utoipa::path(
    post,
    path = "/v1/messages",
    request_body = Value,
    responses(
        (status = 200, body = AnthropicMessageResponse),
        (status = 400, body = AnthropicErrorBody),
        (status = 401, body = AnthropicErrorBody),
        (status = 403, body = AnthropicErrorBody),
        (status = 409, body = AnthropicErrorBody),
        (status = 422, body = AnthropicErrorBody)
    )
)]
pub async fn create_message(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    uri: Uri,
    body: Bytes,
) -> Result<Response, AnthropicRouteError> {
    let bearer_token = anthropic_token(&headers)?;
    let mut value = parse_anthropic_json_body(body)?;
    let source_body = value.clone();
    merge_claude_code_session_header(&mut value, &headers);
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let response_mode = value
        .get("stream")
        .and_then(Value::as_bool)
        .filter(|stream| *stream)
        .map(|_| "streaming".to_string());
    if let Some(resume) = correlate_anthropic_callback(&value)
        .map_err(|error| anthropic_tool_result_error(error.message))?
    {
        let command = anthropic_resume_command(
            &bearer_token,
            resume.callback_task_id,
            resume.tool_results,
            response_mode.clone(),
        );
        match compat_sse::prepare_compatible_resume(state.clone(), command).await {
            Ok(plan) if response_mode.as_deref() == Some("streaming") => {
                return compat_sse::start_anthropic_resume_stream(
                    state,
                    plan.initial_run,
                    model,
                    plan.command,
                )
                .await
                .map_err(Into::into);
            }
            Ok(plan) => {
                let run = execute_anthropic_tool_resume(state, plan.command).await?;
                return Ok(Json(to_anthropic_response(run, model)?).into_response());
            }
            Err(error)
                if error.status == StatusCode::NOT_FOUND && error.code == "callback_task" =>
            {
                // A syntactically valid but stale callback marker is conversation history, not a
                // resume command. Translate the full request below so it creates a fresh run.
            }
            Err(error) => return Err(error.into()),
        }
    }
    let translated = translate_messages_request_with_context_window(
        value,
        anthropic_context_window_request(&headers),
    )?;
    let translation_decision_count = translated.report.decisions.len();
    let mut request = translated.request;
    request.client_protocol_envelope = anthropic_protocol_context_from_ingress(
        uri.query(),
        &headers,
        &source_body,
        request.client_protocol_envelope,
    );
    let model = request.model.clone().unwrap_or(model);
    let response_mode = request.response_mode.clone();
    debug!(
        route = "messages",
        translation_decision_count, "anthropic compatible request translated"
    );
    let run = create_native_run(state.clone(), bearer_token.clone(), request).await?;

    if response_mode.as_deref() == Some("streaming") {
        return compat_sse::start_anthropic_run_stream(state, run, model)
            .await
            .map_err(Into::into);
    }

    let run = native::execute_blocking_native_run(state, bearer_token, run).await?;
    Ok(Json(to_anthropic_response(run, model)?).into_response())
}

#[utoipa::path(
    post,
    path = "/v1/messages/count_tokens",
    request_body = Value,
    responses(
        (status = 200, body = AnthropicCountTokensResponse),
        (status = 400, body = AnthropicErrorBody),
        (status = 401, body = AnthropicErrorBody),
        (status = 409, body = AnthropicErrorBody),
        (status = 422, body = AnthropicErrorBody),
        (status = 429, body = AnthropicErrorBody),
        (status = 502, body = AnthropicErrorBody)
    )
)]
pub async fn count_message_tokens(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    uri: Uri,
    body: Bytes,
) -> Result<Json<AnthropicCountTokensResponse>, AnthropicRouteError> {
    let bearer_token = anthropic_token(&headers)?;
    let mut value = parse_anthropic_json_body(body)?;
    let source_body = value.clone();
    merge_claude_code_session_header(&mut value, &headers);
    let mut translation = translate_messages_request_with_context_window(
        value,
        anthropic_context_window_request(&headers),
    )?;
    translation.request.client_protocol_envelope = anthropic_protocol_context_from_ingress(
        uri.query(),
        &headers,
        &source_body,
        translation.request.client_protocol_envelope,
    );
    translation
        .request
        .execution
        .set_execution_operation(AiNativeOperation::CountTokens);
    debug!(
        route = "messages_count_tokens",
        translation_decision_count = translation.report.decisions.len(),
        "anthropic count tokens request translated"
    );
    let run = create_native_run(state.clone(), bearer_token.clone(), translation.request).await?;
    let run = native::execute_blocking_native_run(state, bearer_token, run).await?;
    let input_tokens = match run.operation_terminal.as_ref() {
        Some(NativeOperationTerminal::CountTokens(receipt)) => receipt.input_tokens(),
        _ => return Err(native::blocking_run_projection_error(&run).into()),
    };
    Ok(Json(to_anthropic_count_tokens_response(input_tokens)))
}

fn anthropic_token(headers: &HeaderMap) -> Result<String, native::NativeApiError> {
    if let Ok(token) = native::bearer_token(headers) {
        return Ok(token);
    }
    headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            native::NativeApiError::new(
                StatusCode::UNAUTHORIZED,
                "not_authenticated",
                "missing Authorization bearer token or x-api-key",
            )
        })
}

fn parse_anthropic_json_body(body: Bytes) -> Result<Value, AnthropicRouteError> {
    serde_json::from_slice::<Value>(&body).map_err(|_| {
        let mut report = TranslationReport::new(TranslationProtocol::AnthropicMessages);
        report.record(
            "$.body",
            None,
            TranslationDecisionKind::Rejected,
            Some("invalid JSON body"),
            TranslationSafeRepresentation::Present,
        );
        AnthropicCompatError {
            message: "invalid JSON body".to_string(),
            error_type: "invalid_request".to_string(),
            report,
        }
        .into()
    })
}

fn merge_claude_code_session_header(value: &mut Value, headers: &HeaderMap) {
    let Some(session_id) = headers
        .get("x-claude-code-session-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let metadata = object.entry("metadata").or_insert_with(|| json!({}));
    let Some(metadata) = metadata.as_object_mut() else {
        return;
    };
    metadata
        .entry("session_id".to_string())
        .or_insert_with(|| Value::String(session_id.to_string()));
}

fn anthropic_protocol_context_from_ingress(
    raw_query: Option<&str>,
    headers: &HeaderMap,
    source_body: &Value,
    translated: Option<ProtocolContextEnvelope>,
) -> Option<ProtocolContextEnvelope> {
    let policy = ClientProtocolIngressPolicy::AnthropicMessages;
    let captured = merge_client_protocol_envelopes(
        policy,
        capture_client_protocol_envelope(
            policy,
            headers.iter().filter_map(|(name, value)| {
                value.to_str().ok().map(|value| (name.as_str(), value))
            }),
        ),
        capture_client_protocol_query(
            policy,
            form_urlencoded::parse(raw_query.unwrap_or_default().as_bytes()),
        ),
    );
    let captured = merge_client_protocol_envelopes(
        policy,
        captured,
        capture_source_protocol_request_body(policy, source_body),
    );
    merge_client_protocol_envelopes(policy, captured, translated)
}

fn anthropic_context_window_request(headers: &HeaderMap) -> Option<AnthropicContextWindowRequest> {
    AnthropicContextWindowRequest::from_beta_values(
        headers
            .get_all(ANTHROPIC_BETA_HEADER_NAME)
            .iter()
            .filter_map(|value| value.to_str().ok()),
    )
}

async fn create_native_run(
    state: Arc<ApiState>,
    bearer_token: String,
    request: NativeRunRequest,
) -> Result<NativeRunResult, native::NativeApiError> {
    let protocol_context = request.client_protocol_envelope.clone();
    let run = ApplicationNativeRunService::new(state.store.clone())
        .with_last_used_cache(state.infrastructure.cache_store())
        .create_native_run(CreateNativeRunCommand {
            bearer_token,
            request,
            protocol: TranslationProtocol::AnthropicMessages,
        })
        .await
        .map_err(native::native_error)?;
    native::stage_client_protocol_context(
        state.infrastructure.provider_transport_store().as_ref(),
        &run,
        protocol_context,
    )
    .await?;
    Ok(run)
}

async fn execute_anthropic_tool_resume(
    state: Arc<ApiState>,
    command: ResumePublishedCallbackCommand,
) -> Result<NativeRunResult, AnthropicRouteError> {
    let runtime_service = OrchestrationRuntimeService::new(
        state.store.clone(),
        ApiProviderRuntime::new(state.provider_runtime.clone()),
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
    .with_provider_transport_store(state.infrastructure.provider_transport_store())
    .with_runtime_event_stream(state.runtime_event_stream.clone());
    let result =
        ApplicationPublishedCallbackResumeService::new(state.store.clone(), runtime_service)
            .with_last_used_cache(state.infrastructure.cache_store())
            .resume_callback(command)
            .await
            .map_err(native::service_error)?;
    Ok(result.run)
}

fn anthropic_resume_command(
    bearer_token: &str,
    callback_task_id: Uuid,
    tool_results: Value,
    response_mode: Option<String>,
) -> ResumePublishedCallbackCommand {
    ResumePublishedCallbackCommand {
        bearer_token: bearer_token.to_string(),
        target: PublishedCallbackResumeTarget::CallbackTask { callback_task_id },
        source: PublishedCallbackResumeSource::AnthropicMessages,
        response_payload: json!({ "tool_results": tool_results }),
        response_mode,
    }
}

fn anthropic_tool_result_error(message: &str) -> AnthropicRouteError {
    AnthropicCompatError {
        message: message.to_string(),
        error_type: "invalid_request".to_string(),
        report: TranslationReport::new(TranslationProtocol::AnthropicMessages),
    }
    .into()
}

fn to_anthropic_response(
    run: NativeRunResult,
    model: String,
) -> Result<AnthropicMessageResponse, AnthropicRouteError> {
    let callback_task_id = callback_task_id_from_required_action(&run);
    let tool_blocks = anthropic_tool_use_blocks(run.tool_calls.as_ref(), callback_task_id);
    let has_tool_blocks = tool_blocks
        .as_ref()
        .is_some_and(|blocks| !blocks.is_empty());
    let terminal_stop_reason = match run.status {
        NativeRunStatus::Succeeded => "end_turn",
        NativeRunStatus::Incomplete => "max_tokens",
        NativeRunStatus::Waiting if has_tool_blocks => "tool_use",
        NativeRunStatus::Waiting => return Err(AnthropicRouteError::RequiredAction),
        NativeRunStatus::Created
        | NativeRunStatus::Queued
        | NativeRunStatus::Running
        | NativeRunStatus::Failed
        | NativeRunStatus::Cancelled => {
            return Err(native::blocking_run_projection_error(&run).into())
        }
    };
    let mut content = Vec::new();
    if let Some(answer) = run.answer {
        if !answer.is_empty() {
            content.push(json!({"type": "text", "text": answer}));
        }
    }
    if let Some(blocks) = tool_blocks {
        content.extend(blocks);
    }
    if content.is_empty() {
        content.push(json!({"type": "text", "text": ""}));
    }
    Ok(AnthropicMessageResponse {
        id: format!("msg_{}", run.id),
        response_type: "message",
        role: "assistant",
        model,
        content,
        stop_reason: if has_tool_blocks {
            "tool_use"
        } else {
            terminal_stop_reason
        },
        usage: anthropic_usage(run.usage),
    })
}

fn anthropic_tool_use_blocks(
    tool_calls: Option<&Value>,
    callback_task_id: Option<Uuid>,
) -> Option<Vec<Value>> {
    let calls = external_llm_tool_calls(tool_calls)?;
    let mapped = calls
        .iter()
        .filter_map(|call| {
            let name = call.get("name").and_then(Value::as_str)?;
            let original_id = call
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("toolu_call")
                .to_string();
            let id = callback_task_id
                .map(|callback_task_id| {
                    encode_anthropic_callback_tool_use_id(callback_task_id, &original_id)
                })
                .unwrap_or(original_id);
            let input = call.get("arguments").cloned().unwrap_or_else(|| json!({}));
            Some(json!({
                "type": "tool_use",
                "id": id,
                "name": name,
                "input": input
            }))
        })
        .collect::<Vec<_>>();
    (!mapped.is_empty()).then_some(mapped)
}

fn callback_task_id_from_required_action(run: &NativeRunResult) -> Option<Uuid> {
    run.required_action
        .as_ref()
        .and_then(|action| action.payload.get("callback_task_id"))
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
}

#[cfg(test)]
mod tests;
