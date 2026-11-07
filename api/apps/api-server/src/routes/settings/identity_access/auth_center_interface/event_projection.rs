use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};

#[derive(serde::Serialize, utoipa::ToSchema)]
struct UpdateConfigEvent<'a> {
    title: &'a str,
    enabled: bool,
    description: &'a Option<Option<String>>,
    self_registration_enabled: bool,
}
#[derive(serde::Serialize, utoipa::ToSchema)]
struct ConfigFieldEvent<'a> {
    key: &'a str,
    label: &'a str,
    r#type: &'a str,
    control: &'a Option<String>,
    read_only: Option<bool>,
    required: Option<bool>,
    pattern: &'a Option<String>,
    min: Option<f64>,
    max: Option<f64>,
    step: Option<f64>,
    precision: Option<u32>,
    description: &'a Option<String>,
}
#[derive(serde::Serialize, utoipa::ToSchema)]
struct EntryEvent<'a> {
    id: Uuid,
    auth_type: &'a str,
    title: &'a str,
    enabled: bool,
    is_builtin: bool,
    sort_order: i32,
    public_ui_block: &'a str,
    default_public_ui_block: &'a Option<String>,
    interface_path_prefixes: &'a [String],
    public_variables: &'a Option<serde_json::Map<String, Value>>,
    context_variables: &'a [auth_center::AuthCenterContextVariableResponse],
    config_schema: Vec<ConfigFieldEvent<'a>>,
    configured_secret_keys: &'a [String],
}
impl<'a> From<&'a auth_center::AuthCenterLoginEntryResponse> for EntryEvent<'a> {
    fn from(value: &'a auth_center::AuthCenterLoginEntryResponse) -> Self {
        Self {
            id: value.id,
            auth_type: &value.auth_type,
            title: &value.title,
            enabled: value.enabled,
            is_builtin: value.is_builtin,
            sort_order: value.sort_order,
            public_ui_block: &value.public_ui_block,
            default_public_ui_block: &value.default_public_ui_block,
            interface_path_prefixes: &value.interface_path_prefixes,
            public_variables: &value.public_variables,
            context_variables: &value.context_variables,
            config_schema: value
                .config_schema
                .iter()
                .map(|field| ConfigFieldEvent {
                    key: &field.key,
                    label: &field.label,
                    r#type: &field.r#type,
                    control: &field.control,
                    read_only: field.read_only,
                    required: field.required,
                    pattern: &field.pattern,
                    min: field.min,
                    max: field.max,
                    step: field.step,
                    precision: field.precision,
                    description: &field.description,
                })
                .collect(),
            configured_secret_keys: &value.configured_secret_keys,
        }
    }
}
#[derive(serde::Serialize, utoipa::ToSchema)]
struct OverviewEvent<'a> {
    default_login_entry_id: Uuid,
    supported_auth_types: &'a [String],
    login_entries: Vec<EntryEvent<'a>>,
    authentication_provider_templates: &'a [auth_center::AuthCenterProviderTemplateResponse],
}
impl ApiEventInput for AuthCenterInput {
    fn document_input_schema(operation: &ApiEventOperationContext) -> Option<Value> {
        (operation.interface_id == "auth_center.login_entries.update.config").then(|| json!({"type":"object","properties":{"path":{"type":"object","properties":{"id":{"type":"string","format":"uuid"}},"required":["id"]},"body":event::schema::<UpdateConfigEvent<'static>>()},"required":["path","body"],"additionalProperties":false}))
    }
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::Overview { locale: _ } => json!({}),
            Self::Create { locale: _, body } => json!({"body":event::to_value(body)?}),
            Self::Copy {
                locale: _,
                id,
                body,
            } => json!({"path":{"id":id},"body":event::to_value(body)?}),
            Self::Delete { id } => json!({"path":{"id":id}}),
            Self::Reorder { locale: _, body } => json!({"body":event::to_value(body)?}),
            Self::UpdateEnabled {
                locale: _,
                id,
                body,
            } => json!({"path":{"id":id},"body":event::to_value(body)?}),
            Self::UpdateConfig {
                locale: _,
                id,
                body,
            } => {
                json!({"path":{"id":id},"body":event::to_value(&UpdateConfigEvent { title:&body.title, enabled:body.enabled, description:&body.description, self_registration_enabled:body.self_registration_enabled })?})
            }
            Self::UpdatePublicUiBlock {
                locale: _,
                id,
                body,
            } => json!({"path":{"id":id},"body":event::to_value(body)?}),
        })
    }
}
impl ApiEventOutput for AuthCenterOutput {
    fn document_output_schema(operation: &ApiEventOperationContext) -> Option<Value> {
        match operation.interface_id.as_str() {
            "auth_center.overview.view" | "auth_center.login_entries.order" => {
                Some(event::schema::<OverviewEvent<'static>>())
            }
            "auth_center.login_entries.delete" => None,
            _ => Some(event::schema::<EntryEvent<'static>>()),
        }
    }
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Overview(value) => event::to_value(&OverviewEvent {
                default_login_entry_id: value.default_login_entry_id,
                supported_auth_types: &value.supported_auth_types,
                login_entries: value.login_entries.iter().map(EntryEvent::from).collect(),
                authentication_provider_templates: &value.authentication_provider_templates,
            }),
            Self::Authenticator(value) => event::to_value(&EntryEvent::from(value)),
            Self::NoContent => Ok(Value::Null),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/security.rs"]
mod tests;
