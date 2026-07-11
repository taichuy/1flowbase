use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{header::ACCEPT_LANGUAGE, HeaderMap, StatusCode},
    routing::{get, patch, post},
    Json, Router,
};
use control_plane::model_provider::{
    CreateModelProviderInstanceCommand, DeleteModelProviderInstanceCommand,
    LocalizedProviderModelDescriptor, ModelProviderBalanceResult, ModelProviderCatalogEntry,
    ModelProviderCatalogView, ModelProviderInstanceView, ModelProviderMainInstanceView,
    ModelProviderModelCatalog, ModelProviderOptionEntry, ModelProviderOptionsView,
    ModelProviderService, PreviewModelProviderModelsCommand, UpdateModelProviderInstanceCommand,
    UpdateModelProviderMainInstanceCommand, ValidateModelProviderResult,
};
use control_plane::ports::{ListModelProviderRequestLogsPageInput, OrchestrationRuntimeRepository};
use plugin_framework::{
    provider_contract::{
        PluginFormCondition, PluginFormFieldSchema, PluginFormOption, PluginFormSchema,
        ProviderModelDescriptor,
    },
    provider_package::ProviderConfigField,
};
use serde::{Deserialize, Serialize};
use storage_durable::MainDurableStore;
use time::format_description::well_known::Rfc3339;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    provider_runtime::ApiProviderRuntime,
    response::ApiSuccess,
    routes::system::LocaleMetaResponse,
};

mod icons;

#[derive(Debug, Deserialize, ToSchema)]
pub struct ConfiguredModelBody {
    pub model_id: String,
    pub enabled: bool,
    pub context_window_override_tokens: Option<u64>,
    pub supports_multimodal: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateModelProviderBody {
    pub installation_id: String,
    pub display_name: String,
    #[serde(default)]
    pub configured_models: Vec<ConfiguredModelBody>,
    #[serde(default)]
    pub enabled_model_ids: Vec<String>,
    #[serde(default)]
    pub included_in_main: Option<bool>,
    pub preview_token: Option<String>,
    #[schema(value_type = Object)]
    pub config: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateModelProviderBody {
    pub display_name: String,
    #[serde(default)]
    pub configured_models: Vec<ConfiguredModelBody>,
    #[serde(default)]
    pub enabled_model_ids: Vec<String>,
    pub included_in_main: bool,
    pub preview_token: Option<String>,
    #[schema(value_type = Object)]
    pub config: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateModelProviderMainInstanceBody {
    pub auto_include_new_instances: bool,
    pub model_distribution_rules: Option<Vec<ModelProviderMainModelDistributionRuleBody>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ModelProviderMainModelDistributionRuleBody {
    pub model_id: String,
    pub distribution_rule: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RevealModelProviderSecretBody {
    pub key: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PreviewModelProviderModelsBody {
    pub installation_id: Option<String>,
    pub instance_id: Option<String>,
    #[schema(value_type = Object)]
    pub config: serde_json::Value,
}

#[derive(Debug, Deserialize, IntoParams, Clone)]
pub struct ModelProviderCatalogQuery {
    pub locale: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ModelProviderConfigFieldResponse {
    pub key: String,
    pub field_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control: Option<String>,
    pub required: bool,
    pub advanced: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[schema(value_type = Object)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<PluginFormOptionResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProviderModelDescriptorResponse {
    pub model_id: String,
    pub display_name: String,
    pub namespace: Option<String>,
    pub label_key: Option<String>,
    pub description_key: Option<String>,
    pub display_name_fallback: Option<String>,
    pub source: String,
    pub supports_streaming: bool,
    pub supports_tool_call: bool,
    pub supports_multimodal: bool,
    pub context_window: Option<u64>,
    pub max_output_tokens: Option<u64>,
    #[schema(value_type = Object)]
    pub provider_metadata: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PluginFormOptionResponse {
    pub label: String,
    #[schema(value_type = Object)]
    pub value: serde_json::Value,
    pub description: Option<String>,
    pub disabled: Option<bool>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PluginFormConditionResponse {
    pub field: String,
    pub operator: String,
    #[schema(value_type = Object)]
    pub value: Option<serde_json::Value>,
    #[schema(value_type = [Object])]
    pub values: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PluginFormFieldSchemaResponse {
    pub key: String,
    pub label: String,
    #[serde(rename = "type")]
    pub field_type: String,
    pub control: Option<String>,
    pub group: Option<String>,
    pub order: Option<i32>,
    pub advanced: Option<bool>,
    pub required: Option<bool>,
    pub send_mode: Option<String>,
    pub enabled_by_default: Option<bool>,
    pub description: Option<String>,
    pub placeholder: Option<String>,
    #[schema(value_type = Object)]
    pub default_value: Option<serde_json::Value>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    pub precision: Option<u32>,
    pub unit: Option<String>,
    pub options: Vec<PluginFormOptionResponse>,
    pub visible_when: Vec<PluginFormConditionResponse>,
    pub disabled_when: Vec<PluginFormConditionResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PluginFormSchemaResponse {
    pub schema_version: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub fields: Vec<PluginFormFieldSchemaResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ModelProviderCatalogEntryResponse {
    pub installation_id: String,
    pub provider_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub plugin_type: String,
    pub namespace: String,
    pub label_key: String,
    pub description_key: Option<String>,
    pub display_name: String,
    pub protocol: String,
    pub help_url: Option<String>,
    pub default_base_url: Option<String>,
    pub model_discovery_mode: String,
    pub supports_model_fetch_without_credentials: bool,
    pub desired_state: String,
    pub availability_status: String,
    pub form_schema: Vec<ModelProviderConfigFieldResponse>,
    pub predefined_models: Vec<ProviderModelDescriptorResponse>,
    pub catalog_refresh_status: String,
    pub catalog_last_error_message: Option<String>,
    pub catalog_refreshed_at: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ModelProviderCatalogResponse {
    pub locale_meta: LocaleMetaResponse,
    #[schema(value_type = Object)]
    pub i18n_catalog: serde_json::Value,
    pub entries: Vec<ModelProviderCatalogEntryResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ConfiguredModelResponse {
    pub model_id: String,
    pub enabled: bool,
    pub context_window_override_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_multimodal: Option<bool>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ModelProviderInstanceResponse {
    pub id: String,
    pub installation_id: String,
    pub provider_code: String,
    pub protocol: String,
    pub display_name: String,
    pub status: String,
    pub included_in_main: bool,
    #[schema(value_type = Object)]
    pub config_json: serde_json::Value,
    pub configured_models: Vec<ConfiguredModelResponse>,
    pub enabled_model_ids: Vec<String>,
    pub catalog_refresh_status: Option<String>,
    pub catalog_last_error_message: Option<String>,
    pub catalog_refreshed_at: Option<String>,
    pub model_count: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ValidateModelProviderResponse {
    pub instance: ModelProviderInstanceResponse,
    #[schema(value_type = Object)]
    pub output: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ModelProviderBalanceInfoResponse {
    pub currency: String,
    pub total_balance: String,
    pub granted_balance: Option<String>,
    pub topped_up_balance: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ModelProviderBalanceResponse {
    pub is_available: bool,
    pub balance_infos: Vec<ModelProviderBalanceInfoResponse>,
    #[schema(value_type = Object)]
    pub provider_metadata: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ModelProviderModelCatalogResponse {
    pub provider_instance_id: String,
    pub refresh_status: String,
    pub source: String,
    pub last_error_message: Option<String>,
    pub refreshed_at: Option<String>,
    pub models: Vec<ProviderModelDescriptorResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PreviewModelProviderModelsResponse {
    pub models: Vec<ProviderModelDescriptorResponse>,
    pub preview_token: String,
    pub expires_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RevealModelProviderSecretResponse {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ModelProviderOptionResponse {
    pub provider_code: String,
    pub plugin_type: String,
    pub namespace: String,
    pub label_key: String,
    pub description_key: Option<String>,
    pub protocol: String,
    pub display_name: String,
    pub icon: Option<String>,
    pub parameter_form: Option<PluginFormSchemaResponse>,
    pub main_instance: ModelProviderMainInstanceSummaryResponse,
    pub model_groups: Vec<ModelProviderOptionGroupResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ModelProviderOptionsResponse {
    pub locale_meta: LocaleMetaResponse,
    #[schema(value_type = Object)]
    pub i18n_catalog: serde_json::Value,
    pub providers: Vec<ModelProviderOptionResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DeletedResponse {
    pub deleted: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ModelProviderMainInstanceResponse {
    pub provider_code: String,
    pub auto_include_new_instances: bool,
    pub model_distribution_rules: Vec<ModelProviderMainModelDistributionRuleResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ModelProviderMainInstanceSummaryResponse {
    pub provider_code: String,
    pub auto_include_new_instances: bool,
    pub group_count: usize,
    pub model_count: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ModelProviderOptionGroupResponse {
    pub model_id: String,
    pub distribution_rule: String,
    pub model: ProviderModelDescriptorResponse,
    pub targets: Vec<ModelProviderOptionTargetResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ModelProviderMainModelDistributionRuleResponse {
    pub model_id: String,
    pub distribution_rule: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ModelProviderOptionTargetResponse {
    pub source_instance_id: String,
    pub source_instance_display_name: String,
    pub model: ProviderModelDescriptorResponse,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ModelProviderRequestLogsQuery {
    pub application_name: Option<String>,
    pub provider_instance_id: Option<Uuid>,
    pub model_id: Option<String>,
    pub status: Option<String>,
    #[serde(default)]
    pub zero_output_only: bool,
    pub started_after: Option<String>,
    pub started_before: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ModelProviderRequestLogResponse {
    pub attempt_id: String,
    pub flow_run_id: String,
    pub application_name: String,
    pub attempt_index: i32,
    pub provider_instance_id: Option<String>,
    pub provider_instance_display_name: Option<String>,
    pub provider_code: String,
    pub protocol: String,
    pub upstream_model_id: String,
    pub reasoning_effort: Option<String>,
    pub status: String,
    pub error_code: Option<String>,
    pub failed_after_first_token: bool,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub started_at: String,
    pub first_token_at: Option<String>,
    pub finished_at: Option<String>,
    pub time_to_first_token_ms: Option<i64>,
    pub total_duration_ms: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ModelProviderRequestLogsPageResponse {
    pub items: Vec<ModelProviderRequestLogResponse>,
    pub total_count: i64,
    pub page: i64,
    pub page_size: i64,
}

pub fn router() -> Router<Arc<ApiState>> {
    Router::new()
        .route("/model-providers/catalog", get(list_catalog))
        .route("/model-providers/request-logs", get(list_request_logs))
        .route(
            "/model-providers",
            get(list_instances).post(create_instance),
        )
        .route(
            "/model-providers/providers/:provider_code/main-instance",
            get(get_main_instance).put(update_main_instance),
        )
        .route(
            "/model-providers/providers/:provider_code/icon",
            get(icons::read_provider_icon),
        )
        .route("/model-providers/preview-models", post(preview_models))
        .route("/model-providers/options", get(list_options))
        .route(
            "/model-providers/:id",
            patch(update_instance).delete(delete_instance),
        )
        .route("/model-providers/:id/validate", post(validate_instance))
        .route("/model-providers/:id/balance", get(get_balance))
        .route("/model-providers/:id/secrets/reveal", post(reveal_secret))
        .route("/model-providers/:id/models", get(list_models))
        .route("/model-providers/:id/models/refresh", post(refresh_models))
}

fn service(state: &ApiState) -> ModelProviderService<MainDurableStore, ApiProviderRuntime> {
    ModelProviderService::new(
        state.store.clone(),
        ApiProviderRuntime::new(state.provider_runtime.clone()),
        state.provider_secret_master_key.clone(),
    )
    .with_node_artifact_context(
        state.api_node_id.clone(),
        state.provider_install_root.clone(),
    )
}

fn format_time(value: time::OffsetDateTime) -> String {
    value.format(&Rfc3339).unwrap()
}

fn format_optional_time(value: Option<time::OffsetDateTime>) -> Option<String> {
    value.map(format_time)
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

fn parse_distribution_rule(raw: &str) -> Result<domain::ModelProviderDistributionRule, ApiError> {
    match raw {
        "none" => Ok(domain::ModelProviderDistributionRule::None),
        "round_robin" => Ok(domain::ModelProviderDistributionRule::RoundRobin),
        _ => {
            Err(control_plane::errors::ControlPlaneError::InvalidInput("distribution_rule").into())
        }
    }
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

fn to_config_field_response(field: ProviderConfigField) -> ModelProviderConfigFieldResponse {
    ModelProviderConfigFieldResponse {
        key: field.key,
        field_type: field.field_type,
        label: field.label,
        control: field.control,
        required: field.required,
        advanced: field.advanced,
        description: field.description,
        placeholder: field.placeholder,
        default_value: field.default_value,
        options: field
            .options
            .into_iter()
            .map(to_plugin_form_option_response)
            .collect(),
    }
}

fn to_model_descriptor_response(
    model: LocalizedProviderModelDescriptor,
) -> ProviderModelDescriptorResponse {
    let descriptor = model.descriptor;
    ProviderModelDescriptorResponse {
        model_id: descriptor.model_id,
        display_name: descriptor.display_name.clone(),
        namespace: model.namespace,
        label_key: model.label_key,
        description_key: model.description_key,
        display_name_fallback: model
            .display_name_fallback
            .or_else(|| Some(descriptor.display_name.clone())),
        source: format!("{:?}", descriptor.source).to_ascii_lowercase(),
        supports_streaming: descriptor.supports_streaming,
        supports_tool_call: descriptor.supports_tool_call,
        supports_multimodal: descriptor.supports_multimodal,
        context_window: descriptor.context_window,
        max_output_tokens: descriptor.max_output_tokens,
        provider_metadata: descriptor.provider_metadata,
    }
}

fn to_plugin_form_option_response(option: PluginFormOption) -> PluginFormOptionResponse {
    PluginFormOptionResponse {
        label: option.label,
        value: option.value,
        description: option.description,
        disabled: option.disabled,
    }
}

fn to_plugin_form_condition_response(
    condition: PluginFormCondition,
) -> PluginFormConditionResponse {
    PluginFormConditionResponse {
        field: condition.field,
        operator: condition.operator,
        value: condition.value,
        values: condition.values,
    }
}

fn to_plugin_form_field_schema_response(
    field: PluginFormFieldSchema,
) -> PluginFormFieldSchemaResponse {
    PluginFormFieldSchemaResponse {
        key: field.key,
        label: field.label,
        field_type: field.field_type,
        control: field.control,
        group: field.group,
        order: field.order,
        advanced: field.advanced,
        required: field.required,
        send_mode: field.send_mode,
        enabled_by_default: field.enabled_by_default,
        description: field.description,
        placeholder: field.placeholder,
        default_value: field.default_value,
        min: field.min,
        max: field.max,
        step: field.step,
        precision: field.precision,
        unit: field.unit,
        options: field
            .options
            .into_iter()
            .map(to_plugin_form_option_response)
            .collect(),
        visible_when: field
            .visible_when
            .into_iter()
            .map(to_plugin_form_condition_response)
            .collect(),
        disabled_when: field
            .disabled_when
            .into_iter()
            .map(to_plugin_form_condition_response)
            .collect(),
    }
}

fn to_plugin_form_schema_response(schema: PluginFormSchema) -> PluginFormSchemaResponse {
    PluginFormSchemaResponse {
        schema_version: schema.schema_version,
        title: schema.title,
        description: schema.description,
        fields: schema
            .fields
            .into_iter()
            .map(to_plugin_form_field_schema_response)
            .collect(),
    }
}

fn to_catalog_response(entry: ModelProviderCatalogEntry) -> ModelProviderCatalogEntryResponse {
    ModelProviderCatalogEntryResponse {
        installation_id: entry.installation_id.to_string(),
        provider_code: entry.provider_code,
        plugin_id: entry.plugin_id,
        plugin_version: entry.plugin_version,
        plugin_type: entry.plugin_type,
        namespace: entry.namespace,
        label_key: entry.label_key,
        description_key: entry.description_key,
        display_name: entry.display_name,
        protocol: entry.protocol,
        help_url: entry.help_url,
        default_base_url: entry.default_base_url,
        model_discovery_mode: entry.model_discovery_mode,
        supports_model_fetch_without_credentials: entry.supports_model_fetch_without_credentials,
        desired_state: entry.desired_state,
        availability_status: entry.availability_status,
        form_schema: entry
            .form_schema
            .into_iter()
            .map(to_config_field_response)
            .collect(),
        predefined_models: entry
            .predefined_models
            .into_iter()
            .map(to_model_descriptor_response)
            .collect(),
        catalog_refresh_status: entry.catalog_refresh_status,
        catalog_last_error_message: entry.catalog_last_error_message,
        catalog_refreshed_at: format_optional_time(entry.catalog_refreshed_at),
    }
}

fn to_catalog_view_response(
    locale_meta: LocaleMetaResponse,
    catalog: ModelProviderCatalogView,
) -> ModelProviderCatalogResponse {
    ModelProviderCatalogResponse {
        locale_meta,
        i18n_catalog: serde_json::to_value(catalog.i18n_catalog).unwrap(),
        entries: catalog
            .entries
            .into_iter()
            .map(to_catalog_response)
            .collect(),
    }
}

fn to_runtime_model_descriptor_response(
    model: ProviderModelDescriptor,
) -> ProviderModelDescriptorResponse {
    let display_name = model.display_name.clone();
    ProviderModelDescriptorResponse {
        model_id: model.model_id,
        display_name: display_name.clone(),
        namespace: None,
        label_key: None,
        description_key: None,
        display_name_fallback: Some(display_name),
        source: format!("{:?}", model.source).to_ascii_lowercase(),
        supports_streaming: model.supports_streaming,
        supports_tool_call: model.supports_tool_call,
        supports_multimodal: model.supports_multimodal,
        context_window: model.context_window,
        max_output_tokens: model.max_output_tokens,
        provider_metadata: model.provider_metadata,
    }
}

fn to_instance_response(view: ModelProviderInstanceView) -> ModelProviderInstanceResponse {
    let model_count = view
        .cache
        .as_ref()
        .and_then(|cache| cache.models_json.as_array().map(|items| items.len()))
        .unwrap_or(0);
    ModelProviderInstanceResponse {
        id: view.instance.id.to_string(),
        installation_id: view.instance.installation_id.to_string(),
        provider_code: view.instance.provider_code,
        protocol: view.instance.protocol,
        display_name: view.instance.display_name,
        status: view.instance.status.as_str().to_string(),
        included_in_main: view.instance.included_in_main,
        config_json: view.instance.config_json,
        configured_models: view
            .instance
            .configured_models
            .into_iter()
            .map(|model| ConfiguredModelResponse {
                model_id: model.model_id,
                enabled: model.enabled,
                context_window_override_tokens: model.context_window_override_tokens,
                supports_multimodal: model.supports_multimodal,
            })
            .collect(),
        enabled_model_ids: view.instance.enabled_model_ids,
        catalog_refresh_status: view
            .cache
            .as_ref()
            .map(|cache| cache.refresh_status.as_str().to_string()),
        catalog_last_error_message: view
            .cache
            .as_ref()
            .and_then(|cache| cache.last_error_message.clone()),
        catalog_refreshed_at: view
            .cache
            .as_ref()
            .and_then(|cache| format_optional_time(cache.refreshed_at)),
        model_count,
    }
}

fn to_validate_response(result: ValidateModelProviderResult) -> ValidateModelProviderResponse {
    ValidateModelProviderResponse {
        instance: to_instance_response(ModelProviderInstanceView {
            instance: result.instance,
            cache: Some(result.cache),
        }),
        output: result.output,
    }
}

fn to_balance_response(result: ModelProviderBalanceResult) -> ModelProviderBalanceResponse {
    ModelProviderBalanceResponse {
        is_available: result.is_available,
        balance_infos: result
            .balance_infos
            .into_iter()
            .map(|info| ModelProviderBalanceInfoResponse {
                currency: info.currency,
                total_balance: info.total_balance,
                granted_balance: info.granted_balance,
                topped_up_balance: info.topped_up_balance,
            })
            .collect(),
        provider_metadata: result.provider_metadata,
    }
}

fn to_main_instance_response(
    view: ModelProviderMainInstanceView,
) -> ModelProviderMainInstanceResponse {
    ModelProviderMainInstanceResponse {
        provider_code: view.provider_code,
        auto_include_new_instances: view.auto_include_new_instances,
        model_distribution_rules: view
            .model_distribution_rules
            .into_iter()
            .map(to_main_model_distribution_rule_response)
            .collect(),
    }
}

fn to_main_model_distribution_rule_response(
    rule: domain::ModelProviderMainModelDistributionRule,
) -> ModelProviderMainModelDistributionRuleResponse {
    ModelProviderMainModelDistributionRuleResponse {
        model_id: rule.model_id,
        distribution_rule: rule.distribution_rule.as_str().to_string(),
    }
}

fn to_main_model_distribution_rule(
    rule: ModelProviderMainModelDistributionRuleBody,
) -> Result<domain::ModelProviderMainModelDistributionRule, ApiError> {
    Ok(domain::ModelProviderMainModelDistributionRule {
        model_id: rule.model_id,
        distribution_rule: parse_distribution_rule(&rule.distribution_rule)?,
    })
}

fn to_model_catalog_response(
    catalog: ModelProviderModelCatalog,
) -> ModelProviderModelCatalogResponse {
    ModelProviderModelCatalogResponse {
        provider_instance_id: catalog.provider_instance_id.to_string(),
        refresh_status: catalog.refresh_status.as_str().to_string(),
        source: catalog.source.as_str().to_string(),
        last_error_message: catalog.last_error_message,
        refreshed_at: format_optional_time(catalog.refreshed_at),
        models: catalog
            .models
            .into_iter()
            .map(to_runtime_model_descriptor_response)
            .collect(),
    }
}

fn to_option_response(option: ModelProviderOptionEntry) -> ModelProviderOptionResponse {
    ModelProviderOptionResponse {
        icon: normalize_provider_icon(&option.provider_code, option.icon),
        provider_code: option.provider_code,
        plugin_type: option.plugin_type,
        namespace: option.namespace,
        label_key: option.label_key,
        description_key: option.description_key,
        protocol: option.protocol,
        display_name: option.display_name,
        parameter_form: option.parameter_form.map(to_plugin_form_schema_response),
        main_instance: ModelProviderMainInstanceSummaryResponse {
            provider_code: option.main_instance.provider_code,
            auto_include_new_instances: option.main_instance.auto_include_new_instances,
            group_count: option.main_instance.group_count,
            model_count: option.main_instance.model_count,
        },
        model_groups: option
            .model_groups
            .into_iter()
            .map(|group| ModelProviderOptionGroupResponse {
                model_id: group.model_id,
                distribution_rule: group.distribution_rule.as_str().to_string(),
                model: to_model_descriptor_response(group.model),
                targets: group
                    .targets
                    .into_iter()
                    .map(|target| ModelProviderOptionTargetResponse {
                        source_instance_id: target.source_instance_id.to_string(),
                        source_instance_display_name: target.source_instance_display_name,
                        model: to_model_descriptor_response(target.model),
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn to_options_view_response(
    locale_meta: LocaleMetaResponse,
    options: ModelProviderOptionsView,
) -> ModelProviderOptionsResponse {
    ModelProviderOptionsResponse {
        locale_meta,
        i18n_catalog: serde_json::to_value(options.i18n_catalog).unwrap(),
        providers: options
            .providers
            .into_iter()
            .map(to_option_response)
            .collect(),
    }
}

fn resolve_locale_meta(
    headers: &HeaderMap,
    query_locale: Option<String>,
    user_preferred_locale: Option<String>,
) -> LocaleMetaResponse {
    runtime_profile::resolve_locale(runtime_profile::LocaleResolutionInput {
        query_locale,
        explicit_header_locale: headers
            .get("x-1flowbase-locale")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
        user_preferred_locale,
        accept_language: headers
            .get(ACCEPT_LANGUAGE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
        fallback_locale: runtime_profile::FALLBACK_LOCALE,
        supported_locales: runtime_profile::SUPPORTED_LOCALES
            .iter()
            .map(|value| value.to_string())
            .collect(),
    })
    .into()
}

fn requested_locales(locale_meta: &LocaleMetaResponse) -> control_plane::i18n::RequestedLocales {
    control_plane::i18n::RequestedLocales::new(
        locale_meta.resolved_locale.clone(),
        locale_meta.fallback_locale.clone(),
    )
}

#[utoipa::path(
    get,
    path = "/api/console/model-providers/catalog",
    operation_id = "model_provider_list_catalog",
    params(ModelProviderCatalogQuery),
    responses((status = 200, body = ModelProviderCatalogResponse), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<ModelProviderCatalogQuery>,
) -> Result<Json<ApiSuccess<ModelProviderCatalogResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let locale_meta = resolve_locale_meta(&headers, query.locale, context.user.preferred_locale);
    let catalog = service(&state)
        .list_catalog(context.user.id, requested_locales(&locale_meta))
        .await?;
    Ok(Json(ApiSuccess::new(to_catalog_view_response(
        locale_meta,
        catalog,
    ))))
}

#[utoipa::path(
    get,
    path = "/api/console/model-providers/request-logs",
    params(ModelProviderRequestLogsQuery),
    responses((status = 200, body = ModelProviderRequestLogsPageResponse), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_request_logs(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<ModelProviderRequestLogsQuery>,
) -> Result<Json<ApiSuccess<ModelProviderRequestLogsPageResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let page = state
        .store
        .list_model_provider_request_logs_page(ListModelProviderRequestLogsPageInput {
            scope_id: context.actor.current_workspace_id,
            application_name: query.application_name,
            provider_instance_id: query.provider_instance_id,
            model_id: query.model_id,
            status: query.status,
            zero_output_only: query.zero_output_only,
            started_after: query
                .started_after
                .as_deref()
                .map(parse_rfc3339_time)
                .transpose()?,
            started_before: query
                .started_before
                .as_deref()
                .map(parse_rfc3339_time)
                .transpose()?,
            page: query.page.unwrap_or(1),
            page_size: query.page_size.unwrap_or(20),
        })
        .await?;
    Ok(Json(ApiSuccess::new(
        ModelProviderRequestLogsPageResponse {
            items: page
                .items
                .into_iter()
                .map(to_request_log_response)
                .collect(),
            total_count: page.total_count,
            page: page.page,
            page_size: page.page_size,
        },
    )))
}

fn to_request_log_response(
    record: control_plane::ports::ModelProviderRequestLogRecord,
) -> ModelProviderRequestLogResponse {
    ModelProviderRequestLogResponse {
        attempt_id: record.attempt_id.to_string(),
        flow_run_id: record.flow_run_id.to_string(),
        application_name: record.application_name,
        attempt_index: record.attempt_index,
        provider_instance_id: record.provider_instance_id.map(|id| id.to_string()),
        provider_instance_display_name: record.provider_instance_display_name,
        provider_code: record.provider_code,
        protocol: record.protocol,
        upstream_model_id: record.upstream_model_id,
        reasoning_effort: record.reasoning_effort,
        status: record.status,
        error_code: record.error_code,
        failed_after_first_token: record.failed_after_first_token,
        input_tokens: record.input_tokens,
        output_tokens: record.output_tokens,
        total_tokens: record.total_tokens,
        started_at: format_time(record.started_at),
        first_token_at: format_optional_time(record.first_token_at),
        finished_at: format_optional_time(record.finished_at),
        time_to_first_token_ms: record
            .first_token_at
            .map(|value| (value - record.started_at).whole_milliseconds() as i64),
        total_duration_ms: record
            .finished_at
            .map(|value| (value - record.started_at).whole_milliseconds() as i64),
    }
}

#[utoipa::path(
    get,
    path = "/api/console/model-providers",
    operation_id = "model_provider_list_instances",
    responses((status = 200, body = [ModelProviderInstanceResponse]), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_instances(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<ModelProviderInstanceResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let instances = service(&state).list_instances(context.user.id).await?;
    Ok(Json(ApiSuccess::new(
        instances.into_iter().map(to_instance_response).collect(),
    )))
}

#[utoipa::path(
    post,
    path = "/api/console/model-providers",
    operation_id = "model_provider_create_instance",
    request_body = CreateModelProviderBody,
    responses((status = 201, body = ModelProviderInstanceResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn create_instance(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateModelProviderBody>,
) -> Result<(StatusCode, Json<ApiSuccess<ModelProviderInstanceResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let created = service(&state)
        .create_instance(CreateModelProviderInstanceCommand {
            actor_user_id: context.user.id,
            installation_id: parse_uuid(&body.installation_id, "installation_id")?,
            display_name: body.display_name,
            config_json: body.config,
            configured_models: body
                .configured_models
                .into_iter()
                .map(|model| domain::ModelProviderConfiguredModel {
                    model_id: model.model_id,
                    enabled: model.enabled,
                    context_window_override_tokens: model.context_window_override_tokens,
                    supports_multimodal: model.supports_multimodal,
                })
                .collect(),
            enabled_model_ids: body.enabled_model_ids,
            included_in_main: body.included_in_main,
            preview_token: body
                .preview_token
                .as_deref()
                .map(|raw| parse_uuid(raw, "preview_token"))
                .transpose()?,
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_instance_response(created))),
    ))
}

#[utoipa::path(
    patch,
    path = "/api/console/model-providers/{id}",
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
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let updated = service(&state)
        .update_instance(UpdateModelProviderInstanceCommand {
            actor_user_id: context.user.id,
            instance_id: parse_uuid(&id, "id")?,
            display_name: body.display_name,
            config_json: body.config,
            configured_models: body
                .configured_models
                .into_iter()
                .map(|model| domain::ModelProviderConfiguredModel {
                    model_id: model.model_id,
                    enabled: model.enabled,
                    context_window_override_tokens: model.context_window_override_tokens,
                    supports_multimodal: model.supports_multimodal,
                })
                .collect(),
            enabled_model_ids: body.enabled_model_ids,
            included_in_main: body.included_in_main,
            preview_token: body
                .preview_token
                .as_deref()
                .map(|raw| parse_uuid(raw, "preview_token"))
                .transpose()?,
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_instance_response(updated))))
}

#[utoipa::path(
    post,
    path = "/api/console/model-providers/{id}/validate",
    operation_id = "model_provider_validate_instance",
    responses((status = 200, body = ValidateModelProviderResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn validate_instance(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<ValidateModelProviderResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let result = service(&state)
        .validate_instance(context.user.id, parse_uuid(&id, "id")?)
        .await?;
    Ok(Json(ApiSuccess::new(to_validate_response(result))))
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
    let context = require_session(&state, &headers).await?;
    let result = service(&state)
        .get_balance(context.user.id, parse_uuid(&id, "id")?)
        .await?;
    Ok(Json(ApiSuccess::new(to_balance_response(result))))
}

#[utoipa::path(
    get,
    path = "/api/console/model-providers/providers/{provider_code}/main-instance",
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
    let context = require_session(&state, &headers).await?;
    let view = service(&state)
        .get_main_instance(context.user.id, &provider_code)
        .await?;
    Ok(Json(ApiSuccess::new(to_main_instance_response(view))))
}

#[utoipa::path(
    put,
    path = "/api/console/model-providers/providers/{provider_code}/main-instance",
    operation_id = "model_provider_update_main_instance",
    request_body = UpdateModelProviderMainInstanceBody,
    responses(
        (status = 200, body = ModelProviderMainInstanceResponse),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn update_main_instance(
    State(state): State<Arc<ApiState>>,
    Path(provider_code): Path<String>,
    headers: HeaderMap,
    Json(body): Json<UpdateModelProviderMainInstanceBody>,
) -> Result<Json<ApiSuccess<ModelProviderMainInstanceResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let model_distribution_rules = body
        .model_distribution_rules
        .map(|rules| {
            rules
                .into_iter()
                .map(to_main_model_distribution_rule)
                .collect::<Result<Vec<_>, ApiError>>()
        })
        .transpose()?;
    let view = service(&state)
        .update_main_instance(UpdateModelProviderMainInstanceCommand {
            actor_user_id: context.user.id,
            provider_code,
            auto_include_new_instances: body.auto_include_new_instances,
            model_distribution_rules,
        })
        .await?;

    Ok(Json(ApiSuccess::new(to_main_instance_response(view))))
}

#[utoipa::path(
    post,
    path = "/api/console/model-providers/preview-models",
    operation_id = "model_provider_preview_models",
    request_body = PreviewModelProviderModelsBody,
    responses((status = 200, body = PreviewModelProviderModelsResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn preview_models(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<PreviewModelProviderModelsBody>,
) -> Result<Json<ApiSuccess<PreviewModelProviderModelsResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let preview = service(&state)
        .preview_models(PreviewModelProviderModelsCommand {
            actor_user_id: context.user.id,
            installation_id: body
                .installation_id
                .as_deref()
                .map(|raw| parse_uuid(raw, "installation_id"))
                .transpose()?,
            instance_id: body
                .instance_id
                .as_deref()
                .map(|raw| parse_uuid(raw, "instance_id"))
                .transpose()?,
            config_json: body.config,
        })
        .await?;
    Ok(Json(ApiSuccess::new(PreviewModelProviderModelsResponse {
        models: preview
            .models
            .into_iter()
            .map(to_runtime_model_descriptor_response)
            .collect(),
        preview_token: preview.preview_token.to_string(),
        expires_at: format_optional_time(Some(preview.expires_at)).unwrap_or_default(),
    })))
}

#[utoipa::path(
    post,
    path = "/api/console/model-providers/{id}/secrets/reveal",
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
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let value = service(&state)
        .reveal_secret(context.user.id, parse_uuid(&id, "id")?, &body.key)
        .await?;
    Ok(Json(ApiSuccess::new(RevealModelProviderSecretResponse {
        key: body.key,
        value,
    })))
}

#[utoipa::path(
    get,
    path = "/api/console/model-providers/{id}/models",
    operation_id = "model_provider_list_models",
    responses((status = 200, body = ModelProviderModelCatalogResponse), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_models(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<ModelProviderModelCatalogResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let catalog = service(&state)
        .list_models(context.user.id, parse_uuid(&id, "id")?)
        .await?;
    Ok(Json(ApiSuccess::new(to_model_catalog_response(catalog))))
}

#[utoipa::path(
    post,
    path = "/api/console/model-providers/{id}/models/refresh",
    operation_id = "model_provider_refresh_models",
    responses((status = 200, body = ModelProviderModelCatalogResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn refresh_models(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<ModelProviderModelCatalogResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let catalog = service(&state)
        .refresh_models(context.user.id, parse_uuid(&id, "id")?)
        .await?;
    Ok(Json(ApiSuccess::new(to_model_catalog_response(catalog))))
}

#[utoipa::path(
    delete,
    path = "/api/console/model-providers/{id}",
    operation_id = "model_provider_delete_instance",
    responses((status = 200, body = DeletedResponse), (status = 409, body = crate::error_response::ErrorBody))
)]
pub async fn delete_instance(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<DeletedResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    service(&state)
        .delete_instance(DeleteModelProviderInstanceCommand {
            actor_user_id: context.user.id,
            instance_id: parse_uuid(&id, "id")?,
        })
        .await?;
    Ok(Json(ApiSuccess::new(DeletedResponse { deleted: true })))
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
    let context = require_session(&state, &headers).await?;
    let locale_meta = resolve_locale_meta(&headers, query.locale, context.user.preferred_locale);
    let options = service(&state)
        .options(context.user.id, requested_locales(&locale_meta))
        .await?;
    Ok(Json(ApiSuccess::new(to_options_view_response(
        locale_meta,
        options,
    ))))
}
