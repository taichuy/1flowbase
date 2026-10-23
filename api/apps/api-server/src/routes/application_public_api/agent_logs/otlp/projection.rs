use super::*;
use crate::extension_bus::managed_projection as mp;
use serde_json::{json, Value};

impl InterfaceContract for Input {
    const CONTRACT_ID: &'static str = "application-agent-logs-otlp-input";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<Value> {
        Some(mp::object_schema([
            ("kind", mp::tag_schema(Self::CONTRACT_ID)),
            ("byte_count", mp::count_schema()),
            ("format", mp::text_schema()),
            ("encoding", mp::text_schema()),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<Value> {
        Some(
            json!({"kind":Self::CONTRACT_ID,"byte_count":self.body.len(),"format":self.format.tag(),"encoding":self.encoding.tag()}),
        )
    }
}
impl InterfaceContract for Output {
    const CONTRACT_ID: &'static str = "application-agent-logs-otlp-output";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<Value> {
        Some(mp::object_schema([
            ("kind", mp::tag_schema(Self::CONTRACT_ID)),
            ("accepted_spans", mp::count_schema()),
            ("rejected_spans", mp::count_schema()),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<Value> {
        Some(
            json!({"kind":Self::CONTRACT_ID,"accepted_spans":self.accepted_spans,"rejected_spans":self.response.partial_success.as_ref().map_or(0,|partial| partial.rejected_spans)}),
        )
    }
}
impl InterfaceContract for TargetError {
    const CONTRACT_ID: &'static str = "application-agent-logs-otlp-error";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<Value> {
        Some(mp::object_schema([(
            "kind",
            mp::tag_schema(Self::CONTRACT_ID),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<Value> {
        Some(json!({"kind":Self::CONTRACT_ID}))
    }
}
