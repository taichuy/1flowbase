use super::*;
#[cfg(test)]
use crate::openapi_interface::contract::ApiOutput;
use crate::openapi_interface::contract::{self as event, ApiInput, ApiOperationContext};
use serde_json::{json, Value};
use std::sync::Arc;

// Raw config is split by the installed provider form into public and encrypted
// fields in the service. That schema is not part of this typed Before carrier.
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct CreateEvent<'a> {
    installation_id: &'a str,
    display_name: &'a str,
    configured_models: &'a [ConfiguredModelBody],
    enabled_model_ids: &'a [String],
    included_in_main: &'a Option<bool>,
}
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct UpdateEvent<'a> {
    display_name: &'a str,
    configured_models: &'a [ConfiguredModelBody],
    enabled_model_ids: &'a [String],
    included_in_main: bool,
}
impl ApiInput for ProviderInstanceLifecycleInput {
    fn document_input_schema(operation: &ApiOperationContext) -> Option<Value> {
        match operation.interface_id.as_str() {
            "model_providers.instances.create" => Some(
                json!({"type":"object","properties":{"body":event::schema::<CreateEvent<'static>>()},"required":["body"],"additionalProperties":false}),
            ),
            "model_providers.instances.update" => Some(
                json!({"type":"object","properties":{"path":{"type":"object","properties":{"id":{"type":"string"}},"required":["id"]},"body":event::schema::<UpdateEvent<'static>>()},"required":["path","body"],"additionalProperties":false}),
            ),
            _ => None,
        }
    }
    fn project_document_input(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::List => json!({}),
            Self::Create(body) => {
                json!({"body":event::to_value(&CreateEvent { installation_id:&body.installation_id, display_name:&body.display_name, configured_models:&body.configured_models, enabled_model_ids:&body.enabled_model_ids, included_in_main:&body.included_in_main })?})
            }
            Self::Update { id, body } => {
                json!({"path":{"id":id},"body":event::to_value(&UpdateEvent { display_name:&body.display_name, configured_models:&body.configured_models, enabled_model_ids:&body.enabled_model_ids, included_in_main:body.included_in_main })?})
            }
            Self::Validate { id } | Self::Delete { id } => json!({"path":{"id":id}}),
        })
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/event_projection.rs"]
mod event_tests;
