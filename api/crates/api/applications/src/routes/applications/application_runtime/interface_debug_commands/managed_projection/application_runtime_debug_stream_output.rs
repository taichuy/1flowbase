use super::*;

impl InterfaceContract for ApplicationRuntimeDebugStreamOutput {
    fn event_projection_schema() -> Option<serde_json::Value> {
        Some(super::event_projection::stream_output_schema())
    }
    fn project_for_event(&self) -> Option<serde_json::Value> {
        Some(super::event_projection::stream_output(self))
    }
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema([(
            "kind",
            mp::tag_schema("ApplicationRuntimeDebugStreamOutput"),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value([(
            "kind",
            serde_json::Value::String("ApplicationRuntimeDebugStreamOutput".to_owned()),
        )]))
    }

    const CONTRACT_ID: &'static str = "console-application-runtime-debug-stream-output";
    const CONTRACT_VERSION: &'static str = "1";
}

impl crate::openapi_interface::contract::ApiOutput for ApplicationRuntimeDebugStreamOutput {
    fn document_output_schema(
        _operation: &crate::openapi_interface::contract::ApiOperationContext,
    ) -> Option<serde_json::Value> {
        Some(
            serde_json::json!({"type":"object","properties":{"application_id":{"type":"string","format":"uuid"},"run_id":{"type":"string","format":"uuid"}},"required":["application_id","run_id"],"additionalProperties":false}),
        )
    }
    fn project_document_output(
        &self,
        _operation: &crate::openapi_interface::contract::ApiOperationContext,
    ) -> Result<serde_json::Value, std::sync::Arc<str>> {
        Ok(serde_json::json!({"application_id":self.application_id,"run_id":self.run_id}))
    }
}
