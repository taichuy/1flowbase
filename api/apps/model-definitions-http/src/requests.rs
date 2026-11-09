use serde::{Deserialize, Deserializer, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CreateModelDefinitionBody {
    pub bound_storage_id: Option<Uuid>,
    pub scope_kind: String,
    pub template_provider: String,
    pub template_code: String,
    pub template_version: String,
    pub code: String,
    pub title: String,
    pub description: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateModelDefinitionBody {
    pub title: Option<String>,
    #[serde(default, deserialize_with = "deserialize_present_optional")]
    pub description: Option<Option<String>>,
    pub status: Option<String>,
    pub external_table_id: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema, Serialize)]
pub struct CreateModelFieldBody {
    pub code: String,
    pub title: String,
    pub description: Option<String>,
    pub external_field_key: Option<String>,
    pub field_kind: String,
    #[serde(default)]
    pub is_required: bool,
    pub api_required: Option<bool>,
    #[serde(default)]
    pub is_unique: bool,
    pub default_value: Option<serde_json::Value>,
    pub display_interface: Option<String>,
    #[serde(default = "empty_json_object")]
    pub display_options: serde_json::Value,
    pub relation_target_model_id: Option<String>,
    #[serde(default = "empty_json_object")]
    pub relation_options: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema, Serialize)]
pub struct UpdateModelFieldBody {
    pub title: String,
    pub description: Option<String>,
    #[serde(default)]
    pub is_required: bool,
    pub api_required: Option<bool>,
    #[serde(default)]
    pub is_unique: bool,
    pub default_value: Option<serde_json::Value>,
    pub display_interface: Option<String>,
    #[serde(default = "empty_json_object")]
    pub display_options: serde_json::Value,
    #[serde(default = "empty_json_object")]
    pub relation_options: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema, Serialize)]
pub struct CreateScopeGrantBody {
    pub scope_kind: String,
    pub scope_id: Uuid,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub permission_profile: String,
    #[serde(default)]
    pub confirm_unsafe_external_source_system_all: bool,
}

#[derive(Debug, Deserialize, ToSchema, Serialize)]
pub struct UpdateScopeGrantBody {
    pub enabled: Option<bool>,
    pub permission_profile: Option<String>,
    #[serde(default)]
    pub confirm_unsafe_external_source_system_all: bool,
}

#[derive(Debug, Deserialize, ToSchema, Serialize)]
pub struct BatchDeleteModelDefinitionsBody {
    #[serde(rename = "filterByTk")]
    #[schema(value_type = Object)]
    pub filter_by_tk: Option<serde_json::Value>,
    #[schema(value_type = Object)]
    pub filter: Option<serde_json::Value>,
    #[serde(default)]
    pub confirmed: bool,
}

#[derive(Debug, Deserialize, utoipa::IntoParams, Serialize)]
#[into_params(parameter_in = Query)]
pub struct ListModelsQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ConfirmationQuery {
    pub confirmed: Option<bool>,
}

fn empty_json_object() -> serde_json::Value {
    serde_json::json!({})
}

fn default_true() -> bool {
    true
}

fn deserialize_present_optional<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}
