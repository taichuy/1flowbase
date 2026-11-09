use super::*;
use crate::openapi_interface::contract::{self as event, ApiInput, ApiOperationContext, ApiOutput};
use serde_json::{json, Value};
use std::sync::Arc;

// This owner invokes commands after upgrade; the registered GET route describes
// the handshake, not Create/Attach/Cancel bodies or opaque subscriptions/runs.
impl ApiInput for AssistantWebSocketCommandInput {
    fn document_input_unavailable(
        _: &ApiOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        Some(domain::event_center::ApiEventUnavailableReason::ProtocolControl)
    }
    fn project_document_input(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::SubscribeConversations { .. }
            | Self::Create { .. }
            | Self::Attach { .. }
            | Self::Cancel { .. }
            | Self::ClientToolResult { .. } => Err(Arc::from(
                "WebSocket command has no HTTP OpenAPI operation payload",
            )),
        }
    }
}
impl ApiOutput for AssistantWebSocketCommandOutput {
    fn document_output_unavailable(
        _: &ApiOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        Some(domain::event_center::ApiEventUnavailableReason::ProtocolControl)
    }
    fn project_document_output(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::ConversationSubscription(_)
            | Self::Run(_)
            | Self::Cancelled
            | Self::ClientToolResult { .. } => Err(Arc::from(
                "WebSocket command response belongs to its protocol adapter",
            )),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/api_event_projection.rs"]
mod api_event_tests;
