use axum::http::{HeaderMap, StatusCode};
use control_plane::application_public_api::{
    compat::openai::{OpenAiCompatError, OpenAiResponsesEndpoint, OpenAiResponsesRequestContext},
    protocol_translation::{
        TranslationDecisionKind, TranslationProtocol, TranslationReport,
        TranslationSafeRepresentation,
    },
    run_service::PublishedCompactError,
};
use plugin_framework::provider_contract::{
    ProviderCompactError, ProviderRuntimeError, ProviderRuntimeErrorKind,
};
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

use super::{OpenAiResponsesObject, OpenAiResponsesUsage, OpenAiRouteError};
use crate::routes::application_public_api::native;

const CODEX_TURN_METADATA_HEADER: &str = "x-codex-turn-metadata";

pub(super) fn has_codex_turn_metadata(headers: &HeaderMap) -> bool {
    headers.contains_key(CODEX_TURN_METADATA_HEADER)
}

/// Captures the trusted Codex turn header after the route has authenticated
/// the application key. Its value stays in the translation context only; it
/// is never copied into the Native request or a persisted run payload.
pub(super) fn responses_request_context(
    headers: &HeaderMap,
    endpoint: OpenAiResponsesEndpoint,
) -> Result<OpenAiResponsesRequestContext, OpenAiRouteError> {
    let context = OpenAiResponsesRequestContext::new(endpoint);
    let mut values = headers.get_all(CODEX_TURN_METADATA_HEADER).iter();
    let Some(value) = values.next() else {
        return Ok(context);
    };
    if values.next().is_some() {
        return Err(invalid_codex_turn_metadata(
            "x-codex-turn-metadata must appear at most once",
        ));
    }
    let value = value.to_str().map_err(|_| {
        invalid_codex_turn_metadata("x-codex-turn-metadata must be valid header text")
    })?;
    let metadata = serde_json::from_str::<Value>(value).map_err(|_| {
        invalid_codex_turn_metadata("x-codex-turn-metadata must contain valid JSON")
    })?;
    Ok(context.with_captured_codex_turn_metadata(metadata))
}

pub(super) fn published_compact_error(error: PublishedCompactError) -> OpenAiRouteError {
    let error = match error {
        PublishedCompactError::NotAuthenticated => native::NativeApiError::new(
            StatusCode::UNAUTHORIZED,
            "not_authenticated",
            "invalid application API key",
        ),
        PublishedCompactError::ApplicationNotPublished => native::NativeApiError::new(
            StatusCode::CONFLICT,
            "application_not_published",
            "application has no active published public API version",
        ),
        PublishedCompactError::RouteUnavailable(error) => native::native_error(
            control_plane::application_public_api::native::NativeRunValidationError::RouteUnavailable(
                error,
            ),
        ),
        PublishedCompactError::InvalidRequest => native::NativeApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "translated Compact request is invalid",
        ),
        PublishedCompactError::ProviderTargetUnavailable => native::NativeApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "provider_compact_unavailable",
            "published Compact provider target is unavailable",
        ),
        PublishedCompactError::Provider(error) => provider_compact_error(error),
    };
    OpenAiRouteError::Native(error)
}

/// The V2 item was already validated by the provider contract. Keep the
/// provider-produced JSON intact so the opaque encrypted content is neither
/// decoded nor synthesized by the HTTP adapter.
pub(super) fn completed_v2_compaction_response(
    model: String,
    response_id: Option<String>,
    compaction_item: Value,
) -> OpenAiResponsesObject {
    OpenAiResponsesObject {
        id: response_id.unwrap_or_else(|| format!("resp_{}", Uuid::now_v7())),
        object: "response",
        created_at: OffsetDateTime::now_utc().unix_timestamp(),
        status: "completed",
        model,
        output: vec![compaction_item],
        output_text: String::new(),
        usage: OpenAiResponsesUsage::default(),
        incomplete_details: None,
        previous_response_id: None,
    }
}

pub(super) fn unexpected_compact_result_error() -> OpenAiRouteError {
    OpenAiRouteError::Native(native::NativeApiError::new(
        StatusCode::BAD_GATEWAY,
        "provider_compact_contract_invalid",
        "provider Compact result did not satisfy the requested profile",
    ))
}

fn invalid_codex_turn_metadata(message: &'static str) -> OpenAiRouteError {
    let mut report = TranslationReport::new(TranslationProtocol::OpenAiResponses);
    report.record(
        "$.ingress.x-codex-turn-metadata",
        None,
        TranslationDecisionKind::Rejected,
        Some(message),
        TranslationSafeRepresentation::Present,
    );
    OpenAiRouteError::Compat(Box::new(OpenAiCompatError {
        message: message.to_string(),
        error_type: "invalid_request_error".to_string(),
        param: Some(CODEX_TURN_METADATA_HEADER.to_string()),
        code: "invalid_request".to_string(),
        report,
    }))
}

fn provider_compact_error(error: ProviderCompactError) -> native::NativeApiError {
    match error {
        ProviderCompactError::Unsupported { capabilities, .. } => native::NativeApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "provider_compact_unsupported",
            format!(
                "provider does not declare required Compact capabilities: {}",
                capabilities.join(", ")
            ),
        ),
        ProviderCompactError::InvalidContract { message } => native::NativeApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "provider_compact_contract_invalid",
            message,
        ),
        ProviderCompactError::Runtime { error } => provider_runtime_compact_error(error),
    }
}

fn provider_runtime_compact_error(error: ProviderRuntimeError) -> native::NativeApiError {
    let (status, code) = match error.kind {
        ProviderRuntimeErrorKind::AuthFailed => (StatusCode::BAD_GATEWAY, "provider_auth_failed"),
        ProviderRuntimeErrorKind::EndpointUnreachable => {
            (StatusCode::BAD_GATEWAY, "provider_endpoint_unreachable")
        }
        ProviderRuntimeErrorKind::ModelNotFound => {
            (StatusCode::UNPROCESSABLE_ENTITY, "provider_model_not_found")
        }
        ProviderRuntimeErrorKind::RateLimited => {
            (StatusCode::TOO_MANY_REQUESTS, "provider_rate_limited")
        }
        ProviderRuntimeErrorKind::ProviderUpstreamError => {
            (StatusCode::BAD_GATEWAY, "provider_upstream_error")
        }
        ProviderRuntimeErrorKind::ProviderInvalidResponse => {
            (StatusCode::BAD_GATEWAY, "provider_invalid_response")
        }
    };
    native::NativeApiError::new(status, code, error.message)
}
