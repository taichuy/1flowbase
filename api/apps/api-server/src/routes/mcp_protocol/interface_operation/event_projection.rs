use super::*;
use crate::event_center::api_projection::{
    ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::Value;

// Native MCP initialization/list/call are protocol operations. Their canonical
// downstream calls already own independent lifecycle observations and lineage.
impl ApiEventInput for McpInvocationInput {
    fn document_input_unavailable(
        _: &ApiEventOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        Some(domain::event_center::ApiEventUnavailableReason::MissingOpenapiContract)
    }
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
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
impl ApiEventOutput for McpInvocationOutput {
    fn document_output_unavailable(
        _: &ApiEventOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        Some(domain::event_center::ApiEventUnavailableReason::MissingOpenapiContract)
    }
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
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
