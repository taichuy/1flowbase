use super::*;
use serde_json::json;

#[test]
fn callback_builder_owns_shared_bodies_after_source_drop_with_exact_legacy_fields() {
    let payload = {
        let call = json!({"id":"call-owned","name":"read","arguments":"source arguments"});
        let result = json!({"tool_call_id":"call-owned","content":"source result"});
        tool_callback_content_payload(None, "call-owned", "read", &call, Some(&result), None)
    };
    let call = json!({"id":"call-owned","name":"read","arguments":"source arguments"});
    let result = json!({"tool_call_id":"call-owned","content":"source result"});
    assert_eq!(
        serde_json::to_value(&payload).unwrap(),
        json!({
            "id":"call-owned", "name":"read", "callback_task_id":null,
            "tool_call_id":"call-owned", "callback_status":"returned", "execution_status":null,
            "request_payload":call, "tool_call":call,
            "callback_payload":result,"parsed_result":result,"tool_result":result,
            "call_usage":null,"result_context_usage":null,"duration_ms":null,"route_trace":null,
        })
    );
    assert!(std::ptr::eq(
        payload.get("request_payload").unwrap(),
        payload.get("tool_call").unwrap()
    ));
    assert!(std::ptr::eq(
        payload.get("callback_payload").unwrap(),
        payload.get("parsed_result").unwrap()
    ));
    assert!(std::ptr::eq(
        payload.get("parsed_result").unwrap(),
        payload.get("tool_result").unwrap()
    ));
}

#[test]
fn callback_builder_keeps_null_result_aliases_and_waiting_status() {
    let call = json!({"id":"waiting"});
    let payload = tool_callback_content_payload(None, "waiting", "tool", &call, None, None);
    assert_eq!(payload["callback_status"], json!("waiting_callback"));
    for field in [
        "callback_payload",
        "parsed_result",
        "tool_result",
        "callback_task_id",
    ] {
        assert!(
            payload.get(field).unwrap().is_null(),
            "{field} remains explicit null"
        );
    }
}
