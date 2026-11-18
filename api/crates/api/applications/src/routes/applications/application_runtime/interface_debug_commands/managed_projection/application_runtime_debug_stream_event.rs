use super::*;

impl InterfaceContract for ApplicationRuntimeDebugStreamEvent {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema([(
            "0",
            mp::object_schema([("is_ok", serde_json::json!({"type":"boolean"}))]),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value([(
            "0",
            mp::object_value([("is_ok", serde_json::Value::Bool((self).0.is_ok()))]),
        )]))
    }

    const CONTRACT_ID: &'static str = "console-application-runtime-debug-stream-event";
    const CONTRACT_VERSION: &'static str = "1";
}

// Stream frames are observations, never the server-stream terminal result.
impl crate::openapi_interface::contract::ApiOutput for ApplicationRuntimeDebugStreamEvent {
    fn document_output_unavailable(
        _operation: &crate::openapi_interface::contract::ApiOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        Some(domain::event_center::ApiEventUnavailableReason::MissingStreamingTerminal)
    }
    fn project_document_output(
        &self,
        _operation: &crate::openapi_interface::contract::ApiOperationContext,
    ) -> Result<serde_json::Value, std::sync::Arc<str>> {
        Err(std::sync::Arc::from(
            "debug_stream_frame_is_not_the_terminal_result",
        ))
    }
}
