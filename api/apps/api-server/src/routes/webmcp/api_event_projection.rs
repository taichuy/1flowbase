use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct ToolEvent {
    is_error: bool,
}
impl ApiEventInput for WebMcpInput {
    fn document_input_schema(operation: &ApiEventOperationContext) -> Option<Value> {
        (operation.interface_id == "webmcp.tools.invoke").then(||json!({"type":"object","properties":{"path":{"type":"object","properties":{"instance_id":{"type":"string"},"operation":{"type":"string"}},"required":["instance_id","operation"],"additionalProperties":false}},"required":["path"],"additionalProperties":false}))
    }
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::Registrations => json!({}),
            // The selected canonical target owns argument credential filtering.
            // This outer carrier has no typed safe codec for opaque arguments.
            Self::Tool {
                instance_id,
                operation,
                arguments: _,
                forward_headers: _,
            } => json!({"path":{"instance_id":instance_id,"operation":operation}}),
        })
    }
}
impl ApiEventOutput for WebMcpOutput {
    fn document_output_schema(operation: &ApiEventOperationContext) -> Option<Value> {
        (operation.interface_id == "webmcp.tools.invoke").then(|| event::schema::<ToolEvent>())
    }
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Registrations(value) => event::to_value(value),
            Self::Tool(value) => event::to_value(&ToolEvent {
                is_error: value.is_error,
            }),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/api_event_projection.rs"]
mod api_event_tests;
