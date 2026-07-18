use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use control_plane::application_public_api::{
    api_keys::ApplicationApiKeyService,
    client_protocol_envelope::{
        capture_client_protocol_envelope, merge_anthropic_messages_envelopes,
        ClientProtocolIngressPolicy,
    },
    compat::anthropic::{translate_messages_request, AnthropicCompatError},
    native::{
        ApplicationNativeRunService, CreateNativeRunCommand, NativeRunRequest, NativeRunResult,
        NativeRunStatus, NativeRunValidationError,
    },
    protocol_translation::{
        TranslationDecisionKind, TranslationProtocol, TranslationReport,
        TranslationSafeRepresentation,
    },
};
use plugin_framework::provider_contract::ClientProtocolEnvelope;
use serde::Serialize;
use serde_json::{json, Value};
use tracing::debug;
use utoipa::ToSchema;
use uuid::Uuid;

mod token_count;

#[cfg(test)]
use crate::routes::application_public_api::tool_callback_ids::decode_anthropic_callback_tool_use_id;
use crate::{
    app_state::ApiState,
    routes::application_public_api::{
        compat_sse, llm_tool_visibility::external_llm_tool_calls, native,
        tool_callback_ids::encode_anthropic_callback_tool_use_id,
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
    let translated = translate_messages_request(value)?;
    let translation_decision_count = translated.report.decisions.len();
    let mut request = translated.request;
    request.client_protocol_envelope = merge_anthropic_messages_envelopes(
        anthropic_client_protocol_envelope_from_headers(&headers),
        request.client_protocol_envelope,
    );
    let model = request.model.clone().unwrap_or_default();
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
    let translation = translate_messages_request(value.clone())?;
    debug!(
        route = "messages_count_tokens",
        translation_decision_count = translation.report.decisions.len(),
        "anthropic count tokens request translated"
    );
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

fn to_anthropic_response(
    run: NativeRunResult,
    model: String,
) -> Result<AnthropicMessageResponse, AnthropicRouteError> {
    let terminal_stop_reason = match run.status {
        NativeRunStatus::Succeeded => "end_turn",
        NativeRunStatus::Incomplete => "max_tokens",
        NativeRunStatus::Waiting => return Err(AnthropicRouteError::RequiredAction),
        NativeRunStatus::Created
        | NativeRunStatus::Queued
        | NativeRunStatus::Running
        | NativeRunStatus::Failed
        | NativeRunStatus::Cancelled => {
            return Err(native::blocking_run_projection_error(&run).into())
        }
    };
    let callback_task_id = callback_task_id_from_required_action(&run);
    let tool_blocks = (run.status == NativeRunStatus::Succeeded)
        .then(|| anthropic_tool_use_blocks(run.tool_calls.as_ref(), callback_task_id))
        .flatten();
    let has_tool_blocks = tool_blocks
        .as_ref()
        .is_some_and(|blocks| !blocks.is_empty());
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
