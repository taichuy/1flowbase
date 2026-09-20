use super::*;

#[test]
fn recovery_failure_projection_retains_bounded_idle_facts_and_rejects_private_values() {
    let first = json!({
        "kind":"websocket_error", "websocket_error_kind":"protocol",
        "failure_phase":"idle", "idle_duration_ms":150_123,
        "socket_incarnation":7, "owner_socket_incarnation":7,
        "transport_generation":4, "routing_token_present":true,
        "association_present":true, "recovery_decision":"retry_websocket",
        "token":"PRIVATE_CANARY", "raw_error":"PRIVATE_CANARY"
    });
    let last = json!({
        "kind":"websocket_close", "close_code":1008,
        "websocket_error_kind":"PRIVATE_CANARY", "failure_phase":"PRIVATE_CANARY",
        "idle_duration_ms":86_400_001, "routing_token_present":"PRIVATE_CANARY",
        "association_present":false, "recovery_decision":"terminal",
        "reason_category":"continuation_unavailable"
    });
    let mut payload = json!({});
    attach(
        &mut payload,
        Some(&json!({KEY:{
            "first_failure":first,"last_failure":last,"attempts":[first,last]
        }})),
    );
    let diagnostic = &payload[KEY];
    for field in [
        "websocket_error_kind",
        "failure_phase",
        "idle_duration_ms",
        "socket_incarnation",
        "owner_socket_incarnation",
        "transport_generation",
        "routing_token_present",
        "association_present",
        "recovery_decision",
    ] {
        assert_eq!(diagnostic["first_failure"][field], first[field], "{field}");
    }
    assert_eq!(diagnostic["last_failure"]["recovery_decision"], "terminal");
    assert_eq!(diagnostic["last_failure"]["association_present"], false);
    assert!(diagnostic["last_failure"].get("idle_duration_ms").is_none());
    assert!(diagnostic["last_failure"]
        .get("routing_token_present")
        .is_none());
    assert_eq!(diagnostic["attempts"].as_array().unwrap().len(), 2);
    assert!(!payload.to_string().contains("PRIVATE_CANARY"));
}

#[test]
fn recovery_failure_projection_accepts_zero_idle_and_bounds_attempt_history() {
    let source = json!({KEY:{"attempts":vec![json!({
        "idle_duration_ms":0, "websocket_error_kind":"queue_bytes_limit",
        "failure_phase":"active", "routing_token_present":false,
        "association_present":false, "recovery_decision":"committed"
    }); 20]}});
    let mut payload = json!({});
    attach(&mut payload, Some(&source));
    let attempts = payload[KEY]["attempts"].as_array().unwrap();
    assert_eq!(attempts.len(), 16);
    assert_eq!(attempts[0]["idle_duration_ms"], 0);
    assert_eq!(attempts[0]["routing_token_present"], false);
    assert_eq!(attempts[0]["websocket_error_kind"], "queue_bytes_limit");
}
