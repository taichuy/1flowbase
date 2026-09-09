use super::*;

impl InterfaceContract for ApplicationRuntimeDebugStreamEvent {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "0",
            mp::object_schema(&[("is_ok", serde_json::json!({"type":"boolean"}))]),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "0",
            mp::object_value(&[("is_ok", serde_json::Value::Bool((&(self).0).is_ok()))]),
        )]))
    }

    const CONTRACT_ID: &'static str = "console-application-runtime-debug-stream-event";
    const CONTRACT_VERSION: &'static str = "1";
}
