use super::*;
#[test]
fn pointers_accept_root_and_escaped_tokens_and_reject_malformed_escapes() {
    for pointer in ["", "/payload/order_id", "/a~1b/~0", "/", "/中文"] {
        assert!(is_json_pointer(pointer));
    }
    for pointer in ["payload", "#/payload", "/~", "/~2", "/a~x"] {
        assert!(!is_json_pointer(pointer));
    }
}
#[test]
fn consumed_is_a_distinct_dequeue_phase_not_an_ack_alias() {
    assert_eq!(
        serde_json::to_value(EventTriggerPhase::Consumed).unwrap(),
        "consumed"
    );
    assert!(serde_json::from_str::<EventTriggerPhase>("\"acknowledged\"").is_err());
    assert!(serde_json::from_str::<EventTriggerPhase>("\"processed\"").is_err());
}
