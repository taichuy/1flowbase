use super::super::*;

fn candidate(
    node: Uuid,
    call: &str,
    name: &str,
    reserved: Option<Uuid>,
) -> crate::ports::RuntimeEventPayload {
    let mut event = debug_stream_events::provider_output_item_done(
        "llm",
        node,
        0,
        json!({"type":"function_call","id":call,"call_id":call,"name":name,"arguments":"{}"}),
    );
    if let Some(reserved) = reserved {
        event.payload["reserved_callback_task_id"] = json!(reserved);
    }
    event
}

#[test]
fn final_client_scope_consumes_only_its_reserved_callback_identity() {
    let node = Uuid::now_v7();
    let reserved = Uuid::now_v7();
    let mut events = vec![
        candidate(node, "internal", "host_private", Some(Uuid::now_v7())),
        candidate(Uuid::now_v7(), "client", "Bash", Some(Uuid::now_v7())),
        candidate(node, "client", "Bash", Some(reserved)),
    ];
    let calls = vec![json!({"id":"client","name":"Bash"})];
    retain_client_tool_deliveries(&mut events, &[("llm", node, &calls)]);
    assert_eq!(events.len(), 1);
    assert_eq!(
        reserved_client_callback_id(&events).unwrap(),
        Some(reserved)
    );
    assert!(events[0].payload.get("committed_delivery").is_none());
}

#[test]
fn legacy_candidates_need_no_reservation_and_inconsistent_reservations_fail_closed() {
    let node = Uuid::now_v7();
    assert_eq!(
        reserved_client_callback_id(&[candidate(node, "client", "Bash", None)]).unwrap(),
        None
    );
    assert!(reserved_client_callback_id(&[
        candidate(node, "a", "Bash", Some(Uuid::now_v7())),
        candidate(node, "b", "Bash", Some(Uuid::now_v7())),
    ])
    .is_err());
}
