use axum::http::StatusCode;
use control_plane::application_public_api::run_service::PublishedCountTokensError;
use plugin_framework::provider_contract::{ProviderCountTokensError, ProviderRuntimeError};

use super::*;

pub(super) fn anthropic_usage(
    usage: Option<control_plane::application_public_api::native::NativeUsage>,
) -> AnthropicUsage {
    let Some(usage) = usage else {
        return AnthropicUsage::default();
    };
    AnthropicUsage {
        input_tokens: usage.prompt_tokens.unwrap_or_default(),
        cache_creation_input_tokens: usage.cache_write_tokens.unwrap_or_default(),
        cache_read_input_tokens: usage
            .cache_read_tokens
            .or(usage.input_cache_hit_tokens)
            .unwrap_or_default(),
        output_tokens: usage.completion_tokens.unwrap_or_default(),
    }
}

pub(super) fn to_anthropic_count_tokens_response(
    input_tokens: u64,
) -> AnthropicCountTokensResponse {
    AnthropicCountTokensResponse { input_tokens }
}

pub(super) fn anthropic_count_tokens_error(
    error: PublishedCountTokensError,
) -> AnthropicRouteError {
    let error = match error {
        PublishedCountTokensError::NotAuthenticated => native::NativeApiError::new(
            StatusCode::UNAUTHORIZED,
            "not_authenticated",
            "invalid application API key",
        ),
        PublishedCountTokensError::ApplicationNotPublished => native::NativeApiError::new(
            StatusCode::CONFLICT,
            "application_not_published",
            "application has no active published public API version",
        ),
        PublishedCountTokensError::RouteUnavailable(error) => {
            native::native_error(control_plane::application_public_api::native::NativeRunValidationError::RouteUnavailable(error))
        }
        PublishedCountTokensError::InvalidRequest => native::NativeApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "translated CountTokens request is invalid",
        ),
        PublishedCountTokensError::ProviderTargetUnavailable => native::NativeApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "provider_count_tokens_unavailable",
            "published CountTokens provider target is unavailable",
        ),
        PublishedCountTokensError::Provider(error) => provider_count_tokens_error(error),
    };
    AnthropicRouteError::Native(error)
}

fn provider_count_tokens_error(error: ProviderCountTokensError) -> native::NativeApiError {
    match error {
        ProviderCountTokensError::Unsupported { capabilities } => native::NativeApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "provider_count_tokens_unsupported",
            format!(
                "provider does not declare required CountTokens capabilities: {}",
                capabilities.join(", ")
            ),
        ),
        ProviderCountTokensError::InvalidContract { message } => native::NativeApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "provider_count_tokens_contract_invalid",
            message,
        ),
        ProviderCountTokensError::Runtime { error } => provider_runtime_count_tokens_error(error),
    }
}

fn provider_runtime_count_tokens_error(error: ProviderRuntimeError) -> native::NativeApiError {
    let (status, code) = match error.kind {
        plugin_framework::provider_contract::ProviderRuntimeErrorKind::AuthFailed => {
            (StatusCode::BAD_GATEWAY, "provider_auth_failed")
        }
        plugin_framework::provider_contract::ProviderRuntimeErrorKind::EndpointUnreachable => {
            (StatusCode::BAD_GATEWAY, "provider_endpoint_unreachable")
        }
        plugin_framework::provider_contract::ProviderRuntimeErrorKind::ModelNotFound => {
            (StatusCode::UNPROCESSABLE_ENTITY, "provider_model_not_found")
        }
        plugin_framework::provider_contract::ProviderRuntimeErrorKind::RateLimited => {
            (StatusCode::TOO_MANY_REQUESTS, "provider_rate_limited")
        }
        plugin_framework::provider_contract::ProviderRuntimeErrorKind::ProviderUpstreamError => {
            (StatusCode::BAD_GATEWAY, "provider_upstream_error")
        }
        plugin_framework::provider_contract::ProviderRuntimeErrorKind::ProviderInvalidResponse => {
            (StatusCode::BAD_GATEWAY, "provider_invalid_response")
        }
    };
    native::NativeApiError::new(status, code, error.message)
}
