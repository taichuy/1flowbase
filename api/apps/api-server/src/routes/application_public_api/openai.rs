use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use control_plane::application_public_api::{
    api_keys::ApplicationApiKeyService,
    callback_resume::{
        ApplicationPublishedCallbackResumeService, PublishedCallbackResumeSource,
        PublishedCallbackResumeTarget, ResumePublishedCallbackCommand,
    },
    callback_tool_ids::decode_openai_callback_tool_call_id,
    client_protocol_envelope::{capture_client_protocol_envelope, ClientProtocolIngressPolicy},
    compat::openai::{
        extract_model_list_from_start_node, response_id_from_run_id, run_id_from_response_id,
        translate_chat_completion_request, translate_response_request_with_context_and_previous,
        OpenAiCompatError, OpenAiCompatibleModel, OpenAiPreviousResponseContext,
        OpenAiResponsesEndpoint,
    },
    native::{
        ApplicationNativeRunService, CreateNativeRunCommand,
        GetNativeRunByProviderResponseIdCommand, GetNativeRunCommand, NativeRunRequest,
        NativeRunResult, NativeRunStatus, NativeRunValidationError,
    },
    protocol_translation::{
        TranslationDecisionKind, TranslationProtocol, TranslationReport,
        TranslationSafeRepresentation,
    },
    publications::{ApplicationPublicationService, LoadActiveApplicationPublicationCommand},
    run_service::ApplicationPublishedRunControlRepository,
};
use control_plane::orchestration_runtime::OrchestrationRuntimeService;
use control_plane::ports::{
    ProviderTransportPayload, ProviderTransportSlotId, ProviderTransportStore,
};
use domain::{AiNativeCompactProfile, AiNativeOperation};
use plugin_framework::provider_contract::{
    ClientProtocolEnvelope, ProviderCompactProfile, ProviderCompactResult,
};
use orchestration_runtime::execution_state::NativeOperationTerminal;
use serde_json::{json, Value};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    provider_runtime::ApiProviderRuntime,
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

struct OpenAiToolResumeRequest {
    callback_task_id: Uuid,
    tool_results: Value,
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
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let response_mode = value
        .get("stream")
        .and_then(Value::as_bool)
        .filter(|value| *value)
        .map(|_| "streaming".to_string());
    if let Some(resume) = openai_chat_tool_resume_request(&value)? {
        let callback_task_id = resume.callback_task_id;
        let command = openai_resume_command(
            &credential.token,
            callback_task_id,
            PublishedCallbackResumeSource::OpenAiChat,
            resume.tool_results,
            response_mode.clone(),
        );
        match compat_sse::prepare_compatible_resume(state.clone(), command).await {
            Ok(plan) if response_mode.as_deref() == Some("streaming") => {
                let completion_id = compat_sse::openai_chat_completion_id_from_callback_task(
                    plan.initial_run.id,
                    callback_task_id,
                );
                return compat_sse::start_openai_chat_resume_stream(
                    state,
                    plan.initial_run,
                    model,
                    completion_id,
                    plan.command,
                )
                .await
                .map_err(Into::into);
            }
            Ok(plan) => {
                let run = execute_openai_tool_resume(state, plan.command).await?;
                let completion_id = compat_sse::openai_chat_completion_id_from_callback_task(
                    run.id,
                    callback_task_id,
                );
                return Ok(Json(to_openai_response(run, model, completion_id)?).into_response());
            }
            Err(error)
                if error.status == StatusCode::NOT_FOUND && error.code == "callback_task" =>
            {
                // Stale compatible tool markers are history and must start a fresh turn.
            }
            Err(error) => return Err(error.into()),
        }
    }
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
    let previous_response_id = optional_string_field(&value, "previous_response_id")?;
    let previous_response = load_previous_response_context(
        state.clone(),
        &credential.token,
        previous_response_id.as_deref(),
    )
    .await?;
    let response_mode = value
        .get("stream")
        .and_then(Value::as_bool)
        .filter(|value| *value)
        .map(|_| "streaming".to_string());
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if endpoint == OpenAiResponsesEndpoint::Responses {
        if let Some(resume) = openai_responses_tool_resume_request(&value)? {
            let command = openai_resume_command(
                &credential.token,
                resume.callback_task_id,
                PublishedCallbackResumeSource::OpenAiResponses,
                resume.tool_results,
                response_mode.clone(),
            );
            match compat_sse::prepare_compatible_resume(state.clone(), command).await {
                Ok(plan) => {
                    ensure_openai_responses_resume_matches_previous_response(
                        state.as_ref(),
                        previous_response_id.as_deref(),
                        resume.callback_task_id,
                    )
                    .await?;
                    if response_mode.as_deref() == Some("streaming") {
                        return compat_sse::start_openai_response_resume_stream(
                            state,
                            plan.initial_run,
                            model,
                            previous_response_id,
                            plan.command,
                        )
                        .await
                        .map_err(Into::into);
                    }
                    let run = execute_openai_tool_resume(state, plan.command).await?;
                    return Ok(Json(to_openai_responses_response(
                        run,
                        model,
                        previous_response_id,
                    )?)
                    .into_response());
                }
                Err(error)
                    if error.status == StatusCode::NOT_FOUND && error.code == "callback_task" =>
                {
                    // Stale compatible tool markers are translated as input to a new turn below.
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
    let provider_transport_wire_body = value.clone();
    let translated = match translate_response_request_with_context_and_previous(
        value,
        request_context,
        previous_response,
    ) {
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
    let operation = *request.execution.execution_operation();
    if matches!(operation, AiNativeOperation::Compact(_)) {
        let payload = ProviderTransportPayload::openai_responses(provider_transport_wire_body)
            .map_err(|_| {
                OpenAiRouteError::Native(native::NativeApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "provider_transport_payload_invalid",
                    "could not stage the provider transport payload",
                ))
            })?;
        request.metadata.set_provider_transport_payload(payload);
    }
    let provider_transport_payload = request.metadata.take_provider_transport_payload();
    let model = request.model.clone().unwrap_or_default();
    let response_mode = request.response_mode.clone();
    match operation {
        AiNativeOperation::Generate(_) => {
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

            let provider_transport_slot = stage_openai_provider_transport(
                state.infrastructure.provider_transport_store().as_ref(),
                run.id,
                operation,
                provider_transport_payload,
            )
            .await?;

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
                return compat_sse::start_openai_response_stream(
                    state,
                    run,
                    model,
                    previous_response_id,
                    provider_transport_slot,
                )
                .await
                .map_err(Into::into);
            }

            let run = native::execute_blocking_native_run_with_provider_transport(
                state,
                credential.token,
                run,
                provider_transport_slot,
            )
            .await?;
            Ok(Json(to_openai_responses_response(
                run,
                model,
                previous_response_id,
            )?)
            .into_response())
        }
        AiNativeOperation::CountTokens => Err(openai_invalid_request(
            "operation",
            "count_tokens is not supported by the OpenAI Responses route",
        )),
        AiNativeOperation::Compact(remote_profile) => {
            let profile = match remote_profile {
                AiNativeCompactProfile::ResponsesCompact => {
                    ProviderCompactProfile::ResponsesCompact
                }
                AiNativeCompactProfile::ResponsesCompactionV2 => {
                    ProviderCompactProfile::ResponsesCompactionV2
                }
            };
            let run = create_native_run(state.clone(), credential.token.clone(), request).await?;
            let provider_transport_slot = stage_openai_provider_transport(
                state.infrastructure.provider_transport_store().as_ref(),
                run.id,
                operation,
                provider_transport_payload,
            )
            .await?;
            let run = native::execute_blocking_native_run_with_provider_transport(
                state,
                credential.token,
                run,
                provider_transport_slot,
            )
            .await?;
            let result = match run.operation_terminal.as_ref() {
                Some(NativeOperationTerminal::Compact(receipt)) => receipt.result().clone(),
                _ => return Err(native::blocking_run_projection_error(&run).into()),
            };

            info!(
                route,
                auth_source = credential.source,
                compaction_profile = profile.as_str(),
                response_mode = response_mode.as_deref().unwrap_or("blocking"),
                model = %model,
                translation_decision_count,
                "openai responses Compact request completed through a durable flow run"
            );

            match (profile, result) {
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
                        return compat_sse::openai_compact_sse_response(response)
                            .map_err(Into::into);
                    }
                    Ok(Json(response).into_response())
                }
                _ => Err(compact::unexpected_compact_result_error()),
            }
        }
    }
}

async fn stage_openai_provider_transport(
    store: &dyn ProviderTransportStore,
    flow_run_id: Uuid,
    operation: AiNativeOperation,
    payload: Option<ProviderTransportPayload>,
) -> Result<Option<ProviderTransportSlotId>, OpenAiRouteError> {
    if matches!(operation, AiNativeOperation::CountTokens) {
        return Ok(None);
    }
    let Some(payload) = payload else {
        return Ok(None);
    };
    let slot = ProviderTransportSlotId::for_flow_run(flow_run_id);
    store.put(slot, payload).await.map_err(|error| {
        warn!(
            flow_run_id = %flow_run_id,
            error = %error,
            "openai responses provider transport staging failed"
        );
        OpenAiRouteError::Native(native::NativeApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "provider_transport_staging_failed",
            "provider transport is temporarily unavailable",
        ))
    })?;
    Ok(Some(slot))
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

async fn execute_openai_tool_resume(
    state: Arc<ApiState>,
    command: ResumePublishedCallbackCommand,
) -> Result<NativeRunResult, OpenAiRouteError> {
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
            .resume_callback(command)
            .await
            .map_err(native::service_error)?;
    Ok(result.run)
}

fn openai_resume_command(
    bearer_token: &str,
    callback_task_id: Uuid,
    source: PublishedCallbackResumeSource,
    tool_results: Value,
    response_mode: Option<String>,
) -> ResumePublishedCallbackCommand {
    ResumePublishedCallbackCommand {
        bearer_token: bearer_token.to_string(),
        target: PublishedCallbackResumeTarget::CallbackTask { callback_task_id },
        source,
        response_payload: json!({ "tool_results": tool_results }),
        response_mode,
    }
}

fn openai_chat_tool_resume_request(
    request: &Value,
) -> Result<Option<OpenAiToolResumeRequest>, OpenAiRouteError> {
    let Some(messages) = request.get("messages").and_then(Value::as_array) else {
        return Ok(None);
    };
    let mut trailing = messages
        .iter()
        .rev()
        .take_while(|message| message.get("role").and_then(Value::as_str) == Some("tool"))
        .collect::<Vec<_>>();
    trailing.reverse();
    decode_openai_tool_results(
        trailing.into_iter().filter_map(|message| {
            message
                .get("tool_call_id")
                .and_then(Value::as_str)
                .map(|id| (id, openai_tool_message_content(message)))
        }),
        "messages",
    )
}

fn openai_responses_tool_resume_request(
    request: &Value,
) -> Result<Option<OpenAiToolResumeRequest>, OpenAiRouteError> {
    let Some(items) = request.get("input").and_then(Value::as_array) else {
        return Ok(None);
    };
    let trailing = items
        .iter()
        .rev()
        .take_while(|item| item.get("type").and_then(Value::as_str) == Some("function_call_output"))
        .collect::<Vec<_>>();
    decode_openai_tool_results(
        trailing.into_iter().rev().filter_map(|item| {
            item.get("call_id").and_then(Value::as_str).map(|id| {
                let output = match item.get("output") {
                    Some(Value::String(output)) => output.clone(),
                    Some(output) => output.to_string(),
                    None => String::new(),
                };
                (id, output)
            })
        }),
        "input",
    )
}

fn decode_openai_tool_results<'a>(
    results: impl Iterator<Item = (&'a str, String)>,
    param: &'static str,
) -> Result<Option<OpenAiToolResumeRequest>, OpenAiRouteError> {
    let mut callback_task_id = None;
    let mut tool_results = Vec::new();
    for (external_id, content) in results {
        let Some((task_id, original_id)) = decode_openai_callback_tool_call_id(external_id) else {
            continue;
        };
        if callback_task_id.is_some_and(|existing| existing != task_id) {
            return Err(openai_invalid_request(
                param,
                "tool results must belong to one callback task",
            ));
        }
        callback_task_id = Some(task_id);
        tool_results.push(json!({ "tool_call_id": original_id, "content": content }));
    }
    Ok(
        callback_task_id.map(|callback_task_id| OpenAiToolResumeRequest {
            callback_task_id,
            tool_results: Value::Array(tool_results),
        }),
    )
}

fn openai_tool_message_content(message: &Value) -> String {
    match message.get("content") {
        Some(Value::String(content)) => content.clone(),
        Some(Value::Null) | None => String::new(),
        Some(content) => content.to_string(),
    }
}

async fn ensure_openai_responses_resume_matches_previous_response(
    state: &ApiState,
    previous_response_id: Option<&str>,
    callback_task_id: Uuid,
) -> Result<(), OpenAiRouteError> {
    let Some(response_id) = previous_response_id else {
        return Ok(());
    };
    let previous_run_id = run_id_from_response_id(response_id)?;
    let callback_task = state
        .store
        .get_published_callback_task(callback_task_id)
        .await
        .map_err(native::service_error)?
        .ok_or_else(|| openai_invalid_request("input", "callback task was not found"))?;
    if callback_task.flow_run_id != previous_run_id {
        return Err(openai_invalid_request(
            "previous_response_id",
            "previous_response_id does not match function_call_output callback",
        ));
    }
    Ok(())
}

async fn load_previous_response_context(
    state: Arc<ApiState>,
    bearer_token: &str,
    previous_response_id: Option<&str>,
) -> Result<Option<OpenAiPreviousResponseContext>, OpenAiRouteError> {
    let Some(response_id) = previous_response_id else {
        return Ok(None);
    };
    let service = ApplicationNativeRunService::new(state.store.clone())
        .with_last_used_cache(state.infrastructure.cache_store());
    let run = match run_id_from_response_id(response_id) {
        Ok(run_id) => match service
            .get_native_run(GetNativeRunCommand {
                bearer_token: bearer_token.to_string(),
                run_id,
            })
            .await
        {
            Ok(run) => run,
            Err(NativeRunValidationError::NotFound) => service
                .get_native_run_by_provider_response_id(GetNativeRunByProviderResponseIdCommand {
                    bearer_token: bearer_token.to_string(),
                    provider_response_id: response_id.to_string(),
                })
                .await
                .map_err(native::native_error)?,
            Err(error) => return Err(native::native_error(error).into()),
        },
        Err(_) => service
            .get_native_run_by_provider_response_id(GetNativeRunByProviderResponseIdCommand {
                bearer_token: bearer_token.to_string(),
                provider_response_id: response_id.to_string(),
            })
            .await
            .map_err(native::native_error)?,
    };
    Ok(Some(OpenAiPreviousResponseContext {
        response_id: response_id.to_string(),
        external_user: string_value(&run.metadata, "external_user"),
        external_conversation_id: string_value(&run.metadata, "external_conversation_id"),
        answer: run.answer,
    }))
}

fn string_value(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn optional_string_field(
    value: &Value,
    field: &'static str,
) -> Result<Option<String>, OpenAiRouteError> {
    match value.get(field) {
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(openai_invalid_request(
            field,
            format!("{field} must be a string"),
        )),
        None => Ok(None),
    }
}

fn openai_invalid_request(param: &'static str, message: impl Into<String>) -> OpenAiRouteError {
    OpenAiCompatError {
        message: message.into(),
        error_type: "invalid_request_error".to_string(),
        param: Some(param.to_string()),
        code: "invalid_request".to_string(),
        report: TranslationReport::new(TranslationProtocol::OpenAiResponses),
    }
    .into()
}

fn to_openai_response(
    run: NativeRunResult,
    model: String,
    completion_id: String,
) -> Result<OpenAiChatCompletionResponse, OpenAiRouteError> {
    let callback_task_id = callback_task_id_from_required_action(&run);
    let tool_calls = openai_tool_calls(run.tool_calls.as_ref(), callback_task_id);
    let finish_reason = match run.status {
        NativeRunStatus::Succeeded => "stop",
        NativeRunStatus::Incomplete => "length",
        NativeRunStatus::Waiting if tool_calls.is_some() => "tool_calls",
        NativeRunStatus::Waiting => return Err(OpenAiRouteError::RequiredAction),
        NativeRunStatus::Created
        | NativeRunStatus::Queued
        | NativeRunStatus::Running
        | NativeRunStatus::Failed
        | NativeRunStatus::Cancelled => {
            return Err(native::blocking_run_projection_error(&run).into())
        }
    };
    let finish_reason = if tool_calls.is_some() {
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
    let callback_task_id = callback_task_id_from_required_action(&run);
    let function_call_items =
        openai_response_function_call_items(run.tool_calls.as_ref(), callback_task_id);
    let (status, incomplete_details) = match run.status {
        NativeRunStatus::Succeeded => ("completed", None),
        NativeRunStatus::Incomplete => (
            "incomplete",
            Some(OpenAiResponsesIncompleteDetails {
                reason: "max_output_tokens",
            }),
        ),
        NativeRunStatus::Waiting if function_call_items.is_some() => ("completed", None),
        NativeRunStatus::Waiting => return Err(OpenAiRouteError::RequiredAction),
        NativeRunStatus::Created
        | NativeRunStatus::Queued
        | NativeRunStatus::Running
        | NativeRunStatus::Failed
        | NativeRunStatus::Cancelled => {
            return Err(native::blocking_run_projection_error(&run).into())
        }
    };
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
