use super::*;

impl InterfaceContract for ApplicationRuntimeDebugStreamOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "kind",
            mp::tag_schema("ApplicationRuntimeDebugStreamOutput"),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "kind",
            serde_json::Value::String("ApplicationRuntimeDebugStreamOutput".to_owned()),
        )]))
    }

    const CONTRACT_ID: &'static str = "console-application-runtime-debug-stream-output";
    const CONTRACT_VERSION: &'static str = "1";
}
