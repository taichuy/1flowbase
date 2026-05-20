use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use control_plane::application_public_api::{
    compat::anthropic::{map_messages_request, AnthropicCompatError},
    native::{
        ApplicationNativeRunService, CreateNativeRunCommand, NativeRunRequest, NativeRunResult,
    },
};
use serde::Serialize;
use serde_json::{json, Value};
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    routes::application_public_api::{compat_sse, native},
};

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
    pub output_tokens: u64,
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
    let request = parse_anthropic_request(body)?;
    let model = request.model.clone().unwrap_or_default();
    let response_mode = request.response_mode.clone();
    let run = create_native_run(state.clone(), bearer_token.clone(), request).await?;

    if response_mode.as_deref() == Some("streaming") {
        return compat_sse::start_anthropic_run_stream(state, run, model)
            .await
            .map_err(Into::into);
    }

    let run = native::execute_blocking_native_run(state, bearer_token, run).await?;
    if run.required_action.is_some() {
        return Err(AnthropicRouteError::RequiredAction);
    }
    Ok(Json(to_anthropic_response(run, model)).into_response())
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

fn parse_anthropic_request(body: Bytes) -> Result<NativeRunRequest, AnthropicRouteError> {
    let value = serde_json::from_slice::<Value>(&body).map_err(|_| AnthropicCompatError {
        message: "invalid JSON body".to_string(),
        error_type: "invalid_request".to_string(),
    })?;
    map_messages_request(value).map_err(Into::into)
}

async fn create_native_run(
    state: Arc<ApiState>,
    bearer_token: String,
    request: NativeRunRequest,
) -> Result<NativeRunResult, native::NativeApiError> {
    ApplicationNativeRunService::new(state.store.clone())
        .create_native_run(CreateNativeRunCommand {
            bearer_token,
            request,
        })
        .await
        .map_err(native::native_error)
}

fn to_anthropic_response(run: NativeRunResult, model: String) -> AnthropicMessageResponse {
    let tool_blocks = anthropic_tool_use_blocks(run.tool_calls.as_ref());
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

fn anthropic_tool_use_blocks(tool_calls: Option<&Value>) -> Option<Vec<Value>> {
    let calls = tool_calls?.as_array()?;
    let mapped = calls
        .iter()
        .filter_map(|call| {
            let name = call.get("name").and_then(Value::as_str)?;
            let id = call
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("toolu_call");
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

fn anthropic_usage(
    usage: Option<control_plane::application_public_api::native::NativeUsage>,
) -> AnthropicUsage {
    let Some(usage) = usage else {
        return AnthropicUsage::default();
    };
    AnthropicUsage {
        input_tokens: usage.prompt_tokens.unwrap_or_default(),
        output_tokens: usage.completion_tokens.unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use control_plane::application_public_api::native::NativeRunStatus;
    use time::OffsetDateTime;
    use uuid::Uuid;

    #[test]
    fn anthropic_response_projects_native_tool_calls() {
        let run = NativeRunResult {
            id: Uuid::nil(),
            application_id: Uuid::nil(),
            api_key_id: Uuid::nil(),
            publication_version_id: Uuid::nil(),
            status: NativeRunStatus::Succeeded,
            node_input_payload: json!({}),
            metadata: json!({}),
            answer: None,
            reasoning: None,
            required_action: None,
            tool_calls: Some(json!([
                {
                    "id": "toolu_123",
                    "name": "lookup_order",
                    "arguments": {"order_id": "order_123"}
                }
            ])),
            usage: None,
            error: None,
            created_at: OffsetDateTime::UNIX_EPOCH,
        };

        let payload = serde_json::to_value(to_anthropic_response(run, "provider/model".into()))
            .expect("anthropic response serializes");

        assert_eq!(payload["stop_reason"], json!("tool_use"));
        assert_eq!(payload["content"][0]["type"], json!("tool_use"));
        assert_eq!(payload["content"][0]["name"], json!("lookup_order"));
        assert_eq!(
            payload["content"][0]["input"]["order_id"],
            json!("order_123")
        );
    }
}
