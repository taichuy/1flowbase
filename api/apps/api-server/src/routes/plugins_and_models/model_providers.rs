use std::{collections::HashSet, sync::Arc};

use access_control::ConsoleRouteOwnership::ConsoleOperation;
use axum::{
    extract::{Path, Query, State},
    http::{header::ACCEPT_LANGUAGE, HeaderMap, StatusCode},
    Json, Router,
};
use control_plane::billing::PricingRule;
use control_plane::model_provider::{
    AuthenticateModelProviderInstanceCommand, AuthenticateModelProviderInstanceResult,
    ClearModelProviderRequestLogsBatchCommand, ClearModelProviderRequestLogsContinuation,
    ConsumeModelProviderResetCreditCommand, ConsumeModelProviderResetCreditResult,
    CreateModelProviderInstanceCommand, DeleteModelProviderInstanceCommand,
    DeleteSelectedModelProviderRequestLogsCommand, ListModelProviderRequestLogsCommand,
    LocalizedProviderModelDescriptor, ModelProviderBalanceResult, ModelProviderCatalogEntry,
    ModelProviderCatalogView, ModelProviderInstanceView, ModelProviderMainInstanceView,
    ModelProviderModelCatalog, ModelProviderOptionEntry, ModelProviderOptionsView,
    ModelProviderResetCreditCount, ModelProviderService, ModelProviderUsageWindowsResult,
    PreviewModelProviderModelsCommand, UpdateModelProviderInstanceCommand,
    UpdateModelProviderMainInstanceCommand, ValidateModelProviderResult,
};
use plugin_framework::{
    provider_contract::{
        PluginFormCondition, PluginFormFieldSchema, PluginFormOption, PluginFormSchema,
        ProviderAuthOperation, ProviderAuthStatus, ProviderAuthUserActionKind,
        ProviderModelDescriptor, ProviderOperationalCapability,
    },
    provider_package::ProviderConfigField,
};
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    provider_runtime::ApiProviderRuntime,
    response::ApiSuccess,
    routes::{
        console_route_assembly::{console_get, ConsoleRouteAssembly},
        system::LocaleMetaResponse,
    },
};

pub(crate) mod catalog_logs_interface;
mod clear_request_log_continuation;
pub(crate) mod discovery_interface;
mod dto;
pub(crate) mod icons;
pub(crate) mod instance_lifecycle_interface;
pub(crate) mod instance_operations_interface;
pub(crate) mod routing_interface;
pub(crate) mod settings_routes;

pub use dto::*;

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    ConsoleRouteAssembly::new()
        .route(
            "/model-providers/providers/:provider_code/icon",
            console_get(
                icons::read_provider_icon,
                ConsoleOperation("model_providers.icons.view".to_string()),
            ),
        )
        .route(
            "/model-providers/options",
            console_get(
                list_options,
                ConsoleOperation("model_providers.options.view".to_string()),
            ),
        )
        .route(
            "/model-providers/:id/balance",
            console_get(
                get_balance,
                ConsoleOperation("model_providers.balance.view".to_string()),
            ),
        )
        .merge(settings_routes::route_assembly())
}

fn format_time(value: time::OffsetDateTime) -> Result<String, ApiError> {
    Ok(value.format(&Rfc3339)?)
}

fn format_optional_time(value: Option<time::OffsetDateTime>) -> Result<Option<String>, ApiError> {
    value.map(format_time).transpose()
}

fn parse_rfc3339_time(raw: &str) -> Result<time::OffsetDateTime, ApiError> {
    time::OffsetDateTime::parse(raw, &Rfc3339).map_err(|_| {
        control_plane::errors::ControlPlaneError::InvalidInput("request_log_time").into()
    })
}

fn parse_uuid(raw: &str, field: &'static str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput(field).into())
}

fn parse_distribution_rule(
    raw: &str,
    rule_version: Option<String>,
    contract_version: Option<String>,
    config: std::collections::BTreeMap<String, ModelProviderDistributionConfigValueBody>,
) -> Result<domain::ModelProviderDistributionRule, ApiError> {
    match raw {
        "none" | "builtin.none" => Ok(domain::ModelProviderDistributionRule::None),
        "round_robin" | "builtin.round_robin" => {
            Ok(domain::ModelProviderDistributionRule::RoundRobin)
        }
        "retry_round_robin" | "builtin.retry_round_robin" => {
            Ok(domain::ModelProviderDistributionRule::RetryRoundRobin)
        }
        _ if valid_distribution_rule_id(raw) => {
            Ok(domain::ModelProviderDistributionRule::Dynamic {
                rule_id: raw.to_string(),
                rule_version: rule_version
                    .filter(|value| !value.trim().is_empty())
                    .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                        "distribution_rule_version",
                    ))?,
                contract_version: contract_version
                    .filter(|value| !value.trim().is_empty())
                    .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                        "distribution_rule_contract_version",
                    ))?,
                config: config
                    .into_iter()
                    .map(|(key, value)| {
                        let value = match value {
                            ModelProviderDistributionConfigValueBody::String(value) => {
                                domain::ModelProviderDistributionConfigValue::String(value)
                            }
                            ModelProviderDistributionConfigValueBody::Integer(value) => {
                                domain::ModelProviderDistributionConfigValue::Integer(value)
                            }
                            ModelProviderDistributionConfigValueBody::Boolean(value) => {
                                domain::ModelProviderDistributionConfigValue::Boolean(value)
                            }
                        };
                        (key, value)
                    })
                    .collect(),
            })
        }
        _ => {
            Err(control_plane::errors::ControlPlaneError::InvalidInput("distribution_rule").into())
        }
    }
}

fn valid_distribution_rule_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '-' | '.' | '@' | '/')
        })
}

fn normalize_provider_icon(provider_code: &str, icon: Option<String>) -> Option<String> {
    let icon = icon?.trim().to_string();

    if icon.is_empty() {
        return None;
    }

    if icon.starts_with("http://")
        || icon.starts_with("https://")
        || icon.starts_with('/')
        || icon.starts_with("data:")
    {
        return Some(icon);
    }

    Some(format!(
        "/api/console/model-providers/providers/{provider_code}/icon"
    ))
}

fn requested_locales(locale_meta: &LocaleMetaResponse) -> control_plane::i18n::RequestedLocales {
    control_plane::i18n::RequestedLocales::new(
        locale_meta.resolved_locale.clone(),
        locale_meta.fallback_locale.clone(),
    )
}

#[utoipa::path(
    get,
    path = "/api/console/settings/model-providers/catalog",
    operation_id = "model_provider_list_catalog",
    params(ModelProviderCatalogQuery),
    responses((status = 200, body = ModelProviderCatalogResponse), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<ModelProviderCatalogQuery>,
) -> Result<Json<ApiSuccess<ModelProviderCatalogResponse>>, ApiError> {
    let locale = catalog_logs_interface::ProviderLocaleHints::from_headers(&headers);
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-providers.catalog.view.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        catalog_logs_interface::ProviderCatalogLogsInput::Catalog { query, locale },
    )
    .await?;
    let catalog_logs_interface::ProviderCatalogLogsOutput::Catalog(response) = output else {
        unreachable!("provider catalog binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/model-providers/request-logs",
    params(ModelProviderRequestLogsQuery),
    responses((status = 200, body = ModelProviderRequestLogsPageResponse), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_request_logs(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<ModelProviderRequestLogsQuery>,
) -> Result<Json<ApiSuccess<ModelProviderRequestLogsPageResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-providers.request-logs.view.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        catalog_logs_interface::ProviderCatalogLogsInput::ListLogs(query),
    )
    .await?;
    let catalog_logs_interface::ProviderCatalogLogsOutput::Logs(response) = output else {
        unreachable!("provider request logs binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    delete,
    path = "/api/console/settings/model-providers/request-logs",
    operation_id = "model_provider_delete_selected_request_logs",
    request_body = DeleteModelProviderRequestLogsBody,
    responses(
        (status = 200, body = DeleteModelProviderRequestLogsResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn delete_selected_request_logs(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<DeleteModelProviderRequestLogsBody>,
) -> Result<Json<ApiSuccess<DeleteModelProviderRequestLogsResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-providers.request-logs.delete.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        catalog_logs_interface::ProviderCatalogLogsInput::DeleteLogs(body),
    )
    .await?;
    let catalog_logs_interface::ProviderCatalogLogsOutput::Deleted(response) = output else {
        unreachable!("provider request logs delete binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/model-providers/request-logs/clear",
    operation_id = "model_provider_clear_request_logs_batch",
    request_body = ClearModelProviderRequestLogsBody,
    responses(
        (status = 200, body = ClearModelProviderRequestLogsResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn clear_request_logs_batch(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiSuccess<ClearModelProviderRequestLogsResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-providers.request-logs.clear.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        catalog_logs_interface::ProviderCatalogLogsInput::ClearLogs(body),
    )
    .await?;
    let catalog_logs_interface::ProviderCatalogLogsOutput::Cleared(response) = output else {
        unreachable!("provider request logs clear binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

fn to_request_log_response(
    record: control_plane::ports::ModelProviderRequestLogRecord,
) -> Result<ModelProviderRequestLogResponse, ApiError> {
    Ok(ModelProviderRequestLogResponse {
        attempt_id: record.attempt_id.to_string(),
        flow_run_id: record.flow_run_id.to_string(),
        node_run_id: record.node_run_id.map(|id| id.to_string()),
        user_id: record.user_id.map(|id| id.to_string()),
        user_account: record.user_account,
        application_id: record.application_id.map(|id| id.to_string()),
        conversation_id: record.conversation_id,
        application_name: record.application_name,
        attempt_index: record.attempt_index,
        is_retry: record.is_retry,
        retry_reason: record.retry_reason,
        provider_instance_id: record.provider_instance_id.map(|id| id.to_string()),
        provider_instance_display_name: record.provider_instance_display_name,
        provider_code: record.provider_code,
        plugin_id: record.plugin_id,
        protocol: record.protocol,
        upstream_model_id: record.upstream_model_id,
        pricing_provider_code: record.pricing_provider_code,
        pricing_model_id: record.pricing_model_id,
        total_cost: record.total_cost,
        currency_code: record.currency_code,
        billing_status: record.billing_status,
        reasoning_effort: record.reasoning_effort,
        status: record.status,
        error_code: record.error_code,
        failed_after_first_token: record.failed_after_first_token,
        input_tokens: record.input_tokens,
        output_tokens: record.output_tokens,
        total_tokens: record.total_tokens,
        input_cache_hit_tokens: record.input_cache_hit_tokens,
        cache_write_tokens: record.cache_write_tokens,
        input_cache_hit_rate: record.input_cache_hit_rate,
        started_at: format_time(record.started_at)?,
        first_token_at: format_optional_time(record.first_token_at)?,
        finished_at: format_optional_time(record.finished_at)?,
        time_to_first_token_ms: record.time_to_first_token_ms,
        total_duration_ms: record.total_duration_ms,
    })
}

#[utoipa::path(
    get,
    path = "/api/console/settings/model-providers/instances",
    operation_id = "model_provider_list_instances",
    responses((status = 200, body = [ModelProviderInstanceResponse]), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_instances(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<ModelProviderInstanceResponse>>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-providers.instances.view.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        instance_lifecycle_interface::ProviderInstanceLifecycleInput::List,
    )
    .await?;
    let instance_lifecycle_interface::ProviderInstanceLifecycleOutput::Instances(instances) =
        output
    else {
        unreachable!("provider instances binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(instances)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/model-providers/instances",
    operation_id = "model_provider_create_instance",
    request_body = CreateModelProviderBody,
    responses((status = 201, body = ModelProviderInstanceResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn create_instance(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateModelProviderBody>,
) -> Result<(StatusCode, Json<ApiSuccess<ModelProviderInstanceResponse>>), ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-providers.instances.create.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        instance_lifecycle_interface::ProviderInstanceLifecycleInput::Create(body),
    )
    .await?;
    let instance_lifecycle_interface::ProviderInstanceLifecycleOutput::Instance(instance) = output
    else {
        unreachable!("provider instance create binding returned a different output")
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(instance))))
}

#[utoipa::path(
    patch,
    path = "/api/console/settings/model-providers/instances/{id}",
    operation_id = "model_provider_update_instance",
    request_body = UpdateModelProviderBody,
    responses((status = 200, body = ModelProviderInstanceResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn update_instance(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<UpdateModelProviderBody>,
) -> Result<Json<ApiSuccess<ModelProviderInstanceResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-providers.instances.update.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        instance_lifecycle_interface::ProviderInstanceLifecycleInput::Update { id, body },
    )
    .await?;
    let instance_lifecycle_interface::ProviderInstanceLifecycleOutput::Instance(instance) = output
    else {
        unreachable!("provider instance update binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(instance)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/model-providers/instances/{id}/validate",
    operation_id = "model_provider_validate_instance",
    responses((status = 200, body = ValidateModelProviderResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn validate_instance(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<ValidateModelProviderResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-providers.instances.validate.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        instance_lifecycle_interface::ProviderInstanceLifecycleInput::Validate { id },
    )
    .await?;
    let instance_lifecycle_interface::ProviderInstanceLifecycleOutput::Validation(result) = output
    else {
        unreachable!("provider instance validate binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(result)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/model-providers/instances/{id}/authenticate",
    operation_id = "model_provider_authenticate_instance",
    request_body = AuthenticateModelProviderInstanceBody,
    responses((status = 200, body = AuthenticateModelProviderInstanceResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn authenticate_instance(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<AuthenticateModelProviderInstanceBody>,
) -> Result<Json<ApiSuccess<AuthenticateModelProviderInstanceResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-providers.instances.authenticate.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        instance_operations_interface::ProviderInstanceOperationsInput::Authenticate { id, body },
    )
    .await?;
    let instance_operations_interface::ProviderInstanceOperationsOutput::Authentication(response) =
        output
    else {
        unreachable!("provider authenticate binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/model-providers/instances/{id}/usage",
    operation_id = "model_provider_get_usage_windows",
    responses((status = 200, body = ModelProviderUsageWindowsResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn get_usage_windows(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<ModelProviderUsageWindowsResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-providers.instances.usage.view.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        instance_operations_interface::ProviderInstanceOperationsInput::Usage { id },
    )
    .await?;
    let instance_operations_interface::ProviderInstanceOperationsOutput::Usage(response) = output
    else {
        unreachable!("provider usage binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/model-providers/instances/{id}/reset-credits",
    operation_id = "model_provider_count_reset_credits",
    responses((status = 200, body = ModelProviderResetCreditCountResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn count_reset_credits(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<ModelProviderResetCreditCountResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-providers.instances.reset-credits.view.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        instance_operations_interface::ProviderInstanceOperationsInput::ResetCredits { id },
    )
    .await?;
    let instance_operations_interface::ProviderInstanceOperationsOutput::ResetCredits(response) =
        output
    else {
        unreachable!("provider reset credits binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/model-providers/instances/{id}/reset-credits/consume",
    operation_id = "model_provider_consume_reset_credit",
    request_body = ConsumeModelProviderResetCreditBody,
    responses((status = 200, body = ConsumeModelProviderResetCreditResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn consume_reset_credit(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<ConsumeModelProviderResetCreditBody>,
) -> Result<Json<ApiSuccess<ConsumeModelProviderResetCreditResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-providers.instances.reset-credits.consume.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        instance_operations_interface::ProviderInstanceOperationsInput::ConsumeResetCredit {
            id,
            body,
        },
    )
    .await?;
    let instance_operations_interface::ProviderInstanceOperationsOutput::Consumed(response) =
        output
    else {
        unreachable!("provider reset credit consume binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/model-providers/{id}/balance",
    operation_id = "model_provider_get_balance",
    responses((status = 200, body = ModelProviderBalanceResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn get_balance(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<ModelProviderBalanceResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-providers.balance.view.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        instance_operations_interface::ProviderInstanceOperationsInput::Balance { id },
    )
    .await?;
    let instance_operations_interface::ProviderInstanceOperationsOutput::Balance(response) = output
    else {
        unreachable!("provider balance binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/model-providers/providers/{provider_code}/main-instance",
    operation_id = "model_provider_get_main_instance",
    responses(
        (status = 200, body = ModelProviderMainInstanceResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_main_instance(
    State(state): State<Arc<ApiState>>,
    Path(provider_code): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<ModelProviderMainInstanceResponse>>, ApiError> {
    let output: routing_interface::ProviderRoutingOutput =
        crate::routes::console_interface::invoke(
            Arc::clone(&state),
            "http.console.model-providers.main-instance.view.v1",
            crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
            routing_interface::ProviderRoutingInput::Get { provider_code },
        )
        .await?;
    Ok(Json(ApiSuccess::new(output.0)))
}

#[utoipa::path(
    put,
    path = "/api/console/settings/model-providers/providers/{provider_code}/main-instance",
    operation_id = "model_provider_update_main_instance",
    request_body = UpdateModelProviderMainInstanceBody,
    responses(
        (status = 200, body = ModelProviderMainInstanceResponse),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody),
        (status = 409, body = crate::error_response::ErrorBody)
    )
)]
pub async fn update_main_instance(
    State(state): State<Arc<ApiState>>,
    Path(provider_code): Path<String>,
    headers: HeaderMap,
    Json(body): Json<UpdateModelProviderMainInstanceBody>,
) -> Result<Json<ApiSuccess<ModelProviderMainInstanceResponse>>, ApiError> {
    let output: routing_interface::ProviderRoutingOutput =
        crate::routes::console_interface::invoke(
            Arc::clone(&state),
            "http.console.model-providers.main-instance.update.v1",
            crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
                state,
                headers,
            },
            routing_interface::ProviderRoutingInput::Update {
                provider_code,
                body,
            },
        )
        .await?;
    Ok(Json(ApiSuccess::new(output.0)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/model-providers/preview-models",
    operation_id = "model_provider_preview_models",
    request_body = PreviewModelProviderModelsBody,
    responses((status = 200, body = PreviewModelProviderModelsResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn preview_models(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<PreviewModelProviderModelsBody>,
) -> Result<Json<ApiSuccess<PreviewModelProviderModelsResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-providers.preview.view.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        instance_operations_interface::ProviderInstanceOperationsInput::Preview(body),
    )
    .await?;
    let instance_operations_interface::ProviderInstanceOperationsOutput::Preview(response) = output
    else {
        unreachable!("provider preview binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/model-providers/instances/{id}/secrets/reveal",
    operation_id = "model_provider_reveal_secret",
    request_body = RevealModelProviderSecretBody,
    responses((status = 200, body = RevealModelProviderSecretResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn reveal_secret(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<RevealModelProviderSecretBody>,
) -> Result<Json<ApiSuccess<RevealModelProviderSecretResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-providers.instances.secrets.reveal.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        instance_operations_interface::ProviderInstanceOperationsInput::Reveal { id, body },
    )
    .await?;
    let instance_operations_interface::ProviderInstanceOperationsOutput::Secret(response) = output
    else {
        unreachable!("provider secret reveal binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/model-providers/instances/{id}/models",
    operation_id = "model_provider_list_models",
    responses((status = 200, body = ModelProviderModelCatalogResponse), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_models(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<ModelProviderModelCatalogResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-providers.instances.models.view.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        discovery_interface::ProviderDiscoveryInput::Models { id },
    )
    .await?;
    let discovery_interface::ProviderDiscoveryOutput::Models(response) = output else {
        unreachable!("provider models binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/model-providers/instances/{id}/models/refresh",
    operation_id = "model_provider_refresh_models",
    responses((status = 200, body = ModelProviderModelCatalogResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn refresh_models(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<ModelProviderModelCatalogResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-providers.instances.models.refresh.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        discovery_interface::ProviderDiscoveryInput::RefreshModels { id },
    )
    .await?;
    let discovery_interface::ProviderDiscoveryOutput::Models(response) = output else {
        unreachable!("provider models refresh binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    delete,
    path = "/api/console/settings/model-providers/instances/{id}",
    operation_id = "model_provider_delete_instance",
    responses((status = 200, body = DeletedResponse), (status = 409, body = crate::error_response::ErrorBody))
)]
pub async fn delete_instance(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<DeletedResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-providers.instances.delete.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        instance_lifecycle_interface::ProviderInstanceLifecycleInput::Delete { id },
    )
    .await?;
    let instance_lifecycle_interface::ProviderInstanceLifecycleOutput::Deleted(response) = output
    else {
        unreachable!("provider instance delete binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/model-providers/options",
    operation_id = "model_provider_list_options",
    params(ModelProviderCatalogQuery),
    responses((status = 200, body = ModelProviderOptionsResponse), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_options(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<ModelProviderCatalogQuery>,
) -> Result<Json<ApiSuccess<ModelProviderOptionsResponse>>, ApiError> {
    let locale = catalog_logs_interface::ProviderLocaleHints::from_headers(&headers);
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-providers.options.view.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        discovery_interface::ProviderDiscoveryInput::Options {
            query,
            locale,
            settings: false,
        },
    )
    .await?;
    let discovery_interface::ProviderDiscoveryOutput::Options(response) = output else {
        unreachable!("provider options binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[cfg(test)]
mod request_log_response_tests {
    use super::*;
    use time::{macros::datetime, Duration};

    #[test]
    fn request_log_response_uses_persisted_duration_fields() {
        let started_at = datetime!(2026-07-11 10:55:28 UTC);
        let node_run_id = Uuid::now_v7();
        let response =
            to_request_log_response(control_plane::ports::ModelProviderRequestLogRecord {
                attempt_id: Uuid::now_v7(),
                flow_run_id: Uuid::now_v7(),
                node_run_id: Some(node_run_id),
                user_id: Some(Uuid::now_v7()),
                user_account: Some("root".into()),
                application_id: Some(Uuid::now_v7()),
                conversation_id: Some("conversation-1".into()),
                application_name: "凡人".into(),
                attempt_index: 1,
                is_retry: false,
                retry_reason: None,
                provider_instance_id: None,
                provider_instance_display_name: Some("Gemini02".into()),
                provider_code: "gemini".into(),
                plugin_id: Some("gemini@0.1.20".into()),
                protocol: "google_genai".into(),
                upstream_model_id: "gemini-3-flash".into(),
                pricing_provider_code: Some("zero".into()),
                pricing_model_id: Some("any".into()),
                total_cost: Some("0".into()),
                currency_code: Some("USD".into()),
                billing_status: Some("settled".into()),
                reasoning_effort: None,
                status: "succeeded".into(),
                error_code: None,
                failed_after_first_token: false,
                input_tokens: Some(3),
                output_tokens: Some(191),
                total_tokens: Some(194),
                input_cache_hit_tokens: Some(97),
                cache_write_tokens: Some(5000),
                input_cache_hit_rate: Some(0.5),
                started_at,
                first_token_at: Some(started_at - Duration::milliseconds(6076)),
                finished_at: Some(started_at),
                time_to_first_token_ms: Some(3350),
                total_duration_ms: Some(7426),
            })
            .expect("representable request log timestamps");

        assert_eq!(response.node_run_id, Some(node_run_id.to_string()));
        assert_eq!(response.time_to_first_token_ms, Some(3350));
        assert_eq!(response.total_duration_ms, Some(7426));
        assert_eq!(response.plugin_id.as_deref(), Some("gemini@0.1.20"));
        assert_eq!(response.input_cache_hit_tokens, Some(97));
        assert_eq!(response.cache_write_tokens, Some(5000));
        assert_eq!(response.input_cache_hit_rate, Some(0.5));
        assert_eq!(response.pricing_provider_code.as_deref(), Some("zero"));
        assert_eq!(response.pricing_model_id.as_deref(), Some("any"));
        assert_eq!(response.total_cost.as_deref(), Some("0"));
        assert_eq!(response.currency_code.as_deref(), Some("USD"));
    }
}

#[cfg(test)]
#[path = "model_providers/_tests/time_response.rs"]
mod time_response_tests;

#[path = "model_providers/response_mapping.rs"]
mod response_mapping;

use response_mapping::*;
