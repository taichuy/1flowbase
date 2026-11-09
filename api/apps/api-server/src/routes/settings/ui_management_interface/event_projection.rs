use super::*;
use crate::openapi_interface::contract::{to_value, ApiInput, ApiOperationContext, ApiOutput};
use serde_json::{json, Value};
use std::sync::Arc;

use domain::event_center::ApiEventUnavailableReason;
impl ApiOutput for UiManagementOutput {
    fn document_output_unavailable(c: &ApiOperationContext) -> Option<ApiEventUnavailableReason> {
        (c.interface_id == "ui_management.plugin_settings_module_asset.view")
            .then_some(ApiEventUnavailableReason::BinaryResponse)
    }
    fn project_document_output(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
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
