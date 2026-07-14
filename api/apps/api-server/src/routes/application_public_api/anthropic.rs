use std::{collections::HashSet, sync::Arc};

use axum::{
    Json,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use control_plane::application_public_api::{
    api_keys::ApplicationApiKeyService,
    callback_resume::{
        ApplicationPublishedCallbackResumeService, PublishedCallbackResumeSource,
        PublishedCallbackResumeTarget, ResumePublishedCallbackCommand,
    },
    client_protocol_envelope::{
        ClientProtocolIngressPolicy, capture_client_protocol_envelope,
        merge_anthropic_messages_envelopes,
    },
    compat::anthropic::{
        AnthropicCompatError, anthropic_content_is_tool_result_only, map_messages_request,
        sanitize_anthropic_compat_assistant_text,
    },
    native::{
        ApplicationNativeRunService, CreateNativeRunCommand, GetNativeRunCommand, NativeRunRequest,
        NativeRunResult, NativeRunValidationError,
    },
    run_service::{
        ApplicationPublishedRunControlRepository, ListWaitingCallbackPublishedRunsInput,
    },
};
use control_plane::orchestration_runtime::OrchestrationRuntimeService;
use plugin_framework::provider_contract::ClientProtocolEnvelope;
use serde::Serialize;
use serde_json::{Value, json};
use utoipa::ToSchema;
use uuid::Uuid;

mod token_count;

use crate::{
    app_state::ApiState,
    provider_runtime::ApiProviderRuntime,
    routes::application_public_api::{
        compat_sse,
        llm_tool_visibility::external_llm_tool_calls,
        native,
        tool_callback_ids::{
            decode_anthropic_callback_tool_use_id, encode_anthropic_callback_tool_use_id,
        },
    },
};
#[cfg(test)]
use token_count::anthropic_count_input_tokens;
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
    Compat(AnthropicCompatError),
    Native(native::NativeApiError),
    RequiredAction,
}

impl From<AnthropicCompatError> for AnthropicRouteError {
    fn from(error: AnthropicCompatError) -> Self {
        Self::Compat(error)
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

#[derive(Debug)]
struct AnthropicToolResumeRequest {
    callback_task_id: Uuid,
    tool_results: Value,
}

struct AnthropicToolResumePlan {
    initial_run: NativeRunResult,
    command: ResumePublishedCallbackCommand,
}

struct AnthropicToolResumeCandidate {
    raw_results: Vec<AnthropicRawToolResult>,
    decoded_results: Vec<AnthropicDecodedToolResult>,
}

struct AnthropicPlainStdoutResumeCandidate {
    content: String,
    tool_use_ids: HashSet<String>,
}

struct AnthropicRawToolResult {
    tool_use_id: String,
    content: Value,
}

struct AnthropicDecodedToolResult {
    callback_task_id: Uuid,
    original_tool_use_id: String,
    content: Value,
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
        (status = 409, body = AnthropicErrorBody)
    )
)]
pub async fn create_message(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, AnthropicRouteError> {
    let bearer_token = anthropic_token(&headers)?;
    let mut value = parse_anthropic_json_body(body)?;
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
    if let Some(resume) =
        anthropic_tool_resume_request_for_route(state.clone(), &bearer_token, &value).await?
    {
        if response_mode.as_deref() == Some("streaming") {
            let resume_plan = prepare_anthropic_tool_resume(
                state.clone(),
                &bearer_token,
                resume.callback_task_id,
                resume.tool_results,
            )
            .await?;
            return compat_sse::start_anthropic_resume_stream(
                state,
                resume_plan.initial_run,
                model,
                resume_plan.command,
            )
            .await
            .map_err(Into::into);
        }
        let run = resume_anthropic_tool_call(
            state,
            &bearer_token,
            resume.callback_task_id,
            resume.tool_results,
        )
        .await?;
        if !anthropic_required_action_is_supported(&run) {
            return Err(AnthropicRouteError::RequiredAction);
        }
        if response_mode.as_deref() == Some("streaming") {
            return Ok(compat_sse::completed_anthropic_stream(run, model));
        }
        return Ok(Json(to_anthropic_response(run, model)).into_response());
    }
    if let Some(resume) =
        anthropic_plain_stdout_resume_request_for_route(state.clone(), &bearer_token, &value)
            .await?
    {
        if response_mode.as_deref() == Some("streaming") {
            let resume_plan = prepare_anthropic_tool_resume(
                state.clone(),
                &bearer_token,
                resume.callback_task_id,
                resume.tool_results,
            )
            .await?;
            return compat_sse::start_anthropic_resume_stream(
                state,
                resume_plan.initial_run,
                model,
                resume_plan.command,
            )
            .await
            .map_err(Into::into);
        }
        let run = resume_anthropic_tool_call(
            state,
            &bearer_token,
            resume.callback_task_id,
            resume.tool_results,
        )
        .await?;
        if !anthropic_required_action_is_supported(&run) {
            return Err(AnthropicRouteError::RequiredAction);
        }
        return Ok(Json(to_anthropic_response(run, model)).into_response());
    }
    let mut request = map_messages_request(value)?;
    request.client_protocol_envelope = merge_anthropic_messages_envelopes(
        anthropic_client_protocol_envelope_from_headers(&headers),
        request.client_protocol_envelope,
    );
    let response_mode = request.response_mode.clone();
    let run = create_native_run(state.clone(), bearer_token.clone(), request).await?;

    if response_mode.as_deref() == Some("streaming") {
        return compat_sse::start_anthropic_run_stream(state, run, model)
            .await
            .map_err(Into::into);
    }

    let run = native::execute_blocking_native_run(state, bearer_token, run).await?;
    if !anthropic_required_action_is_supported(&run) {
        return Err(AnthropicRouteError::RequiredAction);
    }
    Ok(Json(to_anthropic_response(run, model)).into_response())
}

#[utoipa::path(
    post,
    path = "/v1/messages/count_tokens",
    request_body = Value,
    responses(
        (status = 200, body = AnthropicCountTokensResponse),
        (status = 400, body = AnthropicErrorBody),
        (status = 401, body = AnthropicErrorBody)
    )
)]
pub async fn count_message_tokens(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<AnthropicCountTokensResponse>, AnthropicRouteError> {
    let bearer_token = anthropic_token(&headers)?;
    let mut value = parse_anthropic_json_body(body)?;
    merge_claude_code_session_header(&mut value, &headers);
    authenticate_anthropic_token(state.as_ref(), &bearer_token).await?;
    Ok(Json(to_anthropic_count_tokens_response(&value)))
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
        AnthropicCompatError {
            message: "invalid JSON body".to_string(),
            error_type: "invalid_request".to_string(),
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

fn anthropic_client_protocol_envelope_from_headers(
    headers: &HeaderMap,
) -> Option<ClientProtocolEnvelope> {
    capture_client_protocol_envelope(
        ClientProtocolIngressPolicy::AnthropicMessages,
        headers
            .iter()
            .filter_map(|(name, value)| value.to_str().ok().map(|value| (name.as_str(), value))),
    )
}

async fn authenticate_anthropic_token(
    state: &ApiState,
    bearer_token: &str,
) -> Result<(), native::NativeApiError> {
    ApplicationApiKeyService::new(state.store.clone())
        .with_last_used_cache(state.infrastructure.cache_store())
        .authenticate_bearer_token(bearer_token)
        .await
        .map(|_| ())
        .map_err(|_| native::native_error(NativeRunValidationError::NotAuthenticated))
}

async fn create_native_run(
    state: Arc<ApiState>,
    bearer_token: String,
    request: NativeRunRequest,
) -> Result<NativeRunResult, native::NativeApiError> {
    ApplicationNativeRunService::new(state.store.clone())
        .with_last_used_cache(state.infrastructure.cache_store())
        .create_native_run(CreateNativeRunCommand {
            bearer_token,
            request,
        })
        .await
        .map_err(native::native_error)
}

async fn resume_anthropic_tool_call(
    state: Arc<ApiState>,
    bearer_token: &str,
    callback_task_id: Uuid,
    tool_results: Value,
) -> Result<NativeRunResult, AnthropicRouteError> {
    let response_payload = anthropic_tool_resume_response_payload(tool_results);
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
    .with_runtime_event_stream(state.runtime_event_stream.clone());
    let result =
        ApplicationPublishedCallbackResumeService::new(state.store.clone(), runtime_service)
            .with_last_used_cache(state.infrastructure.cache_store())
            .resume_callback(ResumePublishedCallbackCommand {
                bearer_token: bearer_token.to_string(),
                target: PublishedCallbackResumeTarget::CallbackTask { callback_task_id },
                source: PublishedCallbackResumeSource::AnthropicMessages,
                response_payload,
                response_mode: None,
            })
            .await
            .map_err(native::service_error)?;

    Ok(
        control_plane::application_public_api::run_service::native_result_from_run_detail(
            &result.detail,
            native::published_run_metadata(&result.detail.flow_run),
        ),
    )
}

fn anthropic_tool_resume_response_payload(tool_results: Value) -> Value {
    json!({ "tool_results": tool_results })
}

async fn prepare_anthropic_tool_resume(
    state: Arc<ApiState>,
    bearer_token: &str,
    callback_task_id: Uuid,
    tool_results: Value,
) -> Result<AnthropicToolResumePlan, AnthropicRouteError> {
    let callback_task = state
        .store
        .get_published_callback_task(callback_task_id)
        .await
        .map_err(native::service_error)?
        .ok_or_else(|| {
            native::NativeApiError::new(
                StatusCode::NOT_FOUND,
                "callback_task",
                "callback task was not found",
            )
        })?;
    let initial_run = ApplicationNativeRunService::new(state.store.clone())
        .with_last_used_cache(state.infrastructure.cache_store())
        .get_native_run(GetNativeRunCommand {
            bearer_token: bearer_token.to_string(),
            run_id: callback_task.flow_run_id,
        })
        .await
        .map_err(native::native_error)?;

    Ok(AnthropicToolResumePlan {
        initial_run,
        command: ResumePublishedCallbackCommand {
            bearer_token: bearer_token.to_string(),
            target: PublishedCallbackResumeTarget::CallbackTask { callback_task_id },
            source: PublishedCallbackResumeSource::AnthropicMessages,
            response_payload: anthropic_tool_resume_response_payload(tool_results),
            response_mode: Some("streaming".into()),
        },
    })
}

fn to_anthropic_response(run: NativeRunResult, model: String) -> AnthropicMessageResponse {
    let callback_task_id = callback_task_id_from_required_action(&run);
    let tool_blocks = anthropic_tool_use_blocks(run.tool_calls.as_ref(), callback_task_id);
    let has_tool_blocks = tool_blocks
        .as_ref()
        .is_some_and(|blocks| !blocks.is_empty());
    let mut content = Vec::new();
    if let Some(answer) = run.answer {
        if let Some(answer) = sanitize_anthropic_compat_assistant_text(&answer) {
            if !answer.is_empty() {
                content.push(json!({"type": "text", "text": answer}));
            }
        }
    }
    if let Some(blocks) = tool_blocks {
        content.extend(blocks);
    }
    if content.is_empty() {
        content.push(json!({"type": "text", "text": ""}));
    }
    AnthropicMessageResponse {
        id: format!("msg_{}", run.id),
        response_type: "message",
        role: "assistant",
        model,
        content,
        stop_reason: if has_tool_blocks {
            "tool_use"
        } else {
            "end_turn"
        },
        usage: anthropic_usage(run.usage),
    }
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

#[cfg(test)]
fn anthropic_tool_resume_request(
    request: &Value,
) -> Result<Option<AnthropicToolResumeRequest>, AnthropicRouteError> {
    let Some(candidate) = anthropic_tool_resume_candidate(request)? else {
        return Ok(None);
    };
    candidate
        .encoded_resume_request()
        .map(Some)
        .ok_or_else(anthropic_tool_result_orphan_error)
}

async fn anthropic_tool_resume_request_for_route(
    state: Arc<ApiState>,
    bearer_token: &str,
    request: &Value,
) -> Result<Option<AnthropicToolResumeRequest>, AnthropicRouteError> {
    if let Some(candidate) = anthropic_tool_resume_candidate(request)? {
        if let Some(resume) = candidate.encoded_resume_request() {
            return Ok(Some(resume));
        }
        let Some(resume) = resolve_plain_anthropic_tool_resume(
            state,
            bearer_token,
            request,
            &candidate.raw_results,
        )
        .await?
        else {
            return Err(anthropic_tool_result_orphan_error());
        };
        return Ok(Some(resume));
    }

    resolve_embedded_anthropic_encoded_tool_resume(state, bearer_token, request).await
}

fn anthropic_tool_resume_candidate(
    request: &Value,
) -> Result<Option<AnthropicToolResumeCandidate>, AnthropicRouteError> {
    let Some(messages) = request.get("messages").and_then(Value::as_array) else {
        return Ok(None);
    };
    let mut trailing_tool_result_messages = messages
        .iter()
        .rev()
        .take_while(|message| anthropic_message_has_only_tool_results(message))
        .collect::<Vec<_>>();
    if trailing_tool_result_messages.is_empty() {
        return Ok(None);
    }
    trailing_tool_result_messages.reverse();
    let trailing_start = messages.len() - trailing_tool_result_messages.len();
    let matching_tool_use_ids = anthropic_trailing_assistant_tool_use_ids(messages, trailing_start);

    let mut raw_results = Vec::new();
    let mut decoded_results = Vec::new();

    for message in trailing_tool_result_messages {
        let Some(blocks) = message.get("content").and_then(Value::as_array) else {
            continue;
        };
        for block in blocks {
            if block.get("type").and_then(Value::as_str) != Some("tool_result") {
                continue;
            }
            let Some(tool_use_id) = block.get("tool_use_id").and_then(Value::as_str) else {
                return Err(AnthropicCompatError {
                    message: "tool_result tool_use_id is required".to_string(),
                    error_type: "invalid_request".to_string(),
                }
                .into());
            };
            if !matching_tool_use_ids.is_empty() && !matching_tool_use_ids.contains(tool_use_id) {
                continue;
            }
            let content = anthropic_tool_result_content(block);
            raw_results.push(AnthropicRawToolResult {
                tool_use_id: tool_use_id.to_string(),
                content: content.clone(),
            });
            let Some((decoded_callback_task_id, original_tool_use_id)) =
                decode_anthropic_callback_tool_use_id(tool_use_id)
            else {
                continue;
            };
            decoded_results.push(AnthropicDecodedToolResult {
                callback_task_id: decoded_callback_task_id,
                original_tool_use_id,
                content,
            });
        }
    }

    if raw_results.is_empty() {
        return Err(anthropic_tool_result_orphan_error());
    }

    Ok(Some(AnthropicToolResumeCandidate {
        raw_results,
        decoded_results,
    }))
}

impl AnthropicToolResumeCandidate {
    fn encoded_resume_request(&self) -> Option<AnthropicToolResumeRequest> {
        let callback_task_id = self
            .decoded_results
            .last()
            .map(|result| result.callback_task_id)?;
        let mut tool_results = self
            .decoded_results
            .iter()
            .rev()
            .take_while(|result| result.callback_task_id == callback_task_id)
            .map(|result| {
                json!({
                    "tool_call_id": result.original_tool_use_id,
                    "content": result.content,
                })
            })
            .collect::<Vec<_>>();
        tool_results.reverse();

        Some(AnthropicToolResumeRequest {
            callback_task_id,
            tool_results: Value::Array(tool_results),
        })
    }
}

async fn resolve_plain_anthropic_tool_resume(
    state: Arc<ApiState>,
    bearer_token: &str,
    request: &Value,
    raw_results: &[AnthropicRawToolResult],
) -> Result<Option<AnthropicToolResumeRequest>, AnthropicRouteError> {
    if raw_results.is_empty() {
        return Ok(None);
    }
    let native_request = map_messages_request(request.clone())?;
    let Some(external_user) = native_request.conversation.string("user") else {
        return Ok(None);
    };
    let Some(external_conversation_id) = native_request.conversation.string("id") else {
        return Ok(None);
    };
    let actor = ApplicationApiKeyService::new(state.store.clone())
        .with_last_used_cache(state.infrastructure.cache_store())
        .authenticate_bearer_token(bearer_token)
        .await
        .map_err(|_| native::native_error(NativeRunValidationError::NotAuthenticated))?;
    let waiting_run_ids = state
        .store
        .list_waiting_callback_published_flow_run_ids_for_conversation(
            &ListWaitingCallbackPublishedRunsInput {
                application_id: actor.application_id,
                api_key_id: actor.api_key_id,
                external_user,
                external_conversation_id,
                compatibility_mode: "anthropic-messages-v1".to_string(),
            },
        )
        .await
        .map_err(native::service_error)?;

    for waiting_run_id in waiting_run_ids.iter().rev() {
        let Some(detail) = state
            .store
            .get_published_run_detail(actor.application_id, *waiting_run_id)
            .await
            .map_err(native::service_error)?
        else {
            continue;
        };
        for callback_task in detail.callback_tasks.iter().rev() {
            if anthropic_raw_tool_results_match_callback_task(raw_results, callback_task) {
                return Ok(Some(AnthropicToolResumeRequest {
                    callback_task_id: callback_task.id,
                    tool_results: anthropic_raw_tool_results_payload(raw_results),
                }));
            }
        }
    }

    Ok(None)
}

async fn resolve_embedded_anthropic_encoded_tool_resume(
    state: Arc<ApiState>,
    bearer_token: &str,
    request: &Value,
) -> Result<Option<AnthropicToolResumeRequest>, AnthropicRouteError> {
    let decoded_results = anthropic_decoded_tool_results_in_request(request)?;
    if decoded_results.is_empty() {
        return Ok(None);
    }
    let native_request = map_messages_request(request.clone())?;
    let Some(external_user) = native_request.conversation.string("user") else {
        return Ok(None);
    };
    let Some(external_conversation_id) = native_request.conversation.string("id") else {
        return Ok(None);
    };
    let actor = ApplicationApiKeyService::new(state.store.clone())
        .with_last_used_cache(state.infrastructure.cache_store())
        .authenticate_bearer_token(bearer_token)
        .await
        .map_err(|_| native::native_error(NativeRunValidationError::NotAuthenticated))?;
    let waiting_run_ids = state
        .store
        .list_waiting_callback_published_flow_run_ids_for_conversation(
            &ListWaitingCallbackPublishedRunsInput {
                application_id: actor.application_id,
                api_key_id: actor.api_key_id,
                external_user,
                external_conversation_id,
                compatibility_mode: "anthropic-messages-v1".to_string(),
            },
        )
        .await
        .map_err(native::service_error)?;

    for waiting_run_id in waiting_run_ids.iter().rev() {
        let Some(detail) = state
            .store
            .get_published_run_detail(actor.application_id, *waiting_run_id)
            .await
            .map_err(native::service_error)?
        else {
            continue;
        };
        for callback_task in detail.callback_tasks.iter().rev() {
            if let Some(tool_results) =
                anthropic_decoded_tool_results_for_callback(&decoded_results, callback_task)
            {
                return Ok(Some(AnthropicToolResumeRequest {
                    callback_task_id: callback_task.id,
                    tool_results,
                }));
            }
        }
    }

    Ok(None)
}

fn anthropic_decoded_tool_results_in_request(
    request: &Value,
) -> Result<Vec<AnthropicDecodedToolResult>, AnthropicRouteError> {
    let Some(messages) = request.get("messages").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut decoded_results = Vec::new();
    for message in messages {
        let Some(blocks) = message.get("content").and_then(Value::as_array) else {
            continue;
        };
        for block in blocks {
            if block.get("type").and_then(Value::as_str) != Some("tool_result") {
                continue;
            }
            let Some(tool_use_id) = block.get("tool_use_id").and_then(Value::as_str) else {
                return Err(AnthropicCompatError {
                    message: "tool_result tool_use_id is required".to_string(),
                    error_type: "invalid_request".to_string(),
                }
                .into());
            };
            let Some((callback_task_id, original_tool_use_id)) =
                decode_anthropic_callback_tool_use_id(tool_use_id)
            else {
                continue;
            };
            decoded_results.push(AnthropicDecodedToolResult {
                callback_task_id,
                original_tool_use_id,
                content: anthropic_tool_result_content(block),
            });
        }
    }
    Ok(decoded_results)
}

fn anthropic_decoded_tool_results_for_callback(
    decoded_results: &[AnthropicDecodedToolResult],
    callback_task: &domain::CallbackTaskRecord,
) -> Option<Value> {
    if callback_task.status != domain::CallbackTaskStatus::Pending
        || callback_task.callback_kind != "llm_tool_calls"
    {
        return None;
    }
    let tool_calls = callback_task
        .request_payload
        .get("tool_calls")
        .and_then(Value::as_array)?;
    let tool_call_ids = tool_calls
        .iter()
        .filter_map(|tool_call| tool_call.get("id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    if tool_call_ids.is_empty() {
        return None;
    }
    let matching_results = decoded_results
        .iter()
        .filter(|result| result.callback_task_id == callback_task.id)
        .collect::<Vec<_>>();
    if matching_results.len() != tool_call_ids.len() {
        return None;
    }

    let mut tool_results = Vec::with_capacity(tool_call_ids.len());
    for tool_call_id in tool_call_ids {
        let result = matching_results
            .iter()
            .find(|result| result.original_tool_use_id == tool_call_id)?;
        tool_results.push(json!({
            "tool_call_id": tool_call_id,
            "content": result.content.clone(),
        }));
    }
    Some(Value::Array(tool_results))
}

async fn anthropic_plain_stdout_resume_request_for_route(
    state: Arc<ApiState>,
    bearer_token: &str,
    request: &Value,
) -> Result<Option<AnthropicToolResumeRequest>, AnthropicRouteError> {
    let Some(candidate) = anthropic_plain_stdout_resume_candidate(request)? else {
        return Ok(None);
    };
    let native_request = map_messages_request(request.clone())?;
    if !anthropic_request_has_claude_code_context(request, &native_request) {
        return Ok(None);
    }
    let Some(external_user) = native_request.conversation.string("user") else {
        return Ok(None);
    };
    let Some(external_conversation_id) = native_request.conversation.string("id") else {
        return Ok(None);
    };
    let actor = ApplicationApiKeyService::new(state.store.clone())
        .with_last_used_cache(state.infrastructure.cache_store())
        .authenticate_bearer_token(bearer_token)
        .await
        .map_err(|_| native::native_error(NativeRunValidationError::NotAuthenticated))?;
    let waiting_run_ids = state
        .store
        .list_waiting_callback_published_flow_run_ids_for_conversation(
            &ListWaitingCallbackPublishedRunsInput {
                application_id: actor.application_id,
                api_key_id: actor.api_key_id,
                external_user,
                external_conversation_id,
                compatibility_mode: "anthropic-messages-v1".to_string(),
            },
        )
        .await
        .map_err(native::service_error)?;

    for waiting_run_id in waiting_run_ids.iter().rev() {
        let Some(detail) = state
            .store
            .get_published_run_detail(actor.application_id, *waiting_run_id)
            .await
            .map_err(native::service_error)?
        else {
            continue;
        };
        for callback_task in detail.callback_tasks.iter().rev() {
            if let Some(tool_results) =
                anthropic_plain_stdout_tool_results_for_callback(&candidate, callback_task)
            {
                return Ok(Some(AnthropicToolResumeRequest {
                    callback_task_id: callback_task.id,
                    tool_results,
                }));
            }
        }
    }

    Ok(None)
}

fn anthropic_plain_stdout_resume_candidate(
    request: &Value,
) -> Result<Option<AnthropicPlainStdoutResumeCandidate>, AnthropicRouteError> {
    let Some(messages) = request.get("messages").and_then(Value::as_array) else {
        return Ok(None);
    };
    let Some(latest_user_index) = messages
        .iter()
        .rposition(|message| message.get("role").and_then(Value::as_str) == Some("user"))
    else {
        return Ok(None);
    };
    let Some(content) = messages[latest_user_index].get("content") else {
        return Ok(None);
    };
    let Some(content) = anthropic_plain_text_content(content) else {
        return Ok(None);
    };
    if !looks_like_claude_code_tool_stdout(&content) {
        return Ok(None);
    }
    let tool_use_ids = anthropic_trailing_assistant_tool_use_ids(messages, latest_user_index);
    if tool_use_ids.is_empty() {
        return Ok(None);
    }

    Ok(Some(AnthropicPlainStdoutResumeCandidate {
        content,
        tool_use_ids,
    }))
}

fn anthropic_raw_tool_results_match_callback_task(
    raw_results: &[AnthropicRawToolResult],
    callback_task: &domain::CallbackTaskRecord,
) -> bool {
    if callback_task.status != domain::CallbackTaskStatus::Pending
        || callback_task.callback_kind != "llm_tool_calls"
    {
        return false;
    }
    let Some(tool_calls) = callback_task
        .request_payload
        .get("tool_calls")
        .and_then(Value::as_array)
    else {
        return false;
    };
    let tool_call_ids = tool_calls
        .iter()
        .filter_map(|tool_call| tool_call.get("id").and_then(Value::as_str))
        .collect::<HashSet<_>>();
    !tool_call_ids.is_empty()
        && raw_results
            .iter()
            .all(|result| tool_call_ids.contains(result.tool_use_id.as_str()))
}

fn anthropic_raw_tool_results_payload(raw_results: &[AnthropicRawToolResult]) -> Value {
    Value::Array(
        raw_results
            .iter()
            .map(|result| {
                json!({
                    "tool_call_id": result.tool_use_id,
                    "content": result.content,
                })
            })
            .collect(),
    )
}

fn anthropic_plain_stdout_tool_results_for_callback(
    candidate: &AnthropicPlainStdoutResumeCandidate,
    callback_task: &domain::CallbackTaskRecord,
) -> Option<Value> {
    if callback_task.status != domain::CallbackTaskStatus::Pending
        || callback_task.callback_kind != "llm_tool_calls"
    {
        return None;
    }
    let tool_calls = callback_task
        .request_payload
        .get("tool_calls")
        .and_then(Value::as_array)?;
    let tool_call_ids = tool_calls
        .iter()
        .filter_map(|tool_call| tool_call.get("id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    if tool_call_ids.is_empty() {
        return None;
    }
    if candidate.tool_use_ids.len() != tool_call_ids.len()
        || !tool_call_ids
            .iter()
            .all(|tool_call_id| candidate.tool_use_ids.contains(*tool_call_id))
    {
        return None;
    }

    Some(Value::Array(
        tool_call_ids
            .into_iter()
            .map(|tool_call_id| {
                json!({
                    "tool_call_id": tool_call_id,
                    "content": candidate.content.clone(),
                })
            })
            .collect(),
    ))
}

fn anthropic_trailing_assistant_tool_use_ids(
    messages: &[Value],
    trailing_start: usize,
) -> HashSet<String> {
    let mut tool_use_ids = HashSet::new();
    for message in messages[..trailing_start].iter().rev() {
        if message.get("role").and_then(Value::as_str) != Some("assistant") {
            break;
        }
        let Some(blocks) = message.get("content").and_then(Value::as_array) else {
            continue;
        };
        for block in blocks {
            if block.get("type").and_then(Value::as_str) != Some("tool_use") {
                continue;
            }
            if let Some(id) = block.get("id").and_then(Value::as_str) {
                tool_use_ids.insert(id.to_string());
            }
        }
    }
    tool_use_ids
}

fn anthropic_plain_text_content(content: &Value) -> Option<String> {
    if let Some(text) = content.as_str() {
        return Some(text.to_string());
    }
    let blocks = content.as_array()?;
    let mut text = String::new();
    for block in blocks {
        if block.get("type").and_then(Value::as_str) != Some("text") {
            return None;
        }
        let value = block.get("text").and_then(Value::as_str)?;
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(value);
    }
    Some(text)
}

fn looks_like_claude_code_tool_stdout(content: &str) -> bool {
    let content = content.trim();
    if content.is_empty() {
        return false;
    }
    const MARKERS: [&str; 12] = [
        "No matches found",
        "No files found",
        "File does not exist.",
        "Note: your current working directory is",
        "Found ",
        "Took a screenshot",
        "Saved screenshot to",
        "## Pages",
        "## Latest page snapshot",
        "RootWebArea",
        "-rw-r--r--",
        "uid=",
    ];
    MARKERS.iter().any(|marker| content.contains(marker))
        || content.lines().any(|line| {
            line.split_once('\t')
                .is_some_and(|(prefix, _)| prefix.parse::<u32>().is_ok())
        })
}

fn anthropic_request_has_claude_code_context(
    request: &Value,
    native_request: &NativeRunRequest,
) -> bool {
    request
        .get("metadata")
        .and_then(|metadata| metadata.get("session_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
        || native_request.system.as_deref().is_some_and(|system| {
            system.contains("Claude Code") || system.contains("x-anthropic-billing-header")
        })
}

fn anthropic_message_has_only_tool_results(message: &Value) -> bool {
    message.get("role").and_then(Value::as_str) == Some("user")
        && message
            .get("content")
            .is_some_and(anthropic_content_is_tool_result_only)
}

fn anthropic_tool_result_orphan_error() -> AnthropicRouteError {
    AnthropicCompatError {
        message: "tool_result continuation could not be matched to a callback task".to_string(),
        error_type: "tool_result_only_orphan".to_string(),
    }
    .into()
}

fn anthropic_tool_result_content(block: &Value) -> Value {
    let Some(content) = block.get("content") else {
        return Value::String(String::new());
    };
    if let Some(text) = content.as_str() {
        return Value::String(text.to_string());
    }
    if let Some(blocks) = content.as_array() {
        let text = blocks
            .iter()
            .filter_map(|entry| entry.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n");
        if blocks
            .iter()
            .all(|entry| entry.get("type").and_then(Value::as_str) == Some("text"))
        {
            return Value::String(text);
        }
        return Value::Array(blocks.clone());
    }
    Value::String(content.to_string())
}

fn callback_task_id_from_required_action(run: &NativeRunResult) -> Option<Uuid> {
    run.required_action
        .as_ref()
        .and_then(|action| action.payload.get("callback_task_id"))
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
}

fn anthropic_required_action_is_supported(run: &NativeRunResult) -> bool {
    run.required_action.as_ref().is_none_or(|action| {
        action.payload.get("callback_kind").and_then(Value::as_str) == Some("llm_tool_calls")
            && run
                .tool_calls
                .as_ref()
                .is_some_and(|value| value.as_array().is_some_and(|calls| !calls.is_empty()))
    })
}

#[cfg(test)]
mod tests;
