use super::*;
use crate::extension_bus::managed_projection::object_schema as object;
use serde_json::json;

pub(super) fn input_schema() -> Value {
    object([
        ("instance_id", json!({"type":"string","format":"uuid"})),
        // Notification Send's business input is a typed JSON object. Its provider-specific
        // schema comes from a descriptor only when an instance/type has been selected.
        (
            "input_payload",
            json!({"type":"object","additionalProperties":true}),
        ),
    ])
}
pub(super) fn input(input: &NotificationInput) -> Option<Value> {
    match input {
        // Only caller business input is copied. Instance config, managed secrets and
        // authorization are loaded later by the service and never enter this projection.
        NotificationInput::Send(instance_id, input_payload) if input_payload.is_object() => {
            Some(json!({"instance_id":instance_id,"input_payload":input_payload}))
        }
        _ => None,
    }
}
pub(super) fn output_schema() -> Value {
    object([
        ("instance_id", json!({"type":"string","format":"uuid"})),
        ("type_code", json!({"type":"string"})),
        ("result", Value::Bool(true)),
    ])
}
pub(super) fn output(output: &NotificationOutput) -> Option<Value> {
    let output = &output.0;
    // NotificationSendResult.result is the service's validated public provider result.
    Some(
        json!({"instance_id":output.get("instance_id")?,"type_code":output.get("type_code")?,"result":output.get("result")?}),
    )
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "event_projection/_tests/events.rs"]
mod _tests;
