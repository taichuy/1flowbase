use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use control_plane::application_public_api::operation_bindings::{
    ApplicationOperationBindingOperation, ApplicationOperationBindingProjection,
    ApplicationOperationBindingProjectionService, ApplicationOperationBindingTargetOption,
    ApplicationOperationBindingUnsupportedReason, ApplicationPublishedOperationBindingProjection,
    ApplicationPublishedOperationBindingSupport, GetApplicationOperationBindingProjectionCommand,
};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState, error_response::ApiError, middleware::require_session::require_session,
    response::ApiSuccess,
};

use super::{to_operation_bindings_body, ApplicationOperationBindingsBody};

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationOperationBindingProjectionResponse {
    pub editable: bool,
    #[schema(inline)]
    pub draft: ApplicationDraftOperationBindingProjectionResponse,
    #[schema(inline)]
    pub published: Option<ApplicationPublishedOperationBindingsProjectionResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationDraftOperationBindingProjectionResponse {
    #[schema(inline)]
    pub operation_bindings: ApplicationOperationBindingsBody,
    #[schema(inline)]
    pub options: Vec<ApplicationOperationBindingOptionsResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationOperationBindingOptionsResponse {
    pub operation: ApplicationOperationBindingOperationResponse,
    #[schema(inline)]
    pub targets: Vec<ApplicationOperationBindingTargetOptionResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationOperationBindingTargetOptionResponse {
    pub target_node_id: String,
    pub node_alias: String,
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
pub enum ApplicationOperationBindingOperationResponse {
    #[serde(rename = "generate")]
    Generate,
    #[serde(rename = "count_tokens")]
    CountTokens,
    #[serde(rename = "compact.responses_compact")]
    CompactResponsesCompact,
    #[serde(rename = "compact.responses_compaction_v2")]
    CompactResponsesCompactionV2,
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationPublishedOperationBindingStatusResponse {
    Supported,
    Unbound,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationOperationBindingUnsupportedReasonResponse {
    CompiledPlanMissing,
    CompiledPlanMismatch,
    CompiledPlanInvalid,
    TargetMissing,
    TargetNotLlm,
    TargetRuntimeIncomplete,
    ProviderTargetUnavailable,
    ProviderContractUnsupported,
    ProviderManifestUnavailable,
    ProviderCapabilityUnsupported,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationPublishedOperationBindingProjectionResponse {
    pub operation: ApplicationOperationBindingOperationResponse,
    pub target_node_id: Option<String>,
    pub status: ApplicationPublishedOperationBindingStatusResponse,
    #[schema(inline)]
    pub target: Option<ApplicationOperationBindingTargetOptionResponse>,
    pub unsupported_reason: Option<ApplicationOperationBindingUnsupportedReasonResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationPublishedOperationBindingsProjectionResponse {
    pub publication_id: Uuid,
    pub compiled_plan_id: Uuid,
    #[schema(inline)]
    pub bindings: Vec<ApplicationPublishedOperationBindingProjectionResponse>,
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{application_id}/api-operation-bindings",
    params(("application_id" = Uuid, Path, description = "Application id")),
    responses(
        (status = 200, body = ApplicationOperationBindingProjectionResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_application_operation_bindings(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
) -> Result<Json<ApiSuccess<ApplicationOperationBindingProjectionResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let projection = ApplicationOperationBindingProjectionService::new(state.store.clone())
        .get_projection(GetApplicationOperationBindingProjectionCommand {
            actor_user_id: context.user.id,
            application_id,
        })
        .await?;

    Ok(Json(ApiSuccess::new(
        to_operation_binding_projection_response(projection),
    )))
}

fn to_operation_binding_projection_response(
    projection: ApplicationOperationBindingProjection,
) -> ApplicationOperationBindingProjectionResponse {
    ApplicationOperationBindingProjectionResponse {
        editable: projection.editable,
        draft: ApplicationDraftOperationBindingProjectionResponse {
            operation_bindings: to_operation_bindings_body(projection.draft.operation_bindings),
            options: projection
                .draft
                .options
                .into_iter()
                .map(|option| ApplicationOperationBindingOptionsResponse {
                    operation: to_operation_binding_operation_response(option.operation),
                    targets: option
                        .targets
                        .into_iter()
                        .map(to_operation_binding_target_option_response)
                        .collect(),
                })
                .collect(),
        },
        published: projection.published.map(|published| {
            ApplicationPublishedOperationBindingsProjectionResponse {
                publication_id: published.publication_id,
                compiled_plan_id: published.compiled_plan_id,
                bindings: published
                    .bindings
                    .into_iter()
                    .map(to_published_operation_binding_projection_response)
                    .collect(),
            }
        }),
    }
}

fn to_published_operation_binding_projection_response(
    binding: ApplicationPublishedOperationBindingProjection,
) -> ApplicationPublishedOperationBindingProjectionResponse {
    let (status, target, unsupported_reason) = match binding.support {
        ApplicationPublishedOperationBindingSupport::Supported { target } => (
            ApplicationPublishedOperationBindingStatusResponse::Supported,
            Some(to_operation_binding_target_option_response(target)),
            None,
        ),
        ApplicationPublishedOperationBindingSupport::Unbound => (
            ApplicationPublishedOperationBindingStatusResponse::Unbound,
            None,
            None,
        ),
        ApplicationPublishedOperationBindingSupport::Unsupported { target, reason } => (
            ApplicationPublishedOperationBindingStatusResponse::Unsupported,
            target.map(to_operation_binding_target_option_response),
            Some(to_operation_binding_unsupported_reason_response(reason)),
        ),
    };

    ApplicationPublishedOperationBindingProjectionResponse {
        operation: to_operation_binding_operation_response(binding.operation),
        target_node_id: binding.target_node_id,
        status,
        target,
        unsupported_reason,
    }
}

fn to_operation_binding_target_option_response(
    target: ApplicationOperationBindingTargetOption,
) -> ApplicationOperationBindingTargetOptionResponse {
    ApplicationOperationBindingTargetOptionResponse {
        target_node_id: target.target_node_id,
        node_alias: target.node_alias,
    }
}

fn to_operation_binding_operation_response(
    operation: ApplicationOperationBindingOperation,
) -> ApplicationOperationBindingOperationResponse {
    match operation {
        ApplicationOperationBindingOperation::Generate => {
            ApplicationOperationBindingOperationResponse::Generate
        }
        ApplicationOperationBindingOperation::CountTokens => {
            ApplicationOperationBindingOperationResponse::CountTokens
        }
        ApplicationOperationBindingOperation::CompactResponsesCompact => {
            ApplicationOperationBindingOperationResponse::CompactResponsesCompact
        }
        ApplicationOperationBindingOperation::CompactResponsesCompactionV2 => {
            ApplicationOperationBindingOperationResponse::CompactResponsesCompactionV2
        }
    }
}

fn to_operation_binding_unsupported_reason_response(
    reason: ApplicationOperationBindingUnsupportedReason,
) -> ApplicationOperationBindingUnsupportedReasonResponse {
    match reason {
        ApplicationOperationBindingUnsupportedReason::CompiledPlanMissing => {
            ApplicationOperationBindingUnsupportedReasonResponse::CompiledPlanMissing
        }
        ApplicationOperationBindingUnsupportedReason::CompiledPlanMismatch => {
            ApplicationOperationBindingUnsupportedReasonResponse::CompiledPlanMismatch
        }
        ApplicationOperationBindingUnsupportedReason::CompiledPlanInvalid => {
            ApplicationOperationBindingUnsupportedReasonResponse::CompiledPlanInvalid
        }
        ApplicationOperationBindingUnsupportedReason::TargetMissing => {
            ApplicationOperationBindingUnsupportedReasonResponse::TargetMissing
        }
        ApplicationOperationBindingUnsupportedReason::TargetNotLlm => {
            ApplicationOperationBindingUnsupportedReasonResponse::TargetNotLlm
        }
        ApplicationOperationBindingUnsupportedReason::TargetRuntimeIncomplete => {
            ApplicationOperationBindingUnsupportedReasonResponse::TargetRuntimeIncomplete
        }
        ApplicationOperationBindingUnsupportedReason::ProviderTargetUnavailable => {
            ApplicationOperationBindingUnsupportedReasonResponse::ProviderTargetUnavailable
        }
        ApplicationOperationBindingUnsupportedReason::ProviderContractUnsupported => {
            ApplicationOperationBindingUnsupportedReasonResponse::ProviderContractUnsupported
        }
        ApplicationOperationBindingUnsupportedReason::ProviderManifestUnavailable => {
            ApplicationOperationBindingUnsupportedReasonResponse::ProviderManifestUnavailable
        }
        ApplicationOperationBindingUnsupportedReason::ProviderCapabilityUnsupported => {
            ApplicationOperationBindingUnsupportedReasonResponse::ProviderCapabilityUnsupported
        }
    }
}
