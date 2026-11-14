use super::*;
use crate::openapi_interface::contract::{ApiInput, ApiOperationContext, ApiOutput};
use serde_json::Value;

// Native MCP initialization/list/call are protocol operations. Their canonical
// downstream calls already own independent lifecycle observations and lineage.
impl ApiInput for McpInvocationInput {
    fn document_input_unavailable(
        _: &ApiOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        Some(domain::event_center::ApiEventUnavailableReason::MissingOpenapiContract)
    }
    fn project_document_input(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Initialize { .. }
            | Self::InitializedNotification
            | Self::ToolsList { .. }
            | Self::ToolCall { .. } => Err(Arc::from(
                "native MCP has no HTTP OpenAPI operation event view",
            )),
        }
    }
}
impl ApiOutput for McpInvocationOutput {
    fn document_output_unavailable(
        _: &ApiOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        Some(domain::event_center::ApiEventUnavailableReason::MissingOpenapiContract)
    }
    fn project_document_output(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Initialized { .. }
            | Self::NotificationAccepted
            | Self::ToolsListed { .. }
            | Self::ToolCalled(_) => Err(Arc::from(
                "native MCP response is owned by the protocol adapter",
            )),
        }
    }
}
