use super::*;
use std::{future::Future, pin::Pin};

pub(super) struct ResponseExecutionContext {
    pub(super) state: Arc<ApiState>,
    // Retain the frozen registry and authenticated HTTP carrier as one owner.
    pub(super) principal: compatibility_interface::ApplicationInvocationAuthentication,
    pub(super) route_path: String,
    pub(super) delivery: OpenAiResponseDelivery,
    pub(super) model: String,
    pub(super) response_mode: Option<String>,
    pub(super) previous_response_id: Option<String>,
    pub(super) projection_mode: compat_sse::ResponsesProjectionMode,
    pub(super) recorder: Option<ClientTrajectoryRecorder>,
}

pub(super) struct PreparedStartResponse {
    pub(super) context: ResponseExecutionContext,
    pub(super) route: &'static str,
    pub(super) auth_source: &'static str,
    pub(super) translation_decision_count: usize,
    pub(super) blocking_binding_id: &'static str,
    pub(super) request: control_plane::application_public_api::native::NativeRunRequest,
    pub(super) operation: AiNativeOperation,
    pub(super) provider_transport_payload: Option<ProviderTransportPayload>,
}

pub(super) struct PreparedResumeResponse {
    pub(super) context: ResponseExecutionContext,
    pub(super) authentication_binding_id: &'static str,
    pub(super) plan: Box<compat_sse::CompatibleResumePlan>,
}

pub(super) enum PreparedResponseDispatch {
    Start(Box<PreparedStartResponse>),
    Resume(Box<PreparedResumeResponse>),
}

type ResponseExecutionFuture =
    Pin<Box<dyn Future<Output = Result<OpenAiResponseDispatch, OpenAiRouteError>> + Send>>;

/// Construct only the selected variant before polling it. Keeping this factory
/// synchronous prevents its future-construction temporaries from being retained
/// on the caller's poll stack while a provider invocation is in flight.
pub(super) fn execute(prepared: PreparedResponseDispatch) -> ResponseExecutionFuture {
    match prepared {
        PreparedResponseDispatch::Start(start) => match start.operation {
            AiNativeOperation::Generate(_) => Box::pin(execute_generate(*start)),
            AiNativeOperation::CountTokens => Box::pin(async {
                Err(openai_invalid_request(
                    "operation",
                    "count_tokens is not supported by the OpenAI Responses route",
                ))
            }),
            AiNativeOperation::Compact(profile) => Box::pin(execute_compact(*start, profile)),
        },
        PreparedResponseDispatch::Resume(resume) => Box::pin(execute_resume(*resume)),
    }
}

async fn execute_generate(
    start: PreparedStartResponse,
) -> Result<OpenAiResponseDispatch, OpenAiRouteError> {
    let PreparedStartResponse {
        context,
        route,
        auth_source,
        translation_decision_count,
        request,
        operation,
        provider_transport_payload,
        blocking_binding_id: _,
    } = start;
    let ResponseExecutionContext {
        state,
        principal,
        route_path,
        delivery,
        model,
        response_mode,
        previous_response_id,
        projection_mode,
        recorder,
    } = context;
    if response_mode.as_deref() != Some("streaming") {
        let stream_binding_id = if route_path == "/responses" {
            compatibility_interface::OPENAI_RESPONSES_ROOT_STREAM_BINDING_ID
        } else {
            compatibility_interface::OPENAI_RESPONSES_STREAM_BINDING_ID
        };
        let invocation = compatibility_interface::invoke_typed_stream_with_principal(
            state,
            stream_binding_id,
            principal,
            compatibility_interface::CompatibilityBlockingInput {
                command: compatibility_interface::CompatibilityInvocationCommand::Start {
                    request,
                    protocol: TranslationProtocol::OpenAiResponses,
                    provider_transport: Some(
                        compatibility_interface::CompatibilityProviderTransport {
                            operation,
                            payload: provider_transport_payload,
                        },
                    ),
                },
            },
        )
        .await?;
        let response = collect_blocking_native_response(
            invocation,
            model,
            previous_response_id,
            recorder.as_ref(),
            projection_mode,
        )
        .await?;
        info!(
            route,
            auth_source,
            response_mode = "blocking",
            translation_decision_count,
            "openai responses compatible request accepted"
        );
        return Ok(OpenAiResponseDispatch::Http(Json(response).into_response()));
    }

    if matches!(delivery, OpenAiResponseDelivery::Http) {
        let stream_binding_id = if route_path == "/responses" {
            compatibility_interface::OPENAI_RESPONSES_ROOT_STREAM_BINDING_ID
        } else {
            compatibility_interface::OPENAI_RESPONSES_STREAM_BINDING_ID
        };
        let projection = compat_sse::openai_responses_interface_projection_with_mode(
            model,
            previous_response_id,
            projection_mode,
        );
        let response = compatibility_interface::invoke_client_stream_with_principal(
            state,
            stream_binding_id,
            principal,
            compatibility_interface::CompatibilityBlockingInput {
                command: compatibility_interface::CompatibilityInvocationCommand::Start {
                    request,
                    protocol: TranslationProtocol::OpenAiResponses,
                    provider_transport: Some(
                        compatibility_interface::CompatibilityProviderTransport {
                            operation,
                            payload: provider_transport_payload,
                        },
                    ),
                },
            },
            recorder.clone(),
            projection,
        )
        .await?;
        return Ok(OpenAiResponseDispatch::Http(response));
    }

    let runtime = compatibility_interface::invoke_typed_stream_with_principal(
        state,
        compatibility_interface::OPENAI_RESPONSES_WEBSOCKET_STREAM_BINDING_ID,
        principal,
        compatibility_interface::CompatibilityBlockingInput {
            command: compatibility_interface::CompatibilityInvocationCommand::Start {
                request,
                protocol: TranslationProtocol::OpenAiResponses,
                provider_transport: Some(compatibility_interface::CompatibilityProviderTransport {
                    operation,
                    payload: provider_transport_payload,
                }),
            },
        },
    )
    .await?;
    Ok(OpenAiResponseDispatch::TypedEvents(Box::new(
        PreparedOpenAiResponseTurn {
            model,
            previous_response_id,
            runtime,
            projection_mode,
        },
    )))
}

async fn execute_compact(
    start: PreparedStartResponse,
    remote_profile: AiNativeCompactProfile,
) -> Result<OpenAiResponseDispatch, OpenAiRouteError> {
    let PreparedStartResponse {
        context,
        route,
        auth_source,
        translation_decision_count,
        blocking_binding_id,
        request,
        operation,
        provider_transport_payload,
    } = start;
    let ResponseExecutionContext {
        state,
        principal,
        route_path: _,
        delivery,
        model,
        response_mode,
        previous_response_id: _,
        projection_mode: _,
        recorder,
    } = context;
    if matches!(delivery, OpenAiResponseDelivery::TypedEvents) {
        return Err(openai_invalid_request(
            "operation",
            "Compact is a unary operation and cannot open a typed event turn",
        ));
    }
    let profile = match remote_profile {
        AiNativeCompactProfile::ResponsesCompact => ProviderCompactProfile::ResponsesCompact,
        AiNativeCompactProfile::ResponsesCompactionV2 => {
            ProviderCompactProfile::ResponsesCompactionV2
        }
    };
    let run = compatibility_interface::invoke_blocking_with_principal(
        state,
        blocking_binding_id,
        principal,
        compatibility_interface::CompatibilityBlockingInput {
            command: compatibility_interface::CompatibilityInvocationCommand::Start {
                request,
                protocol: TranslationProtocol::OpenAiResponses,
                provider_transport: Some(compatibility_interface::CompatibilityProviderTransport {
                    operation,
                    payload: provider_transport_payload,
                }),
            },
        },
    )
    .await?;
    if let Some(recorder) = recorder.as_ref() {
        recorder.bind_run(run.id, None);
    }
    let result = match run.operation_terminal.as_ref() {
        Some(NativeOperationTerminal::Compact(receipt)) => receipt.result().clone(),
        _ => return Err(native::blocking_run_projection_error(&run).into()),
    };

    info!(
        route,
        auth_source,
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
            Ok(OpenAiResponseDispatch::Http(
                Json(response_items).into_response(),
            ))
        }
        (
            ProviderCompactProfile::ResponsesCompactionV2,
            ProviderCompactResult::CompletedOpaqueCompactionItem {
                response_id,
                compaction_item,
                ..
            },
        ) => {
            let response =
                compact::completed_v2_compaction_response(model, response_id, compaction_item);
            if response_mode.as_deref() == Some("streaming") {
                let response = serde_json::to_value(response).map_err(|_| {
                    OpenAiRouteError::Native(native::NativeApiError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "openai_compact_response_serialization_failed",
                        "could not serialize OpenAI Compact response",
                    ))
                })?;
                return compat_sse::openai_compact_sse_response(response)
                    .map(OpenAiResponseDispatch::Http)
                    .map_err(Into::into);
            }
            Ok(OpenAiResponseDispatch::Http(Json(response).into_response()))
        }
        _ => Err(compact::unexpected_compact_result_error()),
    }
}

async fn execute_resume(
    resume: PreparedResumeResponse,
) -> Result<OpenAiResponseDispatch, OpenAiRouteError> {
    let PreparedResumeResponse {
        context,
        authentication_binding_id,
        plan,
    } = resume;
    let ResponseExecutionContext {
        state,
        principal,
        route_path,
        delivery,
        model,
        response_mode,
        previous_response_id,
        projection_mode,
        recorder,
    } = context;
    let compat_sse::CompatibleResumePlan {
        initial_run,
        command,
        reservation,
    } = *plan;
    if response_mode.as_deref() == Some("streaming") {
        let input = compatibility_interface::CompatibilityBlockingInput {
            command: compatibility_interface::CompatibilityInvocationCommand::Resume {
                initial_run,
                command,
                reservation,
            },
        };
        return match delivery {
            OpenAiResponseDelivery::Http => {
                compatibility_interface::invoke_client_stream_with_principal(
                    state,
                    authentication_binding_id,
                    principal,
                    input,
                    recorder.clone(),
                    compat_sse::openai_responses_interface_projection_with_mode(
                        model,
                        previous_response_id,
                        projection_mode,
                    ),
                )
                .await
                .map(OpenAiResponseDispatch::Http)
                .map_err(Into::into)
            }
            OpenAiResponseDelivery::TypedEvents => {
                let runtime = compatibility_interface::invoke_typed_stream_with_principal(
                    state,
                    compatibility_interface::OPENAI_RESPONSES_WEBSOCKET_STREAM_BINDING_ID,
                    principal,
                    input,
                )
                .await?;
                Ok(OpenAiResponseDispatch::TypedEvents(Box::new(
                    PreparedOpenAiResponseTurn {
                        model,
                        previous_response_id,
                        runtime,
                        projection_mode,
                    },
                )))
            }
        };
    }
    // Match blocking Generate: the typed turn owns background
    // execution, avoiding inline callback polling on the HTTP stack.
    let stream_binding_id = if route_path == "/responses" {
        compatibility_interface::OPENAI_RESPONSES_ROOT_STREAM_BINDING_ID
    } else {
        compatibility_interface::OPENAI_RESPONSES_STREAM_BINDING_ID
    };
    let invocation = compatibility_interface::invoke_typed_stream_with_principal(
        state,
        stream_binding_id,
        principal,
        compatibility_interface::CompatibilityBlockingInput {
            command: compatibility_interface::CompatibilityInvocationCommand::Resume {
                initial_run,
                command,
                reservation,
            },
        },
    )
    .await?;
    let response = collect_blocking_native_response(
        invocation,
        model,
        previous_response_id,
        recorder.as_ref(),
        projection_mode,
    )
    .await?;
    Ok(OpenAiResponseDispatch::Http(Json(response).into_response()))
}
