use super::*;

pub(super) async fn dispatch_response_for_endpoint(
    state: Arc<ApiState>,
    headers: HeaderMap,
    request: OpenAiResponseDispatchRequest,
    preauthenticated_principal: Option<interface_runtime::ApplicationPrincipal>,
) -> Result<OpenAiResponseDispatch, OpenAiRouteError> {
    let OpenAiResponseDispatchRequest {
        route_path,
        raw_query,
        body,
        endpoint,
        delivery,
        transport_connection_scope,
        recorder,
    } = request;
    let route = match endpoint {
        OpenAiResponsesEndpoint::Responses => "responses",
        OpenAiResponsesEndpoint::ResponsesCompact => "responses_compact",
    };
    let credential = if preauthenticated_principal.is_none() {
        Some(match openai_credential(&headers) {
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
        })
    } else {
        None
    };
    let auth_source = credential
        .as_ref()
        .map(|credential| credential.source)
        .unwrap_or("frozen_websocket_principal");
    let parsed = match body {
        OpenAiResponseBody::Raw(body) => {
            parse_openai_json_body(body, TranslationProtocol::OpenAiResponses)
        }
        OpenAiResponseBody::Parsed(value) => Ok(value),
    };
    let mut value = match parsed {
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
    if matches!(delivery, OpenAiResponseDelivery::TypedEvents) {
        value
            .as_object_mut()
            .ok_or_else(|| openai_invalid_request("response", "response must be an object"))?
            .insert("stream".to_string(), Value::Bool(true));
    }
    let responses_envelope = OpenAiResponsesEnvelope::capture(value).map_err(|error| {
        let route_error = OpenAiRouteError::from(error);
        warn_openai_route_error(
            route,
            &route_error,
            "openai responses protocol envelope validation failed",
        );
        route_error
    })?;
    let previous_response_id = responses_envelope.previous_response_id().map(str::to_owned);
    let response_mode = responses_envelope
        .raw_body()
        .get("stream")
        .and_then(Value::as_bool)
        .filter(|value| *value)
        .map(|_| "streaming".to_string());
    let model = responses_envelope
        .raw_body()
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let authentication_binding_id = match (endpoint, &delivery, response_mode.as_deref()) {
        (_, OpenAiResponseDelivery::TypedEvents, _) => {
            compatibility_interface::OPENAI_RESPONSES_WEBSOCKET_STREAM_BINDING_ID
        }
        (OpenAiResponsesEndpoint::ResponsesCompact, _, _) => {
            compatibility_interface::OPENAI_RESPONSES_COMPACT_BINDING_ID
        }
        (OpenAiResponsesEndpoint::Responses, _, Some("streaming"))
            if route_path == "/responses" =>
        {
            compatibility_interface::OPENAI_RESPONSES_ROOT_STREAM_BINDING_ID
        }
        (OpenAiResponsesEndpoint::Responses, _, Some("streaming")) => {
            compatibility_interface::OPENAI_RESPONSES_STREAM_BINDING_ID
        }
        (OpenAiResponsesEndpoint::Responses, _, _) if route_path == "/responses" => {
            compatibility_interface::OPENAI_RESPONSES_ROOT_BINDING_ID
        }
        (OpenAiResponsesEndpoint::Responses, _, _) => {
            compatibility_interface::OPENAI_RESPONSES_BINDING_ID
        }
    };
    let principal = match preauthenticated_principal {
        Some(principal) => principal.into(),
        None => {
            let token = credential
                .as_ref()
                .expect("HTTP credential was extracted")
                .token
                .clone();
            compatibility_interface::authenticate_application_principal(
                state.clone(),
                authentication_binding_id,
                token,
            )
            .await?
        }
    };
    let application_actor = compatibility_interface::application_actor(principal.principal());
    let request_context = compact::responses_request_context(
        &headers,
        endpoint,
        responses_envelope.raw_body(),
        delivery,
    )?;
    let operation_decision =
        classify_response_envelope_operation(&responses_envelope, request_context)
            .map_err(OpenAiRouteError::from)?;
    let is_local_summary = operation_decision.is_local_summary();
    let previous_response = load_previous_response_context_for_actor(
        state.clone(),
        application_actor.clone(),
        previous_response_id.as_deref(),
    )
    .await?;
    let previous_flow_run_id = previous_response
        .as_ref()
        .map(|previous| previous.flow_run_id);
    let previous_session_identity = previous_response
        .as_ref()
        .and_then(|previous| previous.session_identity.as_deref())
        .map(str::to_owned);
    let previous_translation_context = previous_response.map(|previous| previous.translation);
    let mut inference_recovery = None;
    if endpoint == OpenAiResponsesEndpoint::Responses && !is_local_summary {
        let encoded_resume = correlate_openai_responses_callback(
            responses_envelope.raw_body(),
            previous_response_id.as_deref(),
        )
        .map_err(|error| openai_invalid_request(error.param, error.message))?;
        let semantic_resume = if encoded_resume.is_none() {
            control_plane::application_public_api::native_tool_resume::correlate_semantic_responses_callback(&state.store, &application_actor, &responses_envelope)
                .await.map_err(native::service_error)?
        } else {
            None
        };
        let native_resume = if encoded_resume.is_none() && semantic_resume.is_none() {
            control_plane::application_public_api::native_tool_resume::correlate_native_responses_callback_from_envelope(
                &state.store, &application_actor, &responses_envelope,
            ).await.map_err(native::service_error)?
        } else {
            None
        };
        let correlated_from_native_state = native_resume.is_some();
        let native_transport = responses_callback_provider_transport(
            &responses_envelope,
            correlated_from_native_state,
            encoded_resume.is_some(),
        )?;
        let resume = encoded_resume
            .or_else(|| {
                semantic_resume.as_ref().map(|grant| {
                    super::super::callback_adapter::CorrelatedToolCallback {
                        callback_task_id: grant.callback_task_id(),
                        tool_results: grant.payload()["tool_results"].clone(),
                    }
                })
            })
            .or_else(|| {
                native_resume.map(|(task, tool_results)| {
                    super::super::callback_adapter::CorrelatedToolCallback {
                        callback_task_id: task.id,
                        tool_results: tool_results["tool_results"].clone(),
                    }
                })
            });
        if let Some(resume) = resume {
            let mut command = openai_resume_command(
                "",
                resume.callback_task_id,
                PublishedCallbackResumeSource::OpenAiResponses,
                resume.tool_results,
                response_mode.clone(),
            );
            if let Some(grant) = semantic_resume {
                command.response_payload = grant.payload().clone();
                command.responses_continuation = Some(grant);
            }
            let projection_mode =
                compat_sse::ResponsesProjectionMode::from_transport(native_transport.as_ref());
            command.native_transport = native_transport;
            command.transport_connection_scope = transport_connection_scope.clone();
            command.observation_context = recorder.as_ref().map(|recorder| {
                control_plane_contracts::ports::WorkflowObservationContext {
                    client_request_id: recorder.capture_id(),
                    context_flow_run_id: previous_flow_run_id,
                    context_response_id: previous_response_id.clone(),
                    is_resume: true,
                }
            });
            match compat_sse::prepare_compatible_resume_for_actor(
                state.clone(),
                application_actor.clone(),
                command,
            )
            .await
            {
                Ok(compat_sse::CompatibleResumeAdmission::Resume(mut plan)) => {
                    if responses_resume_requires_previous_response_match(
                        correlated_from_native_state
                            || plan.command.responses_continuation.is_some(),
                    ) {
                        if let Err(error) =
                            ensure_openai_responses_resume_matches_previous_response(
                                state.as_ref(),
                                previous_response_id.as_deref(),
                                resume.callback_task_id,
                            )
                            .await
                        {
                            if let Some(reservation) = plan.reservation.as_mut() {
                                reservation.release().await;
                            }
                            return Err(error);
                        }
                    }
                    if response_mode.as_deref() == Some("streaming") {
                        let input = compatibility_interface::CompatibilityBlockingInput {
                            command:
                                compatibility_interface::CompatibilityInvocationCommand::Resume {
                                    initial_run: plan.initial_run,
                                    command: plan.command,
                                    reservation: plan.reservation,
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
                            command:
                                compatibility_interface::CompatibilityInvocationCommand::Resume {
                                    initial_run: plan.initial_run,
                                    command: plan.command,
                                    reservation: plan.reservation,
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
                    return Ok(OpenAiResponseDispatch::Http(Json(response).into_response()));
                }
                Ok(compat_sse::CompatibleResumeAdmission::StartNewTurnFromHistory { recovery }) => {
                    inference_recovery = recovery.map(|grant| *grant);
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
    let translated = match translate_response_envelope_with_decision_and_previous(
        responses_envelope,
        operation_decision,
        previous_translation_context,
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
    if let Some(grant) = inference_recovery {
        request.metadata.set_inference_recovery(grant);
    }
    request.client_protocol_envelope = openai_protocol_context_from_ingress(
        ClientProtocolIngressPolicy::OpenAiResponses,
        raw_query.as_deref(),
        &headers,
        request.client_protocol_envelope,
    );
    if endpoint == OpenAiResponsesEndpoint::Responses {
        session_context::bind_responses_session_context(
            &mut request,
            principal.principal(),
            &headers,
            previous_session_identity.as_deref(),
        )?;
    }
    // Keep socket ownership outside the canonical envelope and its durable/ephemeral
    // protocol-context storage. It is applied only at the host Provider boundary.
    request
        .metadata
        .set_transport_connection_scope(transport_connection_scope);
    request
        .metadata
        .set_observation_context(recorder.as_ref().map(|recorder| {
            control_plane_contracts::ports::WorkflowObservationContext {
                client_request_id: recorder.capture_id(),
                context_flow_run_id: previous_flow_run_id,
                context_response_id: previous_response_id.clone(),
                is_resume: false,
            }
        }));
    let operation = *request.execution.execution_operation();
    let mut provider_transport_payload = request.metadata.take_provider_transport_payload();
    if let Some(previous_flow_run_id) = previous_flow_run_id {
        if let Some(payload) = provider_transport_payload.take() {
            let round_id = previous_response_id
                .as_deref()
                .and_then(|id| run_id_from_response_id(id).ok())
                .ok_or_else(|| {
                    openai_invalid_request(
                        "previous_response_id",
                        "previous response must identify a response round",
                    )
                })?;
            let continuation_slot =
                ProviderContinuationSlotId::for_response_round(previous_flow_run_id, round_id);
            let store = state.infrastructure.provider_transport_store();
            let continuation_exists = store
                .get_continuation(continuation_slot)
                .await
                .map_err(|_| {
                    OpenAiRouteError::Native(native::NativeApiError::new(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "provider_continuation_lookup_failed",
                        "Provider continuation storage is temporarily unavailable",
                    ))
                })?
                .is_some();
            if !continuation_exists {
                return Err(OpenAiRouteError::Native(native::NativeApiError::new(
                    StatusCode::CONFLICT,
                    "ephemeral_continuation_missing",
                    "the previous Provider continuation is no longer available",
                )));
            }
            let continuation = store
                .consume_continuation(continuation_slot)
                .await
                .map_err(|_| {
                    OpenAiRouteError::Native(native::NativeApiError::new(
                        StatusCode::CONFLICT,
                        "provider_continuation_claim_failed",
                        "the previous Provider continuation could not be claimed",
                    ))
                })?;
            let payload = payload
                .bind_openai_continuation(continuation)
                .map_err(|_| {
                    OpenAiRouteError::Native(native::NativeApiError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "provider_continuation_binding_failed",
                        "could not bind the Provider continuation",
                    ))
                })?;
            request.metadata.set_provider_transport_payload(payload);
            provider_transport_payload = request.metadata.take_provider_transport_payload();
        }
    }
    let model = request.model.clone().unwrap_or_default();
    let response_mode = request.response_mode.clone();
    let projection_mode =
        compat_sse::ResponsesProjectionMode::from_transport(provider_transport_payload.as_ref());
    let blocking_binding_id = match endpoint {
        OpenAiResponsesEndpoint::ResponsesCompact => {
            compatibility_interface::OPENAI_RESPONSES_COMPACT_BINDING_ID
        }
        OpenAiResponsesEndpoint::Responses if route_path == "/responses" => {
            compatibility_interface::OPENAI_RESPONSES_ROOT_BINDING_ID
        }
        OpenAiResponsesEndpoint::Responses => compatibility_interface::OPENAI_RESPONSES_BINDING_ID,
    };
    match operation {
        AiNativeOperation::Generate(_) => {
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
            Ok(OpenAiResponseDispatch::TypedEvents(Box::new(
                PreparedOpenAiResponseTurn {
                    model,
                    previous_response_id,
                    runtime,
                    projection_mode,
                },
            )))
        }
        AiNativeOperation::CountTokens => Err(openai_invalid_request(
            "operation",
            "count_tokens is not supported by the OpenAI Responses route",
        )),
        AiNativeOperation::Compact(remote_profile) => {
            if matches!(delivery, OpenAiResponseDelivery::TypedEvents) {
                return Err(openai_invalid_request(
                    "operation",
                    "Compact is a unary operation and cannot open a typed event turn",
                ));
            }
            let profile = match remote_profile {
                AiNativeCompactProfile::ResponsesCompact => {
                    ProviderCompactProfile::ResponsesCompact
                }
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
                            .map(OpenAiResponseDispatch::Http)
                            .map_err(Into::into);
                    }
                    Ok(OpenAiResponseDispatch::Http(Json(response).into_response()))
                }
                _ => Err(compact::unexpected_compact_result_error()),
            }
        }
    }
}
