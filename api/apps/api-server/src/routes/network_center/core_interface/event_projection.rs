use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};

// This input's config is explicitly encrypted as provider secret_json.
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct CreateProviderEvent<'a> {
    installation_id: &'a str,
    display_name: &'a str,
    description: &'a str,
}
impl ApiEventInput for NetworkCenterInput {
    fn document_input_schema(operation: &ApiEventOperationContext) -> Option<Value> {
        (operation.interface_id == "network_egress_providers.create").then(|| json!({"type":"object","properties":{"body":event::schema::<CreateProviderEvent<'static>>()},"required":["body"],"additionalProperties":false}))
    }
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::ListProviders | Self::ListProviderTypes | Self::ListRoutes => json!({}),
            Self::CreateProvider(body) => {
                json!({"body":event::to_value(&CreateProviderEvent { installation_id:&body.installation_id, display_name:&body.display_name, description:&body.description })?})
            }
            Self::UpdateProviderLifecycle { id, body } => {
                json!({"path":{"id":id},"body":event::to_value(body)?})
            }
            Self::SyncProvider { id } => json!({"path":{"id":id}}),
            Self::CreateRoute(body) => json!({"body":event::to_value(body)?}),
            Self::UpdateRoute { route_id, body } => {
                json!({"path":{"route_id":route_id},"body":event::to_value(body)?})
            }
            Self::DeleteRoute { route_id } => json!({"path":{"route_id":route_id}}),
        })
    }
}
impl ApiEventOutput for NetworkCenterOutput {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Providers(value) => event::to_value(value),
            Self::ProviderTypes(value) => event::to_value(value),
            Self::Provider(value) => event::to_value(value),
            Self::Routes(value) => event::to_value(value),
            Self::Route(value) => event::to_value(value),
            Self::Deleted => Ok(Value::Null),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/event_projection.rs"]
mod event_tests;
