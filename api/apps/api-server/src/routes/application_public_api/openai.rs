use std::sync::Arc;

#[cfg(test)]
use crate::routes::application_public_api::tool_callback_ids::decode_openai_callback_tool_call_id;
use axum::{
    body::Bytes,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use control_plane::application_public_api::{
    api_keys::ApplicationApiKeyService,
    client_protocol_envelope::{capture_client_protocol_envelope, ClientProtocolIngressPolicy},
    compat::openai::{
        extract_model_list_from_start_node, response_id_from_run_id,
        translate_chat_completion_request, translate_response_request_with_context,
        OpenAiCompatError, OpenAiCompatibleModel, OpenAiResponsesEndpoint,
    },
    native::{
        ApplicationNativeRunService, CreateNativeRunCommand, NativeExecutionOperation,
        NativeRunRequest, NativeRunResult, NativeRunStatus, NativeRunValidationError,
        RemoteCompactionProfile,
    },
    protocol_translation::{
        TranslationDecisionKind, TranslationProtocol, TranslationReport,
        TranslationSafeRepresentation,
    },
    publications::{ApplicationPublicationService, LoadActiveApplicationPublicationCommand},
    run_service::{ApplicationPublishedCompactService, CompactCommand},
};
use plugin_framework::provider_contract::{
    ClientProtocolEnvelope, ProviderCompactProfile, ProviderCompactResult,
};
use serde_json::{json, Value};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    routes::application_public_api::{
        compat_sse, llm_tool_visibility::external_llm_tool_calls, native,
        tool_callback_ids::encode_openai_callback_tool_call_id,
    },
};

mod compact;
mod model_list;
#[cfg(test)]
mod tests;
mod types;

use model_list::{
    is_codex_model_list_request, to_codex_model_list_response, to_openai_model_list_response,
};
pub use types::{
    OpenAiChatCompletionChoice, OpenAiChatCompletionResponse, OpenAiChatMessage, OpenAiErrorBody,
    OpenAiErrorObject, OpenAiModelListQuery, OpenAiModelListResponse, OpenAiModelObject,
    OpenAiResponsesIncompleteDetails, OpenAiResponsesObject, OpenAiResponsesUsage,
    OpenAiRouteError, OpenAiToolCall, OpenAiToolCallFunction, OpenAiUsage,
};

struct OpenAiCredential {
    token: String,
    source: &'static str,
}
#[utoipa::path(
    post,
    path = "/v1/chat/completions",
    request_body = Value,
    responses(
        (status = 200, body = OpenAiChatCompletionResponse),
        (status = 400, body = OpenAiErrorBody),
        (status = 401, body = OpenAiErrorBody),
        (status = 403, body = OpenAiErrorBody),
        (status = 409, body = OpenAiErrorBody),
        (status = 422, body = OpenAiErrorBody)
    )
)]
pub async fn create_chat_completion(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, OpenAiRouteError> {
    let credential = match openai_credential(&headers) {
        Ok(credential) => credential,
        Err(error) => {
            warn!(
                route = "chat_completions",
                status = error.status.as_u16(),
                code = error.code,
                "openai compatible authentication failed"
            );
            return Err(error.into());
        }
    };
    let value = match parse_openai_json_body(body, TranslationProtocol::OpenAiChat) {
        Ok(value) => value,
        Err(error) => {
            warn_openai_route_error(
                "chat_completions",
                &error,
                "openai compatible JSON validation failed",
            );
            return Err(error);
        }
    };
    let translated = match translate_chat_completion_request(value) {
        Ok(translated) => translated,
        Err(error) => {
            let route_error = OpenAiRouteError::from(error);
            warn_openai_route_error(
                "chat_completions",
                &route_error,
                "openai compatible request validation failed",
            );
            return Err(route_error);
        }
    };
    let translation_decision_count = translated.report.decisions.len();
    let mut request = translated.request;
    request.client_protocol_envelope = openai_client_protocol_envelope_from_headers(&headers);
    let model = request.model.clone().unwrap_or_default();
    let response_mode = request.response_mode.clone();
    let run = match create_native_run(state.clone(), credential.token.clone(), request).await {
        Ok(run) => run,
        Err(error) => {
            warn!(
                route = "chat_completions",
                auth_source = credential.source,
                status = error.status.as_u16(),
                code = error.code,
                "openai compatible native run validation failed"
            );
            return Err(error.into());
        }
    };

    info!(
        route = "chat_completions",
        auth_source = credential.source,
        application_id = %run.application_id,
        flow_run_id = %run.id,
        response_mode = response_mode.as_deref().unwrap_or("blocking"),
        model = %model,
        translation_decision_count,
        "openai compatible chat completion accepted"
    );

    if response_mode.as_deref() == Some("streaming") {
        return compat_sse::start_openai_run_stream(state, run, model)
            .await
            .map_err(Into::into);
    }

    let run = native::execute_blocking_native_run(state, credential.token, run).await?;
    let completion_id = compat_sse::openai_chat_completion_id_from_run_id(run.id);
    Ok(Json(to_openai_response(run, model, completion_id)?).into_response())
}

#[utoipa::path(
    post,
    path = "/v1/responses",
    request_body = Value,
    responses(
        (status = 200, body = OpenAiResponsesObject),
        (status = 400, body = OpenAiErrorBody),
        (status = 401, body = OpenAiErrorBody),
        (status = 403, body = OpenAiErrorBody),
        (status = 409, body = OpenAiErrorBody),
        (status = 422, body = OpenAiErrorBody),
        (status = 429, body = OpenAiErrorBody),
        (status = 502, body = OpenAiErrorBody)
    )
)]
pub async fn create_response(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, OpenAiRouteError> {
    create_response_for_endpoint(state, headers, body, OpenAiResponsesEndpoint::Responses).await
}

#[utoipa::path(
    post,
    path = "/v1/responses/compact",
    request_body = Value,
    responses(
        (status = 200, body = Value),
        (status = 400, body = OpenAiErrorBody),
        (status = 401, body = OpenAiErrorBody),
        (status = 403, body = OpenAiErrorBody),
        (status = 409, body = OpenAiErrorBody),
        (status = 422, body = OpenAiErrorBody),
        (status = 429, body = OpenAiErrorBody),
        (status = 502, body = OpenAiErrorBody)
    )
)]
pub async fn create_response_compact(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, OpenAiRouteError> {
    create_response_for_endpoint(
        state,
        headers,
        body,
        OpenAiResponsesEndpoint::ResponsesCompact,
    )
    .await
}

async fn create_response_for_endpoint(
    state: Arc<ApiState>,
    headers: HeaderMap,
    body: Bytes,
    endpoint: OpenAiResponsesEndpoint,
) -> Result<Response, OpenAiRouteError> {
    let route = match endpoint {
        OpenAiResponsesEndpoint::Responses => "responses",
        OpenAiResponsesEndpoint::ResponsesCompact => "responses_compact",
    };
    let credential = match openai_credential(&headers) {
        Ok(credential) => credential,
        Err(error) => {
            warn!(
                route,
                status = error.status.as_u16(),
                code = error.code,
                "openai responses compatible authentication failed"
            );
            return Err(error.into());
        }
    };
    if compact::has_codex_turn_metadata(&headers) {
        if let Err(error) =
            authenticate_openai_response_credential(state.as_ref(), &credential).await
        {
            warn!(
                route,
                auth_source = credential.source,
                status = error.status.as_u16(),
                code = error.code,
                "openai responses Codex metadata request authentication failed"
            );
            return Err(error.into());
        }
    }
    let request_context = match compact::responses_request_context(&headers, endpoint) {
        Ok(context) => context,
        Err(error) => {
            warn_openai_route_error(
                route,
                &error,
                "openai responses Codex metadata validation failed",
            );
            return Err(error);
        }
    };
    let value = match parse_openai_json_body(body, TranslationProtocol::OpenAiResponses) {
        Ok(value) => value,
        Err(error) => {
            warn_openai_route_error(
                route,
                &error,
                "openai responses compatible JSON validation failed",
            );
            return Err(error);
        }
    };
    let translated = match translate_response_request_with_context(value, request_context) {
        Ok(translated) => translated,
        Err(error) => {
            let route_error = OpenAiRouteError::from(error);
            warn_openai_route_error(
                route,
                &route_error,
                "openai responses compatible request validation failed",
            );
            return Err(route_error);
        }
    };
    let translation_decision_count = translated.report.decisions.len();
    let mut request = translated.request;
    request.client_protocol_envelope = openai_client_protocol_envelope_from_headers(&headers);
    let model = request.model.clone().unwrap_or_default();
    let response_mode = request.response_mode.clone();
    match request.execution.execution_operation().clone() {
        NativeExecutionOperation::Generate(_) => {
            let run =
                match create_native_run(state.clone(), credential.token.clone(), request).await {
                    Ok(run) => run,
                    Err(error) => {
                        warn!(
                            route,
                            auth_source = credential.source,
                            status = error.status.as_u16(),
                            code = error.code,
                            "openai responses compatible native run validation failed"
                        );
                        return Err(error.into());
                    }
                };

            info!(
                route,
                auth_source = credential.source,
                application_id = %run.application_id,
                flow_run_id = %run.id,
                response_mode = response_mode.as_deref().unwrap_or("blocking"),
                model = %model,
                translation_decision_count,
                "openai responses compatible request accepted"
            );

            if response_mode.as_deref() == Some("streaming") {
                return compat_sse::start_openai_response_stream(state, run, model, None)
                    .await
                    .map_err(Into::into);
            }

            let run = native::execute_blocking_native_run(state, credential.token, run).await?;
            Ok(Json(to_openai_responses_response(run, model, None)?).into_response())
        }
        NativeExecutionOperation::Compact(remote_profile) => {
            let profile = match remote_profile {
                RemoteCompactionProfile::ResponsesCompact => {
                    ProviderCompactProfile::ResponsesCompact
                }
                RemoteCompactionProfile::ResponsesCompactionV2 => {
                    ProviderCompactProfile::ResponsesCompactionV2
                }
            };
            let result = match ApplicationPublishedCompactService::new(
                state.store.clone(),
                native::api_provider_runtime(state.as_ref()),
                state.provider_secret_master_key.clone(),
            )
            .with_last_used_cache(state.infrastructure.cache_store())
            .compact(CompactCommand {
                bearer_token: credential.token,
                request,
                profile,
            })
            .await
            {
                Ok(result) => result,
                Err(error) => {
                    let route_error = compact::published_compact_error(error);
                    warn_openai_route_error(
                        route,
                        &route_error,
                        "openai responses Compact request failed without a flow run",
                    );
                    return Err(route_error);
                }
            };

            info!(
                route,
                auth_source = credential.source,
                compaction_profile = profile.as_str(),
                response_mode = response_mode.as_deref().unwrap_or("blocking"),
                model = %model,
                translation_decision_count,
                "openai responses Compact request completed without a flow run"
            );

            match (profile, result.result) {
                (
                    ProviderCompactProfile::ResponsesCompact,
                    ProviderCompactResult::ResponseItems { response_items, .. },
                ) => {
                    // The legacy Compact contract is exactly the provider's ResponseItem[]. It
                    // is intentionally neither wrapped in a Response object nor projected as a
                    // runtime SSE flow.
                    Ok(Json(response_items).into_response())
                }
                (
                    ProviderCompactProfile::ResponsesCompactionV2,
                    ProviderCompactResult::CompletedOpaqueCompactionItem {
                        response_id,
                        compaction_item,
                        encrypted_content: _,
                        ..
                    },
                ) => {
                    let response = compact::completed_v2_compaction_response(
                        model,
                        response_id,
                        compaction_item,
                    );
                    if response_mode.as_deref() == Some("streaming") {
                        let response = serde_json::to_value(response).map_err(|_| {
                            OpenAiRouteError::Native(native::NativeApiError::new(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "openai_compact_response_serialization_failed",
                                "could not serialize OpenAI Compact response",
                            ))
                        })?;
                        return compat_sse::completed_openai_response_stream(response)
                            .map_err(Into::into);
                    }
                    Ok(Json(response).into_response())
                }
                _ => Err(compact::unexpected_compact_result_error()),
            }
        }
    }
}

#[utoipa::path(
    get,
    path = "/v1/models",
    operation_id = "list_openai_compatible_models",
    responses(
        (status = 200, body = OpenAiModelListResponse),
        (status = 401, body = OpenAiErrorBody),
        (status = 403, body = OpenAiErrorBody),
        (status = 409, body = OpenAiErrorBody)
    )
)]
pub async fn list_models(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<OpenAiModelListQuery>,
    headers: HeaderMap,
) -> Result<Response, OpenAiRouteError> {
    let credential = match openai_credential(&headers) {
        Ok(credential) => credential,
        Err(error) => {
            warn!(
                route = "models",
                status = error.status.as_u16(),
                code = error.code,
                "openai compatible model list authentication failed"
            );
            return Err(error.into());
        }
    };
    let actor = match ApplicationApiKeyService::new(state.store.clone())
        .authenticate_bearer_token(&credential.token)
        .await
    {
        Ok(actor) => actor,
        Err(_) => {
            warn!(
                route = "models",
                auth_source = credential.source,
                code = "not_authenticated",
                "openai compatible model list rejected invalid application API key"
            );
            return Err(native::native_error(NativeRunValidationError::NotAuthenticated).into());
        }
    };
    let publication = match ApplicationPublicationService::new(state.store.clone())
        .load_active_publication(LoadActiveApplicationPublicationCommand {
            application_id: actor.application_id,
        })
        .await
    {
        Ok(publication) => publication,
        Err(_) => {
            warn!(
                route = "models",
                auth_source = credential.source,
                application_id = %actor.application_id,
                api_key_id = %actor.api_key_id,
                code = "application_not_published",
                "openai compatible model list has no active publication"
            );
            return Err(
                native::native_error(NativeRunValidationError::ApplicationNotPublished).into(),
            );
        }
    };
    let models = extract_model_list_from_start_node(&publication.document_snapshot);
    let model_count = models.len();

    info!(
        route = "models",
        auth_source = credential.source,
        application_id = %actor.application_id,
        api_key_id = %actor.api_key_id,
        model_count,
        "openai compatible model list returned"
    );

    if is_codex_model_list_request(&query) {
        return Ok(Json(to_codex_model_list_response(models)).into_response());
    }

    Ok(Json(to_openai_model_list_response(
        models,
        publication.created_at.unix_timestamp(),
    ))
    .into_response())
}

fn openai_credential(headers: &HeaderMap) -> Result<OpenAiCredential, native::NativeApiError> {
    if let Ok(token) = native::bearer_token(headers) {
        return Ok(OpenAiCredential {
            token,
            source: "authorization_bearer",
        });
    }
    headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(|token| OpenAiCredential {
            token: token.to_owned(),
            source: "x_api_key",
        })
        .ok_or_else(|| {
            native::NativeApiError::new(
                StatusCode::UNAUTHORIZED,
                "not_authenticated",
                "missing Authorization bearer token or x-api-key",
            )
        })
}

async fn authenticate_openai_response_credential(
    state: &ApiState,
    credential: &OpenAiCredential,
) -> Result<(), native::NativeApiError> {
    ApplicationApiKeyService::new(state.store.clone())
        .with_last_used_cache(state.infrastructure.cache_store())
        .authenticate_bearer_token(&credential.token)
        .await
        .map(|_| ())
        .map_err(|_| native::native_error(NativeRunValidationError::NotAuthenticated))
}

fn openai_client_protocol_envelope_from_headers(
    headers: &HeaderMap,
) -> Option<ClientProtocolEnvelope> {
    capture_client_protocol_envelope(
        ClientProtocolIngressPolicy::DefaultDeny,
        headers
            .iter()
            .filter_map(|(name, value)| value.to_str().ok().map(|value| (name.as_str(), value))),
    )
}

fn parse_openai_json_body(
    body: Bytes,
    protocol: TranslationProtocol,
) -> Result<Value, OpenAiRouteError> {
    serde_json::from_slice::<Value>(&body).map_err(|_| {
        let mut report = TranslationReport::new(protocol);
        report.record(
            "$.body",
            None,
            TranslationDecisionKind::Rejected,
            Some("invalid JSON body"),
            TranslationSafeRepresentation::Present,
        );
        OpenAiCompatError {
            message: "invalid JSON body".to_string(),
            error_type: "invalid_request_error".to_string(),
            param: Some("body".to_string()),
            code: "invalid_request".to_string(),
            report,
        }
        .into()
    })
}

fn warn_openai_route_error(route: &'static str, error: &OpenAiRouteError, message: &'static str) {
    match error {
        OpenAiRouteError::Compat(error) => warn!(
            route,
            code = %error.code,
            param = error.param.as_deref().unwrap_or(""),
            error_type = %error.error_type,
            translation_decision_count = error.report.decisions.len(),
            "{message}"
        ),
        OpenAiRouteError::Native(error) => warn!(
            route,
            status = error.status.as_u16(),
            code = error.code,
            "{message}"
        ),
        OpenAiRouteError::RequiredAction => {
            warn!(route, code = "required_action_not_supported", "{message}")
        }
    }
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

fn to_openai_response(
    run: NativeRunResult,
    model: String,
    completion_id: String,
) -> Result<OpenAiChatCompletionResponse, OpenAiRouteError> {
    let finish_reason = match run.status {
        NativeRunStatus::Succeeded => "stop",
        NativeRunStatus::Incomplete => "length",
        NativeRunStatus::Waiting => return Err(OpenAiRouteError::RequiredAction),
        NativeRunStatus::Created
        | NativeRunStatus::Queued
        | NativeRunStatus::Running
        | NativeRunStatus::Failed
        | NativeRunStatus::Cancelled => {
            return Err(native::blocking_run_projection_error(&run).into())
        }
    };
    let callback_task_id = callback_task_id_from_required_action(&run);
    let tool_calls = openai_tool_calls(run.tool_calls.as_ref(), callback_task_id);
    let finish_reason = if tool_calls.is_some() && run.status == NativeRunStatus::Succeeded {
        "tool_calls"
    } else {
        finish_reason
    };
    Ok(OpenAiChatCompletionResponse {
        id: completion_id,
        object: "chat.completion",
        created: run.created_at.unix_timestamp(),
        model,
        choices: vec![OpenAiChatCompletionChoice {
            index: 0,
            message: OpenAiChatMessage {
                role: "assistant",
                content: if tool_calls.is_some() {
                    run.answer
                } else {
                    Some(run.answer.unwrap_or_default())
                },
                tool_calls,
            },
            finish_reason,
        }],
        usage: openai_usage(run.usage),
    })
}

fn to_openai_responses_response(
    run: NativeRunResult,
    model: String,
    previous_response_id: Option<String>,
) -> Result<OpenAiResponsesObject, OpenAiRouteError> {
    let (status, incomplete_details) = match run.status {
        NativeRunStatus::Succeeded => ("completed", None),
        NativeRunStatus::Incomplete => (
            "incomplete",
            Some(OpenAiResponsesIncompleteDetails {
                reason: "max_output_tokens",
            }),
        ),
        NativeRunStatus::Waiting => return Err(OpenAiRouteError::RequiredAction),
        NativeRunStatus::Created
        | NativeRunStatus::Queued
        | NativeRunStatus::Running
        | NativeRunStatus::Failed
        | NativeRunStatus::Cancelled => {
            return Err(native::blocking_run_projection_error(&run).into())
        }
    };
    let callback_task_id = callback_task_id_from_required_action(&run);
    let function_call_items =
        openai_response_function_call_items(run.tool_calls.as_ref(), callback_task_id);
    let output_text = if function_call_items.is_some() {
        String::new()
    } else {
        run.answer.clone().unwrap_or_default()
    };
    let output = function_call_items
        .unwrap_or_else(|| vec![openai_response_message_item(&run, &output_text, status)]);
    Ok(OpenAiResponsesObject {
        id: response_id_from_run_id(run.id),
        object: "response",
        created_at: run.created_at.unix_timestamp(),
        status,
        model,
        output,
        output_text,
        usage: openai_responses_usage(run.usage),
        incomplete_details,
        previous_response_id,
    })
}

fn openai_response_message_item(
    run: &NativeRunResult,
    output_text: &str,
    status: &'static str,
) -> Value {
    json!({
        "id": format!("msg_{}", run.id),
        "type": "message",
        "status": status,
        "role": "assistant",
        "content": [
            {
                "type": "output_text",
                "text": output_text,
                "annotations": []
            }
        ]
    })
}

fn openai_response_function_call_items(
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
                .unwrap_or("tool_call")
                .to_string();
            let call_id = callback_task_id
                .map(|callback_task_id| {
                    encode_openai_callback_tool_call_id(callback_task_id, &original_id)
                })
                .unwrap_or_else(|| original_id.clone());
            let arguments = call.get("arguments").cloned().unwrap_or_else(|| json!({}));
            Some(json!({
                "id": format!("fc_{}", original_id),
                "type": "function_call",
                "call_id": call_id,
                "name": name,
                "arguments": openai_arguments_string(arguments),
                "status": "completed"
            }))
        })
        .collect::<Vec<_>>();
    (!mapped.is_empty()).then_some(mapped)
}

fn openai_tool_calls(
    tool_calls: Option<&Value>,
    callback_task_id: Option<Uuid>,
) -> Option<Vec<OpenAiToolCall>> {
    let calls = external_llm_tool_calls(tool_calls)?;
    let mapped = calls
        .iter()
        .filter_map(|call| {
            let name = call.get("name").and_then(Value::as_str)?;
            let original_id = call
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("tool_call")
                .to_string();
            let id = callback_task_id
                .map(|callback_task_id| {
                    encode_openai_callback_tool_call_id(callback_task_id, &original_id)
                })
                .unwrap_or(original_id);
            let arguments = call.get("arguments").cloned().unwrap_or_else(|| json!({}));
            Some(OpenAiToolCall {
                id,
                call_type: "function",
                function: OpenAiToolCallFunction {
                    name: name.to_string(),
                    arguments: openai_arguments_string(arguments),
                },
            })
        })
        .collect::<Vec<_>>();
    (!mapped.is_empty()).then_some(mapped)
}

fn openai_arguments_string(arguments: Value) -> String {
    match arguments {
        Value::String(value) => value,
        value => value.to_string(),
    }
}

fn callback_task_id_from_required_action(run: &NativeRunResult) -> Option<Uuid> {
    run.required_action
        .as_ref()
        .and_then(|action| action.payload.get("callback_task_id"))
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
}

fn openai_usage(
    usage: Option<control_plane::application_public_api::native::NativeUsage>,
) -> OpenAiUsage {
    let Some(usage) = usage else {
        return OpenAiUsage::default();
    };
    OpenAiUsage {
        prompt_tokens: usage.prompt_tokens.unwrap_or_default(),
        completion_tokens: usage.completion_tokens.unwrap_or_default(),
        total_tokens: usage.total_tokens.unwrap_or_default(),
    }
}

fn openai_responses_usage(
    usage: Option<control_plane::application_public_api::native::NativeUsage>,
) -> OpenAiResponsesUsage {
    let Some(usage) = usage else {
        return OpenAiResponsesUsage::default();
    };
    OpenAiResponsesUsage {
        input_tokens: usage.prompt_tokens.unwrap_or_default(),
        output_tokens: usage.completion_tokens.unwrap_or_default(),
        total_tokens: usage.total_tokens.unwrap_or_default(),
    }
}
