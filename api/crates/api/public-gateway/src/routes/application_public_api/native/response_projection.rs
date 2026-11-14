use super::*;

pub(crate) fn native_error(
    error: impl Into<control_plane::application_public_api::native::NativeRunError>,
) -> NativeApiError {
    use control_plane::application_public_api::native::NativeRunError;
    let error = match error.into() {
        NativeRunError::Validation(error) => error,
        NativeRunError::Service(error) => return service_error(error),
    };
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
        NativeRunValidationError::UnknownModel => NativeApiError::new(
            StatusCode::BAD_REQUEST,
            "unknown_model",
            "model is not declared by the active publication",
        ),
        NativeRunValidationError::UnsupportedModelParameters(_) => NativeApiError::new(
            StatusCode::BAD_REQUEST,
            "unsupported_model_parameters",
            "model parameters are not supported by the published model",
        ),
        NativeRunValidationError::InvalidPublishedModelConfiguration => NativeApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "invalid_published_model_configuration",
            "published model reasoning configuration is invalid",
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
    if let Some(control_plane::errors::ControlPlaneError::UpstreamUnavailable(name)) =
        error.downcast_ref::<control_plane::errors::ControlPlaneError>()
    {
        return NativeApiError::new(StatusCode::SERVICE_UNAVAILABLE, name, error.to_string());
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
        "unexpected tool result for ",
        "duplicate tool result for ",
        "missing tool result for ",
    ]
    .iter()
    .any(|prefix| message.starts_with(prefix))
}

pub(crate) fn blocking_run_projection_error(run: &NativeRunResult) -> NativeApiError {
    match run.status {
        NativeRunStatus::Failed => {
            let code = if run
                .error
                .as_ref()
                .and_then(|error| error.upstream_error())
                .is_some()
            {
                "provider_upstream_error"
            } else {
                match run
                    .error
                    .as_ref()
                    .map(|error| error.code.as_str())
                    .unwrap_or("runtime_error")
                {
                    "auth_failed" => "auth_failed",
                    "endpoint_unreachable" => "endpoint_unreachable",
                    "model_not_found" => "model_not_found",
                    "provider_affinity_mismatch" => "provider_affinity_mismatch",
                    "provider_transport_unavailable" => "provider_transport_unavailable",
                    "rate_limited" => "rate_limited",
                    "provider_upstream_error" => "provider_upstream_error",
                    "provider_invalid_response" => "provider_invalid_response",
                    _ => "runtime_error",
                }
            };
            let status = run
                .error
                .as_ref()
                .and_then(|error| error.details.get("status_code"))
                .and_then(Value::as_u64)
                .and_then(|status| u16::try_from(status).ok())
                .and_then(|status| StatusCode::from_u16(status).ok())
                .filter(|status| status.is_client_error() || status.is_server_error())
                .unwrap_or(match code {
                    "rate_limited" => StatusCode::TOO_MANY_REQUESTS,
                    "provider_affinity_mismatch" => StatusCode::CONFLICT,
                    "provider_transport_unavailable" => StatusCode::SERVICE_UNAVAILABLE,
                    "endpoint_unreachable"
                    | "provider_upstream_error"
                    | "provider_invalid_response" => StatusCode::BAD_GATEWAY,
                    _ => StatusCode::INTERNAL_SERVER_ERROR,
                });
            let message = run
                .error
                .as_ref()
                .map(|error| error.message.as_str())
                .unwrap_or("published run failed");
            let mut error = NativeApiError::new(status, code, message);
            error.runtime_error = run
                .error
                .as_ref()
                .filter(|runtime| {
                    runtime.upstream_error().is_some() || runtime.code == "provider_upstream_error"
                })
                .cloned()
                .map(Box::new);
            error
        }
        NativeRunStatus::Cancelled => NativeApiError::new(
            StatusCode::CONFLICT,
            "run_cancelled",
            "published run cancelled",
        ),
        NativeRunStatus::Waiting => NativeApiError::new(
            StatusCode::CONFLICT,
            "required_action_not_supported",
            "waiting states are not supported by compatible endpoints; use the Native API to inspect and resume required_action runs",
        ),
        NativeRunStatus::Created | NativeRunStatus::Queued | NativeRunStatus::Running => {
            NativeApiError::new(
                StatusCode::CONFLICT,
                "run_not_terminal",
                "blocking run did not reach a terminal state",
            )
        }
        NativeRunStatus::Succeeded | NativeRunStatus::Incomplete => NativeApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "projection_error",
            "terminal run cannot be projected as an error",
        ),
    }
}
