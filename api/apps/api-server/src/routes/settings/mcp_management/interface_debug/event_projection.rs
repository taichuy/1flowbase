use super::*;
use crate::openapi_interface::contract::{self as event, ApiInput, ApiOperationContext, ApiOutput};
use serde_json::{json, Value};

// Arbitrary downstream arguments can contain a password or a credential-reveal
// request. Its selected interface owns the safe business projection.
#[derive(serde::Serialize, utoipa::ToSchema)]
struct DebugEvent<'a> {
    interface_id: &'a str,
    debug_response_mode: &'static str,
}
impl ApiInput for McpDebugInput {
    fn document_input_schema(_: &ApiOperationContext) -> Option<Value> {
        Some(
            json!({"type":"object","properties":{"body":event::schema::<DebugEvent<'static>>()},"required":["body"],"additionalProperties":false}),
        )
    }
    fn project_document_input(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        let debug_response_mode = match self.body.debug_response_mode {
            debug_execute::McpDebugResponseMode::ToolResult => "tool_result",
            debug_execute::McpDebugResponseMode::DebugDetails => "debug_details",
        };
        Ok(
            json!({"body":event::to_value(&DebugEvent { interface_id:&self.body.interface_id, debug_response_mode })?}),
        )
    }
}
impl ApiOutput for McpDebugOutput {
    fn document_output_unavailable(
        _: &ApiOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        // The outer dispatcher does not carry the selected target's safe event
        // codec. Serializing arbitrary returned JSON could reveal credentials.
        Some(domain::event_center::ApiEventUnavailableReason::CredentialBoundary)
    }
    fn project_document_output(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Json(_) => Err(Arc::from(
                "downstream API owns its safe event response projection",
            )),
            Self::Target(_) => Err(Arc::from(
                "forwarded response headers and payload remain host-owned",
            )),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/event_projection.rs"]
mod event_tests;
