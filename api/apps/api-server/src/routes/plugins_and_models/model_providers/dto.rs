use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::routes::system::LocaleMetaResponse;

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
    pub expected_revision: i64,
    pub model_routing_policies: Option<Vec<ModelProviderMainModelRoutingPolicyBody>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ModelProviderMainModelRoutingPolicyBody {
    pub model_id: String,
    pub distribution_rule: String,
    pub provider_instance_ids: Vec<Uuid>,
    #[serde(default)]
    pub excluded_provider_instance_ids: Vec<Uuid>,
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
    #[schema(value_type = Value)]
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
    #[schema(value_type = Value)]
    pub value: serde_json::Value,
    pub description: Option<String>,
    pub disabled: Option<bool>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PluginFormConditionResponse {
    pub field: String,
    pub operator: String,
    #[schema(value_type = Value)]
    pub value: Option<serde_json::Value>,
    #[schema(value_type = [Value])]
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
    #[schema(value_type = Value)]
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
    pub revision: i64,
    pub model_routing_policies: Vec<ModelProviderMainModelRoutingPolicyResponse>,
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
pub struct ModelProviderMainModelRoutingPolicyResponse {
    pub model_id: String,
    pub distribution_rule: String,
    pub provider_instance_ids: Vec<Uuid>,
    pub excluded_provider_instance_ids: Vec<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ModelProviderOptionTargetResponse {
    pub source_instance_id: String,
    pub source_instance_display_name: String,
    pub routing_enabled: bool,
    pub model: ProviderModelDescriptorResponse,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ModelProviderRequestLogsQuery {
    pub flow_run_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
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
    pub node_run_id: Option<String>,
    pub user_id: Option<String>,
    pub user_account: Option<String>,
    pub application_id: Option<String>,
    pub conversation_id: Option<String>,
    pub application_name: String,
    pub attempt_index: i32,
    pub is_retry: bool,
    pub retry_reason: Option<String>,
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

#[derive(Debug, Deserialize, ToSchema)]
pub struct DeleteModelProviderRequestLogsBody {
    pub attempt_ids: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DeleteModelProviderRequestLogsResponse {
    pub deleted_count: u64,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ClearModelProviderRequestLogsBody {
    pub continuation_token: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ClearModelProviderRequestLogsResponse {
    pub deleted_count: u64,
    pub has_more: bool,
    pub continuation_token: String,
}
