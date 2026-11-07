use super::*;
use crate::event_center::api_projection::{
    to_value, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};
use std::sync::Arc;

use domain::event_center::ApiEventUnavailableReason;
impl ApiEventInput for UiManagementInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::PluginSettingsModuleAsset {
                route_id,
                applied_plugin_version,
                sha256,
            } => {
                json!({"query":{"route_id":route_id,"applied_plugin_version":applied_plugin_version,"sha256":sha256}})
            }
            Self::PluginSettingsPage { route_id } => json!({"query":{"route_id":route_id}}),
            Self::ListTemplates
            | Self::ListComponents
            | Self::CatalogIndex
            | Self::CatalogUpdateStatus => json!({}),
            Self::CreateTemplate(b) => json!({"body":to_value(b)?}),
            Self::ResetDefaultTemplate(b) => json!({"body":to_value(b)?}),
            Self::CreateComponent(b) => json!({"body":to_value(b)?}),
            Self::UpdateTemplate { id, body } => json!({"path":{"id":id},"body":to_value(body)?}),
            Self::PublishTemplate { id, body } => json!({"path":{"id":id},"body":to_value(body)?}),
            Self::UpdateComponent { id, body } => json!({"path":{"id":id},"body":to_value(body)?}),
            Self::SetDefaultTemplate { id }
            | Self::DeleteTemplate { id }
            | Self::GetComponent { id }
            | Self::DeleteComponent { id } => json!({"path":{"id":id}}),
            Self::CatalogPage { page } => json!({"path":{"page":page}}),
            Self::CatalogSearch(v) => {
                json!({"query":{"q":v.q,"page":v.page,"page_size":v.page_size}})
            }
            Self::CatalogDownload { component_code } => {
                json!({"path":{"component_code":component_code}})
            }
            Self::CatalogSyncGroup { source, group } => {
                json!({"path":{"source":source,"group":group}})
            }
        })
    }
}
impl ApiEventOutput for UiManagementOutput {
    fn document_output_unavailable(
        c: &ApiEventOperationContext,
    ) -> Option<ApiEventUnavailableReason> {
        (c.interface_id == "ui_management.plugin_settings_module_asset.view")
            .then_some(ApiEventUnavailableReason::BinaryResponse)
    }
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::PluginSettingsModuleAsset(_) => {
                Err(Arc::from("retained-module-non-json-response"))
            }
            Self::PluginSettingsPage(v) => to_value(v),
            Self::Templates(v) => to_value(v),
            Self::Template(v) => to_value(v),
            Self::Components(v) => to_value(v),
            Self::Component(v) => to_value(v),
            Self::CatalogIndex(v) => to_value(v),
            Self::CatalogPage(v) => to_value(v),
            Self::CatalogSearch(v) => to_value(v),
            Self::CatalogUpdateStatus(v) => to_value(v),
            Self::CatalogComponent(v) => to_value(v),
            Self::CatalogSync(v) => to_value(v),
            Self::NoContent => Ok(Value::Null),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
#[path = "_tests/document_event.rs"]
mod tests;
