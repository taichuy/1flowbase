use super::client_observer::{self, CaptureGuard};
use control_plane::client_trajectory::{
    ClientTrajectoryFrameKind, ClientTrajectoryRecorder, ClientTrajectoryTransport,
};
use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    Json,
};
use control_plane::application_public_api::{
    callback_resume::{
        PublishedCallbackResumeSource, PublishedCallbackResumeTarget,
        ResumePublishedCallbackCommand,
    },
    client_protocol_envelope::{
        capture_client_protocol_envelope, capture_client_protocol_query,
        merge_client_protocol_envelopes, ClientProtocolIngressPolicy,
    },
    compat::openai::{
        classify_response_envelope_operation, response_id_from_run_id, run_id_from_response_id,
        translate_chat_completion_request, translate_response_envelope_with_decision_and_previous,
        OpenAiCompatError, OpenAiCompatibleModel, OpenAiPreviousResponseContext,
        OpenAiResponsesEndpoint, OpenAiResponsesEnvelope,
    },
    native::{
        ApplicationNativeRunService, NativeRunResult, NativeRunStatus, NativeRunValidationError,
    },
    protocol_translation::{
        TranslationDecisionKind, TranslationProtocol, TranslationReport,
        TranslationSafeRepresentation,
    },
    run_service::ApplicationPublishedRunControlRepository,
};
use control_plane::ports::{ProviderContinuationSlotId, ProviderTransportPayload};
use domain::{AiNativeCompactProfile, AiNativeOperation};
use orchestration_runtime::execution_state::NativeOperationTerminal;
use plugin_framework::provider_contract::{
    ProtocolContextEnvelope, ProviderCompactProfile, ProviderCompactResult,
};
use serde_json::{json, Value};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    routes::application_public_api::{
        callback_adapter::{
            correlate_openai_chat_callback, correlate_openai_responses_callback,
            restore_openai_responses_provider_call_ids,
        },
        compat_sse, compatibility_interface,
        llm_tool_visibility::external_llm_tool_calls,
        native,
        tool_callback_ids::encode_openai_callback_tool_call_id,
    },
};

pub(crate) mod chat_usage;
mod compact;
mod model_list;
mod response_dispatch;
use response_dispatch::dispatch_response_for_endpoint;
mod session_context;
#[cfg(test)]
mod tests;
mod types;

use model_list::{
    is_codex_model_list_request, to_codex_model_list_response, to_openai_model_list_response,
};
pub use types::{
    OpenAiChatCompletionChoice, OpenAiChatCompletionResponse, OpenAiChatMessage,
    OpenAiCompletionTokensDetails, OpenAiErrorBody, OpenAiErrorObject, OpenAiModelListQuery,
    OpenAiModelListResponse, OpenAiModelObject, OpenAiPromptTokensDetails,
    OpenAiResponsesIncompleteDetails, OpenAiResponsesInputTokensDetails, OpenAiResponsesObject,
    OpenAiResponsesOutputTokensDetails, OpenAiResponsesUsage, OpenAiRouteError, OpenAiToolCall,
    OpenAiToolCallFunction, OpenAiUsage,
};

pub(super) struct OpenAiCredential {
    pub(super) token: String,
    pub(super) source: &'static str,
}

pub(crate) fn responses_transport_owner_id(
    principal: &interface_runtime::ApplicationPrincipal,
    headers: &HeaderMap,
) -> Result<String, OpenAiRouteError> {
    session_context::responses_session_identity(principal, headers, None)
}

/// A Generate or tool-resume turn accepted by the same ingress used by HTTP
/// Responses, before any public transport projection is selected.
pub(crate) struct PreparedOpenAiResponseTurn {
    model: String,
    previous_response_id: Option<String>,
    runtime: compatibility_interface::CompatibilityTypedStreamInvocation,
    projection_mode: compat_sse::ResponsesProjectionMode,
}

impl PreparedOpenAiResponseTurn {
    pub(crate) fn into_parts(
        self,
    ) -> (
        String,
        Option<String>,
        compatibility_interface::CompatibilityTypedStreamInvocation,
        compat_sse::ResponsesProjectionMode,
    ) {
        (
            self.model,
            self.previous_response_id,
            self.runtime,
            self.projection_mode,
        )
    }
}

#[derive(Clone, Copy)]
enum OpenAiResponseDelivery {
    Http,
    TypedEvents,
}

enum OpenAiResponseDispatch {
    Http(Response),
    TypedEvents(Box<PreparedOpenAiResponseTurn>),
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
    uri: Uri,
    body: Bytes,
) -> Result<Response, OpenAiRouteError> {
    let recorder = client_observer::recorder(&state, ClientTrajectoryTransport::Http);
    if recorder
        .record(ClientTrajectoryFrameKind::Request, &body)
        .await
        .is_err()
    {
        recorder.mark_incomplete();
    }
    let capture = CaptureGuard::new(recorder.clone());
    let response = match dispatch_chat_completion(state, headers, uri, body, recorder).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    };
    Ok(client_observer::observe_response(response, capture))
}

async fn dispatch_chat_completion(
    state: Arc<ApiState>,
    headers: HeaderMap,
    uri: Uri,
    body: Bytes,
    recorder: ClientTrajectoryRecorder,
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
    super::provider_session::session_header(&headers, "session-id")?;
    super::provider_session::session_header(&headers, "thread-id")?;
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
    let mut authenticated_resume = None;
    if let Some(resume) = correlate_openai_chat_callback(&value)
        .map_err(|error| openai_invalid_request(error.param, error.message))?
    {
        let callback_task_id = resume.callback_task_id;
        let resume_binding_id = if response_mode.as_deref() == Some("streaming") {
            if uri.path() == "/chat/completions" {
                compatibility_interface::OPENAI_CHAT_ROOT_STREAM_BINDING_ID
            } else {
                compatibility_interface::OPENAI_CHAT_STREAM_BINDING_ID
            }
        } else if uri.path() == "/chat/completions" {
            compatibility_interface::OPENAI_CHAT_ROOT_BINDING_ID
        } else {
            compatibility_interface::OPENAI_CHAT_BINDING_ID
        };
        let principal = compatibility_interface::authenticate_application_principal(
            state.clone(),
            resume_binding_id,
            credential.token.clone(),
        )
        .await?;
        let actor = compatibility_interface::application_actor(principal.principal());
        let mut command = openai_resume_command(
            "",
            callback_task_id,
            PublishedCallbackResumeSource::OpenAiChat,
            resume.tool_results,
            response_mode.clone(),
        );
        command.observation_context =
            Some(control_plane_contracts::ports::WorkflowObservationContext {
                client_request_id: recorder.capture_id(),
                context_flow_run_id: None,
                context_response_id: None,
                is_resume: true,
            });
        match compat_sse::prepare_compatible_resume_for_actor(state.clone(), actor, command).await {
            Ok(compat_sse::CompatibleResumeAdmission::Resume(plan))
                if response_mode.as_deref() == Some("streaming") =>
            {
                let usage_baseline = load_chat_usage_baseline(&state, callback_task_id).await?;
                let completion_id = compat_sse::openai_chat_completion_id_from_callback_task(
                    plan.initial_run.id,
                    callback_task_id,
                );
                return compatibility_interface::invoke_client_stream_with_principal(
                    state,
                    resume_binding_id,
                    principal,
                    compatibility_interface::CompatibilityBlockingInput {
                        command: compatibility_interface::CompatibilityInvocationCommand::Resume {
                            initial_run: plan.initial_run,
                            command: plan.command,
                            reservation: plan.reservation,
                        },
                    },
                    Some(recorder.clone()),
                    compat_sse::openai_chat_resume_interface_projection(
                        model,
                        completion_id,
                        usage_baseline,
                    ),
                )
                .await
                .map_err(Into::into);
            }
            Ok(compat_sse::CompatibleResumeAdmission::Resume(plan)) => {
                let usage_baseline = load_chat_usage_baseline(&state, callback_task_id).await?;
                let run = compatibility_interface::invoke_blocking_with_principal(
                    state.clone(),
                    resume_binding_id,
                    principal,
                    compatibility_interface::CompatibilityBlockingInput {
                        command: compatibility_interface::CompatibilityInvocationCommand::Resume {
                            initial_run: plan.initial_run,
                            command: plan.command,
                            reservation: plan.reservation,
                        },
                    },
                )
                .await?;
                correlate_blocking_chat_capture(&state, &recorder, run.id).await;
                let completion_id = compat_sse::openai_chat_completion_id_from_callback_task(
                    run.id,
                    callback_task_id,
                );
                return Ok(Json(chat_usage::to_chat_response_for_round(
                    run,
                    model,
                    completion_id,
                    &usage_baseline,
                )?)
                .into_response());
            }
            Ok(compat_sse::CompatibleResumeAdmission::StartNewTurnFromHistory { .. }) => {
                // The callback delivery is complete; re-admit its full history as a new turn.
            }
            Err(error)
                if error.status == StatusCode::NOT_FOUND && error.code == "callback_task" =>
            {
                // Stale compatible tool markers are history and must start a fresh turn.
            }
            Err(error) => return Err(error.into()),
        }
        authenticated_resume = Some(principal);
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
    request.metadata.set_observation_context(Some(
        control_plane_contracts::ports::WorkflowObservationContext {
            client_request_id: recorder.capture_id(),
            context_flow_run_id: None,
            context_response_id: None,
            is_resume: false,
        },
    ));
    request.client_protocol_envelope = openai_protocol_context_from_ingress(
        ClientProtocolIngressPolicy::OpenAiChat,
        uri.query(),
        &headers,
        request.client_protocol_envelope,
    );
    let model = request.model.clone().unwrap_or_default();
    let response_mode = request.response_mode.clone();
    let binding_id = if uri.path() == "/chat/completions" {
        compatibility_interface::OPENAI_CHAT_ROOT_BINDING_ID
    } else {
        compatibility_interface::OPENAI_CHAT_BINDING_ID
    };

    let authentication_binding = if response_mode.as_deref() == Some("streaming") {
        if uri.path() == "/chat/completions" {
            compatibility_interface::OPENAI_CHAT_ROOT_STREAM_BINDING_ID
        } else {
            compatibility_interface::OPENAI_CHAT_STREAM_BINDING_ID
        }
    } else {
        binding_id
    };
    let principal = match authenticated_resume {
        Some(principal) => principal,
        None => {
            compatibility_interface::authenticate_application_principal(
                state.clone(),
                authentication_binding,
                credential.token,
            )
            .await?
        }
    };
    super::provider_session::bind_chat(&mut request, principal.principal(), &headers)?;
    if response_mode.as_deref() == Some("streaming") {
        let stream_binding_id = if uri.path() == "/chat/completions" {
            compatibility_interface::OPENAI_CHAT_ROOT_STREAM_BINDING_ID
        } else {
            compatibility_interface::OPENAI_CHAT_STREAM_BINDING_ID
        };
        return compatibility_interface::invoke_client_stream_with_principal(
            state,
            stream_binding_id,
            principal,
            compatibility_interface::CompatibilityBlockingInput {
                command: compatibility_interface::CompatibilityInvocationCommand::Start {
                    request,
                    protocol: TranslationProtocol::OpenAiChat,
                    provider_transport: None,
                },
            },
            Some(recorder.clone()),
            compat_sse::openai_chat_interface_projection(model),
        )
        .await
        .map_err(Into::into);
    }

    let run = compatibility_interface::invoke_blocking_with_principal(
        state.clone(),
        binding_id,
        principal,
        compatibility_interface::CompatibilityBlockingInput {
            command: compatibility_interface::CompatibilityInvocationCommand::Start {
                request,
                protocol: TranslationProtocol::OpenAiChat,
                provider_transport: None,
            },
        },
    )
    .await?;

    correlate_blocking_chat_capture(&state, &recorder, run.id).await;
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

    let completion_id = compat_sse::openai_chat_completion_id_from_run_id(run.id);
    Ok(Json(to_openai_response(run, model, completion_id)?).into_response())
}

async fn load_chat_usage_baseline(
    state: &ApiState,
    callback_task_id: Uuid,
) -> Result<chat_usage::ChatUsageBaseline, OpenAiRouteError> {
    // Admission already verified the callback's application/API actor ownership.
    let callback = state
        .store
        .get_published_callback_task(callback_task_id)
        .await
        .map_err(|error| {
            native::NativeApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "chat_usage_boundary_failed",
                error.to_string(),
            )
        })?;
    Ok(callback
        .map(|task| chat_usage::ChatUsageBaseline::from_callback_payload(&task.request_payload))
        .unwrap_or(chat_usage::ChatUsageBaseline::Unavailable))
}

/// Blocking execution already persisted provider observations. Read their request-scoped
/// identities without changing the execution mode or protocol projection.
async fn correlate_blocking_chat_capture(
    state: &ApiState,
    recorder: &ClientTrajectoryRecorder,
    flow_run_id: Uuid,
) {
    use control_plane::ports::{ProviderTrajectoryRepository, TrajectorySelection};
    recorder.bind_run(flow_run_id, None);
    let mut cursor = None;
    loop {
        let page = match state
            .store
            .provider_trajectory_filtered_page(
                flow_run_id,
                None,
                cursor,
                100,
                TrajectorySelection {
                    request_id: Some(recorder.capture_id()),
                    target_id: None,
                },
            )
            .await
        {
            Ok(page) => page,
            Err(error) => {
                recorder.mark_incomplete();
                warn!(%flow_run_id, %error, "blocking Chat client node correlation failed");
                return;
            }
        };
        for item in page.items {
            if let Some(node_run_id) = item.metadata["node_run_id"]
                .as_str()
                .and_then(|id| Uuid::parse_str(id).ok())
            {
                recorder.link_llm_node(flow_run_id, node_run_id);
            }
        }
        match page.next_cursor {
            Some(next) if cursor.is_none_or(|previous| next > previous) => cursor = Some(next),
            Some(_) => {
                recorder.mark_incomplete();
                return;
            }
            None => return,
        }
    }
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
    uri: Uri,
    body: Bytes,
) -> Result<Response, OpenAiRouteError> {
    create_response_for_endpoint(
        state,
        headers,
        uri.path().to_string(),
        uri.query().map(ToOwned::to_owned),
        body,
        OpenAiResponsesEndpoint::Responses,
    )
    .await
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
    uri: Uri,
    body: Bytes,
) -> Result<Response, OpenAiRouteError> {
    create_response_for_endpoint(
        state,
        headers,
        uri.path().to_string(),
        uri.query().map(ToOwned::to_owned),
        body,
        OpenAiResponsesEndpoint::ResponsesCompact,
    )
    .await
}

async fn create_response_for_endpoint(
    state: Arc<ApiState>,
    headers: HeaderMap,
    route_path: String,
    raw_query: Option<String>,
    body: Bytes,
    endpoint: OpenAiResponsesEndpoint,
) -> Result<Response, OpenAiRouteError> {
    let recorder = client_observer::recorder(&state, ClientTrajectoryTransport::Http);
    if recorder
        .record(ClientTrajectoryFrameKind::Request, &body)
        .await
        .is_err()
    {
        recorder.mark_incomplete();
    }
    let capture = CaptureGuard::new(recorder.clone());
    let result = dispatch_response_for_endpoint(
        state,
        headers,
        OpenAiResponseDispatchRequest {
            route_path,
            raw_query,
            body: OpenAiResponseBody::Raw(body),
            endpoint,
            delivery: OpenAiResponseDelivery::Http,
            transport_connection_scope: None,
            recorder: Some(recorder),
        },
        None,
    )
    .await;
    let response = match result {
        Err(error) => error.into_response(),
        Ok(OpenAiResponseDispatch::Http(response)) => response,
        Ok(OpenAiResponseDispatch::TypedEvents(_)) => {
            OpenAiRouteError::Native(native::NativeApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "openai_response_delivery_mismatch",
                "OpenAI Responses HTTP delivery produced a typed turn",
            ))
            .into_response()
        }
    };
    Ok(client_observer::observe_response(response, capture))
}

pub(crate) async fn prepare_typed_response_turn(
    state: Arc<ApiState>,
    principal: interface_runtime::ApplicationPrincipal,
    headers: HeaderMap,
    body: Value,
    transport_connection_scope: String,
    recorder: Option<ClientTrajectoryRecorder>,
) -> Result<PreparedOpenAiResponseTurn, OpenAiRouteError> {
    match dispatch_response_for_endpoint(
        state,
        headers,
        OpenAiResponseDispatchRequest {
            route_path: "/v1/responses".to_string(),
            raw_query: None,
            body: OpenAiResponseBody::Parsed(body),
            endpoint: OpenAiResponsesEndpoint::Responses,
            delivery: OpenAiResponseDelivery::TypedEvents,
            transport_connection_scope: Some(transport_connection_scope),
            recorder,
        },
        Some(principal),
    )
    .await?
    {
        OpenAiResponseDispatch::TypedEvents(prepared) => Ok(*prepared),
        OpenAiResponseDispatch::Http(_) => {
            Err(OpenAiRouteError::Native(native::NativeApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "openai_response_delivery_mismatch",
                "OpenAI Responses typed delivery produced an HTTP response",
            )))
        }
    }
}

enum OpenAiResponseBody {
    Raw(Bytes),
    // The WebSocket frame was parsed and size-checked before this common ingress.
    Parsed(Value),
}

struct OpenAiResponseDispatchRequest {
    route_path: String,
    raw_query: Option<String>,
    body: OpenAiResponseBody,
    endpoint: OpenAiResponsesEndpoint,
    delivery: OpenAiResponseDelivery,
    transport_connection_scope: Option<String>,
    recorder: Option<ClientTrajectoryRecorder>,
}

fn responses_callback_provider_transport(
    envelope: &OpenAiResponsesEnvelope,
    native_resume: bool,
    encoded_resume: bool,
) -> Result<Option<ProviderTransportPayload>, OpenAiRouteError> {
    if !native_resume && !encoded_resume {
        return Ok(None);
    }
    // A native callback can still carry older gateway-projected function calls
    // in its full Responses history. Adapter-owned correlation IDs must never
    // cross the Provider boundary, regardless of how the current tail was
    // correlated.
    ProviderTransportPayload::openai_responses(restore_openai_responses_provider_call_ids(
        envelope.raw_body(),
    ))
    .map(Some)
    .map_err(|_| {
        openai_invalid_request(
            "input",
            "Responses callback transport could not be restored",
        )
    })
}

fn responses_resume_requires_previous_response_match(correlated_from_native_state: bool) -> bool {
    !correlated_from_native_state
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
    uri: Uri,
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
    let binding_id = match uri.path() {
        "/models" => compatibility_interface::OPENAI_MODELS_ROOT_BINDING_ID,
        "/v1/chat/completions/models" => compatibility_interface::OPENAI_CHAT_MODELS_BINDING_ID,
        _ => compatibility_interface::OPENAI_MODELS_BINDING_ID,
    };
    let output = compatibility_interface::invoke_models(state, binding_id, credential.token)
        .await
        .map_err(OpenAiRouteError::from)?;
    let model_count = output.models.len();

    info!(
        route = "models",
        auth_source = credential.source,
        model_count,
        "openai compatible model list returned"
    );

    if is_codex_model_list_request(&query) {
        return Ok(Json(to_codex_model_list_response(output.models)).into_response());
    }

    Ok(Json(to_openai_model_list_response(
        output.models,
        output.publication_created_at,
    ))
    .into_response())
}

pub(super) fn openai_credential(
    headers: &HeaderMap,
) -> Result<OpenAiCredential, native::NativeApiError> {
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

fn openai_protocol_context_from_ingress(
    policy: ClientProtocolIngressPolicy,
    raw_query: Option<&str>,
    headers: &HeaderMap,
    translated: Option<ProtocolContextEnvelope>,
) -> Option<ProtocolContextEnvelope> {
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
    let mut envelope = merge_client_protocol_envelopes(policy, captured, translated);
    if let Some(envelope) = envelope.as_mut() {
        envelope
            .headers
            .remove(control_plane::orchestration_runtime::HOST_TRANSPORT_CONNECTION_SCOPE_HEADER);
    }
    envelope
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

fn openai_resume_command(
    bearer_token: &str,
    callback_task_id: Uuid,
    source: PublishedCallbackResumeSource,
    tool_results: Value,
    response_mode: Option<String>,
) -> ResumePublishedCallbackCommand {
    ResumePublishedCallbackCommand {
        responses_continuation: None,
        transport_connection_scope: None,
        observation_context: None,
        reserved_attempt_id: None,
        native_transport: None,
        bearer_token: bearer_token.to_string(),
        target: PublishedCallbackResumeTarget::CallbackTask { callback_task_id },
        source,
        response_payload: json!({ "tool_results": tool_results }),
        response_mode,
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

async fn load_previous_response_context_for_actor(
    state: Arc<ApiState>,
    actor: control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
    previous_response_id: Option<&str>,
) -> Result<Option<LoadedOpenAiPreviousResponseContext>, OpenAiRouteError> {
    let Some(response_id) = previous_response_id else {
        return Ok(None);
    };
    let service = ApplicationNativeRunService::new(state.store.clone())
        .with_last_used_cache(state.infrastructure.cache_store());
    let semantic = state
        .store
        .find_semantic_responses_callbacks_by_response_id(
            actor.workspace_id,
            actor.application_id,
            actor.api_key_id,
            actor.creator_user_id,
            response_id,
        )
        .await
        .map_err(native::service_error)?;
    if semantic.len() > 1 {
        return Err(native::service_error(
            control_plane::errors::ControlPlaneError::Conflict(
                "responses_tool_output_ambiguous_round",
            )
            .into(),
        )
        .into());
    }
    let run = if let Some(callback) = semantic.first() {
        service
            .get_native_run_for_actor(actor.clone(), callback.flow_run_id)
            .await
            .map_err(native::native_error)?
    } else {
        match run_id_from_response_id(response_id) {
            Ok(run_id) => match service
                .get_native_run_for_actor(actor.clone(), run_id)
                .await
            {
                Ok(run) => run,
                Err(NativeRunValidationError::NotFound) => service
                    .get_native_run_by_provider_response_id_for_actor(actor.clone(), response_id)
                    .await
                    .map_err(native::native_error)?,
                Err(error) => return Err(native::native_error(error).into()),
            },
            Err(_) => service
                .get_native_run_by_provider_response_id_for_actor(actor, response_id)
                .await
                .map_err(native::native_error)?,
        }
    };
    ensure_previous_response_is_usable(&run)?;
    let provider_continuation = state
        .infrastructure
        .provider_transport_store()
        .get_continuation(ProviderContinuationSlotId::for_response_round(
            run.id,
            run_id_from_response_id(response_id)?,
        ))
        .await
        .map_err(|_| {
            OpenAiRouteError::Native(native::NativeApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "provider_continuation_lookup_failed",
                "Provider continuation storage is temporarily unavailable",
            ))
        })?;
    Ok(Some(LoadedOpenAiPreviousResponseContext {
        flow_run_id: run.id,
        session_identity: provider_continuation
            .as_ref()
            .and_then(|continuation| continuation.session_identity())
            .map(str::to_owned),
        translation: OpenAiPreviousResponseContext {
            provider_continuation,
            response_id: response_id.to_string(),
            external_user: string_value(&run.metadata, "external_user"),
            external_conversation_id: string_value(&run.metadata, "external_conversation_id"),
            answer: run.answer,
        },
    }))
}

fn ensure_previous_response_is_usable(run: &NativeRunResult) -> Result<(), OpenAiRouteError> {
    if run.status == NativeRunStatus::Cancelled {
        return Err(OpenAiRouteError::Native(native::NativeApiError::new(
            StatusCode::CONFLICT,
            "previous_response_not_usable",
            "the previous response was cancelled or superseded",
        )));
    }
    Ok(())
}

struct LoadedOpenAiPreviousResponseContext {
    flow_run_id: Uuid,
    session_identity: Option<String>,
    translation: OpenAiPreviousResponseContext,
}

fn string_value(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
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

#[path = "openai/response_projection.rs"]
mod response_projection;

pub(super) use response_projection::native_response_id;
pub(super) use response_projection::openai_responses_usage;
pub(super) use response_projection::openai_usage;
#[cfg(test)]
pub(super) use response_projection::to_openai_responses_response;
use response_projection::*;
