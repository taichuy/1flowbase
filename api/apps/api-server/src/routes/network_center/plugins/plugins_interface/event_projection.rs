use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};

fn official_query(query: &OfficialPluginCatalogQuery) -> Value {
    let mut value = serde_json::Map::new();
    if let Some(plugin_type) = &query.plugin_type {
        value.insert("plugin_type".into(), json!(plugin_type));
    }
    if let Some(locale) = &query.locale {
        value.insert("locale".into(), json!(locale));
    }
    if let Some(q) = &query.q {
        value.insert("q".into(), json!(q));
    }
    if let Some(cursor) = &query.cursor {
        value.insert("cursor".into(), json!(cursor));
    }
    if let Some(limit) = query.limit {
        value.insert("limit".into(), json!(limit));
    }
    Value::Object(value)
}
impl ApiEventInput for NetworkPluginInput {
    fn document_input_unavailable(
        operation: &ApiEventOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        (operation.interface_id == "network_egress_plugins.install.upload")
            .then_some(domain::event_center::ApiEventUnavailableReason::BinaryRequest)
    }
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::ListOfficial { query, locale: _ } => json!({"query":official_query(query)}),
            Self::ListFamilies { locale: _ } => json!({}),
            Self::SwitchVersion {
                provider_code,
                body,
                locale: _,
            } => {
                json!({"path":{"provider_code":provider_code},"body":{"installation_id":body.installation_id}})
            }
            Self::UninstallVersion {
                provider_code,
                installation_id,
                locale: _,
            } => json!({"path":{"provider_code":provider_code,"installation_id":installation_id}}),
            Self::UninstallFamily {
                provider_code,
                locale: _,
            } => json!({"path":{"provider_code":provider_code}}),
            Self::InstallOfficial(body) => {
                json!({"body":{"plugin_id":body.plugin_id,"compatibility_override":event::to_value(&body.compatibility_override)?,"risk_override":event::to_value(&body.risk_override)?}})
            }
            Self::InstallUploaded { .. } => {
                return Err(Arc::from("plugin archive has no JSON request event view"))
            }
        })
    }
}
impl ApiEventOutput for NetworkPluginOutput {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Official(value) => event::to_value(value),
            Self::Families(value) => event::to_value(value),
            Self::Installed(value) => event::to_value(value),
            Self::Empty => Ok(Value::Null),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/event_projection.rs"]
mod event_tests;
